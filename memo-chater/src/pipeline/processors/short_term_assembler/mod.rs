//! 短期记忆组装器
//!
//! 将短期记忆注入到对话上下文中

use async_trait::async_trait;
use serde::Deserialize;
use std::path::PathBuf;
use tracing::{info, warn, debug};

use crate::pipeline::{
    processor::{Processor, ProcessorError},
    context::ProcessorContext,
    packet::ConversationPacket,
};
use crate::types::ChatMessage;

/// 组装器配置
#[derive(Debug, Clone, Deserialize)]
struct AssemblerConfig {
    /// 使用的模型（留空则使用助手配置的 processor_model）
    #[serde(default)]
    model: String,
    /// 用户消息模板
    user_message_template: String,
    /// 助手确认消息
    assistant_confirm_message: String,
}

impl Default for AssemblerConfig {
    fn default() -> Self {
        Self {
            model: String::new(),
            user_message_template: r#"【系统消息-短期记忆】现在为你注入短期记忆，你现在正在与{user_name}进行交谈
---短期记忆---
{memories}
---短期记忆结束---
"#.to_string(),
            assistant_confirm_message: "已成功回忆".to_string(),
        }
    }
}

/// 短期记忆组装器
/// 
/// 负责将短期记忆注入到对话上下文中
pub struct ShortTermAssembler {
    config_path: PathBuf,
}

impl ShortTermAssembler {
    pub fn new() -> Self {
        // 配置文件路径：与本模块同目录
        let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/pipeline/processors/short_term_assembler/config.toml");
        
        Self { config_path }
    }

    /// 加载配置
    fn load_config(&self) -> Result<AssemblerConfig, ProcessorError> {
        if self.config_path.exists() {
            let content = std::fs::read_to_string(&self.config_path)
                .map_err(|e| ProcessorError::Config(format!("读取配置文件失败: {}", e)))?;
            
            toml::from_str(&content)
                .map_err(|e| ProcessorError::Config(format!("解析配置文件失败: {}", e)))
        } else {
            warn!("ShortTermAssembler 配置文件不存在，使用默认配置: {:?}", self.config_path);
            Ok(AssemblerConfig::default())
        }
    }

    /// 格式化短期记忆列表
    fn format_memories(&self, packet: &ConversationPacket) -> String {
        let memories = packet.get_short_term_memory_sorted();
        
        if memories.is_empty() {
            return "（暂无短期记忆）".to_string();
        }

        memories
            .iter()
            .map(|m| format!("[{}]{}", m.memory_type, m.summary))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[async_trait]
impl Processor for ShortTermAssembler {
    fn name(&self) -> &'static str {
        "ShortTermAssembler"
    }

    fn requires_memory(&self) -> bool {
        true
    }

    async fn process(
        &self,
        packet: &mut ConversationPacket,
        _ctx: &ProcessorContext,
    ) -> Result<(), ProcessorError> {
        info!("ShortTermAssembler 开始处理");

        // 加载配置
        let config = self.load_config()?;
        debug!("配置加载成功");

        // 检查是否有短期记忆
        let memory_count = packet.get_short_term_memory().len();
        if memory_count == 0 {
            info!("短期记忆为空，跳过组装");
            packet.set_processor_state(self.name(), serde_json::json!({
                "skipped": true,
                "reason": "no_short_term_memory"
            }));
            return Ok(());
        }

        info!("准备注入 {} 条短期记忆", memory_count);

        // 1. 清除系统消息以外的上下文
        let system_msg = packet.messages.iter()
            .find(|m| m.role == "system")
            .cloned();
        
        packet.messages.clear();
        
        if let Some(sys) = system_msg {
            packet.messages.push(sys);
        }

        debug!("已清除非系统消息的上下文");

        // 2. 格式化短期记忆
        let memories_text = self.format_memories(packet);
        debug!("格式化后的记忆:\n{}", memories_text);

        // 3. 构建用户消息
        let user_content = config.user_message_template
            .replace("{user_name}", &packet.user_name)
            .replace("{memories}", &memories_text);

        // 4. 添加用户消息（记忆注入）
        packet.messages.push(ChatMessage::user(&user_content));

        // 5. 加回用户原始输入
        if !packet.user_input.is_empty() {
            packet.messages.push(ChatMessage::user(&packet.user_input));
            debug!("已加回用户原始输入: {}", packet.user_input);
        }

        // // ===== 输出完整上下文结构（调试用）=====
        // info!("========== ShortTermAssembler 组装后完整上下文 ==========");
        // info!("消息总数: {}", packet.messages.len());
        // for (i, msg) in packet.messages.iter().enumerate() {
        //     info!("--- 消息 {} [{}] ---", i, msg.role);
        //     info!("{}", msg.content);
        // }
        // info!("========== 上下文结束 ==========");

        // 保存处理器状态
        packet.set_processor_state(self.name(), serde_json::json!({
            "assembled": true,
            "memory_count": memory_count,
            "context_cleared": true
        }));

        Ok(())
    }
}