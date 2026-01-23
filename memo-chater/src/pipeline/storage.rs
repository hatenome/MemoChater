//! 对话数据包持久化
//!
//! 负责 ConversationPacket 的加载和保存

use std::path::PathBuf;
use tokio::fs;

use super::packet::ConversationPacket;

/// 数据包存储文件名
const PACKET_FILENAME: &str = "conversation_state.json";

/// 数据包存储
pub struct PacketStorage {
    /// 数据根目录
    data_dir: PathBuf,
}

impl PacketStorage {
    /// 创建数据包存储
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// 获取数据包文件路径
    fn get_packet_path(&self, assistant_id: &str, topic_id: &str) -> PathBuf {
        self.data_dir
            .join("assistants")
            .join(assistant_id)
            .join("topics")
            .join(topic_id)
            .join(PACKET_FILENAME)
    }

    /// 加载数据包
    ///
    /// 如果文件不存在，返回 Ok(None)
    pub async fn load(
        &self,
        assistant_id: &str,
        topic_id: &str,
    ) -> Result<Option<ConversationPacket>, StorageError> {
        let path = self.get_packet_path(assistant_id, topic_id);

        if !path.exists() {
            tracing::debug!("数据包文件不存在: {:?}", path);
            return Ok(None);
        }

        let content = fs::read_to_string(&path)
            .await
            .map_err(|e| StorageError::ReadFailed(path.clone(), e.to_string()))?;

        let packet: ConversationPacket = serde_json::from_str(&content)
            .map_err(|e| StorageError::ParseFailed(path.clone(), e.to_string()))?;

        tracing::debug!("加载数据包成功: {:?}", path);
        Ok(Some(packet))
    }

    /// 保存数据包
    pub async fn save(&self, packet: &ConversationPacket) -> Result<(), StorageError> {
        let path = self.get_packet_path(&packet.assistant_id, &packet.topic_id);

        // 确保目录存在
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| StorageError::CreateDirFailed(parent.to_path_buf(), e.to_string()))?;
        }

        let content = serde_json::to_string_pretty(packet)
            .map_err(|e| StorageError::SerializeFailed(e.to_string()))?;

        fs::write(&path, content)
            .await
            .map_err(|e| StorageError::WriteFailed(path.clone(), e.to_string()))?;

        tracing::debug!("保存数据包成功: {:?}", path);
        Ok(())
    }

    /// 删除数据包
    pub async fn delete(
        &self,
        assistant_id: &str,
        topic_id: &str,
    ) -> Result<(), StorageError> {
        let path = self.get_packet_path(assistant_id, topic_id);

        if path.exists() {
            fs::remove_file(&path)
                .await
                .map_err(|e| StorageError::DeleteFailed(path.clone(), e.to_string()))?;
            tracing::debug!("删除数据包成功: {:?}", path);
        }

        Ok(())
    }

    /// 检查数据包是否存在
    pub async fn exists(&self, assistant_id: &str, topic_id: &str) -> bool {
        let path = self.get_packet_path(assistant_id, topic_id);
        path.exists()
    }
}

/// 存储错误
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("读取文件失败 {0:?}: {1}")]
    ReadFailed(PathBuf, String),

    #[error("解析文件失败 {0:?}: {1}")]
    ParseFailed(PathBuf, String),

    #[error("创建目录失败 {0:?}: {1}")]
    CreateDirFailed(PathBuf, String),

    #[error("序列化失败: {0}")]
    SerializeFailed(String),

    #[error("写入文件失败 {0:?}: {1}")]
    WriteFailed(PathBuf, String),

    #[error("删除文件失败 {0:?}: {1}")]
    DeleteFailed(PathBuf, String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_save_and_load() {
        let temp_dir = tempdir().unwrap();
        let storage = PacketStorage::new(temp_dir.path().to_path_buf());

        let packet = ConversationPacket::new(
            "ast_001".to_string(),
            "topic_001".to_string(),
            "秦".to_string(),
            "诺亚".to_string(),
        );

        // 保存
        storage.save(&packet).await.unwrap();

        // 加载
        let loaded = storage.load("ast_001", "topic_001").await.unwrap();
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert_eq!(loaded.assistant_id, "ast_001");
        assert_eq!(loaded.user_name, "秦");
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let temp_dir = tempdir().unwrap();
        let storage = PacketStorage::new(temp_dir.path().to_path_buf());

        let result = storage.load("nonexistent", "topic").await.unwrap();
        assert!(result.is_none());
    }
}