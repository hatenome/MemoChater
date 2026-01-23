//! 记忆管理API

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::state::AppState;
use crate::types::{LongTermMemory, ChatMessage};
use crate::extractor::{MemoryExtractor, ExtractorConfig};

/// 记忆列表查询参数
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub query: Option<String>,
    pub category: Option<String>,
    pub limit: Option<usize>,
}

/// 创建记忆请求
#[derive(Debug, Deserialize)]
pub struct CreateMemoryRequest {
    pub content: String,
    pub category: String,
    pub importance: f32,
    pub tags: Option<Vec<String>>,
}

/// 更新记忆请求
#[derive(Debug, Deserialize)]
pub struct UpdateMemoryRequest {
    pub content: Option<String>,
    pub category: Option<String>,
    pub importance: Option<f32>,
    pub tags: Option<Vec<String>>,
}

/// 记忆响应
#[derive(Debug, Serialize)]
pub struct MemoryResponse {
    pub id: String,
    pub content: String,
    pub category: String,
    pub importance: f32,
    pub access_count: u32,
    pub tags: Vec<String>,
    pub created_at: String,
    pub last_accessed: String,
}

impl From<LongTermMemory> for MemoryResponse {
    fn from(m: LongTermMemory) -> Self {
        Self {
            id: m.id,
            content: m.content,
            category: m.category,
            importance: m.importance,
            access_count: m.access_count,
            tags: m.tags,
            created_at: m.created_at.to_rfc3339(),
            last_accessed: m.last_accessed.to_rfc3339(),
        }
    }
}

/// 搜索结果响应
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub memories: Vec<MemoryWithScore>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct MemoryWithScore {
    pub memory: MemoryResponse,
    pub score: f32,
}

/// API响应包装
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Json<Self> {
        Json(Self {
            success: true,
            data: Some(data),
            error: None,
        })
    }

    pub fn err(msg: impl Into<String>) -> Json<Self> {
        Json(Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        })
    }
}

/// 获取管理页面HTML
pub async fn admin_page() -> Html<&'static str> {
    Html(include_str!("admin.html"))
}

/// 列出/搜索记忆
pub async fn list_memories(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    let manager = state.memory_manager.read().await;
    let store = manager.long_term_store();
    
    let limit = query.limit.unwrap_or(50);
    
    // 如果有搜索词，使用语义搜索
    if let Some(ref q) = query.query {
        if !q.trim().is_empty() {
            match store.search_with_filter(q, limit, query.category.as_deref(), None).await {
                Ok(results) => {
                    let memories: Vec<MemoryWithScore> = results
                        .into_iter()
                        .map(|r| MemoryWithScore {
                            memory: r.memory.into(),
                            score: r.relevance,
                        })
                        .collect();
                    let total = memories.len();
                    return ApiResponse::ok(SearchResponse { memories, total });
                }
                Err(e) => {
                    return ApiResponse::<SearchResponse>::err(format!("搜索失败: {}", e));
                }
            }
        }
    }
    
    // 没有搜索词，返回最近的记忆（通过空查询搜索）
    match store.search("", limit).await {
        Ok(results) => {
            let memories: Vec<MemoryWithScore> = results
                .into_iter()
                .map(|r| MemoryWithScore {
                    memory: r.memory.into(),
                    score: r.relevance,
                })
                .collect();
            let total = memories.len();
            ApiResponse::ok(SearchResponse { memories, total })
        }
        Err(e) => ApiResponse::<SearchResponse>::err(format!("获取记忆失败: {}", e)),
    }
}

/// 获取单条记忆
pub async fn get_memory(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let manager = state.memory_manager.read().await;
    let store = manager.long_term_store();
    
    match store.get(&id).await {
        Ok(Some(memory)) => ApiResponse::ok(MemoryResponse::from(memory)),
        Ok(None) => ApiResponse::<MemoryResponse>::err("记忆不存在"),
        Err(e) => ApiResponse::<MemoryResponse>::err(format!("获取失败: {}", e)),
    }
}

/// 创建记忆
pub async fn create_memory(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateMemoryRequest>,
) -> impl IntoResponse {
    let manager = state.memory_manager.read().await;
    let store = manager.long_term_store();
    
    let mut memory = LongTermMemory::new(req.content, req.category, req.importance);
    if let Some(tags) = req.tags {
        memory.tags = tags;
    }
    
    match store.store(&memory).await {
        Ok(()) => ApiResponse::ok(MemoryResponse::from(memory)),
        Err(e) => ApiResponse::<MemoryResponse>::err(format!("创建失败: {}", e)),
    }
}

/// 删除记忆
pub async fn delete_memory(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let manager = state.memory_manager.read().await;
    let store = manager.long_term_store();
    
    match store.delete(&id).await {
        Ok(()) => ApiResponse::ok("删除成功"),
        Err(e) => ApiResponse::<&str>::err(format!("删除失败: {}", e)),
    }
}

/// 获取统计信息
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let manager = state.memory_manager.read().await;
    let store = manager.long_term_store();
    
    match store.stats().await {
        Ok(stats) => ApiResponse::ok(stats),
        Err(e) => ApiResponse::<crate::memory::StoreStats>::err(format!("获取统计失败: {}", e)),
    }
}

/// 记忆转化请求
#[derive(Debug, Deserialize)]
pub struct ExtractionRequest {
    /// 要提取记忆的对话历史
    pub messages: Vec<ExtractionMessage>,
    /// 是否自动存储提取的记忆（默认false，仅预览）
    #[serde(default)]
    pub auto_store: bool,
}

#[derive(Debug, Deserialize)]
pub struct ExtractionMessage {
    pub role: String,
    pub content: String,
}

/// 记忆转化结果
#[derive(Debug, Serialize)]
pub struct ExtractionResponse {
    /// 是否成功
    pub success: bool,
    /// 提取到的记忆列表
    pub extracted_memories: Vec<String>,
    /// 原始AI响应
    pub raw_response: String,
    /// 已存储的记忆数量（仅当auto_store=true时有值）
    pub stored_count: Option<usize>,
}

/// 触发记忆转化
/// 
/// POST /admin/api/extract
/// 
/// 从对话历史中提取有价值的记忆信息
pub async fn trigger_extraction(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExtractionRequest>,
) -> impl IntoResponse {
    tracing::info!("收到记忆转化请求，消息数: {}, auto_store: {}", req.messages.len(), req.auto_store);
    
    // 转换消息格式
    let messages: Vec<ChatMessage> = req.messages
        .iter()
        .map(|m| ChatMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();
    
    if messages.is_empty() {
        return ApiResponse::ok(ExtractionResponse {
            success: false,
            extracted_memories: vec![],
            raw_response: "消息列表为空".to_string(),
            stored_count: None,
        });
    }
    
    // 创建提取器配置
    let config = ExtractorConfig {
        api_base: state.config.ai.api_base.clone(),
        api_key: state.config.ai.api_key.clone(),
        model: state.config.ai.extractor_model.clone(),
        custom_prompt: None,
        user_name: state.config.roles.user_name.clone(),
        assistant_name: state.config.roles.assistant_name.clone(),
    };
    
    let extractor = MemoryExtractor::new(config);
    
    // 执行提取
    match extractor.extract(&messages).await {
        Ok(result) => {
            let extracted: Vec<String> = result.memories
                .iter()
                .map(|m| m.content.clone())
                .collect();
            
            let mut stored_count = None;
            
            // 如果需要自动存储
            if req.auto_store && !extracted.is_empty() {
                let manager = state.memory_manager.read().await;
                let store = manager.long_term_store();
                
                let mut count = 0;
                for mem in &result.memories {
                    let memory = LongTermMemory::new(
                        mem.content.clone(),
                        "extracted".to_string(),  // 默认分类
                        0.7,  // 默认重要性
                    );
                    
                    if store.store(&memory).await.is_ok() {
                        count += 1;
                    }
                }
                stored_count = Some(count);
                tracing::info!("已存储 {} 条记忆", count);
            }
            
            ApiResponse::ok(ExtractionResponse {
                success: result.parse_success,
                extracted_memories: extracted,
                raw_response: result.raw_response,
                stored_count,
            })
        }
        Err(e) => {
            tracing::error!("记忆转化失败: {}", e);
            ApiResponse::ok(ExtractionResponse {
                success: false,
                extracted_memories: vec![],
                raw_response: format!("提取失败: {}", e),
                stored_count: None,
            })
        }
    }
}

// ========== 待处理池管理 ==========

/// 待处理池状态响应
#[derive(Debug, Serialize)]
pub struct PendingStatusResponse {
    /// 待处理记忆数量
    pub pending_count: usize,
    /// 待处理记忆预览（最多显示10条）
    pub preview: Vec<PendingMemoryPreview>,
}

#[derive(Debug, Serialize)]
pub struct PendingMemoryPreview {
    pub content: String,
    pub category: String,
    pub importance: f32,
    pub source_session: String,
    pub created_at: String,
}

/// 获取待处理池状态
/// 
/// GET /admin/api/pending
pub async fn get_pending_status(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let manager = state.memory_manager.read().await;
    let pending_store = manager.pending_store();
    
    let count = pending_store.len().await;
    
    // 获取预览（不取出，只查看）
    let preview: Vec<PendingMemoryPreview> = pending_store
        .peek(10)
        .await
        .into_iter()
        .map(|m| PendingMemoryPreview {
            content: {
                let chars: Vec<char> = m.content.chars().collect();
                if chars.len() > 100 {
                    format!("{}...", chars[..100].iter().collect::<String>())
                } else {
                    m.content.clone()
                }
            },
            category: m.category.clone(),
            importance: m.importance,
            source_session: m.source_session.clone(),
            created_at: m.created_at.to_rfc3339(),
        })
        .collect();
    
    ApiResponse::ok(PendingStatusResponse {
        pending_count: count,
        preview,
    })
}

/// 处理待处理池请求
#[derive(Debug, Deserialize)]
pub struct ProcessPendingRequest {
    /// 要处理的数量（默认全部）
    pub count: Option<usize>,
}

/// 处理待处理池响应
#[derive(Debug, Serialize)]
pub struct ProcessPendingResponse {
    /// 成功处理的数量
    pub processed: usize,
    /// 失败的数量
    pub failed: usize,
    /// 剩余待处理数量
    pub remaining: usize,
}

/// 处理待处理池中的记忆到长期记忆
/// 
/// POST /admin/api/pending/process
pub async fn process_pending(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ProcessPendingRequest>,
) -> impl IntoResponse {
    let mut manager = state.memory_manager.write().await;
    
    // 获取待处理数量
    let total = manager.pending_store().len().await;
    let count = req.count.unwrap_or(total);
    
    tracing::info!("开始处理待处理池，请求数量: {}, 实际数量: {}", count, total.min(count));
    
    // 取出待处理记忆
    let pending_memories = manager.pending_store_mut().take_batch(count).await;
    
    let mut processed = 0;
    let mut failed = 0;
    
    // 逐个存入长期记忆
    let store = manager.long_term_store();
    for pending in pending_memories {
        let memory = LongTermMemory::new(
            pending.content,
            pending.category,
            pending.importance,
        ).with_session(pending.source_session);
        
        match store.store(&memory).await {
            Ok(()) => {
                processed += 1;
                tracing::debug!("成功存储记忆: {}", memory.id);
            }
            Err(e) => {
                failed += 1;
                tracing::error!("存储记忆失败: {}", e);
            }
        }
    }
    
    let remaining = manager.pending_store().len().await;
    
    tracing::info!("待处理池处理完成: 成功={}, 失败={}, 剩余={}", processed, failed, remaining);
    
    ApiResponse::ok(ProcessPendingResponse {
        processed,
        failed,
        remaining,
    })
}

/// 清空待处理池
/// 
/// DELETE /admin/api/pending
pub async fn clear_pending(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let mut manager = state.memory_manager.write().await;
    
    let count = manager.pending_store().len().await;
    
    // 取出所有但不处理（相当于清空）
    let _ = manager.pending_store_mut().take_batch(count).await;
    
    tracing::info!("已清空待处理池，共 {} 条记忆", count);
    
    ApiResponse::ok(format!("已清空 {} 条待处理记忆", count))
}// ==================== 模型列表 API ====================

/// 模型信息
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub owned_by: String,
}

/// 模型列表响应
#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelInfo>,
}

/// 获取可用模型列表（代理到配置的 AI API）
pub async fn list_models(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<ModelInfo>>>, StatusCode> {
    let api_base = state.config.ai.api_base.trim_end_matches('/');
    let url = format!("{}/models", api_base);
    
    let api_key = state.config.ai.get_api_key().unwrap_or_default();
    
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await;
    
    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<ModelsResponse>().await {
                    Ok(models) => Ok(ApiResponse::ok(models.data)),
                    Err(e) => {
                        tracing::error!("解析模型列表失败: {}", e);
                        Ok(ApiResponse::err(format!("解析失败: {}", e)))
                    }
                }
            } else {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                tracing::error!("获取模型列表失败: {} - {}", status, text);
                Ok(ApiResponse::err(format!("API错误: {}", status)))
            }
        }
Err(e) => {
            tracing::error!("请求模型列表失败: {}", e);
            Ok(ApiResponse::err(format!("请求失败: {}", e)))
        }
    }
}

/// 处理器信息
#[derive(Debug, Serialize)]
pub struct ProcessorInfo {
    pub name: String,
    pub requires_memory: bool,
}

/// 列出所有已注册的处理器
pub async fn list_processors(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<Vec<ProcessorInfo>>> {
    let processor_names = state.dispatcher.list_processors();
    
    // 获取处理器详细信息
    let processors: Vec<ProcessorInfo> = processor_names
        .into_iter()
        .map(|name| {
            // 从 dispatcher 获取处理器的 requires_memory 属性
            let requires_memory = state.dispatcher
                .get_processor(name)
                .map(|p| p.requires_memory())
                .unwrap_or(true);
            
            ProcessorInfo {
                name: name.to_string(),
                requires_memory,
            }
        })
        .collect();
    
    ApiResponse::ok(processors)
}

// ==================== 按助手隔离的记忆API ====================

/// 助手记忆路径参数
#[derive(Debug, Deserialize)]
pub struct AssistantMemoryPath {
    pub assistant_id: String,
}

/// 助手记忆详情路径参数
#[derive(Debug, Deserialize)]
pub struct AssistantMemoryDetailPath {
    pub assistant_id: String,
    pub memory_id: String,
}

/// 列出助手的记忆
pub async fn list_assistant_memories(
    State(state): State<Arc<AppState>>,
    Path(path): Path<AssistantMemoryPath>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    // 先获取助手配置中的 embedding_model
    let embedding_model = match state.assistant_manager.get_assistant(&path.assistant_id).await {
        Ok(config) => Some(config.model.embedding_model.clone()),
        Err(_) => None,
    };
    
    let mut manager = state.memory_manager.write().await;
    
    // 获取助手的记忆存储（传递 embedding_model）
    let store = match manager.get_assistant_long_term_with_embedding(
        &path.assistant_id,
        embedding_model.as_deref(),
    ).await {
        Ok(store) => store,
        Err(e) => {
            return ApiResponse::<SearchResponse>::err(format!("获取助手记忆存储失败: {}", e));
        }
    };
    
    let limit = query.limit.unwrap_or(50);
    
    // 如果有搜索词，使用语义搜索
    if let Some(ref q) = query.query {
        if !q.trim().is_empty() {
            match store.search_with_filter(q, limit, query.category.as_deref(), None).await {
                Ok(results) => {
                    let memories: Vec<MemoryWithScore> = results
                        .into_iter()
                        .map(|r| MemoryWithScore {
                            memory: r.memory.into(),
                            score: r.relevance,
                        })
                        .collect();
                    let total = memories.len();
                    return ApiResponse::ok(SearchResponse { memories, total });
                }
                Err(e) => {
                    return ApiResponse::<SearchResponse>::err(format!("搜索失败: {}", e));
                }
            }
        }
    }
    
    // 没有搜索词，列出所有记忆
    match store.list_all(limit).await {
        Ok(results) => {
            let memories: Vec<MemoryWithScore> = results
                .into_iter()
                .map(|r| MemoryWithScore {
                    memory: r.memory.into(),
                    score: r.relevance,
                })
                .collect();
            let total = memories.len();
            ApiResponse::ok(SearchResponse { memories, total })
        }
        Err(e) => ApiResponse::<SearchResponse>::err(format!("获取记忆失败: {}", e)),
    }
}

/// 获取助手的单条记忆
pub async fn get_assistant_memory(
    State(state): State<Arc<AppState>>,
    Path(path): Path<AssistantMemoryDetailPath>,
) -> impl IntoResponse {
    // 先获取助手配置中的 embedding_model
    let embedding_model = match state.assistant_manager.get_assistant(&path.assistant_id).await {
        Ok(config) => Some(config.model.embedding_model.clone()),
        Err(_) => None,
    };
    
    let mut manager = state.memory_manager.write().await;
    
    let store = match manager.get_assistant_long_term_with_embedding(
        &path.assistant_id,
        embedding_model.as_deref(),
    ).await {
        Ok(store) => store,
        Err(e) => {
            return ApiResponse::<MemoryResponse>::err(format!("获取助手记忆存储失败: {}", e));
        }
    };
    
    match store.get(&path.memory_id).await {
        Ok(Some(memory)) => ApiResponse::ok(MemoryResponse::from(memory)),
        Ok(None) => ApiResponse::<MemoryResponse>::err("记忆不存在"),
        Err(e) => ApiResponse::<MemoryResponse>::err(format!("获取失败: {}", e)),
    }
}

/// 创建助手的记忆
pub async fn create_assistant_memory(
    State(state): State<Arc<AppState>>,
    Path(path): Path<AssistantMemoryPath>,
    Json(req): Json<CreateMemoryRequest>,
) -> impl IntoResponse {
    // 先获取助手配置中的 embedding_model
    let embedding_model = match state.assistant_manager.get_assistant(&path.assistant_id).await {
        Ok(config) => Some(config.model.embedding_model.clone()),
        Err(_) => None,
    };
    
    let mut manager = state.memory_manager.write().await;
    
    let store = match manager.get_assistant_long_term_with_embedding(
        &path.assistant_id,
        embedding_model.as_deref(),
    ).await {
        Ok(store) => store,
        Err(e) => {
            return ApiResponse::<MemoryResponse>::err(format!("获取助手记忆存储失败: {}", e));
        }
    };
    
    let mut memory = LongTermMemory::new(req.content, req.category, req.importance);
    if let Some(tags) = req.tags {
        memory.tags = tags;
    }
    
    match store.store(&memory).await {
        Ok(()) => ApiResponse::ok(MemoryResponse::from(memory)),
        Err(e) => ApiResponse::<MemoryResponse>::err(format!("创建失败: {}", e)),
    }
}

/// 删除助手的记忆
pub async fn delete_assistant_memory(
    State(state): State<Arc<AppState>>,
    Path(path): Path<AssistantMemoryDetailPath>,
) -> impl IntoResponse {
    // 先获取助手配置中的 embedding_model
    let embedding_model = match state.assistant_manager.get_assistant(&path.assistant_id).await {
        Ok(config) => Some(config.model.embedding_model.clone()),
        Err(_) => None,
    };
    
    let mut manager = state.memory_manager.write().await;
    
    let store = match manager.get_assistant_long_term_with_embedding(
        &path.assistant_id,
        embedding_model.as_deref(),
    ).await {
        Ok(store) => store,
        Err(e) => {
            return ApiResponse::<&str>::err(format!("获取助手记忆存储失败: {}", e));
        }
    };
    
    match store.delete(&path.memory_id).await {
        Ok(()) => ApiResponse::ok("删除成功"),
        Err(e) => ApiResponse::<&str>::err(format!("删除失败: {}", e)),
    }
}

/// 获取助手的待处理池状态
pub async fn get_assistant_pending_status(
    State(state): State<Arc<AppState>>,
    Path(path): Path<AssistantMemoryPath>,
) -> impl IntoResponse {
    let mut manager = state.memory_manager.write().await;
    
    // 获取或创建助手记忆
    let assistant = match manager.get_or_create_assistant(&path.assistant_id).await {
        Ok(a) => a,
        Err(e) => {
            return ApiResponse::<PendingStatusResponse>::err(format!("获取助手失败: {}", e));
        }
    };
    
    let pending_count = assistant.pending.len().await;
    let preview_items = assistant.pending.peek(10).await;
    let preview: Vec<PendingMemoryPreview> = preview_items
        .into_iter()
        .map(|m| PendingMemoryPreview {
            content: m.content.clone(),
            category: m.category.clone(),
            importance: m.importance,
            source_session: m.source_session.clone(),
            created_at: m.created_at.to_rfc3339(),
        })
        .collect();
    
    ApiResponse::ok(PendingStatusResponse {
        pending_count,
        preview,
    })
}

/// 处理助手的待处理池
pub async fn process_assistant_pending(
    State(state): State<Arc<AppState>>,
    Path(path): Path<AssistantMemoryPath>,
) -> impl IntoResponse {
    // 先获取助手配置中的 embedding_model
    let embedding_model = match state.assistant_manager.get_assistant(&path.assistant_id).await {
        Ok(config) => Some(config.model.embedding_model.clone()),
        Err(_) => None,
    };
    
    let mut manager = state.memory_manager.write().await;
    
    // 获取助手的长期记忆存储（传递 embedding_model）
    let store = match manager.get_assistant_long_term_with_embedding(
        &path.assistant_id,
        embedding_model.as_deref(),
    ).await {
        Ok(store) => store,
        Err(e) => {
            return ApiResponse::<ProcessPendingResponse>::err(format!("获取助手记忆存储失败: {}", e));
        }
    };
    
    // 获取助手记忆
    let assistant = match manager.get_or_create_assistant(&path.assistant_id).await {
        Ok(a) => a,
        Err(e) => {
            return ApiResponse::<ProcessPendingResponse>::err(format!("获取助手失败: {}", e));
        }
    };
    
    // 取出所有待处理记忆
    let total = assistant.pending.len().await;
    let pending_memories = assistant.pending.take_batch(total).await;
    let mut processed = 0;
    let mut failed = 0;
    
    for pending in pending_memories {
        let memory = LongTermMemory::new(
            pending.content,
            pending.category,
            pending.importance,
        );
        
        match store.store(&memory).await {
            Ok(()) => processed += 1,
            Err(e) => {
                tracing::error!("存储记忆失败: {}", e);
                failed += 1;
            }
        }
    }
    
    ApiResponse::ok(ProcessPendingResponse {
        processed,
        failed,
        remaining: 0,
    })
}

/// 清空助手的待处理池
pub async fn clear_assistant_pending(
    State(state): State<Arc<AppState>>,
    Path(path): Path<AssistantMemoryPath>,
) -> impl IntoResponse {
    let mut manager = state.memory_manager.write().await;
    
    let assistant = match manager.get_or_create_assistant(&path.assistant_id).await {
        Ok(a) => a,
        Err(e) => {
            return ApiResponse::<String>::err(format!("获取助手失败: {}", e));
        }
    };
    
    // 取出所有记忆来清空队列
    let count = assistant.pending.len().await;
    let _ = assistant.pending.take_batch(count).await;
    ApiResponse::ok("已清空待处理池".to_string())
}// ==================== 设置 API ====================

/// 设置响应
#[derive(Debug, Serialize)]
pub struct SettingsResponse {
    pub disable_gemini_thinking: bool,
}

/// 更新设置请求
#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    pub disable_gemini_thinking: Option<bool>,
}

/// 获取设置
pub async fn get_settings(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let settings = SettingsResponse {
        disable_gemini_thinking: state.config.ai.disable_gemini_thinking,
    };
    ApiResponse::ok(settings)
}

/// 更新设置
pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdateSettingsRequest>,
) -> impl IntoResponse {
    // 注意：这里只是返回当前配置，实际修改需要重启服务
    // 因为 AppState 中的 config 是不可变的
    // 如果需要运行时修改，需要使用 RwLock 包装 config
    
    let mut response = SettingsResponse {
        disable_gemini_thinking: state.config.ai.disable_gemini_thinking,
    };
    
    if let Some(disable) = req.disable_gemini_thinking {
        response.disable_gemini_thinking = disable;
        // TODO: 实际保存到配置文件并重新加载
        tracing::info!("设置 disable_gemini_thinking = {}", disable);
    }
    
    ApiResponse::ok(response)
}