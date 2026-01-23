//! 短期记忆展开器
//!
//! 根据前端标记的 should_expand 字段，展开相关短期记忆的详细内容

use async_trait::async_trait;
use tracing::info;
use chrono::{DateTime, Utc};

use crate::pipeline::{
    processor::{Processor, ProcessorError},
    context::ProcessorContext,
    packet::ConversationPacket,
};
use crate::types::ChatMessage;

/// 短期记忆展开器
/// 
/// 根据前端标记的 should_expand 字段，展开相关短期记忆的详细内容
pub struct ShortTermExpander;

impl ShortTermExpander {
    pub fn new() -> Self {
        Self
    }

    /// 格式化时间戳
    fn format_timestamp(timestamp: &DateTime<Utc>) -> String {
        timestamp.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    /// 构建展开后的记忆文本
    fn build_expanded_text(packet: &ConversationPacket) -> String {
        packet.get_short_term_memory()
            .iter()
            .filter(|m| m.should_expand)
            .map(|m| format!(
                "[{}][{}]{}\n{}",
                Self::format_timestamp(&m.timestamp),
                m.memory_type,
                m.summary,
                m.content
            ))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

#[async_trait]
impl Processor for ShortTermExpander {
    fn name(&self) -> &'static str {
        "ShortTermExpander"
    }

    fn requires_memory(&self) -> bool {
        true
    }

    async fn process(
        &self,
        packet: &mut ConversationPacket,
        _ctx: &ProcessorContext,
    ) -> Result<(), ProcessorError> {
        info!("ShortTermExpander 开始处理");

        // 检查是否有需要展开的短期记忆
        let expand_count = packet.get_short_term_memory()
            .iter()
            .filter(|m| m.should_expand)
            .count();

        if expand_count == 0 {
            info!("没有标记需要展开的短期记忆，跳过");
            packet.set_processor_state(self.name(), serde_json::json!({
                "expanded": false,
                "reason": "no_memories_marked_for_expansion"
            }));
            return Ok(());
        }

        info!("发现 {} 条需要展开的短期记忆", expand_count);

        // 构建展开后的文本
        let expanded_text = Self::build_expanded_text(packet);

        if expanded_text.is_empty() {
            info!("展开文本为空，跳过");
            packet.set_processor_state(self.name(), serde_json::json!({
                "expanded": false,
                "reason": "expanded_text_empty"
            }));
            return Ok(());
        }

        // 构建注入消息
        let injection_content = format!(
            "【系统消息-短期记忆】根据{}的标记，以下记忆需要展开\n---展开的短期记忆---\n{}\n---短期记忆结束---",
            packet.user_name,
            expanded_text
        );

        // 插入到倒数第二个位置（用户最新发言之前）
        let insert_pos = if packet.messages.len() >= 1 {
            packet.messages.len() - 1
        } else {
            packet.messages.len()
        };

        packet.messages.insert(insert_pos, ChatMessage::user(&injection_content));

        info!("已在位置 {} 插入展开的记忆内容", insert_pos);

        // 获取展开的记忆ID列表
        let expanded_ids: Vec<String> = packet.get_short_term_memory()
            .iter()
            .filter(|m| m.should_expand)
            .map(|m| m.id.clone())
            .collect();

        // 保存处理器状态
        packet.set_processor_state(self.name(), serde_json::json!({
            "expanded": true,
            "expanded_ids": expanded_ids,
            "expanded_count": expand_count,
            "insert_position": insert_pos
        }));

        Ok(())
    }
}