//! 上下文清理器
//!
//! 清理上下文中为 API 请求而构建的临时内容（如记忆注入消息），
//! 以便后续处理器（如 ContentChunker）能处理干净的对话内容。

use async_trait::async_trait;
use tracing::{info, debug};

use crate::pipeline::{
    processor::{Processor, ProcessorError},
    context::ProcessorContext,
    packet::ConversationPacket,
};
use crate::types::ChatMessage;

/// 上下文清理器
/// 
/// 负责清理上下文中的临时构建内容，保留真实对话
pub struct ContextCleaner;

impl ContextCleaner {
    pub fn new() -> Self {
        Self
    }

    /// 清理短期记忆注入消息（ShortTermAssembler 注入）
    /// 
    /// 匹配条件：
    /// - user 消息以「【系统消息-短期记忆】现在为你注入」开头
    fn should_remove_short_term_memory_injection(msg: &ChatMessage) -> bool {
        msg.role == "user" && msg.content.starts_with("【系统消息-短期记忆】现在为你注入")
    }

    /// 清理短期记忆展开消息（ShortTermExpander 注入）
    /// 
    /// 匹配条件：
    /// - user 消息以「【系统消息-短期记忆】根据」开头
    fn should_remove_short_term_memory_expansion(msg: &ChatMessage) -> bool {
        msg.role == "user" && msg.content.starts_with("【系统消息-短期记忆】根据")
    }

    // ========== 后续可在此添加更多清理规则 ==========

    /// 判断消息是否应该被清理
    fn should_remove(msg: &ChatMessage) -> bool {
        // 依次检查各个清理规则
        Self::should_remove_short_term_memory_injection(msg)
            || Self::should_remove_short_term_memory_expansion(msg)
    }
}

#[async_trait]
impl Processor for ContextCleaner {
    fn name(&self) -> &'static str {
        "ContextCleaner"
    }

    fn requires_memory(&self) -> bool {
        true
    }

    async fn process(
        &self,
        packet: &mut ConversationPacket,
        _ctx: &ProcessorContext,
    ) -> Result<(), ProcessorError> {
        info!("ContextCleaner 开始处理");

        let before_count = packet.messages.len();

        // 过滤掉需要清理的消息
        packet.messages.retain(|msg| {
            let should_keep = !Self::should_remove(msg);
            if !should_keep {
                debug!("清理消息: [{}] {}", msg.role, 
                    if msg.content.chars().count() > 30 { 
                        format!("{}...", msg.content.chars().take(30).collect::<String>()) 
                    } else { 
                        msg.content.clone() 
                    }
                );
            }
            should_keep
        });

        let after_count = packet.messages.len();
        let removed_count = before_count - after_count;

        info!("ContextCleaner 完成，清理了 {} 条消息", removed_count);

        // 保存处理器状态
        packet.set_processor_state(self.name(), serde_json::json!({
            "cleaned": true,
            "removed_count": removed_count,
            "before_count": before_count,
            "after_count": after_count
        }));

        Ok(())
    }
}