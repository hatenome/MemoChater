//! 记忆相关类型
//!
//! 注意：ThinkingEntry 已移至 pipeline::packet 模块，此处仅保留 ThinkingSource 枚举

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 思考来源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThinkingSource {
    /// 用户输入分析
    UserAnalysis,
    /// 记忆检索结果
    MemoryRetrieval,
    /// 工具调用结果
    ToolResult,
    /// AI自我反思
    SelfReflection,
}

/// 短期记忆条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortTermMemory {
    pub id: String,
    /// 概述/标题
    pub summary: String,
    /// 详细内容
    pub content: String,
    /// 记忆类型（如 fact/event/preference/knowledge/task/other）
    pub memory_type: String,
    /// 是否需要展开（前端控制）
    #[serde(default)]
    pub should_expand: bool,
    /// 来源（从长期记忆检索/当前对话提取）
    pub source: MemorySource,
    /// 创建时间
    pub timestamp: DateTime<Utc>,
}

/// 记忆来源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemorySource {
    /// 从长期记忆检索
    LongTermRetrieval,
    /// 当前对话提取
    CurrentConversation,
    /// 工具结果
    ToolResult,
}

/// 长期记忆条目 - 存储在向量库
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongTermMemory {
    pub id: String,
    pub content: String,
    /// 记忆类别（自由字符串，如 fact/preference/event/knowledge 等）
    pub category: String,
    /// 重要性 0.0 - 1.0
    pub importance: f32,
    /// 访问次数
    pub access_count: u32,
    /// 最后访问时间
    pub last_accessed: DateTime<Utc>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 来源会话ID
    pub source_session: Option<String>,
    /// 关联的文件ID列表（存储在FileStore中）
    pub file_refs: Vec<String>,
    /// 标签
    pub tags: Vec<String>,
}

impl LongTermMemory {
    pub fn new(content: String, category: String, importance: f32) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            content,
            category,
            importance,
            access_count: 0,
            last_accessed: now,
            created_at: now,
            source_session: None,
            file_refs: Vec::new(),
            tags: Vec::new(),
        }
    }

    /// 设置来源会话
    pub fn with_session(mut self, session_id: String) -> Self {
        self.source_session = Some(session_id);
        self
    }

    /// 添加文件引用
    pub fn with_file_ref(mut self, file_id: String) -> Self {
        self.file_refs.push(file_id);
        self
    }

    /// 添加标签
    pub fn with_tag(mut self, tag: String) -> Self {
        self.tags.push(tag);
        self
    }

    /// 记录一次访问
    pub fn record_access(&mut self) {
        self.access_count += 1;
        self.last_accessed = Utc::now();
    }
}

/// 文件内容 - JSON文件存储
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFile {
    pub id: String,
    /// 原始引用标识（如 "code_1"）
    pub original_ref: String,
    /// 所属记忆ID
    pub memory_id: String,
    /// 文件类型（code/image/document等）
    pub file_type: String,
    /// 实际内容
    pub content: String,
    /// 代码语言（可选）
    pub language: Option<String>,
    /// 扩展元数据
    pub metadata: serde_json::Value,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

impl MemoryFile {
    pub fn new(
        original_ref: String,
        memory_id: String,
        file_type: String,
        content: String,
        language: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            original_ref,
            memory_id,
            file_type,
            content,
            language,
            metadata: serde_json::json!({}),
            created_at: Utc::now(),
        }
    }
}

/// 解析器输出的原始记忆（含局部ID引用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawExtractedMemory {
    pub content: String,
    pub category: String,
    pub importance: f32,
    pub file_refs: Vec<RawFileRef>,
}

/// 原始文件引用（解析器输出的局部引用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawFileRef {
    /// 局部ID（如 "code_1"）
    pub local_id: String,
    pub file_type: String,
    pub content: String,
    pub language: Option<String>,
}

impl RawExtractedMemory {
    /// 转换为可存储的记忆，同时生成文件记录
    /// 返回 (长期记忆, 文件列表)
    pub fn into_storable(self, session_id: Option<String>) -> (LongTermMemory, Vec<MemoryFile>) {
        let memory_id = uuid::Uuid::new_v4().to_string();

        let mut file_refs = Vec::new();
        let mut files = Vec::new();

        for raw_ref in self.file_refs {
            let file = MemoryFile::new(
                raw_ref.local_id,
                memory_id.clone(),
                raw_ref.file_type,
                raw_ref.content,
                raw_ref.language,
            );
            file_refs.push(file.id.clone());
            files.push(file);
        }

        let mut memory = LongTermMemory::new(self.content, self.category, self.importance);
        memory.id = memory_id;
        memory.source_session = session_id;
        memory.file_refs = file_refs;

        (memory, files)
    }
}

/// 待处理的有价值记忆
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingMemory {
    pub content: String,
    pub category: String,
    pub importance: f32,
    pub source_session: String,
    pub created_at: DateTime<Utc>,
}

/// 对话历史条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationEntry {
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

/// 记忆处理器的输出结构（对应memory_processor.txt的JSON格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorOutput {
    pub thinking_summary: String,
    pub valuable_memories: Vec<ValuableMemory>,
    pub forget_suggestions: Vec<String>,
    pub context_updates: Vec<String>,
}

/// 有价值的记忆（处理器输出）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValuableMemory {
    pub content: String,
    pub category: String,
    pub importance: f32,
}

impl ValuableMemory {
    /// 转换为LongTermMemory
    pub fn to_long_term_memory(&self) -> LongTermMemory {
        LongTermMemory::new(
            self.content.clone(),
            self.category.clone(),
            self.importance,
        )
    }
}