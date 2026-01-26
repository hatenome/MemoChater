//! OpenAI 兼容 API
//!
//! 使用新的可配置流水线架构

use axum::{
    extract::State,
    response::{sse::Event, Sse, IntoResponse},
    Json,
};
use futures::stream::Stream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc};

use axum::extract::Path;
use crate::state::AppState;
use crate::pipeline::{ConversationPacket, PipelineConfig, PipelineTiming, ThinkingEntry};
use crate::ai::AiClient;
use crate::types::{ChatMessage, ShortTermMemory, ThinkingSource, MemorySource};

/// OpenAI 兼容的请求格式（扩展支持助手隔离）
#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// 助手ID（用于记忆隔离）
    #[serde(default)]
    pub assistant_id: Option<String>,
    /// 话题ID（用于会话隔离）
    #[serde(default)]
    pub topic_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// OpenAI 兼容的响应格式
#[derive(Debug, Serialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Debug, Serialize)]
pub struct Choice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: String,
}

#[derive(Debug, Serialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// 流式响应的 chunk
#[derive(Debug, Serialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
}

#[derive(Debug, Serialize)]
pub struct ChunkChoice {
    pub index: u32,
    pub delta: Delta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// 模型列表响应
#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

#[derive(Debug, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

/// 列出可用模型
pub async fn list_models(State(state): State<Arc<AppState>>) -> Json<ModelsResponse> {
    Json(ModelsResponse {
        object: "list".to_string(),
        data: vec![ModelInfo {
            id: state.config.ai.main_model.clone(),
            object: "model".to_string(),
            created: chrono::Utc::now().timestamp(),
            owned_by: "memo-chater".to_string(),
        }],
    })
}

/// 聊天补全接口 - 通过新流水线处理
pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ChatCompletionRequest>,
) -> axum::response::Response {
    tracing::info!("收到请求: model={}, stream={}, assistant_id={:?}, topic_id={:?}", 
        request.model, request.stream, request.assistant_id, request.topic_id);

    // 检查必要参数
    let assistant_id = match &request.assistant_id {
        Some(id) => id.clone(),
        None => {
            tracing::warn!("请求缺少 assistant_id，使用默认值");
            "default".to_string()
        }
    };
    
    let topic_id = match &request.topic_id {
        Some(id) => id.clone(),
        None => {
            tracing::warn!("请求缺少 topic_id，使用默认值");
            "default".to_string()
        }
    };

    if request.stream {
        create_stream_response(state, request, assistant_id, topic_id).await.into_response()
    } else {
        create_response(state, request, assistant_id, topic_id).await.into_response()
    }
}

/// 创建非流式响应
async fn create_response(
    state: Arc<AppState>,
    request: ChatCompletionRequest,
    assistant_id: String,
    topic_id: String,
) -> Json<ChatCompletionResponse> {
    // 获取或创建数据包
    let mut packet = match get_or_create_packet(&state, &assistant_id, &topic_id).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("创建数据包失败: {}", e);
            return error_response(&format!("创建数据包失败: {}", e), &request.model);
        }
    };

    // 提取用户消息
    let user_message = extract_user_message(&request.messages);
    packet.append_user_message(&user_message);

    // 创建处理器上下文
    let ctx = match state.dispatcher.context_factory()
        .create(&assistant_id, &topic_id).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("创建上下文失败: {}", e);
            return error_response(&format!("创建上下文失败: {}", e), &request.model);
        }
    };

    // 获取流水线配置（从助手配置读取）
    let pipeline_config = ctx.assistant_config.pipeline.clone();

    // 执行 on_user_message 处理器
    if let Err(e) = state.dispatcher.dispatch(
        PipelineTiming::OnUserMessage,
        &mut packet,
        &pipeline_config,
        &ctx,
    ).await {
        tracing::warn!("on_user_message 处理器执行失败: {}", e);
    }

    // 执行 before_ai_call 处理器
    if let Err(e) = state.dispatcher.dispatch(
        PipelineTiming::BeforeAiCall,
        &mut packet,
        &pipeline_config,
        &ctx,
    ).await {
        tracing::warn!("before_ai_call 处理器执行失败: {}", e);
    }

    // 调用 AI（使用 AiClient）
    let model = packet.main_model.clone().unwrap_or_else(|| request.model.clone());
    let ai_response = match ctx.ai_client.chat_with_model(&packet.messages, Some(&model)).await {
        Ok(response) => response,
        Err(e) => {
            tracing::error!("AI 调用失败: {}", e);
            return error_response(&format!("AI 调用失败: {}", e), &model);
        }
    };

    // 追加 AI 响应
    packet.append_assistant_message(&ai_response);

    // 执行 after_ai_response 处理器
    if let Err(e) = state.dispatcher.dispatch(
        PipelineTiming::AfterAiResponse,
        &mut packet,
        &pipeline_config,
        &ctx,
    ).await {
        tracing::warn!("after_ai_response 处理器执行失败: {}", e);
    }

    // 轮次结束处理
    packet.end_turn();

    // 持久化数据包
    if let Err(e) = state.packet_storage.save(&packet).await {
        tracing::error!("保存数据包失败: {}", e);
    }

    // 持久化消息到话题历史
    let messages = vec![
        ChatMessage::user(&user_message),
        ChatMessage::assistant(&ai_response),
    ];
    if let Err(e) = state.assistant_manager.append_messages(&assistant_id, &topic_id, messages).await {
        tracing::error!("消息持久化失败: {}", e);
    }

    Json(ChatCompletionResponse {
        id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        object: "chat.completion".to_string(),
        created: chrono::Utc::now().timestamp(),
        model,
        choices: vec![Choice {
            index: 0,
            message: Message {
                role: "assistant".to_string(),
                content: ai_response,
            },
            finish_reason: "stop".to_string(),
        }],
        usage: Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        },
    })
}

/// 创建流式响应
async fn create_stream_response(
    state: Arc<AppState>,
    request: ChatCompletionRequest,
    assistant_id: String,
    topic_id: String,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = async_stream::stream! {
        let id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
        let created = chrono::Utc::now().timestamp();

        // 获取或创建数据包
        let mut packet = match get_or_create_packet(&state, &assistant_id, &topic_id).await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("创建数据包失败: {}", e);
                yield Ok(Event::default().data(format!(r#"{{"error":"{}"}}"#, e)));
                yield Ok(Event::default().data("[DONE]"));
                return;
            }
        };

        // 提取用户消息
        let user_message = extract_user_message(&request.messages);
        packet.append_user_message(&user_message);

        // 创建处理器上下文
        let ctx = match state.dispatcher.context_factory()
            .create(&assistant_id, &topic_id).await {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("创建上下文失败: {}", e);
                yield Ok(Event::default().data(format!(r#"{{"error":"{}"}}"#, e)));
                yield Ok(Event::default().data("[DONE]"));
                return;
            }
        };

        // 获取流水线配置（从助手配置读取）
        let pipeline_config = ctx.assistant_config.pipeline.clone();

        // 执行 on_user_message 处理器
        if let Err(e) = state.dispatcher.dispatch(
            PipelineTiming::OnUserMessage,
            &mut packet,
            &pipeline_config,
            &ctx,
        ).await {
            tracing::warn!("on_user_message 处理器执行失败: {}", e);
        }

        // 执行 before_ai_call 处理器
        if let Err(e) = state.dispatcher.dispatch(
            PipelineTiming::BeforeAiCall,
            &mut packet,
            &pipeline_config,
            &ctx,
        ).await {
            tracing::warn!("before_ai_call 处理器执行失败: {}", e);
        }

        let model = packet.main_model.clone().unwrap_or_else(|| request.model.clone());

        // 保存最终发送给 AI 的 messages（供前端调试查看）
        packet.last_request_messages = packet.messages.clone();

        // 保存 packet（让前端可以立即获取 last_request_messages）
        if let Err(e) = state.packet_storage.save(&packet).await {
            tracing::warn!("保存 packet 失败: {}", e);
        }

        // 发送预处理完成标记（前端可以刷新获取 last_request_messages）
        yield Ok(Event::default().data("[PRE_PROCESS_DONE]"));

        // 发送 role
        let chunk = ChatCompletionChunk {
            id: id.clone(),
            object: "chat.completion.chunk".to_string(),
            created,
            model: model.clone(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: Delta {
                    role: Some("assistant".to_string()),
                    content: None,
                },
                finish_reason: None,
            }],
        };
        yield Ok(Event::default().data(serde_json::to_string(&chunk).unwrap()));

        // 流式调用 AI
        let mut full_response = String::new();
        
        match ctx.ai_client.chat_stream_with_model(&packet.messages, Some(&model)).await {
            Ok(mut ai_stream) => {
                while let Some(result) = ai_stream.next().await {
                    match result {
                        Ok(content) => {
                            full_response.push_str(&content);
                            let chunk = ChatCompletionChunk {
                                id: id.clone(),
                                object: "chat.completion.chunk".to_string(),
                                created,
                                model: model.clone(),
                                choices: vec![ChunkChoice {
                                    index: 0,
                                    delta: Delta {
                                        role: None,
                                        content: Some(content),
                                    },
                                    finish_reason: None,
                                }],
                            };
                            yield Ok(Event::default().data(serde_json::to_string(&chunk).unwrap()));
                        }
                        Err(e) => {
                            tracing::error!("流式响应错误: {}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("AI 流式调用失败: {}", e);
                let chunk = ChatCompletionChunk {
                    id: id.clone(),
                    object: "chat.completion.chunk".to_string(),
                    created,
                    model: model.clone(),
                    choices: vec![ChunkChoice {
                        index: 0,
                        delta: Delta {
                            role: None,
                            content: Some(format!("[错误] {}", e)),
                        },
                        finish_reason: None,
                    }],
                };
                yield Ok(Event::default().data(serde_json::to_string(&chunk).unwrap()));
            }
        }

        // 发送结束标记
        let chunk = ChatCompletionChunk {
            id: id.clone(),
            object: "chat.completion.chunk".to_string(),
            created,
            model: model.clone(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: Delta {
                    role: None,
                    content: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
        };
        yield Ok(Event::default().data(serde_json::to_string(&chunk).unwrap()));
yield Ok(Event::default().data("[DONE]"));

        // 打印AI完整响应
        tracing::info!("========== AI API 响应 ==========");
        let response_preview: String = full_response.chars().take(500).collect();
        let suffix = if full_response.len() > 500 { "..." } else { "" };
        tracing::info!("响应长度: {} 字符", full_response.len());
        tracing::info!("内容: {}{}", response_preview, suffix);
        tracing::info!("=================================");

// 过滤思考标签后追加 AI 响应到数据包
        let cleaned_response = AiClient::strip_thinking_tags(&full_response);
        packet.append_assistant_message(&cleaned_response);

        // 保存当前轮次对话（在后处理之前，避免被清理器清空）
        packet.save_conversation_turn();

        // 后处理（同步执行，确保在流结束前完成）
        let post_result = post_process_and_save(
            &state,
            packet,
            &user_message,
            &full_response,
            &assistant_id,
            &topic_id,
        ).await;

        // 发送后处理完成标记（前端可以据此刷新数据，可以继续下一次对话）
        yield Ok(Event::default().data("[POST_PROCESS_DONE]"));

        // 启动异步后处理任务（不阻塞下一次对话）
        if let Some(result) = post_result {
            let state_clone = state.clone();
            tokio::spawn(async move {
                background_process(state_clone, result).await;
            });
        }
    };

    Sse::new(stream)
}

/// 获取或创建对话数据包
async fn get_or_create_packet(
    state: &Arc<AppState>,
    assistant_id: &str,
    topic_id: &str,
) -> Result<ConversationPacket, String> {
    // 尝试从存储加载
    if let Ok(Some(packet)) = state.packet_storage.load(assistant_id, topic_id).await {
        tracing::debug!("加载已有数据包: {}:{}", assistant_id, topic_id);
        return Ok(packet);
    }

    // 获取助手配置
    let assistant_config = state.assistant_manager
        .get_assistant(assistant_id)
        .await
        .map_err(|e| format!("获取助手配置失败: {}", e))?;

    // 创建新数据包
    let mut packet = ConversationPacket::new(
        assistant_id.to_string(),
        topic_id.to_string(),
        assistant_config.roles.user_name.clone(),
        assistant_config.roles.assistant_name.clone(),
    );

    // 设置系统提示词
    if !assistant_config.system_prompt.is_empty() {
        packet.set_system_message(&assistant_config.system_prompt);
    }

    // 设置模型配置
    packet.main_model = Some(assistant_config.model.main_model.clone());
    packet.processor_model = Some(assistant_config.model.processor_model.clone());
    packet.embedding_model = Some(assistant_config.model.embedding_model.clone());

    tracing::debug!("创建新数据包: {}:{}", assistant_id, topic_id);
    Ok(packet)
}

/// 从请求消息中提取用户消息
fn extract_user_message(messages: &[Message]) -> String {
    messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_default()
}

/// 创建错误响应
fn error_response(error: &str, model: &str) -> Json<ChatCompletionResponse> {
    Json(ChatCompletionResponse {
        id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        object: "chat.completion".to_string(),
        created: chrono::Utc::now().timestamp(),
        model: model.to_string(),
        choices: vec![Choice {
            index: 0,
            message: Message {
                role: "assistant".to_string(),
                content: format!("[错误] {}", error),
            },
            finish_reason: "stop".to_string(),
        }],
        usage: Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        },
    })
}

/// 后处理：执行 after_ai_response 处理器 + 消息持久化
/// 同步后处理结果，用于传递给异步后处理任务
struct PostProcessResult {
    packet: ConversationPacket,
    pipeline_config: PipelineConfig,
    assistant_id: String,
    topic_id: String,
}

async fn post_process_and_save(
    state: &Arc<AppState>,
    mut packet: ConversationPacket,
    user_message: &str,
    ai_response: &str,
    assistant_id: &str,
    topic_id: &str,
) -> Option<PostProcessResult> {
    tracing::info!("post_process_and_save: {}:{}", assistant_id, topic_id);

    // 创建处理器上下文
    let ctx = match state.dispatcher.context_factory()
        .create(assistant_id, topic_id).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("创建上下文失败: {}", e);
            return None;
        }
    };

    // 获取流水线配置（从助手配置读取）
    let pipeline_config = ctx.assistant_config.pipeline.clone();

    // 执行 after_ai_response 处理器（同步）
    if let Err(e) = state.dispatcher.dispatch(
        PipelineTiming::AfterAiResponse,
        &mut packet,
        &pipeline_config,
        &ctx,
    ).await {
        tracing::warn!("after_ai_response 处理器执行失败: {}", e);
    }

    // 轮次结束处理
    packet.end_turn();

    // 持久化数据包
    if let Err(e) = state.packet_storage.save(&packet).await {
        tracing::error!("保存数据包失败: {}", e);
    }

    // 持久化消息到话题历史
    let messages = vec![
        ChatMessage::user(user_message),
        ChatMessage::assistant(ai_response),
    ];
    if let Err(e) = state.assistant_manager.append_messages(assistant_id, topic_id, messages).await {
        tracing::error!("消息持久化失败: {}", e);
    } else {
        tracing::info!("消息已保存: {}:{}", assistant_id, topic_id);
    }

    // 返回结果，用于异步后处理
    Some(PostProcessResult {
        packet,
        pipeline_config,
        assistant_id: assistant_id.to_string(),
        topic_id: topic_id.to_string(),
    })
}

/// 异步后处理任务（不阻塞下一次对话）
async fn background_process(
    state: Arc<AppState>,
    mut result: PostProcessResult,
) {
    if result.pipeline_config.background_process.is_empty() {
        tracing::debug!("无后台处理器配置，跳过");
        return;
    }

    tracing::info!("启动后台处理: {}:{}", result.assistant_id, result.topic_id);

    // 创建处理器上下文
    let ctx = match state.dispatcher.context_factory()
        .create(&result.assistant_id, &result.topic_id).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("后台处理创建上下文失败: {}", e);
            return;
        }
    };

    // 执行 background_process 处理器
    if let Err(e) = state.dispatcher.dispatch(
        PipelineTiming::BackgroundProcess,
        &mut result.packet,
        &result.pipeline_config,
        &ctx,
    ).await {
        tracing::warn!("background_process 处理器执行失败: {}", e);
    }

    // 持久化更新后的数据包
    if let Err(e) = state.packet_storage.save(&result.packet).await {
        tracing::error!("后台处理保存数据包失败: {}", e);
    }

    tracing::info!("后台处理完成: {}:{}", result.assistant_id, result.topic_id);
}// ==================== Packet API ====================

/// 获取对话数据包（思考池和短期记忆）的响应结构
#[derive(Debug, Serialize)]
pub struct PacketMemoryResponse {
    pub success: bool,
    pub data: Option<PacketMemoryData>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PacketMemoryData {
    pub messages: Vec<crate::types::ChatMessage>,
    pub thinking_pool: Vec<ThinkingEntryDto>,
    pub short_term_memory: Vec<ShortTermMemoryDto>,
    pub conversation_turns: Vec<ConversationTurnDto>,
    /// 最终发送给 AI 的 messages（调试用）
    pub last_request_messages: Vec<crate::types::ChatMessage>,
}

/// 对话轮次 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurnDto {
    pub user_message: String,
    pub assistant_message: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingEntryDto {
    pub content: String,
    pub source: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortTermMemoryDto {
    pub id: String,
    /// 概述/标题
    pub summary: String,
    /// 详细内容
    pub content: String,
    /// 记忆类型
    pub memory_type: String,
    /// 是否需要展开（前端控制）
    #[serde(default)]
    pub should_expand: bool,
    pub source: String,
    pub timestamp: String,
}

/// 更新思考池请求
#[derive(Debug, Deserialize)]
pub struct UpdateThinkingPoolRequest {
    pub thinking_pool: Vec<ThinkingEntryDto>,
}

/// 更新短期记忆请求
#[derive(Debug, Deserialize)]
pub struct UpdateShortTermMemoryRequest {
    pub short_term_memory: Vec<ShortTermMemoryDto>,
}

impl From<&ThinkingEntry> for ThinkingEntryDto {
    fn from(entry: &ThinkingEntry) -> Self {
        Self {
            content: entry.content.clone(),
            source: format!("{:?}", entry.source),
            timestamp: entry.timestamp.to_rfc3339(),
        }
    }
}

impl From<&ShortTermMemory> for ShortTermMemoryDto {
    fn from(mem: &ShortTermMemory) -> Self {
        Self {
            id: mem.id.clone(),
            summary: mem.summary.clone(),
            content: mem.content.clone(),
            memory_type: mem.memory_type.clone(),
            should_expand: mem.should_expand,
            source: format!("{:?}", mem.source),
            timestamp: mem.timestamp.to_rfc3339(),
        }
    }
}

/// 获取对话数据包的记忆池
pub async fn get_packet_memory(
    State(state): State<Arc<AppState>>,
    Path((assistant_id, topic_id)): Path<(String, String)>,
) -> Json<PacketMemoryResponse> {
    tracing::debug!("获取数据包记忆: {}:{}", assistant_id, topic_id);
    
    match state.packet_storage.load(&assistant_id, &topic_id).await {
        Ok(Some(packet)) => {
            let data = PacketMemoryData {
                messages: packet.messages.clone(),
                thinking_pool: packet.thinking_pool.iter().map(|e| e.into()).collect(),
                short_term_memory: packet.short_term_memory.iter().map(|m| m.into()).collect(),
                conversation_turns: packet.conversation_turns.iter().map(|t| ConversationTurnDto {
                    user_message: t.user_message.clone(),
                    assistant_message: t.assistant_message.clone(),
                    timestamp: t.timestamp.to_rfc3339(),
                }).collect(),
                last_request_messages: packet.last_request_messages.clone(),
            };
            Json(PacketMemoryResponse {
                success: true,
                data: Some(data),
                error: None,
            })
        }
        Ok(None) => {
            // 数据包不存在，返回空数据
            Json(PacketMemoryResponse {
                success: true,
                data: Some(PacketMemoryData {
                    messages: vec![],
                    thinking_pool: vec![],
                    short_term_memory: vec![],
                    conversation_turns: vec![],
                    last_request_messages: vec![],
                }),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("加载数据包失败: {}", e);
            Json(PacketMemoryResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            })
        }
    }
}

/// 更新思考池
pub async fn update_thinking_pool(
    State(state): State<Arc<AppState>>,
    Path((assistant_id, topic_id)): Path<(String, String)>,
    Json(req): Json<UpdateThinkingPoolRequest>,
) -> Json<PacketMemoryResponse> {
    tracing::debug!("更新思考池: {}:{}", assistant_id, topic_id);
    
    // 加载或创建数据包
    let mut packet = match get_or_create_packet(&state, &assistant_id, &topic_id).await {
        Ok(p) => p,
        Err(e) => {
            return Json(PacketMemoryResponse {
                success: false,
                data: None,
                error: Some(e),
            });
        }
    };
    
    // 更新思考池
    packet.thinking_pool = req.thinking_pool.iter().map(|dto| {
        ThinkingEntry {
            content: dto.content.clone(),
            source: parse_thinking_source(&dto.source),
            timestamp: chrono::DateTime::parse_from_rfc3339(&dto.timestamp)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
        }
    }).collect();
    
    // 保存数据包
    if let Err(e) = state.packet_storage.save(&packet).await {
        return Json(PacketMemoryResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        });
    }
    
    let data = PacketMemoryData {
        messages: packet.messages.clone(),
        thinking_pool: packet.thinking_pool.iter().map(|e| e.into()).collect(),
        short_term_memory: packet.short_term_memory.iter().map(|m| m.into()).collect(),
        conversation_turns: packet.conversation_turns.iter().map(|t| ConversationTurnDto {
            user_message: t.user_message.clone(),
            assistant_message: t.assistant_message.clone(),
            timestamp: t.timestamp.to_rfc3339(),
        }).collect(),
        last_request_messages: packet.last_request_messages.clone(),
    };
    
    Json(PacketMemoryResponse {
        success: true,
        data: Some(data),
        error: None,
    })
}

/// 更新短期记忆
pub async fn update_short_term_memory(
    State(state): State<Arc<AppState>>,
    Path((assistant_id, topic_id)): Path<(String, String)>,
    Json(req): Json<UpdateShortTermMemoryRequest>,
) -> Json<PacketMemoryResponse> {
    tracing::debug!("更新短期记忆: {}:{}", assistant_id, topic_id);
    
    // 加载或创建数据包
    let mut packet = match get_or_create_packet(&state, &assistant_id, &topic_id).await {
        Ok(p) => p,
        Err(e) => {
            return Json(PacketMemoryResponse {
                success: false,
                data: None,
                error: Some(e),
            });
        }
    };
    
    // 更新短期记忆
packet.short_term_memory = req.short_term_memory.iter().map(|dto| {
        ShortTermMemory {
            id: dto.id.clone(),
            summary: dto.summary.clone(),
            content: dto.content.clone(),
            memory_type: dto.memory_type.clone(),
            should_expand: dto.should_expand,
            source: parse_memory_source(&dto.source),
            timestamp: chrono::DateTime::parse_from_rfc3339(&dto.timestamp)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
        }
    }).collect();
    
    // 保存数据包
    if let Err(e) = state.packet_storage.save(&packet).await {
        return Json(PacketMemoryResponse {
            success: false,
            data: None,
            error: Some(e.to_string()),
        });
    }
    
    let data = PacketMemoryData {
        messages: packet.messages.clone(),
        thinking_pool: packet.thinking_pool.iter().map(|e| e.into()).collect(),
        short_term_memory: packet.short_term_memory.iter().map(|m| m.into()).collect(),
        conversation_turns: packet.conversation_turns.iter().map(|t| ConversationTurnDto {
            user_message: t.user_message.clone(),
            assistant_message: t.assistant_message.clone(),
            timestamp: t.timestamp.to_rfc3339(),
        }).collect(),
        last_request_messages: packet.last_request_messages.clone(),
    };
    
    Json(PacketMemoryResponse {
        success: true,
        data: Some(data),
        error: None,
    })
}

/// 解析思考来源
fn parse_thinking_source(s: &str) -> ThinkingSource {
    match s {
        "UserAnalysis" => ThinkingSource::UserAnalysis,
        "MemoryRetrieval" => ThinkingSource::MemoryRetrieval,
        "ToolResult" => ThinkingSource::ToolResult,
        "SelfReflection" => ThinkingSource::SelfReflection,
        _ => ThinkingSource::UserAnalysis,
    }
}

/// 解析记忆来源
fn parse_memory_source(s: &str) -> MemorySource {
    match s {
        "LongTermRetrieval" => MemorySource::LongTermRetrieval,
        "CurrentConversation" => MemorySource::CurrentConversation,
        "ToolResult" => MemorySource::ToolResult,
        _ => MemorySource::CurrentConversation,
    }
}