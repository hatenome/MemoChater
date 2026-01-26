//! 短期记忆向量化器
//!
//! 将短期记忆向量化并持久化到话题专属文件
//! 位置：ContentChunker 之后执行

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, error, info, warn};

use crate::pipeline::{
    context::ProcessorContext,
    packet::ConversationPacket,
    processor::{Processor, ProcessorError},
};
use crate::types::ShortTermMemory;

/// 向量化后的短期记忆条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorizedMemory {
    pub id: String,
    pub summary: String,
    pub content: String,
    pub memory_type: String,
    pub source: String,
    pub timestamp: String,
    pub should_expand: bool,
    pub confidence: f32,
    pub embedding: Vec<f32>,
}

/// 短期记忆向量文件结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortTermVectorFile {
    pub vectors: Vec<VectorizedMemory>,
    pub metadata: VectorFileMetadata,
}

/// 向量文件元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorFileMetadata {
    pub embedding_model: String,
    pub dimension: usize,
    pub last_updated: String,
}

impl ShortTermVectorFile {
    pub fn new(embedding_model: String, dimension: usize) -> Self {
        Self {
            vectors: Vec::new(),
            metadata: VectorFileMetadata {
                embedding_model,
                dimension,
                last_updated: Utc::now().to_rfc3339(),
            },
        }
    }
}

/// 短期记忆向量化器
pub struct ShortTermVectorizer;

impl ShortTermVectorizer {
    pub fn new() -> Self {
        Self
    }

    /// 获取话题的向量文件路径
    fn get_vector_file_path(ctx: &ProcessorContext, packet: &ConversationPacket) -> PathBuf {
        let data_dir = ctx.global_config.data_dir.clone();
        PathBuf::from(data_dir)
            .join("assistants")
            .join(&packet.assistant_id)
            .join("topics")
            .join(&packet.topic_id)
            .join("short_term_vectors.json")
    }

    /// 加载现有的向量文件
    async fn load_vector_file(path: &PathBuf) -> Option<ShortTermVectorFile> {
        match fs::read_to_string(path).await {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(file) => Some(file),
                Err(e) => {
                    warn!("解析向量文件失败: {}", e);
                    None
                }
            },
            Err(_) => None,
        }
    }

    /// 保存向量文件
    async fn save_vector_file(path: &PathBuf, file: &ShortTermVectorFile) -> Result<(), ProcessorError> {
        // 确保目录存在
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                ProcessorError::Internal(format!("创建目录失败: {}", e))
            })?;
        }

        let content = serde_json::to_string_pretty(file).map_err(|e| {
            ProcessorError::Internal(format!("序列化向量文件失败: {}", e))
        })?;

        fs::write(path, content).await.map_err(|e| {
            ProcessorError::Internal(format!("写入向量文件失败: {}", e))
        })?;

        Ok(())
    }

    /// 将ShortTermMemory转换为可序列化的源字符串
    fn memory_source_to_string(memory: &ShortTermMemory) -> String {
        match &memory.source {
            crate::types::MemorySource::LongTermRetrieval => "LongTermRetrieval".to_string(),
            crate::types::MemorySource::CurrentConversation => "CurrentConversation".to_string(),
            crate::types::MemorySource::ToolResult => "ToolResult".to_string(),
        }
    }

    /// 为记忆生成向量
    async fn vectorize_memory(
        &self,
        memory: &ShortTermMemory,
        ctx: &ProcessorContext,
    ) -> Result<VectorizedMemory, ProcessorError> {
        // 使用summary生成embedding
        let embedding = ctx
            .ai_client
            .embedding_with_model(&memory.summary, Some(ctx.embedding_model()))
            .await
            .map_err(|e| ProcessorError::AiError(format!("生成embedding失败: {}", e)))?;

        Ok(VectorizedMemory {
            id: memory.id.clone(),
            summary: memory.summary.clone(),
            content: memory.content.clone(),
            memory_type: memory.memory_type.clone(),
            source: Self::memory_source_to_string(memory),
            timestamp: memory.timestamp.to_rfc3339(),
            should_expand: memory.should_expand,
            confidence: memory.confidence,
            embedding,
        })
    }
}

#[async_trait]
impl Processor for ShortTermVectorizer {
    fn name(&self) -> &'static str {
        "ShortTermVectorizer"
    }

    fn requires_memory(&self) -> bool {
        true
    }

    async fn process(
        &self,
        packet: &mut ConversationPacket,
        ctx: &ProcessorContext,
    ) -> Result<(), ProcessorError> {
        info!("ShortTermVectorizer 开始处理");

        // 获取当前短期记忆
        let memories = packet.get_short_term_memory();
        if memories.is_empty() {
            info!("没有短期记忆需要向量化");
            packet.set_processor_state(
                self.name(),
                serde_json::json!({
                    "vectorized": false,
                    "reason": "no_memories"
                }),
            );
            return Ok(());
        }

        let vector_file_path = Self::get_vector_file_path(ctx, packet);
        debug!("向量文件路径: {:?}", vector_file_path);

        // 加载现有向量文件或创建新的
        let mut vector_file = Self::load_vector_file(&vector_file_path)
            .await
            .unwrap_or_else(|| {
                info!("创建新的向量文件");
                ShortTermVectorFile::new(ctx.embedding_model().to_string(), 0)
            });

        // 收集需要向量化的记忆（新增或更新的）
        let existing_ids: std::collections::HashSet<String> = vector_file
            .vectors
            .iter()
            .map(|v| v.id.clone())
            .collect();

        let mut new_count = 0;
        let mut update_count = 0;

        for memory in memories {
            let is_new = !existing_ids.contains(&memory.id);

            // 生成向量
            match self.vectorize_memory(memory, ctx).await {
                Ok(vectorized) => {
                    // 更新维度信息（首次）
                    if vector_file.metadata.dimension == 0 {
                        vector_file.metadata.dimension = vectorized.embedding.len();
                    }

                    if is_new {
                        vector_file.vectors.push(vectorized);
                        new_count += 1;
                    } else {
                        // 更新已存在的记忆
                        if let Some(existing) = vector_file
                            .vectors
                            .iter_mut()
                            .find(|v| v.id == memory.id)
                        {
                            *existing = vectorized;
                            update_count += 1;
                        }
                    }
                }
                Err(e) => {
                    warn!("向量化记忆 {} 失败: {:?}", memory.id, e);
                    // 继续处理其他记忆
                }
            }
        }

        // 更新元数据
        vector_file.metadata.last_updated = Utc::now().to_rfc3339();
        vector_file.metadata.embedding_model = ctx.embedding_model().to_string();

        // 保存向量文件
        Self::save_vector_file(&vector_file_path, &vector_file).await?;

        info!(
            "向量化完成: 新增 {} 条, 更新 {} 条, 总计 {} 条",
            new_count,
            update_count,
            vector_file.vectors.len()
        );

        packet.set_processor_state(
            self.name(),
            serde_json::json!({
                "vectorized": true,
                "new_count": new_count,
                "update_count": update_count,
                "total_count": vector_file.vectors.len(),
                "file_path": vector_file_path.to_string_lossy()
            }),
        );

        Ok(())
    }
}