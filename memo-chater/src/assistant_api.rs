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

/// API 状态
pub struct AssistantApiState {
    pub manager: Arc<AssistantManager>,
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