//! 关系图引擎 - 核心类型定义

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ============================================================================
// 图作用域
// ============================================================================

/// 图的作用域 - 定位图的存储位置
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GraphScope {
    /// 短期记忆图（话题级别）
    ShortTerm {
        assistant_id: String,
        topic_id: String,
    },
    /// 长期记忆图（助手级别）
    LongTerm { assistant_id: String },
    /// 全局图
    Global,
}

impl GraphScope {
    /// 获取图的存储目录
    pub fn storage_dir(&self, data_dir: &Path) -> PathBuf {
        match self {
            GraphScope::ShortTerm {
                assistant_id,
                topic_id,
            } => data_dir
                .join("assistants")
                .join(assistant_id)
                .join("topics")
                .join(topic_id)
                .join("graphs"),
            GraphScope::LongTerm { assistant_id } => data_dir
                .join("assistants")
                .join(assistant_id)
                .join("long_term_graphs"),
            GraphScope::Global => data_dir.join("global_graphs"),
        }
    }

    /// 生成缓存键
    pub fn cache_key(&self, dimension: &str) -> String {
        match self {
            GraphScope::ShortTerm {
                assistant_id,
                topic_id,
            } => {
                format!("short:{}:{}:{}", assistant_id, topic_id, dimension)
            }
            GraphScope::LongTerm { assistant_id } => {
                format!("long:{}:{}", assistant_id, dimension)
            }
            GraphScope::Global => {
                format!("global:{}", dimension)
            }
        }
    }
}

// ============================================================================
// 记忆引用
// ============================================================================

/// 记忆引用 - 指向记忆本体的位置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MemoryRef {
    /// 短期记忆（在 short_term_vectors.json 中）
    ShortTerm { file_path: String },
    /// 长期记忆（在 Qdrant 中）
    LongTerm { collection: String, point_id: String },
    /// 外部引用
    External { uri: String },
}

// ============================================================================
// 节点
// ============================================================================

/// 图节点 - 记忆的引用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// 节点ID（通常等于记忆ID）
    pub id: String,
    /// 记忆本体的位置引用
    pub memory_ref: MemoryRef,
    /// 节点创建时间
    pub created_at: DateTime<Utc>,
    /// 节点特征（用于边计算）
    #[serde(default)]
    pub features: NodeFeatures,
}

impl Node {
    /// 创建新节点
    pub fn new(id: String, memory_ref: MemoryRef) -> Self {
        Self {
            id,
            memory_ref,
            created_at: Utc::now(),
            features: NodeFeatures::default(),
        }
    }

    /// 获取时间戳（时间维度必需）
    pub fn timestamp(&self) -> Option<DateTime<Utc>> {
        self.features.timestamp
    }
}

/// 节点特征
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeFeatures {
    /// 时间戳（记忆发生时间）
    pub timestamp: Option<DateTime<Utc>>,
    /// 抽取的实体列表
    #[serde(default)]
    pub entities: Vec<String>,
    /// 情感标签
    pub emotion: Option<String>,
    /// 话题标签
    #[serde(default)]
    pub topics: Vec<String>,
    /// 语义向量（可选缓存）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
}

// ============================================================================
// 边
// ============================================================================

/// 有向加权边
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// 源节点ID
    pub source: String,
    /// 目标节点ID
    pub target: String,
    /// 边权重 (0.0 ~ 1.0)
    pub weight: f32,
    /// 关系原因（可解释性）
    pub reason: String,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 是否自动生成
    #[serde(default = "default_true")]
    pub auto_generated: bool,
}

fn default_true() -> bool {
    true
}

impl Edge {
    /// 创建新边
    pub fn new(source: String, target: String, weight: f32, reason: String) -> Self {
        Self {
            source,
            target,
            weight,
            reason,
            created_at: Utc::now(),
            auto_generated: true,
        }
    }
}

// ============================================================================
// 图元数据
// ============================================================================

/// 图元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphMetadata {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub node_count: usize,
    pub edge_count: usize,
}

impl Default for GraphMetadata {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            created_at: now,
            updated_at: now,
            node_count: 0,
            edge_count: 0,
        }
    }
}

// ============================================================================
// 查询相关
// ============================================================================

/// 查询方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueryDirection {
    /// 正向（从锚点出发）
    Forward,
    /// 反向（指向锚点）
    Backward,
    /// 双向
    Both,
}

impl Default for QueryDirection {
    fn default() -> Self {
        Self::Both
    }
}

/// 单图查询请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleGraphQuery {
    /// 锚点节点ID列表
    pub anchors: Vec<String>,
    /// 返回数量限制
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// 最小权重阈值
    #[serde(default)]
    pub min_weight: f32,
    /// 查询方向
    #[serde(default)]
    pub direction: QueryDirection,
}

fn default_limit() -> usize {
    10
}

/// 相关节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedNode {
    pub node_id: String,
    pub weight: f32,
    pub reason: String,
    /// 关系方向
    pub direction: QueryDirection,
}

/// 单图查询结果
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SingleGraphResult {
    /// 相关节点（按权重降序）
    pub nodes: Vec<RelatedNode>,
}

/// 多图联合查询请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiGraphQuery {
    /// 锚点节点ID列表
    pub anchors: Vec<String>,
    /// 各维度权重 <dimension, weight>
    pub dimension_weights: HashMap<String, f32>,
    /// 返回数量限制
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// 最小综合得分阈值
    #[serde(default)]
    pub min_score: f32,
}

/// 带综合得分的节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredNode {
    pub node_id: String,
    /// 综合得分
    pub total_score: f32,
    /// 各维度得分
    pub dimension_scores: HashMap<String, f32>,
    /// 关联原因汇总
    pub reasons: Vec<String>,
}

/// 维度贡献
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionContribution {
    pub node_id: String,
    pub raw_weight: f32,
    pub weighted_score: f32,
    pub reason: String,
}

/// 多图联合查询结果
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MultiGraphResult {
    /// 排序后的相关节点
    pub nodes: Vec<ScoredNode>,
    /// 各维度贡献明细
    pub contributions: HashMap<String, Vec<DimensionContribution>>,
}

// ============================================================================
// 边候选（用于维度处理器）
// ============================================================================

/// 边候选
#[derive(Debug, Clone)]
pub struct EdgeCandidate {
    /// 目标节点ID
    pub target_id: String,
    /// 边权重
    pub weight: f32,
    /// 关系原因
    pub reason: String,
    /// 是否双向（如果是，会自动创建反向边）
    pub bidirectional: bool,
}

// ============================================================================
// 维度配置
// ============================================================================

/// 维度配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DimensionConfig {
    /// 边权重阈值（低于此值不建边）
    #[serde(default = "default_edge_threshold")]
    pub edge_threshold: f32,
    /// 自定义参数
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
}

fn default_edge_threshold() -> f32 {
    0.05
}