//! 记忆管理器
//! 
//! 支持按助手隔离的记忆存储
//! 
//! 注意：SessionMemory 已被移除，思考池和短期记忆现在存储在 ConversationPacket 中

use super::{LongTermMemoryStore, LongTermStoreError, PendingMemoryStore};
use crate::ai::AiClient;
use std::collections::HashMap;
use std::sync::Arc;

/// 记忆管理器配置
#[derive(Debug, Clone)]
pub struct MemoryManagerConfig {
    pub qdrant_url: String,
    /// 基础 collection 名称前缀（实际名称为 {prefix}_{assistant_id}）
    pub collection_name: String,
    pub vector_size: u64,
    /// 基础文件存储目录（实际目录为 {dir}/{assistant_id}/files）
    pub file_storage_dir: String,
}

impl Default for MemoryManagerConfig {
    fn default() -> Self {
        Self {
            qdrant_url: "http://localhost:6333".to_string(),
            collection_name: "memories".to_string(),
            vector_size: 1536,
            file_storage_dir: "./data/assistants".to_string(),
        }
    }
}

/// 单个助手的记忆存储
/// 
/// 注意：会话级别的思考池和短期记忆现在存储在 ConversationPacket 中
pub struct AssistantMemory {
    /// 助手ID
    pub assistant_id: String,
    /// 长期记忆存储（独立的Qdrant collection）
    pub long_term: Arc<LongTermMemoryStore>,
    /// 待处理记忆存储
    pub pending: PendingMemoryStore,
}

/// 记忆管理器 - 统一管理所有助手的记忆存储
/// 
/// 注意：SessionMemory 已被移除，思考池和短期记忆现在存储在 ConversationPacket 中
pub struct MemoryManager {
    config: MemoryManagerConfig,
    ai_client: AiClient,
    /// 按助手ID索引的记忆存储
    assistants: HashMap<String, AssistantMemory>,
    /// 全局待处理记忆（兼容旧逻辑）
    global_pending: PendingMemoryStore,
    /// 全局长期记忆（兼容旧逻辑）
    global_long_term: Option<Arc<LongTermMemoryStore>>,
}

impl MemoryManager {
    /// 异步创建记忆管理器
    pub async fn new(
        config: MemoryManagerConfig,
        ai_client: AiClient,
    ) -> Result<Self, LongTermStoreError> {
        // 创建全局长期记忆存储（兼容旧逻辑）
        let global_collection = format!("{}_global", config.collection_name);
        let global_file_dir = format!("{}/global/files", config.file_storage_dir);
        
        let global_long_term = LongTermMemoryStore::new(
            &config.qdrant_url,
            &global_collection,
            config.vector_size,
            &global_file_dir,
            ai_client.clone(),
        )
        .await?;

        Ok(Self {
            config,
            ai_client,
            assistants: HashMap::new(),
            global_pending: PendingMemoryStore::new(),
            global_long_term: Some(Arc::new(global_long_term)),
        })
    }

    /// 获取或创建助手的记忆存储
    pub async fn get_or_create_assistant(
        &mut self,
        assistant_id: &str,
    ) -> Result<&mut AssistantMemory, LongTermStoreError> {
        self.get_or_create_assistant_with_embedding(assistant_id, None).await
    }

    /// 获取或创建助手的记忆存储（支持指定 embedding 模型）
    pub async fn get_or_create_assistant_with_embedding(
        &mut self,
        assistant_id: &str,
        embedding_model: Option<&str>,
    ) -> Result<&mut AssistantMemory, LongTermStoreError> {
        if !self.assistants.contains_key(assistant_id) {
            // 创建新的助手记忆存储
            let collection_name = format!("{}_{}", self.config.collection_name, assistant_id);
            let file_storage_dir = format!("{}/{}/files", self.config.file_storage_dir, assistant_id);
            
            // 确保目录存在
            if let Err(e) = tokio::fs::create_dir_all(&file_storage_dir).await {
                tracing::warn!("创建文件存储目录失败: {}", e);
            }

            // 根据是否有 embedding 模型选择创建方式
            let long_term = if let Some(model) = embedding_model {
                tracing::info!("助手 {} 使用 embedding 模型: {}", assistant_id, model);
                LongTermMemoryStore::new_with_embedding_model(
                    &self.config.qdrant_url,
                    &collection_name,
                    self.config.vector_size,
                    &file_storage_dir,
                    self.ai_client.clone(),
                    model.to_string(),
                )
                .await?
            } else {
                LongTermMemoryStore::new(
                    &self.config.qdrant_url,
                    &collection_name,
                    self.config.vector_size,
                    &file_storage_dir,
                    self.ai_client.clone(),
                )
                .await?
            };

            let assistant_memory = AssistantMemory {
                assistant_id: assistant_id.to_string(),
                long_term: Arc::new(long_term),
                pending: PendingMemoryStore::new(),
            };

            self.assistants.insert(assistant_id.to_string(), assistant_memory);
            tracing::info!("为助手 {} 创建了独立的记忆存储", assistant_id);
        } else if let Some(model) = embedding_model {
            // 助手已存在，动态更新 embedding 模型
            if let Some(assistant) = self.assistants.get(assistant_id) {
                assistant.long_term.set_embedding_model(Some(model.to_string())).await;
                tracing::debug!("更新助手 {} 的 embedding 模型: {}", assistant_id, model);
            }
        }

        Ok(self.assistants.get_mut(assistant_id).unwrap())
    }

    /// 获取助手的记忆存储（只读）
    pub fn get_assistant(&self, assistant_id: &str) -> Option<&AssistantMemory> {
        self.assistants.get(assistant_id)
    }

    /// 获取助手的长期记忆存储
    pub async fn get_assistant_long_term(
        &mut self,
        assistant_id: &str,
    ) -> Result<Arc<LongTermMemoryStore>, LongTermStoreError> {
        self.get_assistant_long_term_with_embedding(assistant_id, None).await
    }

    /// 获取助手的长期记忆存储（支持指定 embedding 模型）
    pub async fn get_assistant_long_term_with_embedding(
        &mut self,
        assistant_id: &str,
        embedding_model: Option<&str>,
    ) -> Result<Arc<LongTermMemoryStore>, LongTermStoreError> {
        let assistant = self.get_or_create_assistant_with_embedding(assistant_id, embedding_model).await?;
        Ok(Arc::clone(&assistant.long_term))
    }

    /// 获取助手的待处理记忆存储
    pub async fn get_assistant_pending(
        &mut self,
        assistant_id: &str,
    ) -> Result<&mut PendingMemoryStore, LongTermStoreError> {
        let assistant = self.get_or_create_assistant(assistant_id).await?;
        Ok(&mut assistant.pending)
    }

    // ==================== 兼容旧逻辑的方法 ====================

    /// 获取长期记忆存储（兼容旧逻辑）
    pub fn long_term_store(&self) -> &LongTermMemoryStore {
        self.global_long_term.as_ref().expect("全局长期记忆存储未初始化")
    }

    /// 获取长期记忆存储（Arc引用，兼容旧逻辑）
    pub fn long_term_store_arc(&self) -> Arc<LongTermMemoryStore> {
        Arc::clone(self.global_long_term.as_ref().expect("全局长期记忆存储未初始化"))
    }

    /// 获取待处理记忆存储（可变，兼容旧逻辑）
    pub fn pending_store_mut(&mut self) -> &mut PendingMemoryStore {
        &mut self.global_pending
    }

    /// 获取待处理记忆存储（只读，兼容旧逻辑）
    pub fn pending_store(&self) -> &PendingMemoryStore {
        &self.global_pending
    }

    /// 列出所有已加载的助手ID
    pub fn list_loaded_assistants(&self) -> Vec<&str> {
        self.assistants.keys().map(|s| s.as_str()).collect()
    }
}