//! 关系图 API
//!
//! 提供图数据的 REST 接口

use axum::{
    extract::{Path, State},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::graph::{
    DimensionGraph, GraphLocator, GraphScope, Node, Edge, GraphMetadata,
};
use crate::state::AppState;

// ============================================================================
// 响应类型
// ============================================================================

#[derive(Serialize)]
pub struct GraphResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<GraphData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct GraphData {
    pub dimension: String,
    pub metadata: GraphMetadata,
    pub nodes: Vec<NodeData>,
    pub edges: Vec<EdgeData>,
}

#[derive(Serialize)]
pub struct NodeData {
    pub id: String,
    pub label: String,
    pub timestamp: Option<String>,
    pub memory_type: Option<String>,
    pub content_preview: Option<String>,
}

#[derive(Serialize)]
pub struct EdgeData {
    pub from: String,
    pub to: String,
    pub weight: f32,
    pub label: String,
}

#[derive(Serialize)]
pub struct DimensionsResponse {
    pub success: bool,
    pub dimensions: Vec<String>,
}

// ============================================================================
// 路由
// ============================================================================

pub fn graph_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/graphs/:assistant_id/:topic_id/dimensions",
            get(list_dimensions),
        )
        .route(
            "/graphs/:assistant_id/:topic_id/:dimension",
            get(get_graph),
        )
        .route(
            "/graphs/:assistant_id/:topic_id/:dimension/rebuild",
            post(rebuild_graph),
        )
}

// ============================================================================
// 处理函数
// ============================================================================

/// 列出话题的所有维度图
async fn list_dimensions(
    State(state): State<Arc<AppState>>,
    Path((assistant_id, topic_id)): Path<(String, String)>,
) -> Json<DimensionsResponse> {
    let data_dir = std::path::PathBuf::from(&state.config.data_dir);
    let locator = GraphLocator::new(data_dir);
    
    let scope = GraphScope::ShortTerm {
        assistant_id,
        topic_id,
    };
    
    match locator.list_dimensions(&scope).await {
        Ok(dimensions) => Json(DimensionsResponse {
            success: true,
            dimensions,
        }),
        Err(_) => Json(DimensionsResponse {
            success: true,
            dimensions: vec![],
        }),
    }
}

/// 获取指定维度的图数据
async fn get_graph(
    State(state): State<Arc<AppState>>,
    Path((assistant_id, topic_id, dimension)): Path<(String, String, String)>,
) -> Json<GraphResponse> {
    let data_dir = std::path::PathBuf::from(&state.config.data_dir);
    let locator = GraphLocator::new(data_dir.clone());
    
    let scope = GraphScope::ShortTerm {
        assistant_id: assistant_id.clone(),
        topic_id: topic_id.clone(),
    };
    
    // 尝试获取图
    match locator.get(&scope, &dimension).await {
        Ok(graph_lock) => {
            let graph = graph_lock.read().await;
            
            // 如果图为空，尝试从 short_term_vectors.json 构建
            if graph.nodes.is_empty() {
                drop(graph);
                
                // 读取对话记忆库并构建图
                match build_graph_from_memories(&data_dir, &assistant_id, &topic_id, &dimension).await {
                    Ok(data) => Json(GraphResponse {
                        success: true,
                        data: Some(data),
                        error: None,
                    }),
                    Err(e) => Json(GraphResponse {
                        success: false,
                        data: None,
                        error: Some(e),
                    }),
                }
            } else {
                // 转换为响应格式
                let data = convert_graph_to_response(&graph);
                Json(GraphResponse {
                    success: true,
                    data: Some(data),
                    error: None,
                })
            }
        }
        Err(e) => {
            // 图不存在，尝试从记忆构建
            match build_graph_from_memories(&data_dir, &assistant_id, &topic_id, &dimension).await {
                Ok(data) => Json(GraphResponse {
                    success: true,
                    data: Some(data),
                    error: None,
                }),
                Err(build_err) => Json(GraphResponse {
                    success: false,
                    data: None,
                    error: Some(format!("获取图失败: {}, 构建失败: {}", e, build_err)),
                }),
            }
        }
    }
}

/// 从对话记忆库构建图
async fn build_graph_from_memories(
    data_dir: &std::path::Path,
    assistant_id: &str,
    topic_id: &str,
    dimension: &str,
) -> Result<GraphData, String> {
    use crate::graph::{TemporalProcessor, ComputeContext, MemoryRef, NodeFeatures};
    use crate::pipeline::processors::short_term_vectorizer::ShortTermVectorFile;
    
    // 读取 short_term_vectors.json
    let vectors_path = data_dir
        .join("assistants")
        .join(assistant_id)
        .join("topics")
        .join(topic_id)
        .join("short_term_vectors.json");
    
    if !vectors_path.exists() {
        return Err("对话记忆库文件不存在".to_string());
    }
    
    let content = tokio::fs::read_to_string(&vectors_path)
        .await
        .map_err(|e| format!("读取文件失败: {}", e))?;
    
    let vector_file: ShortTermVectorFile = serde_json::from_str(&content)
        .map_err(|e| format!("解析文件失败: {}", e))?;
    
    if vector_file.vectors.is_empty() {
        return Ok(GraphData {
            dimension: dimension.to_string(),
            metadata: GraphMetadata::default(),
            nodes: vec![],
            edges: vec![],
        });
    }
    
    // 构建图
    let mut graph = DimensionGraph::new(dimension);
    let processor = TemporalProcessor::default();
    let ctx = ComputeContext::with_defaults();
    
    // 按时间排序
    let mut memories: Vec<_> = vector_file.vectors.iter().collect();
    memories.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    
    // 逐个添加节点
    for mem in memories {
        let timestamp = chrono::DateTime::parse_from_rfc3339(&mem.timestamp)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .ok();
        
        let node = Node {
            id: mem.id.clone(),
            memory_ref: MemoryRef::ShortTerm {
                file_path: "short_term_vectors.json".to_string(),
            },
            created_at: timestamp.unwrap_or_else(chrono::Utc::now),
            features: NodeFeatures {
                timestamp,
                entities: vec![],
                emotion: None,
                topics: vec![],
                embedding: None,
            },
        };
        
        // 使用时间维度插入
        if let Err(e) = graph.insert_temporal_node(node, &processor, &ctx).await {
            tracing::warn!("插入节点失败: {}", e);
        }
    }
    
    // 转换为响应格式，同时填充记忆内容
    let mut nodes = Vec::new();
    for node in graph.nodes() {
        // 查找对应的记忆内容
        let mem = vector_file.vectors.iter().find(|m| m.id == node.id);
        
        nodes.push(NodeData {
            id: node.id.clone(),
            label: mem.map(|m| truncate(&m.summary, 20)).unwrap_or_else(|| node.id.clone()),
            timestamp: node.features.timestamp.map(|t| t.to_rfc3339()),
            memory_type: mem.map(|m| m.memory_type.clone()),
            content_preview: mem.map(|m| truncate(&m.content, 100)),
        });
    }
    
    let edges: Vec<EdgeData> = graph.edges.iter().map(|e| EdgeData {
        from: e.source.clone(),
        to: e.target.clone(),
        weight: e.weight,
        label: e.reason.clone(),
    }).collect();
    
    Ok(GraphData {
        dimension: dimension.to_string(),
        metadata: graph.metadata.clone(),
        nodes,
        edges,
    })
}

/// 转换图为响应格式
fn convert_graph_to_response(graph: &DimensionGraph) -> GraphData {
    let nodes: Vec<NodeData> = graph.nodes().iter().map(|n| NodeData {
        id: n.id.clone(),
        label: n.id.clone(),
        timestamp: n.features.timestamp.map(|t| t.to_rfc3339()),
        memory_type: None,
        content_preview: None,
    }).collect();
    
    let edges: Vec<EdgeData> = graph.edges.iter().map(|e| EdgeData {
        from: e.source.clone(),
        to: e.target.clone(),
        weight: e.weight,
        label: e.reason.clone(),
    }).collect();
    
    GraphData {
        dimension: graph.dimension.clone(),
        metadata: graph.metadata.clone(),
        nodes,
        edges,
    }
}

/// 截断字符串
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        s.chars().take(max_len).collect::<String>() + "..."
    }
}

/// 重建关系图
async fn rebuild_graph(
    State(state): State<Arc<AppState>>,
    Path((assistant_id, topic_id, dimension)): Path<(String, String, String)>,
) -> Json<GraphResponse> {
    let data_dir = std::path::PathBuf::from(&state.config.data_dir);
    
    // 强制重建图
    match build_graph_from_memories(&data_dir, &assistant_id, &topic_id, &dimension).await {
        Ok(data) => Json(GraphResponse {
            success: true,
            data: Some(data),
            error: None,
        }),
        Err(e) => Json(GraphResponse {
            success: false,
            data: None,
            error: Some(format!("重建失败: {}", e)),
        }),
    }
}