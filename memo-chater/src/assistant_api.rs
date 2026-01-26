//! 助手管理 API
//! 
//! 提供助手和话题的 CRUD 接口

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::assistant::{
    AssistantConfig, AssistantId, AssistantManager, AssistantSummary,
    TopicId, TopicMeta, TopicSummary, TopicType, ModelConfig, AssistantRolesConfig, MemoryConfig,
};
use crate::pipeline::PipelineConfig;
use crate::pipeline::processors::short_term_vectorizer::{ShortTermVectorFile, VectorizedMemory};
use crate::ai::AiClient;

/// API 状态
pub struct AssistantApiState {
    pub manager: Arc<AssistantManager>,
    pub ai_client: Arc<AiClient>,
}

/// 创建助手路由
pub fn assistant_routes(state: Arc<AssistantApiState>) -> Router {
    Router::new()
        // 助手管理
        .route("/assistants", get(list_assistants).post(create_assistant))
        .route("/assistants/:id", get(get_assistant).put(update_assistant).delete(delete_assistant))
        // 话题管理
        .route("/assistants/:assistant_id/topics", get(list_topics).post(create_topic))
        .route("/assistants/:assistant_id/topics/:topic_id", get(get_topic).put(update_topic).delete(delete_topic))
        // 对话历史
        .route("/assistants/:assistant_id/topics/:topic_id/history", get(get_history).delete(clear_history))
        // 消息操作
        .route("/assistants/:assistant_id/topics/:topic_id/messages/:index", put(update_message).delete(delete_message))
        .route("/assistants/:assistant_id/topics/:topic_id/branch", post(create_branch_topic))
        // 对话记忆库
        .route("/assistants/:assistant_id/topics/:topic_id/conversation-memory", get(list_conversation_memory))
        .route("/assistants/:assistant_id/topics/:topic_id/conversation-memory/search", post(search_conversation_memory))
        .route("/assistants/:assistant_id/topics/:topic_id/conversation-memory/rebuild", post(rebuild_conversation_memory))
        .route("/assistants/:assistant_id/topics/:topic_id/conversation-memory/:memory_id", put(update_conversation_memory).delete(delete_conversation_memory))
        .with_state(state)
}

// ==================== 请求/响应结构 ====================

#[derive(Debug, Deserialize)]
pub struct CreateAssistantRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAssistantRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub model: Option<ModelConfig>,
    #[serde(default)]
    pub roles: Option<AssistantRolesConfig>,
    #[serde(default)]
    pub memory: Option<MemoryConfig>,
    #[serde(default)]
    pub pipeline: Option<PipelineConfig>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTopicRequest {
    pub title: String,
    /// 话题类型，默认为普通话题
    #[serde(default)]
    pub topic_type: TopicType,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTopicRequest {
    pub title: String,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }
    
    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

// ==================== 助手 API ====================

/// 列出所有助手
async fn list_assistants(
    State(state): State<Arc<AssistantApiState>>,
) -> Result<Json<ApiResponse<Vec<AssistantSummary>>>, StatusCode> {
    match state.manager.list_assistants().await {
        Ok(assistants) => Ok(Json(ApiResponse::ok(assistants))),
        Err(e) => {
            tracing::error!("列出助手失败: {}", e);
            Ok(Json(ApiResponse::err(e.to_string())))
        }
    }
}

/// 创建助手
async fn create_assistant(
    State(state): State<Arc<AssistantApiState>>,
    Json(req): Json<CreateAssistantRequest>,
) -> Result<Json<ApiResponse<AssistantId>>, StatusCode> {
    match state.manager.create_assistant(req.name, req.description).await {
        Ok(id) => Ok(Json(ApiResponse::ok(id))),
        Err(e) => {
            tracing::error!("创建助手失败: {}", e);
            Ok(Json(ApiResponse::err(e.to_string())))
        }
    }
}

/// 获取助手详情
async fn get_assistant(
    State(state): State<Arc<AssistantApiState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<AssistantConfig>>, StatusCode> {
    match state.manager.get_assistant(&id).await {
        Ok(config) => Ok(Json(ApiResponse::ok(config))),
        Err(e) => {
            tracing::error!("获取助手失败: {}", e);
            Ok(Json(ApiResponse::err(e.to_string())))
        }
    }
}

/// 更新助手
async fn update_assistant(
    State(state): State<Arc<AssistantApiState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateAssistantRequest>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    // 先获取现有配置
    let mut config = match state.manager.get_assistant(&id).await {
        Ok(c) => c,
        Err(e) => return Ok(Json(ApiResponse::err(e.to_string()))),
    };
    
    // 更新字段
    if let Some(name) = req.name {
        config.name = name;
    }
    if let Some(description) = req.description {
        config.description = description;
    }
    if let Some(system_prompt) = req.system_prompt {
        config.system_prompt = system_prompt;
    }
    if let Some(model) = req.model {
        config.model = model;
    }
    if let Some(roles) = req.roles {
        config.roles = roles;
    }
    if let Some(memory) = req.memory {
        config.memory = memory;
    }
    if let Some(pipeline) = req.pipeline {
        config.pipeline = pipeline;
    }
    
    match state.manager.update_assistant(&id, config).await {
        Ok(()) => Ok(Json(ApiResponse::ok(()))),
        Err(e) => {
            tracing::error!("更新助手失败: {}", e);
            Ok(Json(ApiResponse::err(e.to_string())))
        }
    }
}

/// 删除助手
async fn delete_assistant(
    State(state): State<Arc<AssistantApiState>>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    match state.manager.delete_assistant(&id).await {
        Ok(()) => Ok(Json(ApiResponse::ok(()))),
        Err(e) => {
            tracing::error!("删除助手失败: {}", e);
            Ok(Json(ApiResponse::err(e.to_string())))
        }
    }
}

// ==================== 话题 API ====================

/// 列出助手的所有话题
async fn list_topics(
    State(state): State<Arc<AssistantApiState>>,
    Path(assistant_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<TopicSummary>>>, StatusCode> {
    match state.manager.list_topics(&assistant_id).await {
        Ok(topics) => Ok(Json(ApiResponse::ok(topics))),
        Err(e) => {
            tracing::error!("列出话题失败: {}", e);
            Ok(Json(ApiResponse::err(e.to_string())))
        }
    }
}

/// 创建话题
async fn create_topic(
    State(state): State<Arc<AssistantApiState>>,
    Path(assistant_id): Path<String>,
    Json(req): Json<CreateTopicRequest>,
) -> Result<Json<ApiResponse<TopicMeta>>, StatusCode> {
    match state.manager.create_topic(&assistant_id, req.title, req.topic_type).await {
        Ok(topic_meta) => Ok(Json(ApiResponse::ok(topic_meta))),
        Err(e) => {
            tracing::error!("创建话题失败: {}", e);
            Ok(Json(ApiResponse::err(e.to_string())))
        }
    }
}

/// 获取话题详情
async fn get_topic(
    State(state): State<Arc<AssistantApiState>>,
    Path((assistant_id, topic_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<TopicMeta>>, StatusCode> {
    match state.manager.get_topic(&assistant_id, &topic_id).await {
        Ok(meta) => Ok(Json(ApiResponse::ok(meta))),
        Err(e) => {
            tracing::error!("获取话题失败: {}", e);
            Ok(Json(ApiResponse::err(e.to_string())))
        }
    }
}

/// 更新话题
async fn update_topic(
    State(state): State<Arc<AssistantApiState>>,
    Path((assistant_id, topic_id)): Path<(String, String)>,
    Json(req): Json<UpdateTopicRequest>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    match state.manager.update_topic_title(&assistant_id, &topic_id, req.title).await {
        Ok(()) => Ok(Json(ApiResponse::ok(()))),
        Err(e) => {
            tracing::error!("更新话题失败: {}", e);
            Ok(Json(ApiResponse::err(e.to_string())))
        }
    }
}

/// 删除话题
async fn delete_topic(
    State(state): State<Arc<AssistantApiState>>,
    Path((assistant_id, topic_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    match state.manager.delete_topic(&assistant_id, &topic_id).await {
        Ok(()) => Ok(Json(ApiResponse::ok(()))),
        Err(e) => {
            tracing::error!("删除话题失败: {}", e);
            Ok(Json(ApiResponse::err(e.to_string())))
        }
    }
}

// ==================== 对话历史 API ====================

/// 获取对话历史
async fn get_history(
    State(state): State<Arc<AssistantApiState>>,
    Path((assistant_id, topic_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Vec<crate::types::ChatMessage>>>, StatusCode> {
    match state.manager.get_history(&assistant_id, &topic_id).await {
        Ok(messages) => Ok(Json(ApiResponse::ok(messages))),
        Err(e) => {
            tracing::error!("获取历史失败: {}", e);
            Ok(Json(ApiResponse::err(e.to_string())))
        }
    }
}

/// 清空对话历史
async fn clear_history(
    State(state): State<Arc<AssistantApiState>>,
    Path((assistant_id, topic_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    match state.manager.clear_history(&assistant_id, &topic_id).await {
        Ok(()) => Ok(Json(ApiResponse::ok(()))),
        Err(e) => {
            tracing::error!("清空历史失败: {}", e);
            Ok(Json(ApiResponse::err(e.to_string())))
        }
    }
}// ============ 消息操作 API ============

/// 更新消息请求
#[derive(Debug, Deserialize)]
struct UpdateMessageRequest {
    content: String,
}

/// 创建分支话题请求
#[derive(Debug, Deserialize)]
struct CreateBranchRequest {
    /// 从哪个消息索引开始截断（包含该索引）
    from_index: usize,
    /// 新话题标题（可选，默认自动生成）
    title: Option<String>,
}

/// 更新指定索引的消息
async fn update_message(
    State(state): State<Arc<AssistantApiState>>,
    Path((assistant_id, topic_id, index)): Path<(String, String, usize)>,
    Json(req): Json<UpdateMessageRequest>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    match state.manager.update_message(&assistant_id, &topic_id, index, &req.content).await {
        Ok(()) => Ok(Json(ApiResponse::ok(()))),
        Err(e) => {
            tracing::error!("更新消息失败: {}", e);
            Ok(Json(ApiResponse::err(e.to_string())))
        }
    }
}

/// 删除指定索引的消息
async fn delete_message(
    State(state): State<Arc<AssistantApiState>>,
    Path((assistant_id, topic_id, index)): Path<(String, String, usize)>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    match state.manager.delete_message(&assistant_id, &topic_id, index).await {
        Ok(()) => Ok(Json(ApiResponse::ok(()))),
        Err(e) => {
            tracing::error!("删除消息失败: {}", e);
            Ok(Json(ApiResponse::err(e.to_string())))
        }
    }
}

/// 从指定位置创建分支话题
async fn create_branch_topic(
    State(state): State<Arc<AssistantApiState>>,
    Path((assistant_id, topic_id)): Path<(String, String)>,
    Json(req): Json<CreateBranchRequest>,
) -> Result<Json<ApiResponse<TopicMeta>>, StatusCode> {
    match state.manager.create_branch_topic(&assistant_id, &topic_id, req.from_index, req.title).await {
        Ok(meta) => Ok(Json(ApiResponse::ok(meta))),
        Err(e) => {
            tracing::error!("创建分支话题失败: {}", e);
            Ok(Json(ApiResponse::err(e.to_string())))
        }
    }
}

// ==================== 对话记忆库 API ====================

/// 对话记忆库列表响应
#[derive(Debug, Serialize)]
struct ConversationMemoryListResponse {
    memories: Vec<VectorizedMemory>,
    total: usize,
    embedding_model: String,
}

/// 搜索请求
#[derive(Debug, Deserialize)]
struct SearchConversationMemoryRequest {
    query: String,
    #[serde(default = "default_top_k")]
    top_k: usize,
}

fn default_top_k() -> usize {
    10
}

/// 更新记忆请求
#[derive(Debug, Deserialize)]
struct UpdateConversationMemoryRequest {
    summary: Option<String>,
    content: Option<String>,
    memory_type: Option<String>,
    confidence: Option<f32>,
    source: Option<String>,
}

/// 搜索结果
#[derive(Debug, Serialize)]
struct SearchResult {
    memory: VectorizedMemory,
    score: f32,
}

/// 获取向量文件路径
fn get_vector_file_path(data_dir: &str, assistant_id: &str, topic_id: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(data_dir)
        .join("assistants")
        .join(assistant_id)
        .join("topics")
        .join(topic_id)
        .join("short_term_vectors.json")
}

/// 列出对话记忆库
async fn list_conversation_memory(
    State(state): State<Arc<AssistantApiState>>,
    Path((assistant_id, topic_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<ConversationMemoryListResponse>>, StatusCode> {
    let data_dir = state.manager.data_dir();
    let path = get_vector_file_path(&data_dir, &assistant_id, &topic_id);
    
    if !path.exists() {
        return Ok(Json(ApiResponse::ok(ConversationMemoryListResponse {
            memories: vec![],
            total: 0,
            embedding_model: String::new(),
        })));
    }
    
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => {
            match serde_json::from_str::<ShortTermVectorFile>(&content) {
                Ok(file) => {
                    let total = file.vectors.len();
                    Ok(Json(ApiResponse::ok(ConversationMemoryListResponse {
                        memories: file.vectors,
                        total,
                        embedding_model: file.metadata.embedding_model,
                    })))
                }
                Err(e) => {
                    tracing::error!("解析向量文件失败: {}", e);
                    Ok(Json(ApiResponse::err(format!("解析文件失败: {}", e))))
                }
            }
        }
        Err(e) => {
            tracing::error!("读取向量文件失败: {}", e);
            Ok(Json(ApiResponse::err(format!("读取文件失败: {}", e))))
        }
    }
}

/// 搜索对话记忆库
async fn search_conversation_memory(
    State(state): State<Arc<AssistantApiState>>,
    Path((assistant_id, topic_id)): Path<(String, String)>,
    Json(req): Json<SearchConversationMemoryRequest>,
) -> Result<Json<ApiResponse<Vec<SearchResult>>>, StatusCode> {
    let data_dir = state.manager.data_dir();
    let path = get_vector_file_path(&data_dir, &assistant_id, &topic_id);
    
    if !path.exists() {
        return Ok(Json(ApiResponse::ok(vec![])));
    }
    
    // 读取向量文件
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(e) => return Ok(Json(ApiResponse::err(format!("读取文件失败: {}", e)))),
    };
    
    let file: ShortTermVectorFile = match serde_json::from_str(&content) {
        Ok(f) => f,
        Err(e) => return Ok(Json(ApiResponse::err(format!("解析文件失败: {}", e)))),
    };
    
    if file.vectors.is_empty() {
        return Ok(Json(ApiResponse::ok(vec![])));
    }
    
    // 使用向量文件中记录的模型生成embedding
    let embedding_model = &file.metadata.embedding_model;
    let query_embedding = match state.ai_client.embedding_with_model(&req.query, Some(embedding_model)).await {
        Ok(e) => e,
        Err(e) => return Ok(Json(ApiResponse::err(format!("生成embedding失败: {}", e)))),
    };
    
    // 计算加权余弦相似度并排序
    // 权重: summary 0.4, content 0.6
    const SUMMARY_WEIGHT: f32 = 0.4;
    const CONTENT_WEIGHT: f32 = 0.6;
    
    let mut results: Vec<SearchResult> = file.vectors
        .into_iter()
        .map(|mem| {
            let summary_score = cosine_similarity(&query_embedding, &mem.summary_embedding);
            let content_score = cosine_similarity(&query_embedding, &mem.content_embedding);
            let score = SUMMARY_WEIGHT * summary_score + CONTENT_WEIGHT * content_score;
            SearchResult { memory: mem, score }
        })
        .collect();
    
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(req.top_k);
    
    Ok(Json(ApiResponse::ok(results)))
}

/// 更新对话记忆
async fn update_conversation_memory(
    State(state): State<Arc<AssistantApiState>>,
    Path((assistant_id, topic_id, memory_id)): Path<(String, String, String)>,
    Json(req): Json<UpdateConversationMemoryRequest>,
) -> Result<Json<ApiResponse<VectorizedMemory>>, StatusCode> {
    let data_dir = state.manager.data_dir();
    let path = get_vector_file_path(&data_dir, &assistant_id, &topic_id);
    
    if !path.exists() {
        return Ok(Json(ApiResponse::err("向量文件不存在")));
    }
    
    // 读取文件
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(e) => return Ok(Json(ApiResponse::err(format!("读取文件失败: {}", e)))),
    };
    
    let mut file: ShortTermVectorFile = match serde_json::from_str(&content) {
        Ok(f) => f,
        Err(e) => return Ok(Json(ApiResponse::err(format!("解析文件失败: {}", e)))),
    };
    
    // 查找并更新记忆
    let memory = match file.vectors.iter_mut().find(|m| m.id == memory_id) {
        Some(m) => m,
        None => return Ok(Json(ApiResponse::err("记忆不存在"))),
    };
    
    let mut need_reembed = false;
    
    if let Some(summary) = req.summary {
        memory.summary = summary;
        need_reembed = true;
    }
    if let Some(content) = req.content {
        memory.content = content;
        need_reembed = true;
    }
    if let Some(memory_type) = req.memory_type {
        memory.memory_type = memory_type;
    }
    if let Some(confidence) = req.confidence {
        memory.confidence = confidence;
    }
    if let Some(source) = req.source {
        memory.source = source;
    }
    
    // 如果summary或content变了，重新生成双向量
    if need_reembed {
        let embedding_model = &file.metadata.embedding_model;
        match state.ai_client.embedding_with_model(&memory.summary, Some(embedding_model)).await {
            Ok(e) => memory.summary_embedding = e,
            Err(e) => return Ok(Json(ApiResponse::err(format!("重新生成summary embedding失败: {}", e)))),
        }
        match state.ai_client.embedding_with_model(&memory.content, Some(embedding_model)).await {
            Ok(e) => memory.content_embedding = e,
            Err(e) => return Ok(Json(ApiResponse::err(format!("重新生成content embedding失败: {}", e)))),
        }
    }
    
    let updated_memory = memory.clone();
    
    // 更新元数据时间
    file.metadata.last_updated = chrono::Utc::now().to_rfc3339();
    
    // 保存文件
    let json = match serde_json::to_string_pretty(&file) {
        Ok(j) => j,
        Err(e) => return Ok(Json(ApiResponse::err(format!("序列化失败: {}", e)))),
    };
    
    if let Err(e) = tokio::fs::write(&path, json).await {
        return Ok(Json(ApiResponse::err(format!("写入文件失败: {}", e))));
    }
    
    Ok(Json(ApiResponse::ok(updated_memory)))
}

/// 删除对话记忆
async fn delete_conversation_memory(
    State(state): State<Arc<AssistantApiState>>,
    Path((assistant_id, topic_id, memory_id)): Path<(String, String, String)>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let data_dir = state.manager.data_dir();
    let path = get_vector_file_path(&data_dir, &assistant_id, &topic_id);
    
    if !path.exists() {
        return Ok(Json(ApiResponse::err("向量文件不存在")));
    }
    
    // 读取文件
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(e) => return Ok(Json(ApiResponse::err(format!("读取文件失败: {}", e)))),
    };
    
    let mut file: ShortTermVectorFile = match serde_json::from_str(&content) {
        Ok(f) => f,
        Err(e) => return Ok(Json(ApiResponse::err(format!("解析文件失败: {}", e)))),
    };
    
    // 删除记忆
    let original_len = file.vectors.len();
    file.vectors.retain(|m| m.id != memory_id);
    
    if file.vectors.len() == original_len {
        return Ok(Json(ApiResponse::err("记忆不存在")));
    }
    
    // 更新元数据时间
    file.metadata.last_updated = chrono::Utc::now().to_rfc3339();
    
    // 保存文件
    let json = match serde_json::to_string_pretty(&file) {
        Ok(j) => j,
        Err(e) => return Ok(Json(ApiResponse::err(format!("序列化失败: {}", e)))),
    };
    
    if let Err(e) = tokio::fs::write(&path, json).await {
        return Ok(Json(ApiResponse::err(format!("写入文件失败: {}", e))));
    }
    
    Ok(Json(ApiResponse::ok(())))
}

/// 计算余弦相似度
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    
    dot / (norm_a * norm_b)
}

// ==================== 重建对话向量库 ====================

/// 重建响应
#[derive(Debug, Serialize)]
struct RebuildConversationMemoryResponse {
    success: bool,
    rebuilt: usize,
    total: usize,
    embedding_model: String,
}

/// 重建对话记忆库（从 packet 的 short_term_memory 重新生成向量）
async fn rebuild_conversation_memory(
    State(state): State<Arc<AssistantApiState>>,
    Path((assistant_id, topic_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<RebuildConversationMemoryResponse>>, StatusCode> {
    // 获取 data_dir
    let data_dir = state.manager.data_dir();
    
    // 1. 读取 conversation_state.json (packet)
    let packet_path = std::path::PathBuf::from(&data_dir)
        .join("assistants")
        .join(&assistant_id)
        .join("topics")
        .join(&topic_id)
        .join("conversation_state.json");
    
    let packet_content = match tokio::fs::read_to_string(&packet_path).await {
        Ok(c) => c,
        Err(e) => {
            return Ok(Json(ApiResponse::err(format!("读取 packet 失败: {}", e))));
        }
    };
    
    let packet: crate::pipeline::ConversationPacket = match serde_json::from_str(&packet_content) {
        Ok(p) => p,
        Err(e) => {
            return Ok(Json(ApiResponse::err(format!("解析 packet 失败: {}", e))));
        }
    };
    
    let short_term_memories = packet.short_term_memory;
    let total = short_term_memories.len();
    
    if total == 0 {
        return Ok(Json(ApiResponse::ok(RebuildConversationMemoryResponse {
            success: true,
            rebuilt: 0,
            total: 0,
            embedding_model: "none".to_string(),
        })));
    }
    
    // 2. 获取 embedding 模型
    let embedding_model = packet.embedding_model.unwrap_or_else(|| "text-embedding-3-small".to_string());
    
    // 3. 为每条记忆生成 embedding
    let mut vectorized_memories = Vec::new();
    let mut rebuilt_count = 0;
    
    for memory in &short_term_memories {
        let source_str = match &memory.source {
            crate::types::MemorySource::LongTermRetrieval => "LongTermRetrieval",
            crate::types::MemorySource::CurrentConversation => "CurrentConversation",
            crate::types::MemorySource::ToolResult => "ToolResult",
        };
        
        // 生成 summary embedding
        let summary_embedding = match state.ai_client.embedding_with_model(&memory.summary, Some(&embedding_model)).await {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("生成 summary embedding 失败 (id={}): {}", memory.id, e);
                continue;
            }
        };
        
        // 生成 content embedding
        let content_embedding = match state.ai_client.embedding_with_model(&memory.content, Some(&embedding_model)).await {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("生成 content embedding 失败 (id={}): {}", memory.id, e);
                continue;
            }
        };
        
        vectorized_memories.push(VectorizedMemory {
            id: memory.id.clone(),
            summary: memory.summary.clone(),
            content: memory.content.clone(),
            memory_type: memory.memory_type.clone(),
            source: source_str.to_string(),
            timestamp: memory.timestamp.to_rfc3339(),
            should_expand: memory.should_expand,
            confidence: 1.0, // 新记忆默认置信度 1.0
            summary_embedding,
            content_embedding,
        });
        rebuilt_count += 1;
    }
    
    // 4. 构建新的向量文件
    let dimension = vectorized_memories.first().map(|v| v.summary_embedding.len()).unwrap_or(0);
    let vector_file = ShortTermVectorFile {
        vectors: vectorized_memories,
        metadata: crate::pipeline::processors::short_term_vectorizer::VectorFileMetadata {
            embedding_model: embedding_model.clone(),
            dimension,
            last_updated: chrono::Utc::now().to_rfc3339(),
        },
    };
    
    // 5. 写入向量文件
    let vector_path = get_vector_file_path(&data_dir, &assistant_id, &topic_id);
    
    let json = match serde_json::to_string_pretty(&vector_file) {
        Ok(j) => j,
        Err(e) => {
            return Ok(Json(ApiResponse::err(format!("序列化失败: {}", e))));
        }
    };
    
    if let Err(e) = tokio::fs::write(&vector_path, json).await {
        return Ok(Json(ApiResponse::err(format!("写入向量文件失败: {}", e))));
    }
    
    tracing::info!(
        "重建对话向量库完成: assistant={}, topic={}, rebuilt={}/{}",
        assistant_id, topic_id, rebuilt_count, total
    );
    
    Ok(Json(ApiResponse::ok(RebuildConversationMemoryResponse {
        success: true,
        rebuilt: rebuilt_count,
        total,
        embedding_model,
    })))
}