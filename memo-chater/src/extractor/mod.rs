//! 信息提取模块
//! 
//! 负责从对话历史中提取结构化的记忆信息。
//! 这是一个独立的、可测试的组件。

mod memory_extractor;
mod types;

pub use memory_extractor::MemoryExtractor;
pub use types::*;