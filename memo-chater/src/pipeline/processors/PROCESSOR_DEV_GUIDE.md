# 处理器开发指南

> MemoChater 流水线处理器开发完整指南
> 最后更新：2026-01-05

## 一、架构概览

### 1.1 核心组件关系

```
┌─────────────────────────────────────────────────────────────────┐
│                        PipelineDispatcher                        │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  注册表: HashMap<String, Arc<dyn Processor>>            │    │
│  │  - HistorySimplifier                                     │    │
│  │  - MemoryAssembler                                       │    │
│  │  - ShortTermAssembler                                    │    │
│  │  - ContextCleaner                                        │    │
│  │  - SubconsciousProcessor                                 │    │
│  │  - ContentChunker                                        │    │
│  │  - MemoryCommitter                                       │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  dispatch(timing, packet, config, ctx)                   │    │
│  │  按 PipelineConfig 中配置的顺序执行处理器                │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
                               │
           ┌───────────────────┼───────────────────┐
           ▼                   ▼                   ▼
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│ ConversationPacket│  │ ProcessorContext │  │ PipelineConfig  │
│ (可变数据包)      │  │ (只读上下文)     │  │ (处理器列表)    │
└─────────────────┘  └─────────────────┘  └─────────────────┘
```

### 1.2 处理时机 (PipelineTiming)

| 时机 | 触发点 | 典型用途 |
|------|--------|----------|
| `OnUserMessage` | 用户消息追加到 messages 后 | 历史简化、记忆检索、上下文装配 |
| `BeforeAiCall` | 发送给 AI API 前 | 最终上下文调整（预留） |
| `OnStreamStart` | 开始收到 AI 流式响应 | 预留 |
| `OnStreamChunk` | 收到每个 chunk 时 | 预留 |
| `AfterAiResponse` | AI 响应完整接收后（同步） | 上下文清理、内容切块、记忆提交 |
| `BackgroundProcess` | 同步处理完成后（异步） | 耗时但非关键的后台任务 |

### 1.3 默认流水线配置

```rust
// on_user_message 阶段
1. HistorySimplifier  - 简化/压缩历史对话
2. MemoryAssembler    - 检索并装配长期记忆

// after_ai_response 阶段
1. SubconsciousProcessor - 潜意识层面分析
2. ContentChunker        - 内容切块
3. MemoryCommitter       - 提交记忆到存储
```

---

## 二、核心数据结构

### 2.1 ConversationPacket（对话数据包）

数据包是流水线中流转的核心数据结构，**处理器通过修改它来传递数据**。

```rust
pub struct ConversationPacket {
    // ===== 定位信息 =====
    pub assistant_id: String,           // 助手ID
    pub topic_id: String,               // 话题ID

    // ===== 身份信息 =====
    pub user_id: Option<String>,        // 用户ID（可选）
    pub user_name: String,              // 用户人设名（如 "秦"）
    pub assistant_name: String,         // 助手人设名（如 "诺亚"）

    // ===== 对话内容 =====
    pub messages: Vec<ChatMessage>,     // 完整消息列表（直接用于API请求）

    // ===== 记忆池（跨轮次保留）=====
    pub thinking_pool: Vec<ThinkingEntry>,      // 思考池 - AI内部推理
    pub short_term_memory: Vec<ShortTermMemory>, // 短期记忆池

    // ===== 处理器状态 =====
    pub current_states: HashMap<String, Value>, // 当前轮次状态
    pub history_states: VecDeque<HashMap<String, Value>>, // 历史轮次（最近2轮）

    // ===== 流程控制 =====
    pub last_processor: Option<String>, // 最后成功的处理器
    pub user_input: String,             // 本轮用户原始输入
    pub ai_response: Option<String>,    // AI响应

    // ===== 模型配置（运行时填充）=====
    pub main_model: Option<String>,
    pub processor_model: Option<String>,
    pub embedding_model: Option<String>,
}
```

#### 关键方法

```rust
// ===== 消息操作 =====
packet.append_user_message("你好");           // 追加用户消息
packet.append_assistant_message("你好！");    // 追加助手消息
packet.set_system_message("你是一个助手");    // 设置系统消息

// ===== 思考池操作 =====
packet.add_thinking("分析用户意图".to_string(), ThinkingSource::UserAnalysis);
packet.get_thinking();                        // 获取思考池
packet.clear_thinking();                      // 清空思考池

// ===== 短期记忆操作 =====
packet.add_short_term_memory(memory);         // 添加短期记忆
packet.add_short_term_memories(vec![...]);    // 批量添加
packet.get_short_term_memory();               // 获取短期记忆
packet.get_short_term_memory_sorted();        // 按相关性排序获取
packet.decay_short_term_memory(0.8);          // 衰减（降低相关性）
packet.clear_short_term_memory();             // 清空

// ===== 处理器状态操作 =====
packet.set_processor_state("MyProcessor", json!({"key": "value"}));
packet.get_previous_state("MyProcessor");     // 获取上一轮状态
packet.end_turn();                            // 轮次结束（状态轮转）
```

### 2.2 ProcessorContext（处理器上下文）

上下文提供处理器执行所需的**只读**公共依赖。

```rust
pub struct ProcessorContext {
    pub assistant_config: AssistantConfig,  // 助手配置
    pub assistant_id: String,
    pub topic_id: String,
    pub topic_type: TopicType,              // Normal 或 Memory
    pub ai_client: Arc<AiClient>,           // AI客户端
    pub global_config: Arc<GlobalConfig>,   // 全局配置
    pub assistant_manager: Arc<AssistantManager>,
    pub memory_manager: Arc<RwLock<MemoryManager>>, // 记忆管理器
}
```

#### 便捷方法

```rust
ctx.is_memory_enabled()      // 是否启用记忆（基于话题类型）
ctx.topic_type()             // 获取话题类型
ctx.main_model()             // 主模型名称
ctx.processor_model()        // 处理模型名称
ctx.embedding_model()        // Embedding模型名称
ctx.user_name()              // 用户名
ctx.assistant_name()         // 助手名
ctx.system_prompt()          // 系统提示词
ctx.retrieval_count()        // 记忆检索数量
ctx.relevance_threshold()    // 相关性阈值
```

### 2.3 相关类型定义

```rust
// 聊天消息
pub struct ChatMessage {
    pub role: String,    // "user" | "assistant" | "system"
    pub content: String,
}

impl ChatMessage {
    pub fn user(content: &str) -> Self;
    pub fn assistant(content: &str) -> Self;
    pub fn system(content: &str) -> Self;
}

// 思考来源
pub enum ThinkingSource {
    UserAnalysis,      // 用户输入分析
    MemoryRetrieval,   // 记忆检索结果
    ToolResult,        // 工具调用结果
    SelfReflection,    // AI自我反思
}

// 短期记忆（结构化字段）
pub struct ShortTermMemory {
    pub id: String,
    pub summary: String,       // 概述/标题
    pub content: String,       // 详细内容
    pub memory_type: String,   // 类型：fact/event/preference/knowledge/task/other
    pub relevance: f32,        // 0.0 - 1.0
    pub confidence: f32,       // 置信度 0.0 - 1.0
    pub source: MemorySource,
    pub timestamp: DateTime<Utc>,
}

// 记忆来源
pub enum MemorySource {
    LongTermRetrieval,     // 从长期记忆检索
    CurrentConversation,   // 当前对话提取
    ToolResult,            // 工具结果
}

// 长期记忆
pub struct LongTermMemory {
    pub id: String,
    pub content: String,
    pub category: String,      // fact/preference/event/knowledge 等
    pub importance: f32,       // 0.0 - 1.0
    pub access_count: u32,
    pub last_accessed: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub source_session: Option<String>,
    pub file_refs: Vec<String>,
    pub tags: Vec<String>,
}
```

---

## 三、Processor Trait 详解

### 3.1 Trait 定义

```rust
#[async_trait]
pub trait Processor: Send + Sync {
    /// 处理器名称（用于配置引用和状态字典键名）
    fn name(&self) -> &'static str;

    /// 是否需要记忆功能开启才执行
    /// 返回 true 时，普通话题会跳过此处理器
    fn requires_memory(&self) -> bool;

    /// 执行处理
    async fn process(
        &self,
        packet: &mut ConversationPacket,
        ctx: &ProcessorContext,
    ) -> Result<(), ProcessorError>;
}
```

### 3.2 ProcessorError 错误类型

```rust
pub enum ProcessorError {
    Internal(String),      // 处理器内部错误
    Config(String),        // 配置错误
    Service(String),       // 依赖服务错误
    AiError(String),       // AI调用错误
    MemoryError(String),   // 记忆操作错误
}
```

### 3.3 错误处理策略

- 处理器返回 `Err` 时，**流水线不会中断**
- 调度器会记录警告日志，然后继续执行下一个处理器
- 这保证了单个处理器失败不会影响整体对话流程

---

## 四、开发新处理器

### 4.1 基础模板

```rust
//! 我的处理器
//!
//! 处理器功能描述

use async_trait::async_trait;
use crate::pipeline::{
    processor::{Processor, ProcessorError},
    context::ProcessorContext,
    packet::ConversationPacket,
};

/// 我的处理器
pub struct MyProcessor;

impl MyProcessor {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Processor for MyProcessor {
    fn name(&self) -> &'static str {
        "MyProcessor"  // 必须与配置中的名称一致
    }

    fn requires_memory(&self) -> bool {
        true  // 是否需要记忆功能
    }

    async fn process(
        &self,
        packet: &mut ConversationPacket,
        ctx: &ProcessorContext,
    ) -> Result<(), ProcessorError> {
        // 实现处理逻辑
        
        // 记录处理器状态（可选）
        packet.set_processor_state(self.name(), serde_json::json!({
            "processed": true,
            "some_metric": 42
        }));
        
        Ok(())
    }
}
```

### 4.2 注册处理器

在 `src/pipeline/processors/mod.rs` 中：

```rust
mod my_processor;
pub use my_processor::MyProcessor;

pub fn create_all_processors() -> Vec<Arc<dyn Processor>> {
    vec![
        Arc::new(HistorySimplifier::new()),
        Arc::new(MemoryAssembler::new()),
        Arc::new(SubconsciousProcessor::new()),
        Arc::new(ContentChunker::new()),
        Arc::new(MemoryCommitter::new()),
        Arc::new(MyProcessor::new()),  // 添加新处理器
    ]
}
```

### 4.3 配置处理器执行

在 `src/pipeline/config.rs` 的默认配置中添加，或通过助手配置动态指定：

```rust
fn default_on_user_message() -> Vec<ProcessorEntry> {
    vec![
        ProcessorEntry::with_description("HistorySimplifier", "简化历史对话"),
        ProcessorEntry::with_description("MyProcessor", "我的处理器描述"),
        ProcessorEntry::with_description("MemoryAssembler", "装配记忆"),
    ]
}
```

---

## 五、常见开发模式

### 5.1 调用 AI 进行分析

```rust
async fn process(
    &self,
    packet: &mut ConversationPacket,
    ctx: &ProcessorContext,
) -> Result<(), ProcessorError> {
    // 构建分析提示
    let prompt = format!(
        "分析以下对话内容：\n{}",
        packet.user_input
    );
    
    let messages = vec![
        ChatMessage::system("你是一个分析助手"),
        ChatMessage::user(&prompt),
    ];
    
    // 使用处理模型调用AI
    let response = ctx.ai_client
        .chat_with_model(&messages, Some(ctx.processor_model()))
        .await
        .map_err(|e| ProcessorError::AiError(e.to_string()))?;
    
    // 处理响应...
    
    Ok(())
}
```

### 5.2 检索长期记忆

```rust
async fn process(
    &self,
    packet: &mut ConversationPacket,
    ctx: &ProcessorContext,
) -> Result<(), ProcessorError> {
    // 获取记忆管理器
    let mut memory_manager = ctx.memory_manager.write().await;
    
    // 获取助手的长期记忆存储
    let long_term = memory_manager
        .get_assistant_long_term_with_embedding(
            &ctx.assistant_id,
            Some(ctx.embedding_model()),
        )
        .await
        .map_err(|e| ProcessorError::MemoryError(e.to_string()))?;
    
    // 搜索相关记忆
    let results = long_term
        .search(&packet.user_input, ctx.retrieval_count())
        .await
        .map_err(|e| ProcessorError::MemoryError(e.to_string()))?;
    
    // 转换为短期记忆并注入
    for result in results {
        if result.relevance >= ctx.relevance_threshold() {
            packet.add_short_term_memory(ShortTermMemory {
                id: result.memory.id,
                content: result.memory.content,
                relevance: result.relevance,
                source: MemorySource::LongTermRetrieval,
                timestamp: Utc::now(),
            });
        }
    }
    
    Ok(())
}
```

### 5.3 存储新记忆

```rust
async fn process(
    &self,
    packet: &mut ConversationPacket,
    ctx: &ProcessorContext,
) -> Result<(), ProcessorError> {
    // 从处理器状态获取待存储的记忆（假设由前一个处理器提取）
    let chunks = packet.current_states
        .get("ContentChunker")
        .and_then(|v| v.get("chunks"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    
    if chunks.is_empty() {
        return Ok(());
    }
    
    // 获取记忆管理器
    let mut memory_manager = ctx.memory_manager.write().await;
    let long_term = memory_manager
        .get_assistant_long_term_with_embedding(
            &ctx.assistant_id,
            Some(ctx.embedding_model()),
        )
        .await
        .map_err(|e| ProcessorError::MemoryError(e.to_string()))?;
    
    // 批量存储
    let memories: Vec<LongTermMemory> = chunks
        .iter()
        .filter_map(|chunk| {
            let content = chunk.get("content")?.as_str()?;
            let category = chunk.get("category")?.as_str().unwrap_or("general");
            let importance = chunk.get("importance")?.as_f64().unwrap_or(0.5) as f32;
            
            Some(LongTermMemory::new(
                content.to_string(),
                category.to_string(),
                importance,
            ))
        })
        .collect();
    
    long_term
        .store_batch(&memories)
        .await
        .map_err(|e| ProcessorError::MemoryError(e.to_string()))?;
    
    packet.set_processor_state(self.name(), json!({
        "committed": memories.len()
    }));
    
    Ok(())
}
```

### 5.4 修改消息上下文

```rust
async fn process(
    &self,
    packet: &mut ConversationPacket,
    ctx: &ProcessorContext,
) -> Result<(), ProcessorError> {
    // 获取短期记忆
    let memories = packet.get_short_term_memory_sorted();
    
    if memories.is_empty() {
        return Ok(());
    }
    
    // 构建记忆注入文本
    let memory_text = memories
        .iter()
        .take(5)
        .map(|m| format!("- {}", m.content))
        .collect::<Vec<_>>()
        .join("\n");
    
    // 修改系统提示词，注入记忆
    let enhanced_prompt = format!(
        "{}\n\n【相关记忆】\n{}",
        ctx.system_prompt(),
        memory_text
    );
    
    packet.set_system_message(&enhanced_prompt);
    
    Ok(())
}
```

### 5.5 跨轮次状态访问

```rust
async fn process(
    &self,
    packet: &mut ConversationPacket,
    ctx: &ProcessorContext,
) -> Result<(), ProcessorError> {
    // 获取上一轮的状态
    if let Some(prev_state) = packet.get_previous_state(self.name()) {
        let prev_count = prev_state.get("count")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        
        // 基于上一轮状态进行处理
        let new_count = prev_count + 1;
        
        packet.set_processor_state(self.name(), json!({
            "count": new_count
        }));
    } else {
        // 首次执行
        packet.set_processor_state(self.name(), json!({
            "count": 1
        }));
    }
    
    Ok(())
}
```

### 5.6 添加思考条目

```rust
async fn process(
    &self,
    packet: &mut ConversationPacket,
    ctx: &ProcessorContext,
) -> Result<(), ProcessorError> {
    // 分析用户输入
    let analysis = format!("用户询问了关于 {} 的问题", extract_topic(&packet.user_input));
    
    // 添加到思考池
    packet.add_thinking(
        analysis,
        ThinkingSource::UserAnalysis,
    );
    
    Ok(())
}
```

---

## 六、调试与测试

### 6.1 日志输出

```rust
use tracing::{debug, info, warn, error};

async fn process(&self, packet: &mut ConversationPacket, ctx: &ProcessorContext) -> Result<(), ProcessorError> {
    info!("处理器 {} 开始执行", self.name());
    debug!("用户输入: {}", packet.user_input);
    
    // ... 处理逻辑 ...
    
    if some_condition {
        warn!("检测到异常情况: {}", detail);
    }
    
    info!("处理器 {} 执行完成", self.name());
    Ok(())
}
```

### 6.2 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_my_processor() {
        let processor = MyProcessor::new();
        
        // 创建测试数据包
        let mut packet = ConversationPacket::new(
            "test_assistant".to_string(),
            "test_topic".to_string(),
            "测试用户".to_string(),
            "测试助手".to_string(),
        );
        packet.append_user_message("测试消息");
        
        // 创建模拟上下文（需要构建测试用的 ProcessorContext）
        // let ctx = create_test_context();
        
        // 执行处理器
        // let result = processor.process(&mut packet, &ctx).await;
        // assert!(result.is_ok());
        
        // 验证状态
        assert!(packet.current_states.contains_key("MyProcessor"));
    }
}
```

---

## 七、最佳实践

### 7.1 设计原则

1. **单一职责**：每个处理器只做一件事
2. **幂等性**：多次执行应产生相同结果
3. **容错性**：优雅处理错误，不影响流水线
4. **可观测性**：充分的日志和状态记录

### 7.2 性能考虑

1. **异步 I/O**：处理器内的网络请求（AI API、Qdrant）使用 async/await，避免阻塞 Tokio 运行时
2. **批量操作**：尽量使用批量 API（如 `store_batch`、`embedding_batch`），减少网络往返
3. **缓存利用**：利用 `current_states` 在处理器间传递中间结果，避免重复计算
4. **按需执行**：通过 `requires_memory()` 控制是否执行，普通话题跳过记忆处理器

### 7.3 状态管理

1. **状态命名**：使用处理器名称作为状态键
2. **状态结构**：使用 JSON 对象，便于扩展
3. **历史访问**：通过 `get_previous_state` 访问上一轮状态
4. **轮次边界**：`end_turn()` 会自动轮转状态

### 7.4 记忆操作

1. **模型一致性**：存储和检索使用相同的 embedding 模型
2. **相关性过滤**：使用 `relevance_threshold` 过滤低相关记忆
3. **衰减机制**：定期调用 `decay_short_term_memory` 清理过时记忆
4. **分类标签**：合理使用 category 和 tags 便于检索

---

## 八、现有处理器参考

| 处理器 | 职责 | 时机 | requires_memory | 状态 |
|--------|------|------|-----------------|------|
| HistorySimplifier | 简化/压缩历史对话 | on_user_message | true | 骨架 |
| **ShortTermAssembler** | 注入短期记忆摘要 | on_user_message | true | **已实现** ✅ |
| **ShortTermExpander** | 选择性展开短期记忆 | on_user_message | true | **已实现** ✅ |
| **ContextCleaner** | 清理临时注入消息 | after_ai_response | true | **已实现** ✅ |
| SubconsciousProcessor | 潜意识层面分析 | after_ai_response | true | 骨架 |
| **ContentChunker** | 内容切块 | after_ai_response | true | **已实现** ✅ |
| MemoryCommitter | 提交记忆到存储 | after_ai_response | true | 骨架 |

### ContentChunker 实现详情

**配置文件：** `processors/content_chunker/config.toml`
```toml
model = ""  # 留空使用 processor_model
prompt = "..."  # 切块提示词
```

**输出格式：** 存入短期记忆
```
【[summary]】-【[content]】-【[type]】
```

**副作用：** 清空 packet.messages（保留 system 消息）

---

## 九、文件结构

```
src/pipeline/
├── mod.rs              # 模块导出
├── packet.rs           # ConversationPacket 定义
├── processor.rs        # Processor trait 定义
├── context.rs          # ProcessorContext 定义
├── config.rs           # PipelineConfig 定义
├── dispatcher.rs       # PipelineDispatcher 调度器
├── storage.rs          # PacketStorage 持久化
├── processors/         # 处理器实现
│   ├── mod.rs          # 处理器模块导出
│   ├── history_simplifier/
│   ├── short_term_assembler/   # 短期记忆组装器
│   ├── short_term_expander/    # 短期记忆展开器 ⭐
│   ├── context_cleaner/        # 上下文清理器
│   ├── subconscious_processor/
│   ├── content_chunker/
│   └── memory_committer/
└── PROCESSOR_DEV_GUIDE.md  # 本文档
```

---

## 十、快速检查清单

开发新处理器时，确保：

- [ ] 实现 `Processor` trait 的所有方法
- [ ] `name()` 返回唯一的静态字符串
- [ ] `requires_memory()` 根据实际需求返回正确值
- [ ] 在 `processors/mod.rs` 中导出并注册
- [ ] 在 `config.rs` 或助手配置中添加到合适的时机
- [ ] 添加适当的日志输出
- [ ] 处理所有可能的错误情况
- [ ] 编写单元测试配置中添加到合适的时机
- [ ] 添加适当的日志输出
- [ ] 处理所有可能的错误情况
- [ ] 编写单元测试 [ ] 添加适当的日志输出
- [ ] 处理所有可能的错误情况
- [ ] 编写单元测试配置中添加到合适的时机
- [ ] 添加适当的日志输出
- [ ] 处理所有可能的错误情况
- [ ] 编写单元测试