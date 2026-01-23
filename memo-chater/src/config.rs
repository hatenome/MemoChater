//! 配置管理
//! 
//! 分为两层：
//! - GlobalConfig: 全局配置（服务端口、Qdrant、AI API地址）
//! - AssistantConfig: 助手配置（模型、参数、提示词）- 定义在 assistant/types.rs

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 全局配置（存储在 data_dir/global_config.toml 或根目录 config.toml）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// 服务监听地址
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,
    
    /// 数据存储路径（助手、话题、记忆都存储在此目录下）
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    
    /// AI API 配置
    #[serde(default)]
    pub ai: AiApiConfig,
    
    /// Qdrant 配置
    #[serde(default)]
    pub qdrant: QdrantConfig,
    
    /// 角色名称配置（兼容旧代码，新代码应从 AssistantConfig 获取）
    #[serde(default)]
    pub roles: RolesConfig,
    
    /// 提示词目录（兼容旧代码）
    #[serde(default = "default_prompts_dir")]
    pub prompts_dir: PathBuf,
}

fn default_prompts_dir() -> PathBuf { PathBuf::from("./prompts") }

fn default_listen_addr() -> String { "0.0.0.0:7892".to_string() }
fn default_data_dir() -> PathBuf { PathBuf::from("./data") }

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            listen_addr: default_listen_addr(),
            data_dir: default_data_dir(),
            ai: AiApiConfig::default(),
            qdrant: QdrantConfig::default(),
            roles: RolesConfig::default(),
            prompts_dir: default_prompts_dir(),
        }
    }
}

/// 角色名称配置（兼容旧代码）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolesConfig {
    /// 用户角色名称
    #[serde(default = "default_user_name")]
    pub user_name: String,
    
    /// 助理角色名称
    #[serde(default = "default_assistant_name")]
    pub assistant_name: String,
}

fn default_user_name() -> String { "用户".to_string() }
fn default_assistant_name() -> String { "助手".to_string() }

impl Default for RolesConfig {
    fn default() -> Self {
        Self {
            user_name: default_user_name(),
            assistant_name: default_assistant_name(),
        }
    }
}

impl GlobalConfig {
    /// 从文件加载配置
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| ConfigError::IoError(e.to_string()))?;
        
        let config: GlobalConfig = toml::from_str(&content)
            .map_err(|e| ConfigError::ParseError(e.to_string()))?;
        
        Ok(config)
    }

    /// 从默认路径加载配置
    pub fn load_default() -> Result<Self, ConfigError> {
        let paths = ["config.toml", "./config.toml", "../config.toml"];
        
        for path in paths {
            if Path::new(path).exists() {
                return Self::load(path);
            }
        }
        
        // 如果没有配置文件，使用默认配置
        tracing::warn!("未找到配置文件，使用默认配置");
        Ok(Self::default())
    }
    
    /// 获取助手存储根目录
    pub fn assistants_dir(&self) -> PathBuf {
        self.data_dir.join("assistants")
    }
}

/// AI API 配置（全局共享，包含默认模型配置用于兼容旧代码）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiApiConfig {
    /// OpenAI API 基础 URL
    #[serde(default = "default_api_base")]
    pub api_base: String,
    
    /// API Key（直接配置或从环境变量读取）
    #[serde(default)]
    pub api_key: String,
    
    /// 禁用 Gemini 模型的思考功能（通过 thinking_budget: 0）
    #[serde(default)]
    pub disable_gemini_thinking: bool,
    
    // ===== 以下为兼容旧代码的默认模型配置 =====
    // 新代码应从 AssistantConfig 获取模型配置
    
    /// 主模型（默认值，可被助手配置覆盖）
    #[serde(default = "default_main_model")]
    pub main_model: String,
    
    /// 处理模型
    #[serde(default = "default_processor_model")]
    pub processor_model: String,
    
    /// Embedding 模型
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
    
    /// 信息提取模型
    #[serde(default = "default_extractor_model")]
    pub extractor_model: String,
}

fn default_api_base() -> String { "https://api.openai.com/v1".to_string() }
fn default_main_model() -> String { "gpt-4o-mini".to_string() }
fn default_processor_model() -> String { "gpt-4o-mini".to_string() }
fn default_embedding_model() -> String { "text-embedding-3-small".to_string() }
fn default_extractor_model() -> String { "gpt-4o-mini".to_string() }

impl Default for AiApiConfig {
    fn default() -> Self {
        Self {
            api_base: default_api_base(),
            api_key: String::new(),
            disable_gemini_thinking: false,
            main_model: default_main_model(),
            processor_model: default_processor_model(),
            embedding_model: default_embedding_model(),
            extractor_model: default_extractor_model(),
        }
    }
}

impl AiApiConfig {
    /// 获取 API Key（优先使用配置文件，其次环境变量）
    pub fn get_api_key(&self) -> Option<String> {
        if !self.api_key.is_empty() {
            return Some(self.api_key.clone());
        }
        std::env::var("OPENAI_API_KEY").ok()
    }
}

/// Qdrant 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantConfig {
    /// 是否启用内嵌模式
    #[serde(default = "default_embedded")]
    pub embedded: bool,
    
    /// Qdrant 可执行文件路径
    #[serde(default = "default_exe_path")]
    pub exe_path: PathBuf,
    
    /// 存储目录
    #[serde(default = "default_storage_path")]
    pub storage_path: PathBuf,
    
    /// 监听端口
    #[serde(default = "default_port")]
    pub port: u16,
    
    /// 外部 Qdrant URL（embedded=false 时使用）
    #[serde(default = "default_external_url")]
    pub external_url: String,
}

fn default_embedded() -> bool { true }
fn default_exe_path() -> PathBuf { PathBuf::from("../qdrant-x86_64-pc-windows-msvc/qdrant.exe") }
fn default_storage_path() -> PathBuf { PathBuf::from("../qdrant-x86_64-pc-windows-msvc/storage") }
fn default_port() -> u16 { 6333 }
fn default_external_url() -> String { "http://127.0.0.1:6333".to_string() }

impl Default for QdrantConfig {
    fn default() -> Self {
        Self {
            embedded: default_embedded(),
            exe_path: default_exe_path(),
            storage_path: default_storage_path(),
            port: default_port(),
            external_url: default_external_url(),
        }
    }
}

impl QdrantConfig {
    /// 获取 Qdrant URL
    pub fn url(&self) -> String {
        if self.embedded {
            format!("http://127.0.0.1:{}", self.port)
        } else {
            self.external_url.clone()
        }
    }
}

/// 配置错误
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO错误: {0}")]
    IoError(String),
    
    #[error("解析错误: {0}")]
    ParseError(String),
}

// ==================== 兼容性别名 ====================
// 为了减少对现有代码的改动，保留一些别名

/// 兼容旧代码的 AppConfig 别名
pub type AppConfig = GlobalConfig;

/// 兼容旧代码的 AiConfig（包含模型配置）
/// 注意：模型配置现在应该从 AssistantConfig 获取
#[derive(Debug, Clone)]
pub struct AiConfig {
    pub api_base: String,
    pub api_key: String,
    pub main_model: String,
    pub processor_model: String,
    pub embedding_model: String,
    pub extractor_model: String,
}

impl AiConfig {
    
    
    pub fn get_api_key(&self) -> Option<String> {
        if !self.api_key.is_empty() {
            return Some(self.api_key.clone());
        }
        std::env::var("OPENAI_API_KEY").ok()
    }
}

