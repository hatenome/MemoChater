//! 助手管理器

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::Utc;

use super::types::*;
use super::storage::{AssistantStorage, StorageError};
use crate::types::ChatMessage;

/// 助手管理器错误
#[derive(Debug, thiserror::Error)]
pub enum ManagerError {
    #[error("存储错误: {0}")]
    Storage(#[from] StorageError),
    
    #[error("助手已存在: {0}")]
    AssistantExists(String),
    
    #[error("助手不存在: {0}")]
    AssistantNotFound(String),
    
    #[error("话题不存在: {0}")]
    TopicNotFound(String),
}

/// 助手管理器
/// 
/// 负责助手和话题的CRUD操作，以及运行时缓存管理
pub struct AssistantManager {
    storage: AssistantStorage,
    /// 缓存的助手配置
    config_cache: RwLock<HashMap<AssistantId, AssistantConfig>>,
}

impl AssistantManager {
    /// 创建新的管理器
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            storage: AssistantStorage::new(data_dir),
            config_cache: RwLock::new(HashMap::new()),
        }
    }
    
    /// 初始化（确保目录存在）
    pub async fn init(&self) -> Result<(), ManagerError> {
        self.storage.ensure_base_dir().await?;
        Ok(())
    }
    
    // ==================== 助手管理 ====================
    
    /// 创建新助手
    pub async fn create_assistant(&self, name: String, description: String) -> Result<AssistantId, ManagerError> {
        let id = generate_id("ast");
        
        if self.storage.assistant_exists(&id).await {
            return Err(ManagerError::AssistantExists(id));
        }
        
        let mut config = AssistantConfig::default();
        config.name = name;
        config.description = description;
        
        self.storage.create_assistant(&id, &config).await?;
        
        // 更新缓存
        self.config_cache.write().await.insert(id.clone(), config);
        
        Ok(id)
    }
    
    /// 获取助手配置
    pub async fn get_assistant(&self, id: &str) -> Result<AssistantConfig, ManagerError> {
        // 先查缓存
        if let Some(config) = self.config_cache.read().await.get(id) {
            return Ok(config.clone());
        }
        
        // 从存储加载
        let config = self.storage.load_assistant_config(id).await?;
        
        // 更新缓存
        self.config_cache.write().await.insert(id.to_string(), config.clone());
        
        Ok(config)
    }
    
    /// 更新助手配置
    pub async fn update_assistant(&self, id: &str, mut config: AssistantConfig) -> Result<(), ManagerError> {
        if !self.storage.assistant_exists(id).await {
            return Err(ManagerError::AssistantNotFound(id.to_string()));
        }
        
        config.updated_at = Utc::now();
        self.storage.save_assistant_config(id, &config).await?;
        
        // 更新缓存
        self.config_cache.write().await.insert(id.to_string(), config);
        
        Ok(())
    }
    
    /// 删除助手
    pub async fn delete_assistant(&self, id: &str) -> Result<(), ManagerError> {
        self.storage.delete_assistant(id).await?;
        self.config_cache.write().await.remove(id);
        Ok(())
    }
    
    /// 列出所有助手
    pub async fn list_assistants(&self) -> Result<Vec<AssistantSummary>, ManagerError> {
        let ids = self.storage.list_assistant_ids().await?;
        let mut summaries = Vec::new();
        
        for id in ids {
            if let Ok(config) = self.get_assistant(&id).await {
                let topic_count = self.storage.list_topic_ids(&id).await.unwrap_or_default().len();
                summaries.push(AssistantSummary {
                    id,
                    name: config.name,
                    description: config.description,
                    topic_count,
                    created_at: config.created_at,
                    updated_at: config.updated_at,
                });
            }
        }
        
        // 按更新时间倒序
        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        
        Ok(summaries)
    }
    
    // ==================== 话题管理 ====================
    
    /// 创建新话题
    pub async fn create_topic(&self, assistant_id: &str, title: String, topic_type: TopicType) -> Result<TopicMeta, ManagerError> {
        if !self.storage.assistant_exists(assistant_id).await {
            return Err(ManagerError::AssistantNotFound(assistant_id.to_string()));
        }
        
        let topic_id = generate_id("topic");
        let topic_meta = self.storage.create_topic(assistant_id, &topic_id, &title, topic_type).await?;
        
        Ok(topic_meta)
    }
    
    /// 获取话题元信息
    pub async fn get_topic(&self, assistant_id: &str, topic_id: &str) -> Result<TopicMeta, ManagerError> {
        let meta = self.storage.load_topic_meta(assistant_id, topic_id).await?;
        Ok(meta)
    }
    
    /// 更新话题标题
    pub async fn update_topic_title(&self, assistant_id: &str, topic_id: &str, title: String) -> Result<(), ManagerError> {
        let mut meta = self.storage.load_topic_meta(assistant_id, topic_id).await?;
        meta.title = title;
        meta.updated_at = Utc::now();
        self.storage.save_topic_meta(assistant_id, topic_id, &meta).await?;
        Ok(())
    }
    
    /// 删除话题
    pub async fn delete_topic(&self, assistant_id: &str, topic_id: &str) -> Result<(), ManagerError> {
        self.storage.delete_topic(assistant_id, topic_id).await?;
        Ok(())
    }
    
    /// 列出助手的所有话题
    pub async fn list_topics(&self, assistant_id: &str) -> Result<Vec<TopicSummary>, ManagerError> {
        let ids = self.storage.list_topic_ids(assistant_id).await?;
        let mut summaries = Vec::new();
        
        for topic_id in ids {
            if let Ok(meta) = self.storage.load_topic_meta(assistant_id, &topic_id).await {
                summaries.push(TopicSummary {
                    id: meta.id,
                    assistant_id: assistant_id.to_string(),
                    title: meta.title,
                    topic_type: meta.topic_type,
                    message_count: meta.message_count,
                    created_at: meta.created_at,
                    updated_at: meta.updated_at,
                });
            }
        }
        
        // 按更新时间倒序
        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        
        Ok(summaries)
    }
    
    // ==================== 对话历史管理 ====================
    
    /// 获取对话历史
    pub async fn get_history(&self, assistant_id: &str, topic_id: &str) -> Result<Vec<ChatMessage>, ManagerError> {
        let messages = self.storage.load_history(assistant_id, topic_id).await?;
        Ok(messages)
    }
    
    /// 保存对话历史
    pub async fn save_history(&self, assistant_id: &str, topic_id: &str, messages: &[ChatMessage]) -> Result<(), ManagerError> {
        self.storage.save_history(assistant_id, topic_id, messages).await?;
        Ok(())
    }
    
    /// 追加消息
    pub async fn append_message(&self, assistant_id: &str, topic_id: &str, message: ChatMessage) -> Result<(), ManagerError> {
        self.storage.append_message(assistant_id, topic_id, message).await?;
        Ok(())
    }
    
    /// 批量追加消息（一次IO，效率更高）
    pub async fn append_messages(&self, assistant_id: &str, topic_id: &str, messages: Vec<ChatMessage>) -> Result<(), ManagerError> {
        self.storage.append_messages(assistant_id, topic_id, messages).await?;
        Ok(())
    }
    
    /// 清空对话历史
    pub async fn clear_history(&self, assistant_id: &str, topic_id: &str) -> Result<(), ManagerError> {
        self.storage.save_history(assistant_id, topic_id, &[]).await?;
        Ok(())
    }
    
    /// 更新指定索引的消息
    pub async fn update_message(&self, assistant_id: &str, topic_id: &str, index: usize, content: &str) -> Result<(), ManagerError> {
        let mut messages = self.storage.load_history(assistant_id, topic_id).await?;
        if index >= messages.len() {
            return Err(ManagerError::TopicNotFound(format!("消息索引 {} 超出范围", index)));
        }
        messages[index].content = content.to_string();
        self.storage.save_history(assistant_id, topic_id, &messages).await?;
        Ok(())
    }
    
    /// 删除指定索引的消息
    pub async fn delete_message(&self, assistant_id: &str, topic_id: &str, index: usize) -> Result<(), ManagerError> {
        let mut messages = self.storage.load_history(assistant_id, topic_id).await?;
        if index >= messages.len() {
            return Err(ManagerError::TopicNotFound(format!("消息索引 {} 超出范围", index)));
        }
        messages.remove(index);
        self.storage.save_history(assistant_id, topic_id, &messages).await?;
        Ok(())
    }
    
    /// 从指定位置创建分支话题
    pub async fn create_branch_topic(
        &self, 
        assistant_id: &str, 
        topic_id: &str, 
        from_index: usize,
        title: Option<String>,
    ) -> Result<TopicMeta, ManagerError> {
        // 加载原话题的元信息（获取话题类型）
        let original_meta = self.storage.load_topic_meta(assistant_id, topic_id).await?;
        
        // 加载原话题的消息
        let messages = self.storage.load_history(assistant_id, topic_id).await?;
        if from_index > messages.len() {
            return Err(ManagerError::TopicNotFound(format!("消息索引 {} 超出范围", from_index)));
        }
        
        // 截取消息（从0到from_index，不包含from_index）
        let branch_messages: Vec<ChatMessage> = messages[..from_index].to_vec();
        
        // 生成分支话题标题
        let branch_title = title.unwrap_or_else(|| {
            format!("分支话题 (从消息#{})", from_index)
        });
        
        // 创建新话题ID
        let new_topic_id = generate_id("topic");
        
        // 创建新话题（继承原话题的类型）
        let new_topic = self.storage.create_topic(assistant_id, &new_topic_id, &branch_title, original_meta.topic_type).await?;
        
        // 保存截取的消息到新话题
        if !branch_messages.is_empty() {
            self.storage.save_history(assistant_id, &new_topic.id, &branch_messages).await?;
        }
        
        Ok(new_topic)
    }
}

/// 生成唯一ID
fn generate_id(prefix: &str) -> String {
    let timestamp = Utc::now().timestamp_millis();
    let random: u32 = rand::random::<u32>() % 10000;
    format!("{}_{:x}_{:04}", prefix, timestamp, random)
}