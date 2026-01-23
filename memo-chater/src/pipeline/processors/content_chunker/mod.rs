//! 内容切块器
//!
//! 将对话内容按逻辑切分成独立的信息块，便于后续存储和检索

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{info, warn, debug};

use crate::pipeline::{
    processor::{Processor, ProcessorError},
    context::ProcessorContext,
    packet::ConversationPacket,
};
use crate::types::{ChatMessage, ShortTermMemory, MemorySource};

/// 切块配置
#[derive(Debug, Clone, Deserialize)]
struct ChunkerConfig {
    /// 使用的模型（留空则使用助手配置的 processor_model）
    #[serde(default)]
    model: String,
    /// 切块提示词
    prompt: String,
}

impl Default for ChunkerConfig {
    fn default() -> Self {
        Self {
            model: String::new(),
            prompt: r#"你是一个对话内容分析专家。请将以下对话内容按逻辑或步骤切分成独立的信息块。

要求：
1. 每个块应该是一个完整的逻辑单元（一个话题、一个步骤、一个结论等）
2. 为每个块生成一个简洁的总结标题（不超过50字）
3. 保留块的详细内容，内容可以包含任意字符

输出格式（XML，内容用CDATA包裹）：
<chunks>
  <chunk>
    <summary>简洁的总结标题</summary>
    <content><![CDATA[该块的详细内容，可包含任意字符]]></content>
    <type>fact</type>
  </chunk>
</chunks>

type可选值：fact/event/preference/knowledge/task/other

对话内容：
{conversation}

请直接输出XML，不要有其他内容。"#.to_string(),
        }
    }
}

/// 切块结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentChunk {
    /// 简洁的总结标题
    pub summary: String,
    /// 详细内容
    pub content: String,
    /// 块类型
    #[serde(rename = "type")]
    pub chunk_type: String,
}

/// 内容切块器
/// 
/// 负责将对话内容切分成适合向量化存储的块
pub struct ContentChunker {
    config_path: PathBuf,
}

impl ContentChunker {
    pub fn new() -> Self {
        // 配置文件路径：与本模块同目录
        let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/pipeline/processors/content_chunker/config.toml");
        
        Self { config_path }
    }

    /// 加载配置
    fn load_config(&self) -> Result<ChunkerConfig, ProcessorError> {
        if self.config_path.exists() {
            let content = std::fs::read_to_string(&self.config_path)
                .map_err(|e| ProcessorError::Config(format!("读取配置文件失败: {}", e)))?;
            
            toml::from_str(&content)
                .map_err(|e| ProcessorError::Config(format!("解析配置文件失败: {}", e)))
        } else {
            warn!("ContentChunker 配置文件不存在，使用默认配置: {:?}", self.config_path);
            Ok(ChunkerConfig::default())
        }
    }

    /// 格式化对话内容
    fn format_conversation(&self, messages: &[ChatMessage], user_name: &str, assistant_name: &str) -> String {
        messages
            .iter()
            .filter(|m| m.role != "system") // 排除系统消息
            .map(|m| {
                let role_name = match m.role.as_str() {
                    "user" => user_name,
                    "assistant" => assistant_name,
                    _ => &m.role,
                };
                format!("【{}】: {}", role_name, m.content)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// 解析大模型返回的 XML 格式切块结果
    /// 
    /// XML + CDATA 格式比 JSON 更抗特殊字符干扰
    fn parse_chunks(&self, response: &str) -> Result<Vec<ContentChunk>, ProcessorError> {
        // 提取 <chunks>...</chunks> 内容
        let xml_content = self.extract_xml_content(response)?;
        
        // 解析每个 <chunk> 块
        let mut chunks = Vec::new();
        let mut search_start = 0;
        
        while let Some(chunk_start) = xml_content[search_start..].find("<chunk>") {
            let chunk_start = search_start + chunk_start;
            
            if let Some(chunk_end) = xml_content[chunk_start..].find("</chunk>") {
                let chunk_end = chunk_start + chunk_end + "</chunk>".len();
                let chunk_xml = &xml_content[chunk_start..chunk_end];
                
                if let Some(chunk) = self.parse_single_chunk(chunk_xml) {
                    chunks.push(chunk);
                }
                
                search_start = chunk_end;
            } else {
                break;
            }
        }
        
        if chunks.is_empty() {
            let preview: String = response.chars().take(500).collect();
            return Err(ProcessorError::Internal(format!(
                "未能从响应中解析出任何切块。响应预览: {}...", 
                preview
            )));
        }
        
        debug!("XML解析成功，共 {} 个切块", chunks.len());
        Ok(chunks)
    }

    /// 提取 XML 内容
    /// 
    /// 直接查找 <chunks> 和 </chunks> 标签，不依赖代码块提取
    /// 因为 CDATA 内容可能包含 ``` 导致代码块提取出错
    fn extract_xml_content(&self, response: &str) -> Result<String, ProcessorError> {
        // 直接查找 <chunks> 标签（最可靠的方式）
        if let Some(start) = response.find("<chunks>") {
            // 从 <chunks> 开始往后找 </chunks>
            if let Some(end_offset) = response[start..].find("</chunks>") {
                let end = start + end_offset + "</chunks>".len();
                return Ok(response[start..end].to_string());
            }
        }
        
        // 兜底：可能没有 <chunks> 包裹，直接找第一个 <chunk> 到最后一个 </chunk>
        if let Some(first_chunk) = response.find("<chunk>") {
            if let Some(last_chunk_end) = response.rfind("</chunk>") {
                let end = last_chunk_end + "</chunk>".len();
                return Ok(response[first_chunk..end].to_string());
            }
        }
        
        let preview: String = response.chars().take(300).collect();
        Err(ProcessorError::Internal(format!(
            "未找到有效的 XML 结构。响应预览: {}...", 
            preview
        )))
    }

    /// 解析单个 chunk XML
    fn parse_single_chunk(&self, chunk_xml: &str) -> Option<ContentChunk> {
        let summary = self.extract_tag_content(chunk_xml, "summary")?;
        let content = self.extract_tag_content_with_cdata(chunk_xml, "content")?;
        let chunk_type = self.extract_tag_content(chunk_xml, "type")
            .unwrap_or_else(|| "other".to_string());
        
        Some(ContentChunk {
            summary,
            content,
            chunk_type,
        })
    }

    /// 尝试部分解析，尽可能提取能解析的 chunk
    /// 
    /// 即使整体解析失败，也尝试逐个提取有效的 chunk
    fn try_partial_parse(&self, response: &str) -> Option<Vec<ContentChunk>> {
        let mut chunks = Vec::new();
        let mut search_start = 0;
        
        // 逐个查找 <chunk>...</chunk> 块
        while let Some(chunk_start) = response[search_start..].find("<chunk>") {
            let chunk_start = search_start + chunk_start;
            
            if let Some(chunk_end_offset) = response[chunk_start..].find("</chunk>") {
                let chunk_end = chunk_start + chunk_end_offset + "</chunk>".len();
                let chunk_xml = &response[chunk_start..chunk_end];
                
                // 尝试解析单个 chunk，失败则跳过
                if let Some(chunk) = self.parse_single_chunk(chunk_xml) {
                    chunks.push(chunk);
                } else {
                    debug!("部分解析：跳过无法解析的 chunk");
                }
                
                search_start = chunk_end;
            } else {
                // 找不到结束标签，停止搜索
                break;
            }
        }
        
        if chunks.is_empty() {
            None
        } else {
            Some(chunks)
        }
    }

    /// 提取普通标签内容
    fn extract_tag_content(&self, xml: &str, tag: &str) -> Option<String> {
        let open_tag = format!("<{}>", tag);
        let close_tag = format!("</{}>", tag);
        
        if let Some(start) = xml.find(&open_tag) {
            let content_start = start + open_tag.len();
            if let Some(end) = xml[content_start..].find(&close_tag) {
                let content = xml[content_start..content_start + end].trim();
                return Some(content.to_string());
            }
        }
        None
    }

    /// 提取可能包含 CDATA 的标签内容
    fn extract_tag_content_with_cdata(&self, xml: &str, tag: &str) -> Option<String> {
        let open_tag = format!("<{}>", tag);
        let close_tag = format!("</{}>", tag);
        
        if let Some(start) = xml.find(&open_tag) {
            let content_start = start + open_tag.len();
            if let Some(end) = xml[content_start..].find(&close_tag) {
                let raw_content = &xml[content_start..content_start + end];
                
                // 检查是否有 CDATA
                if let Some(cdata_start) = raw_content.find("<![CDATA[") {
                    if let Some(cdata_end) = raw_content.find("]]>") {
                        let cdata_content = &raw_content[cdata_start + 9..cdata_end];
                        return Some(cdata_content.to_string());
                    }
                }
                
                // 没有 CDATA，直接返回内容
                return Some(raw_content.trim().to_string());
            }
        }
        None
    }
}

#[async_trait]
impl Processor for ContentChunker {
    fn name(&self) -> &'static str {
        "ContentChunker"
    }

    fn requires_memory(&self) -> bool {
        true
    }

    async fn process(
        &self,
        packet: &mut ConversationPacket,
        ctx: &ProcessorContext,
    ) -> Result<(), ProcessorError> {
        info!("ContentChunker 开始处理");

        // 加载配置
        let config = self.load_config()?;
        debug!("配置加载成功，模型: {}", if config.model.is_empty() { "使用默认" } else { &config.model });

        // 格式化对话内容
        let conversation = self.format_conversation(
            &packet.messages,
            &packet.user_name,
            &packet.assistant_name,
        );

        if conversation.is_empty() {
            info!("对话内容为空，跳过切块");
            packet.set_processor_state(self.name(), serde_json::json!({
                "skipped": true,
                "reason": "empty_conversation"
            }));
            return Ok(());
        }

        debug!("对话内容长度: {} 字符", conversation.len());

        // 构建提示词（替换所有占位符）
        let prompt = config.prompt
            .replace("{conversation}", &conversation)
            .replace("{user_name}", &packet.user_name)
            .replace("{assistant_name}", &packet.assistant_name);

        // 确定使用的模型
        let model = if config.model.is_empty() {
            ctx.processor_model()
        } else {
            &config.model
        };

        info!("调用模型 {} 进行切块分析", model);

        // 3次重试机制
        let max_retries = 3;
        let mut best_chunks: Vec<ContentChunk> = Vec::new();
        let mut last_error: Option<String> = None;

        for attempt in 1..=max_retries {
            info!("切块尝试 {}/{}", attempt, max_retries);

            // 调用大模型
            let messages = vec![ChatMessage::user(&prompt)];
            let response = match ctx.ai_client
                .chat_with_model(&messages, Some(model))
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    warn!("第 {} 次调用模型失败: {}", attempt, e);
                    last_error = Some(format!("调用模型失败: {}", e));
                    continue;
                }
            };

            debug!("模型响应: {}", response);

            // 解析结果
            match self.parse_chunks(&response) {
                Ok(chunks) => {
                    info!("第 {} 次尝试成功，解析出 {} 个切块", attempt, chunks.len());
                    best_chunks = chunks;
                    last_error = None;
                    break; // 成功，直接使用
                }
                Err(e) => {
                    warn!("第 {} 次解析失败: {}", attempt, e);
                    last_error = Some(format!("{}", e));
                    
                    // 尝试部分解析，保留能解析的内容
                    if let Some(partial) = self.try_partial_parse(&response) {
                        if partial.len() > best_chunks.len() {
                            info!("部分解析成功，获得 {} 个切块（优于之前的 {} 个）", 
                                partial.len(), best_chunks.len());
                            best_chunks = partial;
                        }
                    }
                }
            }
        }

        // 检查最终结果
        if best_chunks.is_empty() {
            if let Some(err) = last_error {
                return Err(ProcessorError::Internal(format!(
                    "切块失败，3次重试均未成功: {}", err
                )));
            }
            return Err(ProcessorError::Internal(
                "切块失败，未能解析出任何内容".to_string()
            ));
        }

        let chunks = best_chunks;
        info!("切块完成，共 {} 个块", chunks.len());

        // 将切块结果转换为短期记忆并存入
        for (i, chunk) in chunks.iter().enumerate() {
            // 安全截断：按字符而非字节
            let content_preview = if chunk.content.chars().count() > 50 {
                format!("{}...", chunk.content.chars().take(50).collect::<String>())
            } else {
                chunk.content.clone()
            };

            info!(
                "块 {}: [{}] {} - {}",
                i + 1,
                chunk.chunk_type,
                chunk.summary,
                content_preview
            );

            // 创建短期记忆（使用结构化字段）
            let memory = ShortTermMemory {
                id: format!("chunk_{}_{}", Utc::now().timestamp_millis(), i),
                summary: chunk.summary.clone(),
                content: chunk.content.clone(),
                memory_type: chunk.chunk_type.clone(),
                relevance: 1.0, // 新切块的相关性设为最高
                confidence: 1.0, // 置信度默认1.0
                should_expand: false, // 新切块默认不展开
                source: MemorySource::CurrentConversation,
                timestamp: Utc::now(),
            };

            packet.add_short_term_memory(memory);
        }

        info!("已将 {} 个切块存入短期记忆", chunks.len());

        // 清空对话上下文（保留 system 消息）
        let system_msg = packet.messages.iter()
            .find(|m| m.role == "system")
            .cloned();
        
        packet.messages.clear();
        
        if let Some(sys) = system_msg {
            packet.messages.push(sys);
        }

        info!("已清空对话上下文");

        // 保存到处理器状态（供后续处理器使用）
        packet.set_processor_state(self.name(), serde_json::json!({
            "chunked": true,
            "chunk_count": chunks.len(),
            "chunks": chunks,
            "context_cleared": true
        }));

        Ok(())
    }
}