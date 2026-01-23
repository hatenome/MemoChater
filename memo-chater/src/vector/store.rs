//! 通用向量存储实现
//! 
//! 基于 Qdrant HTTP API 的向量存储封装，提供统一的增删改查接口

use crate::vector::types::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 通用向量存储
/// 
/// 封装 Qdrant HTTP API，提供简洁的向量操作接口
pub struct VectorStore {
    client: Client,
    base_url: String,
    config: VectorStoreConfig,
}

impl VectorStore {
    /// 创建新的向量存储实例
    pub async fn new(config: VectorStoreConfig) -> Result<Self, VectorStoreError> {
        let client = Client::builder()
            .no_proxy()
            .build()
            .map_err(|e| VectorStoreError::ConnectionError(e.to_string()))?;

        let store = Self {
            client,
            base_url: config.qdrant_url.trim_end_matches('/').to_string(),
            config,
        };
        
        // 确保 collection 存在
        store.ensure_collection().await?;
        
        Ok(store)
    }

    /// 确保 collection 存在，不存在则创建
    async fn ensure_collection(&self) -> Result<(), VectorStoreError> {
        let url = format!("{}/collections/{}", self.base_url, self.config.collection_name);
        
        let response = self.client.get(&url).send().await
            .map_err(|e| VectorStoreError::ConnectionError(e.to_string()))?;

        if response.status().as_u16() == 404 {
            self.create_collection().await?;
        } else if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(VectorStoreError::CollectionError(error));
        }

        Ok(())
    }

    /// 创建 collection
    async fn create_collection(&self) -> Result<(), VectorStoreError> {
        let url = format!("{}/collections/{}", self.base_url, self.config.collection_name);
        
        let distance = match self.config.distance {
            DistanceMetric::Cosine => "Cosine",
            DistanceMetric::Euclidean => "Euclid",
            DistanceMetric::Dot => "Dot",
        };

        let body = serde_json::json!({
            "vectors": {
                "size": self.config.vector_size,
                "distance": distance
            }
        });

        let response = self.client.put(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| VectorStoreError::CollectionError(e.to_string()))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(VectorStoreError::CollectionError(error));
        }

        tracing::info!("Created collection: {}", self.config.collection_name);
        Ok(())
    }

    /// 插入单个向量点
    pub async fn upsert(&self, point: VectorPoint) -> Result<(), VectorStoreError> {
        self.upsert_batch(vec![point]).await
    }

    /// 批量插入向量点
    pub async fn upsert_batch(&self, points: Vec<VectorPoint>) -> Result<(), VectorStoreError> {
        if points.is_empty() {
            return Ok(());
        }

        let url = format!("{}/collections/{}/points?wait=true", 
            self.base_url, self.config.collection_name);
        
        let qdrant_points: Vec<QdrantPoint> = points
            .into_iter()
            .map(|p| QdrantPoint {
                id: p.id,
                vector: p.vector,
                payload: p.payload.into_iter()
                    .map(|(k, v)| (k, payload_to_json(v)))
                    .collect(),
            })
            .collect();

        let body = serde_json::json!({
            "points": qdrant_points
        });

        let response = self.client.put(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| VectorStoreError::PointError(e.to_string()))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(VectorStoreError::PointError(error));
        }

        Ok(())
    }

    /// 向量搜索
    pub async fn search(
        &self,
        vector: Vec<f32>,
        top_k: u64,
        filter: Option<SearchFilter>,
    ) -> Result<Vec<SearchResult>, VectorStoreError> {
        let url = format!("{}/collections/{}/points/search", 
            self.base_url, self.config.collection_name);

        let mut body = serde_json::json!({
            "vector": vector,
            "limit": top_k,
            "with_payload": true
        });

        if let Some(f) = filter {
            body["filter"] = build_filter_json(f);
        }

        let response = self.client.post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| VectorStoreError::SearchError(e.to_string()))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(VectorStoreError::SearchError(error));
        }

        let search_response: QdrantSearchResponse = response.json().await
            .map_err(|e| VectorStoreError::SearchError(e.to_string()))?;

        let results = search_response.result
            .into_iter()
            .map(|r| SearchResult {
                id: match r.id {
                    QdrantPointId::String(s) => s,
                    QdrantPointId::Num(n) => n.to_string(),
                },
                score: r.score,
                payload: r.payload.unwrap_or_default()
                    .into_iter()
                    .map(|(k, v)| (k, json_to_payload(v)))
                    .collect(),
            })
            .collect();

        Ok(results)
    }

    /// 根据ID删除向量点
    pub async fn delete(&self, id: &str) -> Result<(), VectorStoreError> {
        self.delete_batch(&[id.to_string()]).await
    }

    /// 批量删除向量点
    pub async fn delete_batch(&self, ids: &[String]) -> Result<(), VectorStoreError> {
        if ids.is_empty() {
            return Ok(());
        }

        let url = format!("{}/collections/{}/points/delete?wait=true", 
            self.base_url, self.config.collection_name);

        let body = serde_json::json!({
            "points": ids
        });

        let response = self.client.post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| VectorStoreError::PointError(e.to_string()))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(VectorStoreError::PointError(error));
        }

        Ok(())
    }

    /// 更新向量点的 payload
    pub async fn update_payload(
        &self,
        id: &str,
        payload: HashMap<String, PayloadValue>,
    ) -> Result<(), VectorStoreError> {
        let url = format!("{}/collections/{}/points/payload?wait=true", 
            self.base_url, self.config.collection_name);

        let json_payload: HashMap<String, serde_json::Value> = payload
            .into_iter()
            .map(|(k, v)| (k, payload_to_json(v)))
            .collect();

        let body = serde_json::json!({
            "points": [id],
            "payload": json_payload
        });

        let response = self.client.post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| VectorStoreError::PointError(e.to_string()))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(VectorStoreError::PointError(error));
        }

        Ok(())
    }

    /// 获取 collection 中的点数量
    pub async fn count(&self) -> Result<u64, VectorStoreError> {
        let url = format!("{}/collections/{}", self.base_url, self.config.collection_name);

        let response = self.client.get(&url).send().await
            .map_err(|e| VectorStoreError::CollectionError(e.to_string()))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(VectorStoreError::CollectionError(error));
        }

        let info: QdrantCollectionInfo = response.json().await
            .map_err(|e| VectorStoreError::CollectionError(e.to_string()))?;

        Ok(info.result.points_count.unwrap_or(0))
    }

    /// 滚动获取所有记录（不需要向量）
    pub async fn scroll(&self, limit: u64, filter: Option<SearchFilter>) -> Result<Vec<SearchResult>, VectorStoreError> {
        let url = format!("{}/collections/{}/points/scroll", 
            self.base_url, self.config.collection_name);

        let mut body = serde_json::json!({
            "limit": limit,
            "with_payload": true,
            "with_vector": false
        });

        if let Some(f) = filter {
            body["filter"] = build_filter_json(f);
        }

        let response = self.client.post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| VectorStoreError::SearchError(e.to_string()))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(VectorStoreError::SearchError(error));
        }

        let scroll_response: QdrantScrollResponse = response.json().await
            .map_err(|e| VectorStoreError::SearchError(e.to_string()))?;

        let results = scroll_response.result.points
            .into_iter()
            .map(|r| SearchResult {
                id: match r.id {
                    QdrantPointId::String(s) => s,
                    QdrantPointId::Num(n) => n.to_string(),
                },
                score: 1.0, // scroll 没有分数，默认为1.0
                payload: r.payload.unwrap_or_default()
                    .into_iter()
                    .map(|(k, v)| (k, json_to_payload(v)))
                    .collect(),
            })
            .collect();

        Ok(results)
    }

    /// 获取配置
    pub fn config(&self) -> &VectorStoreConfig {
        &self.config
    }

    /// 删除整个 collection
    pub async fn drop_collection(&self) -> Result<(), VectorStoreError> {
        let url = format!("{}/collections/{}", self.base_url, self.config.collection_name);

        let response = self.client.delete(&url).send().await
            .map_err(|e| VectorStoreError::CollectionError(e.to_string()))?;

        if !response.status().is_success() {
            let error = response.text().await.unwrap_or_default();
            return Err(VectorStoreError::CollectionError(error));
        }

        tracing::info!("Dropped collection: {}", self.config.collection_name);
        Ok(())
    }
}

// ============ Qdrant API 数据结构 ============

#[derive(Debug, Serialize)]
struct QdrantPoint {
    id: String,
    vector: Vec<f32>,
    payload: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct QdrantSearchResponse {
    result: Vec<QdrantScoredPoint>,
}

#[derive(Debug, Deserialize)]
struct QdrantScoredPoint {
    id: QdrantPointId,
    score: f32,
    payload: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum QdrantPointId {
    String(String),
    Num(u64),
}

#[derive(Debug, Deserialize)]
struct QdrantCollectionInfo {
    result: QdrantCollectionResult,
}

#[derive(Debug, Deserialize)]
struct QdrantCollectionResult {
    points_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct QdrantScrollResponse {
    result: QdrantScrollResult,
}

#[derive(Debug, Deserialize)]
struct QdrantScrollResult {
    points: Vec<QdrantScrollPoint>,
}

#[derive(Debug, Deserialize)]
struct QdrantScrollPoint {
    id: QdrantPointId,
    payload: Option<HashMap<String, serde_json::Value>>,
}

// ============ 辅助函数 ============

fn payload_to_json(value: PayloadValue) -> serde_json::Value {
    match value {
        PayloadValue::String(s) => serde_json::Value::String(s),
        PayloadValue::Integer(i) => serde_json::Value::Number(i.into()),
        PayloadValue::Float(f) => serde_json::json!(f),
        PayloadValue::Bool(b) => serde_json::Value::Bool(b),
        PayloadValue::List(list) => {
            serde_json::Value::Array(list.into_iter().map(payload_to_json).collect())
        }
    }
}

fn json_to_payload(value: serde_json::Value) -> PayloadValue {
    match value {
        serde_json::Value::String(s) => PayloadValue::String(s),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                PayloadValue::Integer(i)
            } else {
                PayloadValue::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::Bool(b) => PayloadValue::Bool(b),
        serde_json::Value::Array(arr) => {
            PayloadValue::List(arr.into_iter().map(json_to_payload).collect())
        }
        _ => PayloadValue::String(String::new()),
    }
}

fn build_filter_json(filter: SearchFilter) -> serde_json::Value {
    let mut result = serde_json::Map::new();

    if !filter.must.is_empty() {
        result.insert("must".to_string(), 
            serde_json::Value::Array(filter.must.into_iter().map(condition_to_json).collect()));
    }
    if !filter.must_not.is_empty() {
        result.insert("must_not".to_string(), 
            serde_json::Value::Array(filter.must_not.into_iter().map(condition_to_json).collect()));
    }
    if !filter.should.is_empty() {
        result.insert("should".to_string(), 
            serde_json::Value::Array(filter.should.into_iter().map(condition_to_json).collect()));
    }

    serde_json::Value::Object(result)
}

fn condition_to_json(cond: FilterCondition) -> serde_json::Value {
    match cond {
        FilterCondition::Match { field, value } => {
            let match_value = match value {
                PayloadValue::String(s) => serde_json::json!({ "value": s }),
                PayloadValue::Integer(i) => serde_json::json!({ "value": i }),
                PayloadValue::Bool(b) => serde_json::json!({ "value": b }),
                _ => serde_json::json!({ "value": null }),
            };
            serde_json::json!({
                "key": field,
                "match": match_value
            })
        }
        FilterCondition::Range { field, gte, lte } => {
            let mut range = serde_json::Map::new();
            if let Some(g) = gte {
                range.insert("gte".to_string(), serde_json::json!(g));
            }
            if let Some(l) = lte {
                range.insert("lte".to_string(), serde_json::json!(l));
            }
            serde_json::json!({
                "key": field,
                "range": range
            })
        }
    }
}