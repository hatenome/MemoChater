//! 关系图引擎 - 查询逻辑

use crate::graph::dimension_graph::DimensionGraph;
use crate::graph::types::*;

impl DimensionGraph {
    /// 单图查询
    pub fn query(&self, request: &SingleGraphQuery) -> SingleGraphResult {
        let mut results: Vec<RelatedNode> = Vec::new();

        for anchor in &request.anchors {
            // 根据方向查询
            match request.direction {
                QueryDirection::Forward => {
                    self.collect_forward(anchor, request.min_weight, &mut results);
                }
                QueryDirection::Backward => {
                    self.collect_backward(anchor, request.min_weight, &mut results);
                }
                QueryDirection::Both => {
                    self.collect_forward(anchor, request.min_weight, &mut results);
                    self.collect_backward(anchor, request.min_weight, &mut results);
                }
            }
        }

        // 去重（按node_id），保留权重最高的
        let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let mut deduped: Vec<RelatedNode> = Vec::new();

        for node in results {
            if let Some(&idx) = seen.get(&node.node_id) {
                if node.weight > deduped[idx].weight {
                    deduped[idx] = node;
                }
            } else {
                seen.insert(node.node_id.clone(), deduped.len());
                deduped.push(node);
            }
        }

        // 排除锚点自身
        deduped.retain(|n| !request.anchors.contains(&n.node_id));

        // 按权重降序排序
        deduped.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(std::cmp::Ordering::Equal));

        // 限制数量
        deduped.truncate(request.limit);

        SingleGraphResult { nodes: deduped }
    }

    /// 收集正向邻居
    fn collect_forward(&self, anchor: &str, min_weight: f32, results: &mut Vec<RelatedNode>) {
        for edge in self.edges_from(anchor) {
            if edge.weight >= min_weight {
                results.push(RelatedNode {
                    node_id: edge.target.clone(),
                    weight: edge.weight,
                    reason: edge.reason.clone(),
                    direction: QueryDirection::Forward,
                });
            }
        }
    }

    /// 收集反向邻居
    fn collect_backward(&self, anchor: &str, min_weight: f32, results: &mut Vec<RelatedNode>) {
        for edge in self.edges_to(anchor) {
            if edge.weight >= min_weight {
                results.push(RelatedNode {
                    node_id: edge.source.clone(),
                    weight: edge.weight,
                    reason: edge.reason.clone(),
                    direction: QueryDirection::Backward,
                });
            }
        }
    }

    // ========================================================================
    // 时间维度专用查询
    // ========================================================================

    /// 时间维度：获取时间上的前 N 个节点
    pub fn temporal_before(&self, node_id: &str, limit: usize) -> Vec<&Node> {
        let mut result = Vec::new();
        let mut current = node_id.to_string();

        while result.len() < limit {
            // 反向查找：谁指向 current
            if let Some(edge) = self.edges.iter().find(|e| e.target == current) {
                if let Some(node) = self.get_node(&edge.source) {
                    result.push(node);
                    current = edge.source.clone();
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        result
    }

    /// 时间维度：获取时间上的后 N 个节点
    pub fn temporal_after(&self, node_id: &str, limit: usize) -> Vec<&Node> {
        let mut result = Vec::new();
        let mut current = node_id.to_string();

        while result.len() < limit {
            // 正向查找：current 指向谁
            if let Some(edge) = self.edges.iter().find(|e| e.source == current) {
                if let Some(node) = self.get_node(&edge.target) {
                    result.push(node);
                    current = edge.target.clone();
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        result
    }
}