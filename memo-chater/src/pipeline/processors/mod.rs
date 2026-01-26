//! 处理器模块
//!
//! 包含所有流水线处理器的实现

mod history_simplifier;
mod subconscious_processor;
mod content_chunker;
mod short_term_assembler;
mod context_cleaner;
mod short_term_expander;
pub mod short_term_vectorizer;

pub use history_simplifier::HistorySimplifier;
pub use subconscious_processor::SubconsciousProcessor;
pub use content_chunker::ContentChunker;
pub use short_term_assembler::ShortTermAssembler;
pub use context_cleaner::ContextCleaner;
pub use short_term_expander::ShortTermExpander;
pub use short_term_vectorizer::ShortTermVectorizer;

use std::sync::Arc;
use super::processor::Processor;

/// 创建所有处理器的列表
pub fn create_all_processors() -> Vec<Arc<dyn Processor>> {
    vec![
        Arc::new(HistorySimplifier::new()),
        Arc::new(SubconsciousProcessor::new()),
        Arc::new(ContentChunker::new()),
        Arc::new(ShortTermAssembler::new()),
Arc::new(ContextCleaner::new()),
        Arc::new(ShortTermExpander::new()),
        Arc::new(ShortTermVectorizer::new()),
    ]
}