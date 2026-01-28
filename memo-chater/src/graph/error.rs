//! 关系图引擎 - 错误类型定义

use thiserror::Error;

/// 图引擎错误
#[derive(Debug, Error)]
pub enum GraphError {
    #[error("图不存在: {dimension}")]
    NotFound { dimension: String },

    #[error("节点不存在: {node_id}")]
    NodeNotFound { node_id: String },

    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("维度处理器未注册: {dimension}")]
    ProcessorNotRegistered { dimension: String },

    #[error("维度处理错误: {0}")]
    DimensionError(#[from] DimensionError),
}

/// 维度处理错误
#[derive(Debug, Error)]
pub enum DimensionError {
    #[error("缺少必要特征: {0}")]
    MissingFeature(&'static str),

    #[error("计算错误: {0}")]
    ComputeError(String),

    #[error("AI调用错误: {0}")]
    AiError(String),
}

/// 图引擎结果类型
pub type GraphResult<T> = Result<T, GraphError>;