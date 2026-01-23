//! 处理器接口定义
//!
//! 所有流水线处理器都需要实现 Processor trait

use thiserror::Error;

use super::packet::ConversationPacket;
use super::context::ProcessorContext;

/// 处理器错误
#[derive(Debug, Error)]
pub enum ProcessorError {
    #[error("处理器内部错误: {0}")]
    Internal(String),

    #[error("配置错误: {0}")]
    Config(String),

    #[error("依赖服务错误: {0}")]
    Service(String),

    #[error("AI 调用错误: {0}")]
    AiError(String),

    #[error("记忆操作错误: {0}")]
    MemoryError(String),
}

/// 处理器统一接口
///
/// 所有流水线处理器都需要实现此 trait。
/// 处理器通过修改 ConversationPacket 来传递数据。
///
/// 使用 `#[async_trait::async_trait]` 宏来支持 async fn。
#[async_trait::async_trait]
pub trait Processor: Send + Sync {
    /// 处理器名称（用于配置引用和状态字典键名）
    fn name(&self) -> &'static str;

    /// 是否需要记忆功能开启才执行
    ///
    /// 记忆相关处理器应返回 true，普通处理器返回 false
    /// **必须实现**，强制开发者明确声明
    fn requires_memory(&self) -> bool;

    /// 执行处理
    ///
    /// # 参数
    /// - `packet`: 对话数据包（可变引用，直接修改）
    /// - `ctx`: 处理器上下文（提供公共依赖）
    ///
    /// # 返回
    /// - `Ok(())` 表示处理成功
    /// - `Err(e)` 表示处理失败，流水线将跳过此处理器，使用原数据包继续
    async fn process(
        &self,
        packet: &mut ConversationPacket,
        ctx: &ProcessorContext,
    ) -> Result<(), ProcessorError>;
}