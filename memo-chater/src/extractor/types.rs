//! 提取器相关类型定义

use serde::{Deserialize, Serialize};

/// 提取出的单条记忆
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedMemory {
    /// 记忆内容
    pub content: String,
    
    /// 记忆类型（可选，由AI判断）
    /// 如：fact, preference, emotion, project, knowledge, decision 等
    #[serde(default)]
    pub memory_type: Option<String>,
    
    /// 重要性评分（可选，1-10）
    #[serde(default)]
    pub importance: Option<u8>,
    
    /// 相关实体/关键词（可选）
    #[serde(default)]
    pub entities: Vec<String>,
}

/// 提取结果
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    /// 提取出的记忆列表
    pub memories: Vec<ExtractedMemory>,
    
    /// 原始AI响应（用于调试）
    pub raw_response: String,
    
    /// 解析是否完全成功
    pub parse_success: bool,
    
    /// 解析警告信息
    pub warnings: Vec<String>,
}

/// 提取器配置
#[derive(Debug, Clone)]
pub struct ExtractorConfig {
    /// AI API 基础URL
    pub api_base: String,
    
    /// API Key
    pub api_key: String,
    
    /// 使用的模型名称
    pub model: String,
    
    /// 自定义提示词（可选，为空则使用默认）
    pub custom_prompt: Option<String>,
    
    /// 用户角色名称（用于记忆提取时的角色标识，默认"我"）
    #[allow(dead_code)]
    pub user_name: String,
    
    /// 助理角色名称（用于记忆提取时的角色标识，默认"你"）
    #[allow(dead_code)]
    pub assistant_name: String,
}

/// 提取器错误
#[derive(Debug, thiserror::Error)]
pub enum ExtractorError {
    #[error("AI调用失败: {0}")]
    AiError(String),
    
    #[error("解析失败: {0}")]
    ParseError(String),
    
    #[error("配置错误: {0}")]
    ConfigError(String),
}