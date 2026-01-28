//! 关系图引擎 - 单维度图实现

use crate::graph::error::{GraphError, GraphResult};
use crate::graph::types::*;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;

// ============================================================================
// 邻接表索引（运行时缓存）
// ============================================================================

/// 邻接表索引
#[derive(Debug, Default)]
struct AdjacencyIndex {
    /// 正向邻接表 <source_id, Vec<(target_id, weight)>>
    forward: HashMap<String, Vec<(String, f32)>>,
    /// 反向邻接表 <target_id, Vec<(source_id, weight)>>
    backward: HashMap<String, Vec<(String, f32)>>,
}

impl AdjacencyIndex {
    fn new() -> Self {
        Self::default()
    }

    fn build_from_edges(edges: &[Edge]) -> Self {
        let mut index = Self::new();
        for edge in edges {
            index
                .forward
                .entry(edge.source.clone())
                .or_default()
                .push((edge.target.clone(), edge.weight));
            index
                .backward
                .entry(edge.target.clone())
                .or_default()
                .push((edge.source.clone(), edge.weight));
        }
        index
    }

    fn add_edge(&mut self, source: &str, target: &str, weight: f32) {
        self.forward
            .entry(source.to_string())
            .or_default()
            .push((target.to_string(), weight));
        self.backward
            .entry(target.to_string())
            .or_default()
            .push((source.to_string(), weight));
    }

    fn remove_edge(&mut self, source: &str, target: &str) {
        if let Some(targets) = self.forward.get_mut(source) {
            targets.retain(|(t, _)| t != target);
        }
        if let Some(sources) = self.backward.get_mut(target) {
            sources.retain(|(s, _)| s != source);
        }
    }

    fn remove_node(&mut self, node_id: &str) {
        // 移除所有从该节点出发的边
        self.forward.remove(node_id);
        // 移除所有指向该节点的边
        self.backward.remove(node_id);
        // 从其他节点的邻接表中移除
        for targets in self.forward.values_mut() {
            targets.retain(|(t, _)| t != node_id);
        }
        for sources in self.backward.values_mut() {
            sources.retain(|(s, _)| s != node_id);
        }
    }
}

// ============================================================================
// 单维度图
// ============================================================================

/// 单维度图 - 完全独立的图实例
#[derive(Debug, Serialize, Deserialize)]
pub struct DimensionGraph {
    /// 维度名称
    pub dimension: String,
    /// 版本号
    pub version: String,
    /// 元数据
    pub metadata: GraphMetadata,
    /// 节点列表
    pub nodes: Vec<Node>,
    /// 边列表
    pub edges: Vec<Edge>,

    /// 运行时：存储路径（不序列化）
    #[serde(skip)]
    storage_path: Option<PathBuf>,
    /// 运行时：邻接表缓存（不序列化）
    #[serde(skip)]
    adjacency_cache: Option<AdjacencyIndex>,
    /// 运行时：节点索引（不序列化）
    #[serde(skip)]
    node_index: Option<HashMap<String, usize>>,
}

impl DimensionGraph {
    // ========================================================================
    // 生命周期
    // ========================================================================

    /// 创建空图
    pub fn new(dimension: &str) -> Self {
        Self {
            dimension: dimension.to_string(),
            version: "1.0".to_string(),
            metadata: GraphMetadata::default(),
            nodes: Vec::new(),
            edges: Vec::new(),
            storage_path: None,
            adjacency_cache: None,
            node_index: None,
        }
    }

    /// 从文件加载
    pub async fn load(path: &Path) -> GraphResult<Self> {
        let content = fs::read_to_string(path).await?;
        let mut graph: Self = serde_json::from_str(&content)?;
        graph.storage_path = Some(path.to_path_buf());
        graph.rebuild_caches();
        Ok(graph)
    }

    /// 保存到文件
    pub async fn save(&self) -> GraphResult<()> {
        let path = self
            .storage_path
            .as_ref()
            .ok_or_else(|| GraphError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "存储路径未设置",
            )))?;

        // 确保目录存在
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let content = serde_json::to_string_pretty(self)?;
        let mut file = fs::File::create(path).await?;
        file.write_all(content.as_bytes()).await?;
        file.flush().await?;

        Ok(())
    }

    /// 设置存储路径
    pub fn set_storage_path(&mut self, path: PathBuf) {
        self.storage_path = Some(path);
    }

    /// 获取存储路径
    pub fn storage_path(&self) -> Option<&Path> {
        self.storage_path.as_deref()
    }

    // ========================================================================
    // 节点操作
    // ========================================================================

    /// 添加节点
    pub fn add_node(&mut self, node: Node) {
        let id = node.id.clone();
        let index = self.nodes.len();
        self.nodes.push(node);

        // 更新索引
        if let Some(ref mut node_index) = self.node_index {
            node_index.insert(id, index);
        }

        self.update_metadata();
    }

    /// 移除节点（同时移除相关边）
    pub fn remove_node(&mut self, node_id: &str) -> Option<Node> {
        let index = self.nodes.iter().position(|n| n.id == node_id)?;
        let node = self.nodes.remove(index);

        // 移除相关边
        self.edges
            .retain(|e| e.source != node_id && e.target != node_id);

        // 更新缓存
        if let Some(ref mut cache) = self.adjacency_cache {
            cache.remove_node(node_id);
        }

        // 重建节点索引（因为位置变了）
        self.rebuild_node_index();
        self.update_metadata();

        Some(node)
    }

    /// 获取节点
    pub fn get_node(&self, node_id: &str) -> Option<&Node> {
        if let Some(ref node_index) = self.node_index {
            node_index.get(node_id).map(|&i| &self.nodes[i])
        } else {
            self.nodes.iter().find(|n| n.id == node_id)
        }
    }

    /// 获取可变节点
    pub fn get_node_mut(&mut self, node_id: &str) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == node_id)
    }

    /// 获取所有节点
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// 节点数量
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 检查节点是否存在
    pub fn has_node(&self, node_id: &str) -> bool {
        if let Some(ref node_index) = self.node_index {
            node_index.contains_key(node_id)
        } else {
            self.nodes.iter().any(|n| n.id == node_id)
        }
    }

    // ========================================================================
    // 边操作
    // ========================================================================

    /// 添加边
    pub fn add_edge(&mut self, edge: Edge) {
        // 更新缓存
        if let Some(ref mut cache) = self.adjacency_cache {
            cache.add_edge(&edge.source, &edge.target, edge.weight);
        }

        self.edges.push(edge);
        self.update_metadata();
    }

    /// 批量添加边
    pub fn add_edges(&mut self, edges: Vec<Edge>) {
        for edge in edges {
            self.add_edge(edge);
        }
    }

    /// 移除边
    pub fn remove_edge(&mut self, source: &str, target: &str) -> Option<Edge> {
        let index = self
            .edges
            .iter()
            .position(|e| e.source == source && e.target == target)?;
        let edge = self.edges.remove(index);

        // 更新缓存
        if let Some(ref mut cache) = self.adjacency_cache {
            cache.remove_edge(source, target);
        }

        self.update_metadata();
        Some(edge)
    }

    /// 获取从指定节点出发的边
    pub fn edges_from(&self, source: &str) -> Vec<&Edge> {
        self.edges.iter().filter(|e| e.source == source).collect()
    }

    /// 获取指向指定节点的边（反向查询）
    pub fn edges_to(&self, target: &str) -> Vec<&Edge> {
        self.edges.iter().filter(|e| e.target == target).collect()
    }

    /// 获取两节点间的边
    pub fn edge_between(&self, source: &str, target: &str) -> Option<&Edge> {
        self.edges
            .iter()
            .find(|e| e.source == source && e.target == target)
    }

    /// 边数量
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    // ========================================================================
    // 缓存管理
    // ========================================================================

    /// 重建所有缓存
    pub fn rebuild_caches(&mut self) {
        self.rebuild_adjacency_cache();
        self.rebuild_node_index();
    }

    /// 重建邻接表缓存
    pub fn rebuild_adjacency_cache(&mut self) {
        self.adjacency_cache = Some(AdjacencyIndex::build_from_edges(&self.edges));
    }

    /// 重建节点索引
    fn rebuild_node_index(&mut self) {
        let mut index = HashMap::new();
        for (i, node) in self.nodes.iter().enumerate() {
            index.insert(node.id.clone(), i);
        }
        self.node_index = Some(index);
    }

    /// 更新元数据
    fn update_metadata(&mut self) {
        self.metadata.updated_at = Utc::now();
        self.metadata.node_count = self.nodes.len();
        self.metadata.edge_count = self.edges.len();
    }

    // ========================================================================
    // 邻接表快速查询
    // ========================================================================

    /// 获取正向邻居（从该节点出发能到达的节点）
    pub fn forward_neighbors(&self, node_id: &str) -> Vec<(&str, f32)> {
        if let Some(ref cache) = self.adjacency_cache {
            cache
                .forward
                .get(node_id)
                .map(|v| v.iter().map(|(id, w)| (id.as_str(), *w)).collect())
                .unwrap_or_default()
        } else {
            self.edges
                .iter()
                .filter(|e| e.source == node_id)
                .map(|e| (e.target.as_str(), e.weight))
                .collect()
        }
    }

    /// 获取反向邻居（能到达该节点的节点）
    pub fn backward_neighbors(&self, node_id: &str) -> Vec<(&str, f32)> {
        if let Some(ref cache) = self.adjacency_cache {
            cache
                .backward
                .get(node_id)
                .map(|v| v.iter().map(|(id, w)| (id.as_str(), *w)).collect())
                .unwrap_or_default()
        } else {
            self.edges
                .iter()
                .filter(|e| e.target == node_id)
                .map(|e| (e.source.as_str(), e.weight))
                .collect()
        }
    }
}