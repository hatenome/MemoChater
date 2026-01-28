//! 关系图引擎（RelationGraphEngine）
//!
//! MemoChater 第二核心模块，提供多维记忆关系管理。
//!
//! ## 核心概念
//!
//! - **DimensionGraph**: 单维度图，完全独立的图实例
//! - **GraphLocator**: 图定位器，按需加载/创建图
//! - **GraphScope**: 图作用域，定位图的存储位置
//! - **DimensionProcessor**: 维度处理器，定义边的计算逻辑
//!
//! ## 使用示例
//!
//! ```ignore
//! use memo_chater::graph::{GraphLocator, GraphScope, SingleGraphQuery};
//!
//! let locator = GraphLocator::new(data_dir);
//! let scope = GraphScope::ShortTerm {
//!     assistant_id: "ast_001".to_string(),
//!     topic_id: "topic_001".to_string(),
//! };
//!
//! let graph = locator.get(&scope, "temporal").await?;
//! let result = graph.read().await.query(&query);
//! ```

mod dimension_graph;
pub mod dimensions;
mod error;
mod locator;
mod processor;
mod query;
mod types;

// 重导出核心类型
pub use dimension_graph::DimensionGraph;
pub use error::{DimensionError, GraphError, GraphResult};
pub use locator::GraphLocator;
pub use processor::{ComputeContext, DimensionProcessor, ProcessorRegistry};
pub use types::*;

// 内置维度处理器
pub use dimensions::TemporalProcessor;