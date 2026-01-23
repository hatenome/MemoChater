//! 长期记忆存储
//!
//! 基于 Qdrant 向量数据库的长期记忆存储

use crate::ai::client::AiClient;
use crate::storage::FileStore;
use crate::types::{LongTermMemory, MemoryFile, RawExtractedMemory};
use crate::vector::{
    PayloadValue, SearchFilter, SearchResult, VectorPoint, VectorStore, VectorStoreConfig,
    VectorStoreError,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 长期记忆存储错误
#[derive(Debug, thiserror::Error)]
pub enum LongTermStoreError {
    #[error("向量存储错误: {0}")]
    VectorStore(#[from] VectorStoreError),
    #[error("AI调用错误: {0}")]
    AiError(String),
    #[error("文件存储错误: {0}")]
    FileStore(String),
    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// 检索到的记忆
#[derive(Debug, Clone)]
pub struct RetrievedMemory {
    pub memory: LongTermMemory,
    pub relevance: f32,
}

/// 长期记忆存储
pub struct LongTermMemoryStore {
    vector_store: VectorStore,
    file_store: Arc<RwLock<FileStore>>,
    ai_client: AiClient,
    /// 覆盖的 embedding 模型（如果设置，优先使用此模型）
    /// 使用 RwLock 支持动态更新
    embedding_model_override: Arc<RwLock<Option<String>>>,
}

impl LongTermMemoryStore {
    /// 创建新的长期记忆存储
    pub async fn new(
        qdrant_url: &str,
        collection_name: &str,
        vector_size: u64,
        file_storage_dir: &str,
        ai_client: AiClient,
    ) -> Result<Self, LongTermStoreError> {
        let config = VectorStoreConfig::new(collection_name, vector_size)
            .with_url(qdrant_url);

        let vector_store = VectorStore::new(config).await?;

        let file_store = FileStore::new(file_storage_dir)
            .await
            .map_err(|e| LongTermStoreError::FileStore(e.to_string()))?;

        Ok(Self {
            vector_store,
            file_store: Arc::new(RwLock::new(file_store)),
            ai_client,
            embedding_model_override: Arc::new(RwLock::new(None)),
        })
    }

    /// 创建带有指定 embedding 模型的存储
    pub async fn new_with_embedding_model(
        qdrant_url: &str,
        collection_name: &str,
        vector_size: u64,
        file_storage_dir: &str,
        ai_client: AiClient,
        embedding_model: String,
    ) -> Result<Self, LongTermStoreError> {
        let config = VectorStoreConfig::new(collection_name, vector_size)
            .with_url(qdrant_url);

        let vector_store = VectorStore::new(config).await?;

        let file_store = FileStore::new(file_storage_dir)
            .await
            .map_err(|e| LongTermStoreError::FileStore(e.to_string()))?;

        Ok(Self {
            vector_store,
            file_store: Arc::new(RwLock::new(file_store)),
            ai_client,
            embedding_model_override: Arc::new(RwLock::new(Some(embedding_model))),
        })
    }

    /// 动态更新 embedding 模型
    pub async fn set_embedding_model(&self, model: Option<String>) {
        *self.embedding_model_override.write().await = model;
    }

    /// 获取当前的 embedding 模型覆盖
    pub async fn get_embedding_model_override(&self) -> Option<String> {
        self.embedding_model_override.read().await.clone()
    }

    /// 生成 embedding（使用覆盖模型或默认模型）
    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>, LongTermStoreError> {
        let model_override = self.embedding_model_override.read().await;
        self.ai_client
            .embedding_with_model(text, model_override.as_deref())
            .await
            .map_err(|e| LongTermStoreError::AiError(e.to_string()))
    }

    /// 存储记忆（自动生成embedding）
    pub async fn store(&self, memory: &LongTermMemory) -> Result<(), LongTermStoreError> {
        // 生成embedding
        let embedding = self.generate_embedding(&memory.content).await?;

        // 构建payload
        let payload = self.memory_to_payload(memory);

        let point = VectorPoint {
            id: memory.id.clone(),
            vector: embedding,
            payload,
        };

        self.vector_store.upsert(point).await?;
        Ok(())
    }

    /// 存储原始提取的记忆（含文件分离处理）
    pub async fn store_raw(
        &self,
        raw: RawExtractedMemory,
        session_id: Option<String>,
    ) -> Result<LongTermMemory, LongTermStoreError> {
        let (memory, files) = raw.into_storable(session_id);

        // 存储关联文件
        if !files.is_empty() {
            let mut file_store = self.file_store.write().await;
            file_store
                .store_batch(&files)
                .await
                .map_err(|e| LongTermStoreError::FileStore(e.to_string()))?;
        }

        // 存储记忆到向量库
        self.store(&memory).await?;

        Ok(memory)
    }

    /// 批量存储记忆
    /// 批量生成 embedding（使用覆盖模型或默认模型）
    async fn generate_embedding_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, LongTermStoreError> {
        let model_override = self.embedding_model_override.read().await;
        self.ai_client
            .embedding_batch_with_model(texts, model_override.as_deref())
            .await
            .map_err(|e| LongTermStoreError::AiError(e.to_string()))
    }

    pub async fn store_batch(&self, memories: &[LongTermMemory]) -> Result<(), LongTermStoreError> {
        if memories.is_empty() {
            return Ok(());
        }

        // 批量生成embedding（使用覆盖模型或默认模型）
        let contents: Vec<String> = memories.iter().map(|m| m.content.clone()).collect();
        let embeddings = self.generate_embedding_batch(&contents).await?;

        // 构建向量点
        let points: Vec<VectorPoint> = memories
            .iter()
            .zip(embeddings.into_iter())
            .map(|(memory, embedding)| VectorPoint {
                id: memory.id.clone(),
                vector: embedding,
                payload: self.memory_to_payload(memory),
            })
            .collect();

        self.vector_store.upsert_batch(points).await?;
        Ok(())
    }

    /// 语义搜索相关记忆
    pub async fn search(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<RetrievedMemory>, LongTermStoreError> {
        self.search_with_filter(query, top_k, None, None).await
    }

    /// 带过滤条件的语义搜索
    pub async fn search_with_filter(
        &self,
        query: &str,
        top_k: usize,
        category: Option<&str>,
        min_importance: Option<f32>,
    ) -> Result<Vec<RetrievedMemory>, LongTermStoreError> {
        // 生成查询向量（使用覆盖模型或默认模型）
        let query_embedding = self.generate_embedding(query).await?;

        // 构建过滤条件
        let filter = self.build_filter(category, min_importance);

        // 执行搜索
        let results = self
            .vector_store
            .search(query_embedding, top_k as u64, filter)
            .await?;

        // 转换结果
        let memories = results
            .into_iter()
            .filter_map(|r| self.payload_to_memory(&r).map(|m| RetrievedMemory {
                memory: m,
                relevance: r.score,
            }))
            .collect();

        Ok(memories)
    }

    /// 根据ID获取记忆
    pub async fn get(&self, id: &str) -> Result<Option<LongTermMemory>, LongTermStoreError> {
        // Qdrant没有直接的get by id，用filter搜索
        let filter = SearchFilter::new().must_match("id", id);

        // 用零向量搜索（只依赖filter）
        let dummy_vector = vec![0.0f32; self.vector_store.config().vector_size as usize];
        let results = self
            .vector_store
            .search(dummy_vector, 1, Some(filter))
            .await?;

        Ok(results.first().and_then(|r| self.payload_to_memory(r)))
    }

    /// 更新记忆访问记录
    pub async fn record_access(&self, id: &str) -> Result<(), LongTermStoreError> {
        let now = chrono::Utc::now().timestamp();

        // 先获取当前access_count
        if let Some(memory) = self.get(id).await? {
            let mut payload = HashMap::new();
            payload.insert(
                "access_count".to_string(),
                PayloadValue::Integer((memory.access_count + 1) as i64),
            );
            payload.insert("last_accessed".to_string(), PayloadValue::Integer(now));

            self.vector_store.update_payload(id, payload).await?;
        }

        Ok(())
    }

    /// 删除记忆（同时删除关联文件）
    pub async fn delete(&self, id: &str) -> Result<(), LongTermStoreError> {
        // 删除关联文件
        let mut file_store = self.file_store.write().await;
        let _ = file_store.delete_by_memory(id).await;

        // 删除向量
        self.vector_store.delete(id).await?;
        Ok(())
    }

    /// 获取记忆关联的文件
    pub async fn get_files(&self, memory_id: &str) -> Result<Vec<MemoryFile>, LongTermStoreError> {
        let file_store = self.file_store.read().await;
        file_store
            .get_by_memory(memory_id)
            .await
            .map_err(|e| LongTermStoreError::FileStore(e.to_string()))
    }

    /// 获取存储统计
    pub async fn stats(&self) -> Result<StoreStats, LongTermStoreError> {
        let vector_count = self.vector_store.count().await?;
        let file_stats = self.file_store.read().await.stats();

        Ok(StoreStats {
            memory_count: vector_count,
            file_count: file_stats.total_files,
        })
    }

    /// 列出所有记忆（不需要搜索词）
    pub async fn list_all(&self, limit: usize) -> Result<Vec<RetrievedMemory>, LongTermStoreError> {
        // 使用 scroll API 获取所有记录
        let results = self
            .vector_store
            .scroll(limit as u64, None)
            .await?;

        // 转换结果
        let memories = results
            .into_iter()
            .filter_map(|r| self.payload_to_memory(&r).map(|m| RetrievedMemory {
                memory: m,
                relevance: 1.0, // 无搜索时默认相关度为1.0
            }))
            .collect();

        Ok(memories)
    }

    // ============ 私有辅助方法 ============

    fn memory_to_payload(&self, memory: &LongTermMemory) -> HashMap<String, PayloadValue> {
        let mut payload = HashMap::new();
        payload.insert("id".to_string(), PayloadValue::String(memory.id.clone()));
        payload.insert(
            "content".to_string(),
            PayloadValue::String(memory.content.clone()),
        );
        payload.insert(
            "category".to_string(),
            PayloadValue::String(memory.category.clone()),
        );
        payload.insert(
            "importance".to_string(),
            PayloadValue::Float(memory.importance as f64),
        );
        payload.insert(
            "access_count".to_string(),
            PayloadValue::Integer(memory.access_count as i64),
        );
        payload.insert(
            "last_accessed".to_string(),
            PayloadValue::Integer(memory.last_accessed.timestamp()),
        );
        payload.insert(
            "created_at".to_string(),
            PayloadValue::Integer(memory.created_at.timestamp()),
        );

        if let Some(ref session) = memory.source_session {
            payload.insert(
                "source_session".to_string(),
                PayloadValue::String(session.clone()),
            );
        }

        if !memory.file_refs.is_empty() {
            payload.insert(
                "file_refs".to_string(),
                PayloadValue::List(
                    memory
                        .file_refs
                        .iter()
                        .map(|s| PayloadValue::String(s.clone()))
                        .collect(),
                ),
            );
        }

        if !memory.tags.is_empty() {
            payload.insert(
                "tags".to_string(),
                PayloadValue::List(
                    memory
                        .tags
                        .iter()
                        .map(|s| PayloadValue::String(s.clone()))
                        .collect(),
                ),
            );
        }

        payload
    }

    fn payload_to_memory(&self, result: &SearchResult) -> Option<LongTermMemory> {
        let id = result.get_string("id")?.to_string();
        let content = result.get_string("content")?.to_string();
        let category = result.get_string("category")?.to_string();
        let importance = result.get_float("importance")? as f32;
        let access_count = result.get_integer("access_count")? as u32;
        let last_accessed_ts = result.get_integer("last_accessed")?;
        let created_at_ts = result.get_integer("created_at")?;

        let last_accessed = chrono::DateTime::from_timestamp(last_accessed_ts, 0)?
            .with_timezone(&chrono::Utc);
        let created_at = chrono::DateTime::from_timestamp(created_at_ts, 0)?
            .with_timezone(&chrono::Utc);

        let source_session = result.get_string("source_session").map(|s| s.to_string());

        // 解析file_refs和tags
        let file_refs = self.extract_string_list(&result.payload, "file_refs");
        let tags = self.extract_string_list(&result.payload, "tags");

        Some(LongTermMemory {
            id,
            content,
            category,
            importance,
            access_count,
            last_accessed,
            created_at,
            source_session,
            file_refs,
            tags,
        })
    }

    fn extract_string_list(
        &self,
        payload: &HashMap<String, PayloadValue>,
        key: &str,
    ) -> Vec<String> {
        match payload.get(key) {
            Some(PayloadValue::List(list)) => list
                .iter()
                .filter_map(|v| match v {
                    PayloadValue::String(s) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    fn build_filter(
        &self,
        category: Option<&str>,
        min_importance: Option<f32>,
    ) -> Option<SearchFilter> {
        let mut filter = SearchFilter::new();
        let mut has_condition = false;

        if let Some(cat) = category {
            filter = filter.must_match("category", cat);
            has_condition = true;
        }

        if let Some(min_imp) = min_importance {
            filter = filter.must_range("importance", Some(min_imp as f64), None);
            has_condition = true;
        }

        if has_condition {
            Some(filter)
        } else {
            None
        }
    }
}

/// 存储统计
#[derive(Debug, Clone, serde::Serialize)]
pub struct StoreStats {
    pub memory_count: u64,
    pub file_count: usize,
}