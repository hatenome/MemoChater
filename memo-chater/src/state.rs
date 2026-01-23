//! 应用状态定义

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::AppConfig;
use crate::pipeline::{PipelineDispatcher, PacketStorage};
use crate::assistant::AssistantManager;
use crate::memory::MemoryManager;

/// 应用状态
pub struct AppState {
    pub config: AppConfig,
    pub assistant_manager: Arc<AssistantManager>,
    /// 流水线调度器
    pub dispatcher: Arc<PipelineDispatcher>,
    /// 数据包存储
    pub packet_storage: Arc<PacketStorage>,
    /// 记忆管理器（供 admin_api 使用）
    pub memory_manager: Arc<RwLock<MemoryManager>>,
}