//! 处理器上下文
//!
//! 提供处理器执行所需的公共依赖

use std::sync::Arc;

use crate::ai::AiClient;
use crate::assistant::{AssistantConfig, AssistantManager, TopicType};
use crate::config::GlobalConfig;
use crate::memory::MemoryManager;
use tokio::sync::RwLock;

/// 处理器上下文 - 提供处理器执行所需的公共依赖
///
/// 注意：记忆相关的存储需要通过 memory_manager 动态获取，
/// 因为它们涉及可变借用，不适合在上下文中持有引用。
pub struct ProcessorContext {
    /// 助手配置
    pub assistant_config: AssistantConfig,

    /// 助手 ID
    pub assistant_id: String,

    /// 话题 ID
    pub topic_id: String,

    /// 话题类型（决定是否启用记忆功能）
    pub topic_type: TopicType,

    /// AI 客户端（供需要调用AI的处理器使用）
    pub ai_client: Arc<AiClient>,

    /// 全局配置
    pub global_config: Arc<GlobalConfig>,

    /// 助手管理器
    pub assistant_manager: Arc<AssistantManager>,

    /// 记忆管理器（处理器需要时通过此获取记忆存储）
    pub memory_manager: Arc<RwLock<MemoryManager>>,
}

impl ProcessorContext {
    /// 创建新的处理器上下文
    pub fn new(
        assistant_config: AssistantConfig,
        assistant_id: String,
        topic_id: String,
        topic_type: TopicType,
        ai_client: Arc<AiClient>,
        global_config: Arc<GlobalConfig>,
        assistant_manager: Arc<AssistantManager>,
        memory_manager: Arc<RwLock<MemoryManager>>,
    ) -> Self {
        Self {
            assistant_config,
            assistant_id,
            topic_id,
            topic_type,
            ai_client,
            global_config,
            assistant_manager,
            memory_manager,
        }
    }

    /// 获取记忆是否启用（基于话题类型）
    /// 
    /// 只有记忆话题才启用记忆功能
    pub fn is_memory_enabled(&self) -> bool {
        self.topic_type == TopicType::Memory
    }

    /// 获取话题类型
    pub fn topic_type(&self) -> &TopicType {
        &self.topic_type
    }

    /// 获取主模型名称
    pub fn main_model(&self) -> &str {
        &self.assistant_config.model.main_model
    }

    /// 获取处理模型名称
    pub fn processor_model(&self) -> &str {
        &self.assistant_config.model.processor_model
    }

    /// 获取 Embedding 模型名称
    pub fn embedding_model(&self) -> &str {
        &self.assistant_config.model.embedding_model
    }

    /// 获取用户名
    pub fn user_name(&self) -> &str {
        &self.assistant_config.roles.user_name
    }

    /// 获取助手名
    pub fn assistant_name(&self) -> &str {
        &self.assistant_config.roles.assistant_name
    }

    /// 获取系统提示词
    pub fn system_prompt(&self) -> &str {
        &self.assistant_config.system_prompt
    }

    /// 获取记忆检索数量
    pub fn retrieval_count(&self) -> usize {
        self.assistant_config.memory.retrieval_count
    }

    /// 获取记忆相关性阈值
    pub fn relevance_threshold(&self) -> f32 {
        self.assistant_config.memory.relevance_threshold
    }
}

/// 处理器上下文工厂
///
/// 用于根据助手ID和话题ID创建处理器上下文
pub struct ProcessorContextFactory {
    pub ai_client: Arc<AiClient>,
    pub global_config: Arc<GlobalConfig>,
    pub assistant_manager: Arc<AssistantManager>,
    pub memory_manager: Arc<RwLock<MemoryManager>>,
}

impl ProcessorContextFactory {
    pub fn new(
        ai_client: Arc<AiClient>,
        global_config: Arc<GlobalConfig>,
        assistant_manager: Arc<AssistantManager>,
        memory_manager: Arc<RwLock<MemoryManager>>,
    ) -> Self {
        Self {
            ai_client,
            global_config,
            assistant_manager,
            memory_manager,
        }
    }

    /// 根据助手ID和话题ID创建处理器上下文
    pub async fn create(
        &self,
        assistant_id: &str,
        topic_id: &str,
    ) -> Result<ProcessorContext, String> {
        // 获取助手配置
        let assistant_config = self
            .assistant_manager
            .get_assistant(assistant_id)
            .await
            .map_err(|e| format!("获取助手配置失败: {}", e))?;

        // 获取话题类型
        let topic_meta = self
            .assistant_manager
            .get_topic(assistant_id, topic_id)
            .await
            .map_err(|e| format!("获取话题信息失败: {}", e))?;

        Ok(ProcessorContext::new(
            assistant_config,
            assistant_id.to_string(),
            topic_id.to_string(),
            topic_meta.topic_type,
            self.ai_client.clone(),
            self.global_config.clone(),
            self.assistant_manager.clone(),
            self.memory_manager.clone(),
        ))
    }
}