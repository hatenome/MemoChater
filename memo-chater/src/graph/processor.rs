//! 关系图引擎 - 维度处理器 trait

use crate::graph::error::DimensionError;
use crate::graph::types::*;
use async_trait::async_trait;
use std::sync::Arc;

// ============================================================================
// 计算上下文
// ============================================================================

/// 计算上下文（用于边计算时的依赖注入）
pub struct ComputeContext {
    /// 维度配置
    pub config: DimensionConfig,
    // 未来可扩展：AI客户端等
}

impl ComputeContext {
    /// 创建默认上下文
    pub fn new(config: DimensionConfig) -> Self {
        Self { config }
    }

    /// 创建带默认配置的上下文
    pub fn with_defaults() -> Self {
        Self {
            config: DimensionConfig::default(),
        }
    }
}

// ============================================================================
// 维度处理器 Trait
// ============================================================================

/// 维度处理器 - 定义如何计算该维度的关系
#[async_trait]
pub trait DimensionProcessor: Send + Sync {
    /// 维度名称（用于文件命名）
    fn dimension(&self) -> &'static str;

    /// 维度描述
    fn description(&self) -> &'static str;

    /// 计算新节点与现有节点的边
    ///
    /// # 参数
    /// - `new_node`: 新加入的节点
    /// - `existing_nodes`: 现有节点列表
    /// - `ctx`: 计算上下文
    ///
    /// # 返回
    /// 边候选列表
    async fn compute_edges(
        &self,
        new_node: &Node,
        existing_nodes: &[Node],
        ctx: &ComputeContext,
    ) -> Result<Vec<EdgeCandidate>, DimensionError>;

    /// 查询时的默认权重
    fn default_query_weight(&self) -> f32 {
        1.0
    }
}

// ============================================================================
// 处理器注册表
// ============================================================================

/// 处理器注册表
pub struct ProcessorRegistry {
    processors: Vec<Arc<dyn DimensionProcessor>>,
}

impl ProcessorRegistry {
    pub fn new() -> Self {
        Self {
            processors: Vec::new(),
        }
    }

    pub fn register(&mut self, processor: Arc<dyn DimensionProcessor>) {
        self.processors.push(processor);
    }

    pub fn get(&self, dimension: &str) -> Option<Arc<dyn DimensionProcessor>> {
        self.processors
            .iter()
            .find(|p| p.dimension() == dimension)
            .cloned()
    }

    pub fn all(&self) -> &[Arc<dyn DimensionProcessor>] {
        &self.processors
    }

    pub fn dimensions(&self) -> Vec<&'static str> {
        self.processors.iter().map(|p| p.dimension()).collect()
    }
}

impl Default for ProcessorRegistry {
    fn default() -> Self {
        Self::new()
    }
}