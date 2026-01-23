//! 对话数据包 - 流水线中流转的核心数据结构
//!
//! ConversationPacket 跟随对话生命周期存在，持久化存储于话题文件夹中。
//! 包含思考池和短期记忆池，供所有处理器安全访问。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

use crate::types::{ChatMessage, ShortTermMemory, ThinkingSource};

/// 思考条目 - 存储AI的内部推理（内嵌于Packet）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingEntry {
    pub content: String,
    pub source: ThinkingSource,
    pub timestamp: DateTime<Utc>,
}

/// 对话轮次 - 一轮完整的用户提问 + AI回复
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    /// 用户消息
    pub user_message: String,
    /// AI回复
    pub assistant_message: String,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
}

/// 对话数据包 - 跟随对话生命周期的核心数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationPacket {
    // ===== 定位信息 =====
    /// 助手ID（用于定位配置和记忆存储）
    pub assistant_id: String,
    /// 话题ID
    pub topic_id: String,

    // ===== 身份信息 =====
    /// 用户ID（可选，用于跨助手追踪）
    #[serde(default)]
    pub user_id: Option<String>,
    /// 用户的人设名（如 "秦"，用于记忆中标识）
    pub user_name: String,
    /// 助手的人设名（如 "诺亚"，用于记忆中标识）
    pub assistant_name: String,

    // ===== 对话内容 =====
    /// 完整的消息列表（直接用于 API 请求的 messages 字段）
    pub messages: Vec<ChatMessage>,

    // ===== 记忆池（跨轮次保留）=====
    /// 思考池 - AI内部推理过程
    #[serde(default)]
    pub thinking_pool: Vec<ThinkingEntry>,
    /// 短期记忆池 - 检索注入的相关信息
    #[serde(default)]
    pub short_term_memory: Vec<ShortTermMemory>,

    // ===== 处理器状态 =====
    /// 当前轮次的处理器状态 <处理器名, 状态数据>
    #[serde(default)]
    pub current_states: HashMap<String, serde_json::Value>,
    /// 历史轮次的处理器状态（保留最近2轮，共3轮可访问）
    #[serde(default)]
    pub history_states: VecDeque<HashMap<String, serde_json::Value>>,

    // ===== 历史对话轮次 =====
    /// 历史对话轮次（每轮包含 user + assistant）
    #[serde(default)]
    pub conversation_turns: Vec<ConversationTurn>,

    // ===== 流程控制 =====
    /// 最后成功通过的处理器名称
    #[serde(default)]
    pub last_processor: Option<String>,
    /// 本轮用户原始输入（不随上下文修改而变化）
    #[serde(default)]
    pub user_input: String,
    /// AI 响应（核心AI调用后填充）
    #[serde(default)]
    pub ai_response: Option<String>,

    /// 最终发送给 AI 的 messages（before_ai_call 后、实际调用前保存）
    #[serde(default)]
    pub last_request_messages: Vec<ChatMessage>,

    // ===== 模型配置（运行时填充）=====
    /// 主对话模型
    #[serde(default)]
    pub main_model: Option<String>,
    /// 处理模型
    #[serde(default)]
    pub processor_model: Option<String>,
    /// Embedding 模型
    #[serde(default)]
    pub embedding_model: Option<String>,
}

impl ConversationPacket {
    /// 创建新的对话数据包
    pub fn new(
        assistant_id: String,
        topic_id: String,
        user_name: String,
        assistant_name: String,
    ) -> Self {
        Self {
            assistant_id,
            topic_id,
            user_id: None,
            user_name,
            assistant_name,
            messages: Vec::new(),
            thinking_pool: Vec::new(),
            short_term_memory: Vec::new(),
            conversation_turns: Vec::new(),
            current_states: HashMap::new(),
            history_states: VecDeque::new(),
            last_processor: None,
            user_input: String::new(),
            ai_response: None,
            last_request_messages: Vec::new(),
            main_model: None,
            processor_model: None,
            embedding_model: None,
        }
    }

    // ===== 消息操作 =====

    /// 追加用户消息
    pub fn append_user_message(&mut self, content: &str) {
        self.messages.push(ChatMessage::user(content));
        self.user_input = content.to_string();
    }

    /// 追加助手消息
    pub fn append_assistant_message(&mut self, content: &str) {
        self.messages.push(ChatMessage::assistant(content));
        self.ai_response = Some(content.to_string());
    }

    /// 保存当前轮次的对话（user_input + ai_response）
    /// 
    /// 在 AI 响应完成后、after_ai_response 处理器执行前调用
    pub fn save_conversation_turn(&mut self) {
        if !self.user_input.is_empty() {
            if let Some(ref ai_response) = self.ai_response {
                self.conversation_turns.push(ConversationTurn {
                    user_message: self.user_input.clone(),
                    assistant_message: ai_response.clone(),
                    timestamp: Utc::now(),
                });
            }
        }
    }

    /// 获取最后一轮对话
    pub fn get_last_turn(&self) -> Option<&ConversationTurn> {
        self.conversation_turns.last()
    }

    /// 获取所有对话轮次
    pub fn get_all_turns(&self) -> &[ConversationTurn] {
        &self.conversation_turns
    }

    /// 设置系统消息（如果不存在则插入到开头）
    pub fn set_system_message(&mut self, content: &str) {
        if let Some(first) = self.messages.first_mut() {
            if first.role == "system" {
                first.content = content.to_string();
                return;
            }
        }
        self.messages.insert(0, ChatMessage::system(content));
    }

    // ===== 思考池操作 =====

    /// 添加思考条目
    pub fn add_thinking(&mut self, content: String, source: ThinkingSource) {
        self.thinking_pool.push(ThinkingEntry {
            content,
            source,
            timestamp: Utc::now(),
        });
    }

    /// 清空思考池
    pub fn clear_thinking(&mut self) {
        self.thinking_pool.clear();
    }

    /// 获取思考池内容
    pub fn get_thinking(&self) -> &[ThinkingEntry] {
        &self.thinking_pool
    }

    // ===== 短期记忆操作 =====

    /// 添加短期记忆
    pub fn add_short_term_memory(&mut self, memory: ShortTermMemory) {
        self.short_term_memory.push(memory);
    }

    /// 批量添加短期记忆
    pub fn add_short_term_memories(&mut self, memories: Vec<ShortTermMemory>) {
        self.short_term_memory.extend(memories);
    }

    /// 清空短期记忆
    pub fn clear_short_term_memory(&mut self) {
        self.short_term_memory.clear();
    }

    /// 获取短期记忆
    pub fn get_short_term_memory(&self) -> &[ShortTermMemory] {
        &self.short_term_memory
    }

    /// 按相关性排序获取短期记忆
    pub fn get_short_term_memory_sorted(&self) -> Vec<&ShortTermMemory> {
        let mut sorted: Vec<_> = self.short_term_memory.iter().collect();
        sorted.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));
        sorted
    }

    /// 衰减短期记忆（降低相关性分数）
    pub fn decay_short_term_memory(&mut self, decay_factor: f32) {
        for memory in &mut self.short_term_memory {
            memory.relevance *= decay_factor;
        }
        // 移除相关性过低的记忆
        self.short_term_memory.retain(|m| m.relevance > 0.1);
    }

    // ===== 处理器状态操作 =====

    /// 轮次结束处理：状态轮转（思考池和短期记忆保留）
    pub fn end_turn(&mut self) {
        // 将当前状态推入历史
        if !self.current_states.is_empty() {
            self.history_states.push_front(self.current_states.clone());
        }
        // 保持历史状态最多2轮
        while self.history_states.len() > 2 {
            self.history_states.pop_back();
        }
        // 清空当前轮次状态（思考池和短期记忆保留）
        self.current_states.clear();
        self.last_processor = None;
        self.user_input.clear();
        self.ai_response = None;
    }

    /// 获取上一轮的处理器状态
    pub fn get_previous_state(&self, processor_name: &str) -> Option<&serde_json::Value> {
        self.history_states
            .front()
            .and_then(|states| states.get(processor_name))
    }

    /// 记录处理器状态
    pub fn set_processor_state(&mut self, processor_name: &str, state: serde_json::Value) {
        self.current_states.insert(processor_name.to_string(), state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MemorySource;

    #[test]
    fn test_packet_creation() {
        let packet = ConversationPacket::new(
            "ast_001".to_string(),
            "topic_001".to_string(),
            "秦".to_string(),
            "诺亚".to_string(),
        );
        assert_eq!(packet.assistant_id, "ast_001");
        assert_eq!(packet.user_name, "秦");
        assert!(packet.messages.is_empty());
        assert!(packet.thinking_pool.is_empty());
        assert!(packet.short_term_memory.is_empty());
    }

    #[test]
    fn test_thinking_pool() {
        let mut packet = ConversationPacket::new(
            "ast_001".to_string(),
            "topic_001".to_string(),
            "秦".to_string(),
            "诺亚".to_string(),
        );
        
        packet.add_thinking("用户询问了天气".to_string(), ThinkingSource::UserAnalysis);
        packet.add_thinking("检索到相关记忆".to_string(), ThinkingSource::MemoryRetrieval);
        
        assert_eq!(packet.thinking_pool.len(), 2);
        
        // 轮次结束后思考池保留
        packet.end_turn();
        assert_eq!(packet.thinking_pool.len(), 2);
        
        // 手动清空
        packet.clear_thinking();
        assert!(packet.thinking_pool.is_empty());
    }

    #[test]
    fn test_short_term_memory() {
        let mut packet = ConversationPacket::new(
            "ast_001".to_string(),
            "topic_001".to_string(),
            "秦".to_string(),
            "诺亚".to_string(),
        );
        
        let mem1 = ShortTermMemory {
            id: "mem_1".to_string(),
            summary: "用户喜欢编程".to_string(),
            content: "用户喜欢编程，经常讨论代码相关话题".to_string(),
            memory_type: "preference".to_string(),
            relevance: 0.9,
            confidence: 1.0,
            source: MemorySource::LongTermRetrieval,
            timestamp: Utc::now(),
        };
        let mem2 = ShortTermMemory {
            id: "mem_2".to_string(),
            summary: "用户住在北京".to_string(),
            content: "用户住在北京".to_string(),
            memory_type: "fact".to_string(),
            relevance: 0.7,
            confidence: 1.0,
            source: MemorySource::LongTermRetrieval,
            timestamp: Utc::now(),
        };
        
        packet.add_short_term_memory(mem1);
        packet.add_short_term_memory(mem2);
        
        assert_eq!(packet.short_term_memory.len(), 2);
        
        // 按相关性排序
        let sorted = packet.get_short_term_memory_sorted();
        assert_eq!(sorted[0].relevance, 0.9);
        assert_eq!(sorted[1].relevance, 0.7);
        
        // 轮次结束后短期记忆保留
        packet.end_turn();
        assert_eq!(packet.short_term_memory.len(), 2);
        
        // 衰减测试
        packet.decay_short_term_memory(0.5);
        assert!(packet.short_term_memory[0].relevance < 0.5);
    }

    #[test]
    fn test_end_turn() {
        let mut packet = ConversationPacket::new(
            "ast_001".to_string(),
            "topic_001".to_string(),
            "秦".to_string(),
            "诺亚".to_string(),
        );
        
        packet.set_processor_state("TestProcessor", serde_json::json!({"count": 1}));
        packet.end_turn();
        
        assert!(packet.current_states.is_empty());
        assert_eq!(packet.history_states.len(), 1);
        
        let prev = packet.get_previous_state("TestProcessor");
        assert!(prev.is_some());
    }
}