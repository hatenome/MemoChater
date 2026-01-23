//! 助手相关类型定义

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::pipeline::PipelineConfig;

/// 助手ID
pub type AssistantId = String;
/// 话题ID  
pub type TopicId = String;

/// 助手配置（存储在 assistant/config.toml）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantConfig {
    /// 助手显示名称
    pub name: String,
    
    /// 助手描述
    #[serde(default)]
    pub description: String,
    
    /// 系统提示词
    #[serde(default)]
    pub system_prompt: String,
    
    /// 模型配置
    #[serde(default)]
    pub model: ModelConfig,
    
    /// 角色名称配置
    #[serde(default)]
    pub roles: AssistantRolesConfig,
    
    /// 记忆功能配置
    #[serde(default)]
    pub memory: MemoryConfig,
    
    /// 流水线配置
    #[serde(default)]
    pub pipeline: PipelineConfig,
    
    /// 创建时间
    pub created_at: DateTime<Utc>,
    
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

impl Default for AssistantConfig {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            name: "新助手".to_string(),
            description: String::new(),
            system_prompt: String::new(),
            model: ModelConfig::default(),
            roles: AssistantRolesConfig::default(),
            memory: MemoryConfig::default(),
            pipeline: PipelineConfig::default(),
            created_at: now,
            updated_at: now,
        }
    }
}

/// 模型配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// 主模型（用于对话）
    #[serde(default = "default_main_model")]
    pub main_model: String,
    
    /// 处理模型（用于记忆处理，可用便宜模型）
    #[serde(default = "default_processor_model")]
    pub processor_model: String,
    
    /// Embedding 模型
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
    
    /// 信息提取模型
    #[serde(default = "default_extractor_model")]
    pub extractor_model: String,
    
    /// 温度参数
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    
    /// 最大输出token
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

fn default_main_model() -> String { "gpt-4o-mini".to_string() }
fn default_processor_model() -> String { "gpt-4o-mini".to_string() }
fn default_embedding_model() -> String { "text-embedding-3-small".to_string() }
fn default_extractor_model() -> String { "gpt-4o-mini".to_string() }
fn default_temperature() -> f32 { 0.7 }
fn default_max_tokens() -> u32 { 4096 }

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            main_model: default_main_model(),
            processor_model: default_processor_model(),
            embedding_model: default_embedding_model(),
            extractor_model: default_extractor_model(),
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
        }
    }
}

/// 助手角色名称配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantRolesConfig {
    /// 用户角色名称（用于记忆提取）
    #[serde(default = "default_user_name")]
    pub user_name: String,
    
    /// 助理角色名称（用于记忆提取）
    #[serde(default = "default_assistant_name")]
    pub assistant_name: String,
}

fn default_user_name() -> String { "用户".to_string() }
fn default_assistant_name() -> String { "助手".to_string() }

impl Default for AssistantRolesConfig {
    fn default() -> Self {
        Self {
            user_name: default_user_name(),
            assistant_name: default_assistant_name(),
        }
    }
}

/// 记忆功能配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// 是否启用长期记忆
    #[serde(default = "default_memory_enabled")]
    pub enabled: bool,
    
    /// 记忆检索数量
    #[serde(default = "default_retrieval_count")]
    pub retrieval_count: usize,
    
    /// 记忆相关性阈值
    #[serde(default = "default_relevance_threshold")]
    pub relevance_threshold: f32,
}

fn default_memory_enabled() -> bool { true }
fn default_retrieval_count() -> usize { 5 }
fn default_relevance_threshold() -> f32 { 0.6 }

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: default_memory_enabled(),
            retrieval_count: default_retrieval_count(),
            relevance_threshold: default_relevance_threshold(),
        }
    }
}

/// 话题类型
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TopicType {
    /// 普通话题：纯对话，无记忆功能
    #[default]
    Normal,
    /// 记忆话题：启用完整记忆系统
    Memory,
}

/// 话题元信息（存储在 topic/meta.toml）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicMeta {
    /// 话题ID
    pub id: TopicId,
    
    /// 话题标题
    pub title: String,
    
    /// 话题类型（普通/记忆）
    #[serde(default)]
    pub topic_type: TopicType,
    
    /// 创建时间
    pub created_at: DateTime<Utc>,
    
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    
    /// 消息数量
    #[serde(default)]
    pub message_count: usize,
}

impl TopicMeta {
    pub fn new(id: TopicId, title: String, topic_type: TopicType) -> Self {
        let now = Utc::now();
        Self {
            id,
            title,
            topic_type,
            created_at: now,
            updated_at: now,
            message_count: 0,
        }
    }
}

/// 助手摘要信息（用于列表展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantSummary {
    pub id: AssistantId,
    pub name: String,
    pub description: String,
    pub topic_count: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 话题摘要信息（用于列表展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicSummary {
    pub id: TopicId,
    pub assistant_id: AssistantId,
    pub title: String,
    /// 话题类型（普通/记忆）
    pub topic_type: TopicType,
    pub message_count: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}