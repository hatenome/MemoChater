//! 处理流水线
//!
//! 可配置、可扩展的处理器架构
//!
//! ## 核心组件
//! - `ConversationPacket`: 对话数据包，流水线中流转的核心数据结构
//! - `Processor`: 处理器 trait，所有处理器的统一接口
//! - `ProcessorContext`: 处理器上下文，提供公共依赖
//! - `PipelineDispatcher`: 流水线调度器，按配置执行处理器
//! - `PipelineConfig`: 流水线配置，定义各时机的处理器列表
//!
//! ## 处理时机
//! 1. `on_user_message`: 用户发言追加到 messages 后
//! 2. `before_ai_call`: 发送给 AI API 前
//! 3. `on_stream_start`: 开始收到 AI 流式响应（预留）
//! 4. `on_stream_chunk`: 收到每个 chunk 时（预留）
//! 5. `after_ai_response`: AI 响应完整接收后
//!
//! ## 默认处理器
//! - `LongTermRetriever`: 长期记忆检索
//! - `ShortTermInjector`: 短期记忆注入
//! - `ContextAssembler`: 上下文组装
//! - `ResponseAnalyzer`: 响应分析
//! - `MemoryExtractor`: 记忆提取
//! - `MemoryCommitter`: 记忆提交

pub mod packet;
pub mod processor;
pub mod context;
pub mod config;
pub mod dispatcher;
pub mod storage;
pub mod processors;

// 导出核心类型
pub use packet::{ConversationPacket, ThinkingEntry};
pub use processor::{Processor, ProcessorError};
pub use context::{ProcessorContext, ProcessorContextFactory};
pub use config::{PipelineConfig, ProcessorEntry};
pub use dispatcher::{PipelineDispatcher, PipelineTiming, DispatcherError};
pub use storage::{PacketStorage, StorageError};
pub use processors::create_all_processors;