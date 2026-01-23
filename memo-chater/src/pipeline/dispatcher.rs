//! 流水线调度器
//!
//! 负责按配置顺序执行处理器

use std::collections::HashMap;
use std::sync::Arc;

use super::context::{ProcessorContext, ProcessorContextFactory};
use super::packet::ConversationPacket;
use super::processor::Processor;
use super::config::PipelineConfig;

/// 流水线时机
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineTiming {
    /// 用户发言追加到 messages 后
    OnUserMessage,
    /// 发送给 AI API 前
    BeforeAiCall,
    /// 开始收到 AI 流式响应
    OnStreamStart,
    /// 收到每个 chunk 时
    OnStreamChunk,
    /// AI 响应完整接收后（同步，阻塞下一次对话）
    AfterAiResponse,
    /// 后台异步处理（不阻塞下一次对话）
    BackgroundProcess,
}

/// 流水线错误
#[derive(Debug, thiserror::Error)]
pub enum DispatcherError {
    #[error("上下文创建失败: {0}")]
    ContextCreation(String),

    #[error("处理器执行失败: {0}")]
    ProcessorFailed(String),
}

/// 流水线调度器
pub struct PipelineDispatcher {
    /// 处理器注册表 <处理器名, 处理器实例>
    processors: HashMap<String, Arc<dyn Processor>>,
    /// 处理器上下文工厂
    context_factory: Arc<ProcessorContextFactory>,
}

impl PipelineDispatcher {
    /// 创建调度器
    pub fn new(context_factory: Arc<ProcessorContextFactory>) -> Self {
        Self {
            processors: HashMap::new(),
            context_factory,
        }
    }

    /// 注册处理器
    pub fn register(&mut self, processor: Arc<dyn Processor>) {
        let name = processor.name().to_string();
        tracing::debug!("注册处理器: {}", name);
        self.processors.insert(name, processor);
    }

    /// 批量注册处理器
    pub fn register_all(&mut self, processors: Vec<Arc<dyn Processor>>) {
        for processor in processors {
            self.register(processor);
        }
    }

    /// 获取已注册的处理器列表
    pub fn list_processors(&self) -> Vec<&str> {
        self.processors.keys().map(|s| s.as_str()).collect()
    }

    /// 获取指定名称的处理器
    pub fn get_processor(&self, name: &str) -> Option<&Arc<dyn Processor>> {
        self.processors.get(name)
    }

    /// 执行指定时机的所有处理器
    ///
    /// # 参数
    /// - `timing`: 流水线时机
    /// - `packet`: 对话数据包
    /// - `pipeline_config`: 流水线配置
    /// - `ctx`: 处理器上下文
    ///
    /// # 返回
    /// 成功返回 Ok(())，即使部分处理器失败也会继续执行
    pub async fn dispatch(
        &self,
        timing: PipelineTiming,
        packet: &mut ConversationPacket,
        pipeline_config: &PipelineConfig,
        ctx: &ProcessorContext,
    ) -> Result<(), DispatcherError> {
        let processor_names = match timing {
            PipelineTiming::OnUserMessage => &pipeline_config.on_user_message,
            PipelineTiming::BeforeAiCall => &pipeline_config.before_ai_call,
            PipelineTiming::OnStreamStart => &pipeline_config.on_stream_start,
            PipelineTiming::OnStreamChunk => &pipeline_config.on_stream_chunk,
            PipelineTiming::AfterAiResponse => &pipeline_config.after_ai_response,
            PipelineTiming::BackgroundProcess => &pipeline_config.background_process,
        };

        if processor_names.is_empty() {
            tracing::debug!("时机 {:?} 无处理器配置，跳过", timing);
            return Ok(());
        }

        let names: Vec<&str> = processor_names.iter().map(|e| e.name.as_str()).collect();
        tracing::info!(
            "执行时机 {:?}，处理器列表: {:?}",
            timing,
            names
        );

        let memory_enabled = ctx.is_memory_enabled();

        for entry in processor_names {
            let name = &entry.name;
            if let Some(processor) = self.processors.get(name) {
                // 检查是否需要跳过（记忆功能未启用）
                if processor.requires_memory() && !memory_enabled {
                    tracing::debug!("跳过处理器 {} (记忆功能未启用)", name);
                    continue;
                }

                // 执行处理器
                tracing::debug!("执行处理器: {} ({})", name, entry.description);
                match processor.process(packet, ctx).await {
                    Ok(()) => {
                        packet.last_processor = Some(name.clone());
                        tracing::debug!("处理器 {} 执行成功", name);
                    }
                    Err(e) => {
                        tracing::warn!("处理器 {} 执行失败，跳过: {}", name, e);
                        // 继续下一个处理器，不中断流水线
                    }
                }
            } else {
                tracing::warn!("未找到处理器: {}", name);
            }
        }

        Ok(())
    }

    /// 获取上下文工厂
    pub fn context_factory(&self) -> &Arc<ProcessorContextFactory> {
        &self.context_factory
    }
}