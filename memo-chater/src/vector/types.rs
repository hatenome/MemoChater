//! 向量存储相关类型定义

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 向量存储配置
#[derive(Debug, Clone)]
pub struct VectorStoreConfig {
    /// Qdrant 服务地址
    pub qdrant_url: String,
    /// Collection 名称
    pub collection_name: String,
    /// 向量维度
    pub vector_size: u64,
    /// 距离度量方式
    pub distance: DistanceMetric,
}

impl VectorStoreConfig {
    pub fn new(collection_name: impl Into<String>, vector_size: u64) -> Self {
        Self {
            qdrant_url: "http://localhost:6333".to_string(),
            collection_name: collection_name.into(),
            vector_size,
            distance: DistanceMetric::Cosine,
        }
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.qdrant_url = url.into();
        self
    }

    pub fn with_distance(mut self, distance: DistanceMetric) -> Self {
        self.distance = distance;
        self
    }
}

/// 距离度量方式
#[derive(Debug, Clone, Copy, Default)]
pub enum DistanceMetric {
    #[default]
    Cosine,
    Euclidean,
    Dot,
}

/// 向量点 - 存储到向量库的基本单元
#[derive(Debug, Clone)]
pub struct VectorPoint {
    /// 唯一标识符
    pub id: String,
    /// 向量数据
    pub vector: Vec<f32>,
    /// 元数据（payload）
    pub payload: HashMap<String, PayloadValue>,
}

impl VectorPoint {
    pub fn new(id: impl Into<String>, vector: Vec<f32>) -> Self {
        Self {
            id: id.into(),
            vector,
            payload: HashMap::new(),
        }
    }

    pub fn with_payload(mut self, key: impl Into<String>, value: impl Into<PayloadValue>) -> Self {
        self.payload.insert(key.into(), value.into());
        self
    }
}

/// Payload 值类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PayloadValue {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    List(Vec<PayloadValue>),
}

impl From<String> for PayloadValue {
    fn from(s: String) -> Self {
        PayloadValue::String(s)
    }
}

impl From<&str> for PayloadValue {
    fn from(s: &str) -> Self {
        PayloadValue::String(s.to_string())
    }
}

impl From<i64> for PayloadValue {
    fn from(i: i64) -> Self {
        PayloadValue::Integer(i)
    }
}

impl From<i32> for PayloadValue {
    fn from(i: i32) -> Self {
        PayloadValue::Integer(i as i64)
    }
}

impl From<f64> for PayloadValue {
    fn from(f: f64) -> Self {
        PayloadValue::Float(f)
    }
}

impl From<f32> for PayloadValue {
    fn from(f: f32) -> Self {
        PayloadValue::Float(f as f64)
    }
}

impl From<bool> for PayloadValue {
    fn from(b: bool) -> Self {
        PayloadValue::Bool(b)
    }
}

/// 搜索结果
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// 点ID
    pub id: String,
    /// 相似度分数
    pub score: f32,
    /// 元数据
    pub payload: HashMap<String, PayloadValue>,
}

impl SearchResult {
    /// 获取字符串类型的 payload 值
    pub fn get_string(&self, key: &str) -> Option<&str> {
        match self.payload.get(key) {
            Some(PayloadValue::String(s)) => Some(s),
            _ => None,
        }
    }

    /// 获取整数类型的 payload 值
    pub fn get_integer(&self, key: &str) -> Option<i64> {
        match self.payload.get(key) {
            Some(PayloadValue::Integer(i)) => Some(*i),
            _ => None,
        }
    }

    /// 获取浮点类型的 payload 值
    pub fn get_float(&self, key: &str) -> Option<f64> {
        match self.payload.get(key) {
            Some(PayloadValue::Float(f)) => Some(*f),
            _ => None,
        }
    }
}

/// 过滤条件
#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    /// 必须匹配的条件
    pub must: Vec<FilterCondition>,
    /// 必须不匹配的条件
    pub must_not: Vec<FilterCondition>,
    /// 至少匹配一个的条件
    pub should: Vec<FilterCondition>,
}

impl SearchFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn must_match(mut self, field: impl Into<String>, value: impl Into<PayloadValue>) -> Self {
        self.must.push(FilterCondition::Match {
            field: field.into(),
            value: value.into(),
        });
        self
    }

    pub fn must_not_match(mut self, field: impl Into<String>, value: impl Into<PayloadValue>) -> Self {
        self.must_not.push(FilterCondition::Match {
            field: field.into(),
            value: value.into(),
        });
        self
    }

    pub fn should_match(mut self, field: impl Into<String>, value: impl Into<PayloadValue>) -> Self {
        self.should.push(FilterCondition::Match {
            field: field.into(),
            value: value.into(),
        });
        self
    }

    pub fn must_range(mut self, field: impl Into<String>, min: Option<f64>, max: Option<f64>) -> Self {
        self.must.push(FilterCondition::Range {
            field: field.into(),
            gte: min,
            lte: max,
        });
        self
    }
}

/// 过滤条件类型
#[derive(Debug, Clone)]
pub enum FilterCondition {
    /// 精确匹配
    Match { field: String, value: PayloadValue },
    /// 范围匹配
    Range { field: String, gte: Option<f64>, lte: Option<f64> },
}

/// 向量存储错误
#[derive(Debug, thiserror::Error)]
pub enum VectorStoreError {
    #[error("连接错误: {0}")]
    ConnectionError(String),
    #[error("Collection 操作错误: {0}")]
    CollectionError(String),
    #[error("点操作错误: {0}")]
    PointError(String),
    #[error("搜索错误: {0}")]
    SearchError(String),
    #[error("序列化错误: {0}")]
    SerializationError(String),
}