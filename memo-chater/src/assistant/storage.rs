//! 助手文件系统存储操作

use std::path::{Path, PathBuf};
use tokio::fs;
use crate::types::ChatMessage;
use super::types::*;

/// 存储错误
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("序列化错误: {0}")]
    Serialize(String),
    
    #[error("反序列化错误: {0}")]
    Deserialize(String),
    
    #[error("助手不存在: {0}")]
    AssistantNotFound(String),
    
    #[error("话题不存在: {0}")]
    TopicNotFound(String),
}

/// 助手存储操作
pub struct AssistantStorage {
    /// 助手根目录 (data_dir/assistants)
    base_path: PathBuf,
}

impl AssistantStorage {
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            base_path: data_dir.as_ref().join("assistants"),
        }
    }
    
    /// 确保基础目录存在
    pub async fn ensure_base_dir(&self) -> Result<(), StorageError> {
        fs::create_dir_all(&self.base_path).await?;
        Ok(())
    }
    
    // ==================== 路径辅助方法 ====================
    
    /// 获取助手目录路径
    fn assistant_dir(&self, assistant_id: &str) -> PathBuf {
        self.base_path.join(assistant_id)
    }
    
    /// 获取助手配置文件路径
    fn assistant_config_path(&self, assistant_id: &str) -> PathBuf {
        self.assistant_dir(assistant_id).join("config.toml")
    }
    
    /// 获取话题目录路径
    fn topics_dir(&self, assistant_id: &str) -> PathBuf {
        self.assistant_dir(assistant_id).join("topics")
    }
    
    /// 获取单个话题目录路径
    fn topic_dir(&self, assistant_id: &str, topic_id: &str) -> PathBuf {
        self.topics_dir(assistant_id).join(topic_id)
    }
    
    /// 获取话题元信息文件路径
    fn topic_meta_path(&self, assistant_id: &str, topic_id: &str) -> PathBuf {
        self.topic_dir(assistant_id, topic_id).join("meta.toml")
    }
    
    /// 获取话题历史文件路径
    fn topic_history_path(&self, assistant_id: &str, topic_id: &str) -> PathBuf {
        self.topic_dir(assistant_id, topic_id).join("history.json")
    }
    
    /// 获取助手记忆目录路径
    fn memory_dir(&self, assistant_id: &str) -> PathBuf {
        self.assistant_dir(assistant_id).join("memory")
    }
    
    // ==================== 助手操作 ====================
    
    /// 创建助手
    pub async fn create_assistant(&self, id: &str, config: &AssistantConfig) -> Result<(), StorageError> {
        let dir = self.assistant_dir(id);
        fs::create_dir_all(&dir).await?;
        fs::create_dir_all(self.topics_dir(id)).await?;
        fs::create_dir_all(self.memory_dir(id)).await?;
        
        self.save_assistant_config(id, config).await?;
        Ok(())
    }
    
    /// 保存助手配置
    pub async fn save_assistant_config(&self, id: &str, config: &AssistantConfig) -> Result<(), StorageError> {
        let path = self.assistant_config_path(id);
        let content = toml::to_string_pretty(config)
            .map_err(|e| StorageError::Serialize(e.to_string()))?;
        fs::write(path, content).await?;
        Ok(())
    }
    
    /// 加载助手配置
    pub async fn load_assistant_config(&self, id: &str) -> Result<AssistantConfig, StorageError> {
        let path = self.assistant_config_path(id);
        if !path.exists() {
            return Err(StorageError::AssistantNotFound(id.to_string()));
        }
        
        let content = fs::read_to_string(path).await?;
        let config: AssistantConfig = toml::from_str(&content)
            .map_err(|e| StorageError::Deserialize(e.to_string()))?;
        Ok(config)
    }
    
    /// 删除助手（包括所有话题）
    pub async fn delete_assistant(&self, id: &str) -> Result<(), StorageError> {
        let dir = self.assistant_dir(id);
        if fs::try_exists(&dir).await.unwrap_or(false) {
            fs::remove_dir_all(dir).await?;
        }
        Ok(())
    }
    
    /// 列出所有助手ID
    pub async fn list_assistant_ids(&self) -> Result<Vec<String>, StorageError> {
        self.ensure_base_dir().await?;
        
        let mut ids = Vec::new();
        let mut entries = fs::read_dir(&self.base_path).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    // 检查是否有config.toml
                    let config_path = self.assistant_config_path(name);
                    if fs::try_exists(&config_path).await.unwrap_or(false) {
                        ids.push(name.to_string());
                    }
                }
            }
        }
        
        Ok(ids)
    }
    
    /// 检查助手是否存在
    pub async fn assistant_exists(&self, id: &str) -> bool {
        fs::try_exists(self.assistant_config_path(id)).await.unwrap_or(false)
    }
    
    // ==================== 话题操作 ====================
    
    /// 创建话题
    pub async fn create_topic(&self, assistant_id: &str, topic_id: &str, title: &str, topic_type: TopicType) -> Result<TopicMeta, StorageError> {
        if !self.assistant_exists(assistant_id).await {
            return Err(StorageError::AssistantNotFound(assistant_id.to_string()));
        }
        
        let dir = self.topic_dir(assistant_id, topic_id);
        fs::create_dir_all(&dir).await?;
        
        let meta = TopicMeta::new(topic_id.to_string(), title.to_string(), topic_type);
        self.save_topic_meta(assistant_id, topic_id, &meta).await?;
        
        // 创建空的历史文件
        let history_path = self.topic_history_path(assistant_id, topic_id);
        fs::write(history_path, "[]").await?;
        
        Ok(meta)
    }
    
    /// 保存话题元信息
    pub async fn save_topic_meta(&self, assistant_id: &str, topic_id: &str, meta: &TopicMeta) -> Result<(), StorageError> {
        let path = self.topic_meta_path(assistant_id, topic_id);
        let content = toml::to_string_pretty(meta)
            .map_err(|e| StorageError::Serialize(e.to_string()))?;
        fs::write(path, content).await?;
        Ok(())
    }
    
    /// 加载话题元信息
    pub async fn load_topic_meta(&self, assistant_id: &str, topic_id: &str) -> Result<TopicMeta, StorageError> {
        let path = self.topic_meta_path(assistant_id, topic_id);
        if !fs::try_exists(&path).await.unwrap_or(false) {
            return Err(StorageError::TopicNotFound(topic_id.to_string()));
        }
        
        let content = fs::read_to_string(path).await?;
        let meta: TopicMeta = toml::from_str(&content)
            .map_err(|e| StorageError::Deserialize(e.to_string()))?;
        Ok(meta)
    }
    
    /// 删除话题
    pub async fn delete_topic(&self, assistant_id: &str, topic_id: &str) -> Result<(), StorageError> {
        let dir = self.topic_dir(assistant_id, topic_id);
        if fs::try_exists(&dir).await.unwrap_or(false) {
            fs::remove_dir_all(dir).await?;
        }
        Ok(())
    }
    
    /// 列出助手的所有话题ID
    pub async fn list_topic_ids(&self, assistant_id: &str) -> Result<Vec<String>, StorageError> {
        let topics_dir = self.topics_dir(assistant_id);
        if !fs::try_exists(&topics_dir).await.unwrap_or(false) {
            return Ok(Vec::new());
        }
        
        let mut ids = Vec::new();
        let mut entries = fs::read_dir(&topics_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    ids.push(name.to_string());
                }
            }
        }
        
        Ok(ids)
    }
    
    // ==================== 对话历史操作 ====================
    
    /// 加载对话历史
    pub async fn load_history(&self, assistant_id: &str, topic_id: &str) -> Result<Vec<ChatMessage>, StorageError> {
        let path = self.topic_history_path(assistant_id, topic_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        
        let content = fs::read_to_string(path).await?;
        let messages: Vec<ChatMessage> = serde_json::from_str(&content)
            .map_err(|e| StorageError::Deserialize(e.to_string()))?;
        Ok(messages)
    }
    
    /// 保存对话历史
    pub async fn save_history(&self, assistant_id: &str, topic_id: &str, messages: &[ChatMessage]) -> Result<(), StorageError> {
        let path = self.topic_history_path(assistant_id, topic_id);
        let content = serde_json::to_string_pretty(messages)
            .map_err(|e| StorageError::Serialize(e.to_string()))?;
        fs::write(path, content).await?;
        
        // 更新话题元信息的消息数量
        if let Ok(mut meta) = self.load_topic_meta(assistant_id, topic_id).await {
            meta.message_count = messages.len();
            meta.updated_at = chrono::Utc::now();
            let _ = self.save_topic_meta(assistant_id, topic_id, &meta).await;
        }
        
        Ok(())
    }
    
    /// 追加消息到历史
    pub async fn append_message(&self, assistant_id: &str, topic_id: &str, message: ChatMessage) -> Result<(), StorageError> {
        let mut messages = self.load_history(assistant_id, topic_id).await?;
        messages.push(message);
        self.save_history(assistant_id, topic_id, &messages).await
    }
    
    /// 批量追加消息（一次读写，效率更高）
    pub async fn append_messages(&self, assistant_id: &str, topic_id: &str, new_messages: Vec<ChatMessage>) -> Result<(), StorageError> {
        if new_messages.is_empty() {
            return Ok(());
        }
        let mut messages = self.load_history(assistant_id, topic_id).await?;
        messages.extend(new_messages);
        self.save_history(assistant_id, topic_id, &messages).await
    }
}