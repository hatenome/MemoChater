//! 消息相关类型

use serde::{Deserialize, Serialize};

/// 聊天消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

/// 处理后的用户输入
#[derive(Debug, Clone)]
pub struct ProcessedInput {
    /// 助手ID（用于记忆隔离）
    pub assistant_id: Option<String>,
    /// 话题ID（用于会话隔离）
    pub topic_id: Option<String>,
    /// 会话ID（兼容旧逻辑，优先使用 assistant_id + topic_id）
    pub session_id: Option<String>,
    /// 用户消息内容
    pub user_message: String,
    /// 客户端传入的 system message（如果有）
    pub system_message: Option<String>,
    /// 请求中指定的模型名（用于动态切换模型）
    pub model: String,
    /// 是否流式
    pub stream: bool,
}