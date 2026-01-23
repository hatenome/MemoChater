//! 记忆提取器实现
//! 
//! 从对话历史中提取结构化的记忆信息

use crate::ai::AiClient;
use crate::types::ChatMessage;
use super::types::{ExtractedMemory, ExtractionResult, ExtractorConfig, ExtractorError};


/// 记忆提取器
/// 
/// 负责调用AI从对话中提取有价值的记忆信息
pub struct MemoryExtractor {
    config: ExtractorConfig,
    ai_client: AiClient,
}

impl MemoryExtractor {
    /// 创建新的提取器实例
    pub fn new(config: ExtractorConfig) -> Self {
        let ai_client = AiClient::new(
            config.api_base.clone(),
            config.api_key.clone(),
            config.model.clone(),
        );
        
        Self { config, ai_client }
    }

    /// 从对话历史中提取记忆
    /// 
    /// # Arguments
    /// * `messages` - 对话历史
    /// 
    /// # Returns
    /// 提取结果，包含解析出的记忆列表
    pub async fn extract(&self, messages: &[ChatMessage]) -> Result<ExtractionResult, ExtractorError> {
        // 1. 构建对话文本
        let conversation_text = self.format_conversation(messages);
        
        // 2. 构建提示词
        let system_prompt = self.get_system_prompt();
        let user_prompt = format!(
            "请分析以下对话，提取所有有价值的信息：\n\n{}", 
            conversation_text
        );
        
        // 3. 调用AI
        let raw_response = self.call_ai(&system_prompt, &user_prompt).await?;
        
        // 4. 解析响应
        self.parse_response(&raw_response)
    }

    /// 格式化对话为文本
    fn format_conversation(&self, messages: &[ChatMessage]) -> String {
        messages
            .iter()
            .map(|m| {
                let role_name = match m.role.as_str() {
                    "user" => &self.config.user_name,
                    "assistant" => &self.config.assistant_name,
                    other => other,
                };
                format!("[{}]: {}", role_name, m.content)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// 获取系统提示词
    fn get_system_prompt(&self) -> String {
        if let Some(custom) = &self.config.custom_prompt {
            return custom.clone();
        }
        
        // 尝试从文件加载
        if let Ok(content) = std::fs::read_to_string("./prompts/memory_extractor.txt") {
            return content;
        }
        
        DEFAULT_EXTRACTION_PROMPT.to_string()
    }

    /// 调用AI获取响应
    async fn call_ai(&self, system_prompt: &str, user_prompt: &str) -> Result<String, ExtractorError> {
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_prompt.to_string(),
            },
        ];

        self.ai_client
            .chat(&messages)
            .await
            .map_err(|e| ExtractorError::AiError(e.to_string()))
    }

    /// 解析AI响应，提取Memory标签
    fn parse_response(&self, raw_response: &str) -> Result<ExtractionResult, ExtractorError> {
        let mut memories = Vec::new();
        let mut warnings = Vec::new();
        
        // 使用正则提取 <Memory>...</Memory> 标签
        let re = regex::Regex::new(r"<Memory>([\s\S]*?)</Memory>")
            .map_err(|e| ExtractorError::ParseError(e.to_string()))?;
        
        for cap in re.captures_iter(raw_response) {
            if let Some(content) = cap.get(1) {
                let memory_content = content.as_str().trim();
                
                // 尝试解析为JSON格式（如果AI返回了结构化数据）
                if let Ok(parsed) = serde_json::from_str::<ExtractedMemory>(memory_content) {
                    memories.push(parsed);
                } else {
                    // 否则作为纯文本记忆
                    memories.push(ExtractedMemory {
                        content: memory_content.to_string(),
                        memory_type: None,
                        importance: None,
                        entities: Vec::new(),
                    });
                }
            }
        }
        
        // 如果没有找到任何Memory标签，记录警告
        if memories.is_empty() && !raw_response.trim().is_empty() {
            warnings.push("AI响应中未找到<Memory>标签".to_string());
        }
        
        Ok(ExtractionResult {
            memories,
            raw_response: raw_response.to_string(),
            parse_success: warnings.is_empty(),
            warnings,
        })
    }
}

// ============ 默认提示词 ============

const DEFAULT_EXTRACTION_PROMPT: &str = r#"你是一个信息提取专家。你的任务是从对话中提取所有有价值的、值得长期记忆的信息。

【提取原则】
1. 提取用户的个人信息（姓名、身份、职业等）
2. 提取用户的偏好和习惯
3. 提取重要的事实和决策
4. 提取项目进度和技术选型
5. 提取情感状态和重要约定
6. 忽略无意义的寒暄和过渡语

【输出格式】
将每条提取的信息用 <Memory></Memory> 标签包裹。
每个标签内只放一条独立的信息。
信息要简洁、完整、可独立理解。

【示例输出】
<Memory>用户的名字是张三</Memory>
<Memory>用户是一名Rust程序员，主要做后端开发</Memory>
<Memory>用户偏好函数式编程风格，不喜欢过度嵌套</Memory>
<Memory>2025-12-22 用户决定使用Qdrant作为向量数据库</Memory>

【注意事项】
- 如果对话中没有值得提取的信息，直接回复"无有价值信息"
- 不要编造信息，只提取对话中明确提到的内容
- 时间敏感的信息要标注日期
"#;

// API结构体已移至 AiClient，此处不再需要