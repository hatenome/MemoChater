//! 流水线配置
//!
//! 定义流水线各时机的处理器列表

use serde::{Deserialize, Serialize};

/// 处理器条目（包含名称和描述）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorEntry {
    /// 处理器名称（用于匹配注册的处理器）
    pub name: String,
    /// 处理器描述（用户自定义说明）
    #[serde(default)]
    pub description: String,
}

impl ProcessorEntry {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
        }
    }
    
    pub fn with_description(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
        }
    }
}

/// 流水线配置
///
/// 定义各个时机执行的处理器列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// 用户发言后执行的处理器（按顺序）
    #[serde(default = "default_on_user_message")]
    pub on_user_message: Vec<ProcessorEntry>,

    /// 发给 AI 前执行的处理器
    #[serde(default)]
    pub before_ai_call: Vec<ProcessorEntry>,

    /// 开始收到返回后（预留）
    #[serde(default)]
    pub on_stream_start: Vec<ProcessorEntry>,

    /// 收到 chunk 时（预留）
    #[serde(default)]
    pub on_stream_chunk: Vec<ProcessorEntry>,

    /// AI 响应结束后执行的处理器（同步，阻塞下一次对话）
    #[serde(default = "default_after_ai_response")]
    pub after_ai_response: Vec<ProcessorEntry>,

    /// 后台异步处理器（不阻塞下一次对话）
    #[serde(default)]
    pub background_process: Vec<ProcessorEntry>,
}

fn default_on_user_message() -> Vec<ProcessorEntry> {
    vec![
        ProcessorEntry::with_description("HistorySimplifier", "简化/压缩历史对话，减少上下文长度"),
        ProcessorEntry::with_description("MemoryAssembler", "将检索到的记忆装配到上下文中"),
    ]
}

fn default_after_ai_response() -> Vec<ProcessorEntry> {
    vec![
        ProcessorEntry::with_description("SubconsciousProcessor", "处理潜意识层面的信息"),
        ProcessorEntry::with_description("ContentChunker", "将内容切分成适合存储的块"),
        ProcessorEntry::with_description("ShortTermVectorizer", "将短期记忆向量化并存储到话题文件"),
        ProcessorEntry::with_description("MemoryCommitter", "将记忆块提交到存储系统"),
    ]
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            on_user_message: default_on_user_message(),
            before_ai_call: vec![],
            on_stream_start: vec![],
            on_stream_chunk: vec![],
            after_ai_response: default_after_ai_response(),
            background_process: vec![],
        }
    }
}

impl PipelineConfig {
    /// 创建空配置（不执行任何处理器）
    pub fn empty() -> Self {
        Self {
            on_user_message: vec![],
            before_ai_call: vec![],
            on_stream_start: vec![],
            on_stream_chunk: vec![],
            after_ai_response: vec![],
            background_process: vec![],
        }
    }

    /// 创建仅包含历史简化的最小配置
    pub fn minimal() -> Self {
        Self {
            on_user_message: vec![ProcessorEntry::new("HistorySimplifier")],
            before_ai_call: vec![],
            on_stream_start: vec![],
            on_stream_chunk: vec![],
            after_ai_response: vec![],
            background_process: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PipelineConfig::default();
        assert_eq!(config.on_user_message.len(), 2);
        assert_eq!(config.after_ai_response.len(), 3);
        assert!(config.before_ai_call.is_empty());
        // 验证描述不为空
        assert!(!config.on_user_message[0].description.is_empty());
    }

    #[test]
    fn test_empty_config() {
        let config = PipelineConfig::empty();
        assert!(config.on_user_message.is_empty());
        assert!(config.after_ai_response.is_empty());
    }
    
    #[test]
    fn test_processor_entry() {
        let entry = ProcessorEntry::with_description("Test", "测试描述");
        assert_eq!(entry.name, "Test");
        assert_eq!(entry.description, "测试描述");
    }
}