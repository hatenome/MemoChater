//! 关系图引擎 - 图定位器

use crate::graph::dimension_graph::DimensionGraph;
use crate::graph::error::{GraphError, GraphResult};
use crate::graph::processor::DimensionProcessor;
use crate::graph::types::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

/// 图定位器 - 管理图的加载和缓存
pub struct GraphLocator {
    /// 数据根目录
    data_dir: PathBuf,
    /// 已加载的图缓存 <cache_key, Graph>
    cache: RwLock<HashMap<String, Arc<RwLock<DimensionGraph>>>>,
    /// 已注册的维度处理器
    processors: HashMap<String, Arc<dyn DimensionProcessor>>,
}

impl GraphLocator {
    /// 创建定位器
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            cache: RwLock::new(HashMap::new()),
            processors: HashMap::new(),
        }
    }

    /// 注册维度处理器
    pub fn register_processor(&mut self, processor: Arc<dyn DimensionProcessor>) {
        let dimension = processor.dimension().to_string();
        self.processors.insert(dimension, processor);
    }

    /// 获取维度处理器
    pub fn processor(&self, dimension: &str) -> Option<Arc<dyn DimensionProcessor>> {
        self.processors.get(dimension).cloned()
    }

    /// 获取所有已注册的维度
    pub fn registered_dimensions(&self) -> Vec<&str> {
        self.processors.keys().map(|s| s.as_str()).collect()
    }

    /// 获取指定作用域、指定维度的图
    ///
    /// 如果图不存在，会创建空图
    pub async fn get(
        &self,
        scope: &GraphScope,
        dimension: &str,
    ) -> GraphResult<Arc<RwLock<DimensionGraph>>> {
        let cache_key = scope.cache_key(dimension);

        // 先检查缓存
        {
            let cache = self.cache.read().await;
            if let Some(graph) = cache.get(&cache_key) {
                return Ok(graph.clone());
            }
        }

        // 缓存未命中，尝试从文件加载或创建新图
        let storage_dir = scope.storage_dir(&self.data_dir);
        let file_path = storage_dir.join(format!("{}.json", dimension));

        let graph = if file_path.exists() {
            // 从文件加载
            DimensionGraph::load(&file_path).await?
        } else {
            // 创建新图
            let mut graph = DimensionGraph::new(dimension);
            graph.set_storage_path(file_path);
            graph
        };

        let graph = Arc::new(RwLock::new(graph));

        // 存入缓存
        {
            let mut cache = self.cache.write().await;
            cache.insert(cache_key, graph.clone());
        }

        Ok(graph)
    }

    /// 获取指定作用域的所有已存在维度
    pub async fn list_dimensions(&self, scope: &GraphScope) -> GraphResult<Vec<String>> {
        let storage_dir = scope.storage_dir(&self.data_dir);

        if !storage_dir.exists() {
            return Ok(Vec::new());
        }

        let mut dimensions = Vec::new();
        let mut entries = fs::read_dir(&storage_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "json" {
                        if let Some(stem) = path.file_stem() {
                            if let Some(name) = stem.to_str() {
                                dimensions.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }

        Ok(dimensions)
    }

    /// 删除指定作用域的所有图
    pub async fn delete_all(&self, scope: &GraphScope) -> GraphResult<()> {
        let storage_dir = scope.storage_dir(&self.data_dir);

        // 从缓存中移除
        {
            let mut cache = self.cache.write().await;
            let prefix = match scope {
                GraphScope::ShortTerm {
                    assistant_id,
                    topic_id,
                } => format!("short:{}:{}:", assistant_id, topic_id),
                GraphScope::LongTerm { assistant_id } => format!("long:{}:", assistant_id),
                GraphScope::Global => "global:".to_string(),
            };
            cache.retain(|k, _| !k.starts_with(&prefix));
        }

        // 删除文件
        if storage_dir.exists() {
            fs::remove_dir_all(&storage_dir).await?;
        }

        Ok(())
    }

    /// 删除指定维度的图
    pub async fn delete(&self, scope: &GraphScope, dimension: &str) -> GraphResult<()> {
        let cache_key = scope.cache_key(dimension);

        // 从缓存中移除
        {
            let mut cache = self.cache.write().await;
            cache.remove(&cache_key);
        }

        // 删除文件
        let storage_dir = scope.storage_dir(&self.data_dir);
        let file_path = storage_dir.join(format!("{}.json", dimension));
        if file_path.exists() {
            fs::remove_file(&file_path).await?;
        }

        Ok(())
    }

    /// 清理缓存
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// 持久化所有已修改的图
    pub async fn flush_all(&self) -> GraphResult<()> {
        let cache = self.cache.read().await;
        for graph in cache.values() {
            let graph = graph.read().await;
            graph.save().await?;
        }
        Ok(())
    }

    /// 持久化指定作用域的所有图
    pub async fn flush(&self, scope: &GraphScope) -> GraphResult<()> {
        let prefix = match scope {
            GraphScope::ShortTerm {
                assistant_id,
                topic_id,
            } => format!("short:{}:{}:", assistant_id, topic_id),
            GraphScope::LongTerm { assistant_id } => format!("long:{}:", assistant_id),
            GraphScope::Global => "global:".to_string(),
        };

        let cache = self.cache.read().await;
        for (key, graph) in cache.iter() {
            if key.starts_with(&prefix) {
                let graph = graph.read().await;
                graph.save().await?;
            }
        }
        Ok(())
    }

    // ========================================================================
    // 多图联合查询
    // ========================================================================

    /// 多图联合查询
    pub async fn query_multi(
        &self,
        scope: &GraphScope,
        request: &MultiGraphQuery,
    ) -> GraphResult<MultiGraphResult> {
        let mut all_scores: HashMap<String, ScoredNode> = HashMap::new();
        let mut contributions: HashMap<String, Vec<DimensionContribution>> = HashMap::new();

        // 遍历每个维度
        for (dimension, &dim_weight) in &request.dimension_weights {
            if dim_weight <= 0.0 {
                continue;
            }

            // 获取该维度的图
            let graph = match self.get(scope, dimension).await {
                Ok(g) => g,
                Err(GraphError::NotFound { .. }) => continue,
                Err(e) => return Err(e),
            };

            let graph = graph.read().await;

            // 单图查询
            let single_query = SingleGraphQuery {
                anchors: request.anchors.clone(),
                limit: request.limit * 2, // 多取一些，后面会合并
                min_weight: 0.0,
                direction: QueryDirection::Both,
            };

            let result = graph.query(&single_query);

            // 记录维度贡献
            let mut dim_contributions = Vec::new();

            for node in result.nodes {
                let weighted_score = node.weight * dim_weight;

                dim_contributions.push(DimensionContribution {
                    node_id: node.node_id.clone(),
                    raw_weight: node.weight,
                    weighted_score,
                    reason: node.reason.clone(),
                });

                // 累加到总分
                let entry = all_scores.entry(node.node_id.clone()).or_insert_with(|| {
                    ScoredNode {
                        node_id: node.node_id.clone(),
                        total_score: 0.0,
                        dimension_scores: HashMap::new(),
                        reasons: Vec::new(),
                    }
                });

                entry.total_score += weighted_score;
                entry
                    .dimension_scores
                    .insert(dimension.clone(), node.weight);
                entry.reasons.push(node.reason);
            }

            contributions.insert(dimension.clone(), dim_contributions);
        }

        // 转换为列表并排序
        let mut nodes: Vec<ScoredNode> = all_scores.into_values().collect();
        nodes.sort_by(|a, b| {
            b.total_score
                .partial_cmp(&a.total_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 过滤和限制
        nodes.retain(|n| n.total_score >= request.min_score);
        nodes.truncate(request.limit);

        Ok(MultiGraphResult {
            nodes,
            contributions,
        })
    }
}