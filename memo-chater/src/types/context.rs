//! 上下文相关类型

use super::ChatMessage;

/// 组装好的AI上下文
#[derive(Debug, Clone)]
pub struct AiContext {
    /// 系统提示词
    pub system_prompt: String,
    /// 思考池内容（转换为消息格式）
    pub thinking_messages: Vec<ChatMessage>,
    /// 短期记忆（转换为消息格式）
    pub memory_messages: Vec<ChatMessage>,
    /// 用户当前输入
    pub user_message: ChatMessage,
    /// 指定使用的模型（如果有）
    pub model: Option<String>,
}

impl AiContext {
    /// 转换为发送给AI的消息列表
    pub fn to_messages(&self) -> Vec<ChatMessage> {
        let mut messages = Vec::new();
        
        // 1. 系统提示词
        messages.push(ChatMessage::system(&self.system_prompt));
        
        // 2. 思考池内容（作为系统消息的一部分）
        if !self.thinking_messages.is_empty() {
            let thinking_content = self.thinking_messages
                .iter()
                .map(|m| m.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            messages.push(ChatMessage::system(format!(
                "[内部思考]\n{}",
                thinking_content
            )));
        }
        
        // 3. 短期记忆
        if !self.memory_messages.is_empty() {
            let memory_content = self.memory_messages
                .iter()
                .map(|m| m.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            messages.push(ChatMessage::system(format!(
                "[相关记忆]\n{}",
                memory_content
            )));
        }
        
        // 4. 用户输入
        messages.push(self.user_message.clone());
        
        messages
    }
}

/// 2号AI的处理结果
#[derive(Debug, Clone)]
pub struct ProcessorResult {
    /// 思考摘要（加入思考池）
    pub thinking_summary: Option<String>,
    /// 值得长期记忆的内容
    pub valuable_memories: Vec<PendingMemoryItem>,
    /// 建议遗忘的短期记忆ID
    pub forget_suggestions: Vec<String>,
    /// 上下文更新
    pub context_updates: Vec<String>,
}

/// 待加入长期记忆的条目
#[derive(Debug, Clone)]
pub struct PendingMemoryItem {
    pub content: String,
    pub category: String,
    pub importance: f32,
}