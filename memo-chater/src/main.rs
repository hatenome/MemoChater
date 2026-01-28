mod api;
mod config;
mod state;
mod storage;
mod types;
mod memory;
mod ai;
mod pipeline;
mod vector;
mod qdrant;
mod extractor;
mod admin_api;
mod assistant;
mod assistant_api;
mod graph;
mod graph_api;

use axum::{routing::post, Router};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::ai::AiClient;
use crate::config::AppConfig;
use crate::memory::MemoryManager;
use crate::pipeline::{PipelineDispatcher, ProcessorContextFactory, PacketStorage, create_all_processors};
use crate::qdrant::QdrantManager;
use crate::state::AppState;
use crate::assistant::AssistantManager;
use crate::assistant_api::{assistant_routes, AssistantApiState};
use crate::graph_api::graph_routes;

#[tokio::main]
async fn main() {
    // 初始化日志
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("memo_chater=debug".parse().unwrap()))
        .init();

    tracing::info!("MemoChater 启动中...");

    // 加载配置
    let config = AppConfig::load_default().expect("加载配置失败");
    tracing::info!("配置加载成功: api_base={}", config.ai.api_base);

    // 启动内嵌 Qdrant（如果启用）
    let mut qdrant_manager = if config.qdrant.embedded {
        tracing::info!("内嵌 Qdrant 模式已启用");
        let mut manager = QdrantManager::new(
            config.qdrant.exe_path.clone(),
            config.qdrant.storage_path.clone(),
            config.qdrant.port,
        );
        
        if let Err(e) = manager.start().await {
            tracing::error!("启动内嵌 Qdrant 失败: {}", e);
            tracing::error!("请检查 Qdrant 可执行文件路径: {}", config.qdrant.exe_path.display());
            std::process::exit(1);
        }
        
        Some(manager)
    } else {
        tracing::info!("使用外部 Qdrant: {}", config.qdrant.external_url);
        None
    };

    // 设置 Ctrl+C 处理
    let shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        tracing::info!("收到关闭信号，正在优雅关闭...");
    };

    // 创建 AI 客户端
    let ai_client = AiClient::from_config(&config.ai).expect("创建 AI 客户端失败");
    let key_preview = config.ai.api_key.chars().take(10).collect::<String>();
    tracing::info!(
        "AI 客户端创建成功: model={}, api_base={}, key_preview={}...",
        config.ai.main_model,
        config.ai.api_base,
        key_preview
    );

    // 创建记忆管理器
    let qdrant_url = if config.qdrant.embedded {
        format!("http://localhost:{}", config.qdrant.port)
    } else {
        config.qdrant.external_url.clone()
    };
    
    let memory_config = memory::MemoryManagerConfig {
        qdrant_url,
        collection_name: "long_term_memories".to_string(),
        vector_size: 4096,  // qwen3-embedding:latest 输出4096维
        file_storage_dir: "./data/files".to_string(),
    };
    
    let memory_manager = match MemoryManager::new(memory_config, ai_client.clone()).await {
        Ok(manager) => Arc::new(RwLock::new(manager)),
        Err(e) => {
            tracing::error!("创建记忆管理器失败: {}", e);
            std::process::exit(1);
        }
    };

    let listen_addr = config.listen_addr.clone();
    let data_dir = config.data_dir.clone();

    // 创建助手管理器
    let assistant_manager = Arc::new(AssistantManager::new(&data_dir));

    // ===== 创建流水线调度器 =====
    let ai_client_arc = Arc::new(ai_client);
    let global_config = Arc::new(config.clone());
    
    // 创建上下文工厂
    let context_factory = Arc::new(ProcessorContextFactory::new(
        ai_client_arc.clone(),
        global_config,
        assistant_manager.clone(),
        memory_manager.clone(),
    ));
    
    // 创建调度器并注册所有处理器
    let mut dispatcher = PipelineDispatcher::new(context_factory);
    dispatcher.register_all(create_all_processors());
    let dispatcher = Arc::new(dispatcher);
    
    // 创建数据包存储
    let packet_storage = Arc::new(PacketStorage::new(data_dir.clone()));
    
    tracing::info!("流水线调度器初始化完成，已注册处理器: {:?}", dispatcher.list_processors());
    
    // 初始化状态
    let state = Arc::new(AppState {
        config,
        assistant_manager: assistant_manager.clone(),
        dispatcher,
        packet_storage,
        memory_manager: memory_manager.clone(),
    });

    // 助手API状态
    let assistant_state = Arc::new(AssistantApiState {
        manager: assistant_manager,
        ai_client: ai_client_arc.clone(),
    });

    // CORS 配置
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // 路由 - 助手API使用独立状态
    let assistant_router = assistant_routes(assistant_state);
    
    // 图API路由
    let graph_router = graph_routes().with_state(state.clone());
    
    // 主应用路由
    let main_router = Router::new()
        // OpenAI 兼容 API
        .route("/v1/chat/completions", post(api::chat_completions))
        .route("/v1/models", axum::routing::get(api::list_models))
        // 管理界面
        .route("/admin", axum::routing::get(admin_api::admin_page))
        .route("/admin/api/memories", axum::routing::get(admin_api::list_memories))
        .route("/admin/api/memories", post(admin_api::create_memory))
        .route("/admin/api/memories/:id", axum::routing::get(admin_api::get_memory))
        .route("/admin/api/memories/:id", axum::routing::delete(admin_api::delete_memory))
        .route("/admin/api/stats", axum::routing::get(admin_api::get_stats))
        // 记忆转化API
        .route("/admin/api/extract", post(admin_api::trigger_extraction))
        // 待处理池管理API
        .route("/admin/api/pending", axum::routing::get(admin_api::get_pending_status))
        .route("/admin/api/pending", axum::routing::delete(admin_api::clear_pending))
        .route("/admin/api/pending/process", post(admin_api::process_pending))
        .route("/admin/api/models", axum::routing::get(admin_api::list_models))
        .route("/admin/api/processors", axum::routing::get(admin_api::list_processors))
        .route("/admin/api/settings", axum::routing::get(admin_api::get_settings))
        .route("/admin/api/settings", axum::routing::put(admin_api::update_settings))
        // 按助手隔离的记忆API
        .route("/assistants/:assistant_id/memories", axum::routing::get(admin_api::list_assistant_memories))
        .route("/assistants/:assistant_id/memories", post(admin_api::create_assistant_memory))
        .route("/assistants/:assistant_id/memories/:memory_id", axum::routing::get(admin_api::get_assistant_memory))
        .route("/assistants/:assistant_id/memories/:memory_id", axum::routing::delete(admin_api::delete_assistant_memory))
        .route("/assistants/:assistant_id/pending", axum::routing::get(admin_api::get_assistant_pending_status))
        .route("/assistants/:assistant_id/pending", axum::routing::delete(admin_api::clear_assistant_pending))
        .route("/assistants/:assistant_id/pending/process", post(admin_api::process_assistant_pending))
        // Packet 记忆池 API
        .route("/assistants/:assistant_id/topics/:topic_id/packet", axum::routing::get(api::get_packet_memory))
        .route("/assistants/:assistant_id/topics/:topic_id/packet/thinking", axum::routing::put(api::update_thinking_pool))
        .route("/assistants/:assistant_id/topics/:topic_id/packet/short-term", axum::routing::put(api::update_short_term_memory))
        .with_state(state);
    
    // 合并路由
    let app = Router::new()
        .merge(main_router)
        .merge(assistant_router)
        .merge(graph_router)
        .layer(cors);

    // 启动服务
    tracing::info!("监听地址: {}", listen_addr);
    
    let listener = tokio::net::TcpListener::bind(&listen_addr).await.unwrap();
    
    // 使用 graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await
        .unwrap();

    // 关闭 Qdrant
    if let Some(ref mut manager) = qdrant_manager {
        tracing::info!("正在关闭内嵌 Qdrant...");
        if let Err(e) = manager.stop() {
            tracing::warn!("关闭 Qdrant 时出错: {}", e);
        }
    }

    tracing::info!("MemoChater 已关闭");
}