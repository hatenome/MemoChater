//! 文件内容存储
//!
//! 使用 JSON 文件存储记忆关联的文件内容（代码块等）
//! 避免 SQLite 的 C 编译依赖

use crate::types::MemoryFile;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// 文件存储错误
#[derive(Debug, thiserror::Error)]
pub enum FileStoreError {
    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("文件不存在: {0}")]
    NotFound(String),
}

/// 存储索引结构
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct FileIndex {
    /// 文件ID -> 文件名映射
    files: HashMap<String, String>,
    /// 记忆ID -> 文件ID列表映射
    memory_files: HashMap<String, Vec<String>>,
}

/// 文件内容存储
/// 
/// 存储结构：
/// ```
/// storage_dir/
/// ├── index.json          # 索引文件
/// └── files/
///     ├── {uuid1}.json    # 单个文件内容
///     ├── {uuid2}.json
///     └── ...
/// ```
pub struct FileStore {
    storage_dir: PathBuf,
    index: FileIndex,
}

impl FileStore {
    /// 创建或加载文件存储
    pub async fn new(storage_dir: impl Into<PathBuf>) -> Result<Self, FileStoreError> {
        let storage_dir = storage_dir.into();
        
        // 确保目录存在
        fs::create_dir_all(&storage_dir).await?;
        fs::create_dir_all(storage_dir.join("files")).await?;
        
        // 加载或创建索引
        let index_path = storage_dir.join("index.json");
        let index = if index_path.exists() {
            let content = fs::read_to_string(&index_path).await?;
            serde_json::from_str(&content)?
        } else {
            FileIndex::default()
        };
        
        Ok(Self { storage_dir, index })
    }

    /// 保存索引
    async fn save_index(&self) -> Result<(), FileStoreError> {
        let index_path = self.storage_dir.join("index.json");
        let content = serde_json::to_string_pretty(&self.index)?;
        fs::write(&index_path, content).await?;
        Ok(())
    }

    /// 存储单个文件
    pub async fn store(&mut self, file: &MemoryFile) -> Result<(), FileStoreError> {
        // 写入文件内容
        let file_path = self.storage_dir.join("files").join(format!("{}.json", file.id));
        let content = serde_json::to_string_pretty(file)?;
        fs::write(&file_path, content).await?;
        
        // 更新索引
        self.index.files.insert(file.id.clone(), format!("{}.json", file.id));
        self.index.memory_files
            .entry(file.memory_id.clone())
            .or_default()
            .push(file.id.clone());
        
        self.save_index().await?;
        Ok(())
    }

    /// 批量存储文件
    pub async fn store_batch(&mut self, files: &[MemoryFile]) -> Result<(), FileStoreError> {
        for file in files {
            // 写入文件内容
            let file_path = self.storage_dir.join("files").join(format!("{}.json", file.id));
            let content = serde_json::to_string_pretty(file)?;
            fs::write(&file_path, content).await?;
            
            // 更新索引
            self.index.files.insert(file.id.clone(), format!("{}.json", file.id));
            self.index.memory_files
                .entry(file.memory_id.clone())
                .or_default()
                .push(file.id.clone());
        }
        
        self.save_index().await?;
        Ok(())
    }

    /// 根据ID获取文件
    pub async fn get(&self, id: &str) -> Result<MemoryFile, FileStoreError> {
        let filename = self.index.files.get(id)
            .ok_or_else(|| FileStoreError::NotFound(id.to_string()))?;
        
        let file_path = self.storage_dir.join("files").join(filename);
        let content = fs::read_to_string(&file_path).await?;
        let file: MemoryFile = serde_json::from_str(&content)?;
        Ok(file)
    }

    /// 获取记忆关联的所有文件
    pub async fn get_by_memory(&self, memory_id: &str) -> Result<Vec<MemoryFile>, FileStoreError> {
        let file_ids = self.index.memory_files.get(memory_id)
            .cloned()
            .unwrap_or_default();
        
        let mut files = Vec::new();
        for id in file_ids {
            match self.get(&id).await {
                Ok(file) => files.push(file),
                Err(FileStoreError::NotFound(_)) => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(files)
    }

    /// 删除文件
    pub async fn delete(&mut self, id: &str) -> Result<(), FileStoreError> {
        if let Some(filename) = self.index.files.remove(id) {
            let file_path = self.storage_dir.join("files").join(&filename);
            if file_path.exists() {
                fs::remove_file(&file_path).await?;
            }
            
            // 从memory_files中移除
            for file_ids in self.index.memory_files.values_mut() {
                file_ids.retain(|fid| fid != id);
            }
            
            self.save_index().await?;
        }
        Ok(())
    }

    /// 删除记忆关联的所有文件
    pub async fn delete_by_memory(&mut self, memory_id: &str) -> Result<usize, FileStoreError> {
        let file_ids = self.index.memory_files.remove(memory_id)
            .unwrap_or_default();
        
        let count = file_ids.len();
        for id in file_ids {
            if let Some(filename) = self.index.files.remove(&id) {
                let file_path = self.storage_dir.join("files").join(&filename);
                if file_path.exists() {
                    let _ = fs::remove_file(&file_path).await;
                }
            }
        }
        
        self.save_index().await?;
        Ok(count)
    }

    /// 获取存储统计
    pub fn stats(&self) -> FileStoreStats {
        FileStoreStats {
            total_files: self.index.files.len(),
            total_memories: self.index.memory_files.len(),
        }
    }
}

/// 存储统计
#[derive(Debug, Clone)]
pub struct FileStoreStats {
    pub total_files: usize,
    pub total_memories: usize,
}