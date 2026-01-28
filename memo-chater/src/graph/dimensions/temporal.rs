//! 时间维度处理器
//!
//! 单链有向图结构，按时间顺序连接相邻记忆。
//! 边方向：旧 → 新

use crate::graph::error::DimensionError;
use crate::graph::processor::{ComputeContext, DimensionProcessor};
use crate::graph::types::{EdgeCandidate, Node};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

// ============================================================================
// G2-02: 时间权重函数（Sigmoid 衰减）
// ============================================================================

/// 计算时间邻近权重
///
/// 使用变形的 Sigmoid 函数：weight = 2 / (1 + e^(k * Δt))
/// - Δt = 0 时，weight = 1.0
/// - Δt → ∞ 时，weight → 0.0
/// - k 控制衰减速度
///
/// # 参数
/// - `time_diff_seconds`: 时间差（秒）
/// - `half_life_hours`: 半衰期（小时），即权重降到0.5时的时间差
pub fn compute_temporal_weight(time_diff_seconds: f64, half_life_hours: f64) -> f32 {
    if time_diff_seconds <= 0.0 {
        return 1.0;
    }

    let half_life_seconds = half_life_hours * 3600.0;
    // k = ln(3) / half_life，使得 Δt = half_life 时 weight = 0.5
    let k = 3.0_f64.ln() / half_life_seconds;
    let weight = 2.0 / (1.0 + (k * time_diff_seconds).exp());
    weight as f32
}

/// 格式化时间差为可读字符串
pub fn format_time_diff(seconds: f64) -> String {
    if seconds < 60.0 {
        format!("时间距离: {:.0}秒", seconds)
    } else if seconds < 3600.0 {
        format!("时间距离: {:.0}分钟", seconds / 60.0)
    } else if seconds < 86400.0 {
        format!("时间距离: {:.1}小时", seconds / 3600.0)
    } else {
        format!("时间距离: {:.1}天", seconds / 86400.0)
    }
}

// ============================================================================
// G2-01: TemporalProcessor
// ============================================================================

/// 时间维度处理器
pub struct TemporalProcessor {
    /// 半衰期（小时）
    pub half_life_hours: f64,
}

impl Default for TemporalProcessor {
    fn default() -> Self {
        Self {
            half_life_hours: 24.0,
        }
    }
}

impl TemporalProcessor {
    /// 创建指定半衰期的处理器
    pub fn new(half_life_hours: f64) -> Self {
        Self { half_life_hours }
    }

    /// 从配置参数中获取半衰期
    pub fn from_config(config: &crate::graph::types::DimensionConfig) -> Self {
        let half_life = config
            .params
            .get("half_life_hours")
            .and_then(|v| v.as_f64())
            .unwrap_or(24.0);
        Self::new(half_life)
    }
}

#[async_trait]
impl DimensionProcessor for TemporalProcessor {
    fn dimension(&self) -> &'static str {
        "temporal"
    }

    fn description(&self) -> &'static str {
        "时间邻近关系 - 单链有向图，按时间顺序连接相邻记忆"
    }

    /// 计算新节点与现有节点的边
    ///
    /// 时间维度特殊：只与时间上最近的前驱和后继建边
    async fn compute_edges(
        &self,
        new_node: &Node,
        existing_nodes: &[Node],
        _ctx: &ComputeContext,
    ) -> Result<Vec<EdgeCandidate>, DimensionError> {
        let new_ts = new_node
            .features
            .timestamp
            .ok_or(DimensionError::MissingFeature("timestamp"))?;

        // 找到时间上的前驱和后继
        let (predecessor, successor) = find_temporal_neighbors(new_ts, existing_nodes);

        let mut edges = Vec::new();

        // 前驱 → 新节点（边从 predecessor 指向 new_node）
        if let Some((pred_id, pred_ts)) = predecessor {
            let diff = (new_ts - pred_ts).num_seconds().abs() as f64;
            let weight = compute_temporal_weight(diff, self.half_life_hours);

            edges.push(EdgeCandidate {
                target_id: new_node.id.clone(), // 边指向新节点
                weight,
                reason: format_time_diff(diff),
                bidirectional: false,
            });

            // 记录前驱ID，用于后续处理
            edges.push(EdgeCandidate {
                target_id: format!("__pred__:{}", pred_id), // 特殊标记
                weight: 0.0,
                reason: String::new(),
                bidirectional: false,
            });
        }

        // 新节点 → 后继（边从 new_node 指向 successor）
        if let Some((succ_id, succ_ts)) = successor {
            let diff = (succ_ts - new_ts).num_seconds().abs() as f64;
            let weight = compute_temporal_weight(diff, self.half_life_hours);

            edges.push(EdgeCandidate {
                target_id: succ_id,
                weight,
                reason: format_time_diff(diff),
                bidirectional: false,
            });
        }

        Ok(edges)
    }

    fn default_query_weight(&self) -> f32 {
        0.3 // 时间维度默认权重较低
    }
}

// ============================================================================
// G2-03: 辅助函数
// ============================================================================

/// 找到时间上的前驱和后继
///
/// 返回 (前驱(id, timestamp), 后继(id, timestamp))
fn find_temporal_neighbors(
    new_ts: DateTime<Utc>,
    existing_nodes: &[Node],
) -> (Option<(String, DateTime<Utc>)>, Option<(String, DateTime<Utc>)>) {
    let mut predecessor: Option<(String, DateTime<Utc>)> = None;
    let mut successor: Option<(String, DateTime<Utc>)> = None;

    for node in existing_nodes {
        if let Some(ts) = node.features.timestamp {
            if ts < new_ts {
                // 候选前驱：取最接近的（时间最大的）
                if predecessor.is_none() || ts > predecessor.as_ref().unwrap().1 {
                    predecessor = Some((node.id.clone(), ts));
                }
            } else if ts > new_ts {
                // 候选后继：取最接近的（时间最小的）
                if successor.is_none() || ts < successor.as_ref().unwrap().1 {
                    successor = Some((node.id.clone(), ts));
                }
            }
            // ts == new_ts 的情况忽略（同一时刻的记忆）
        }
    }

    (predecessor, successor)
}

// ============================================================================
// G2-03: DimensionGraph 时间维度专用方法
// ============================================================================

use crate::graph::dimension_graph::DimensionGraph;
use crate::graph::error::GraphError;
use crate::graph::types::Edge;

impl DimensionGraph {
    /// 时间维度专用：按时间顺序插入节点
    ///
    /// 自动处理单链结构：
    /// 1. 找到前驱和后继
    /// 2. 删除前驱→后继的旧边
    /// 3. 创建前驱→新节点、新节点→后继的新边
    pub async fn insert_temporal_node(
        &mut self,
        node: Node,
        processor: &TemporalProcessor,
        _ctx: &ComputeContext,
    ) -> Result<usize, GraphError> {
        let new_ts = node
            .features
            .timestamp
            .ok_or(GraphError::DimensionError(DimensionError::MissingFeature(
                "timestamp",
            )))?;

        // 找前驱后继
        let (pred_id, succ_id) = self.find_temporal_neighbors_internal(&new_ts);

        // 删除旧边（前驱→后继）
        if let (Some(ref p), Some(ref s)) = (&pred_id, &succ_id) {
            self.remove_edge(p, s);
        }

        // 添加节点
        let node_id = node.id.clone();
        self.add_node(node);

        // 添加边
        let mut edge_count = 0;

        // 前驱 → 新节点
        if let Some(ref pred) = pred_id {
            if let Some(pred_node) = self.get_node(pred) {
                if let Some(pred_ts) = pred_node.features.timestamp {
                    let diff = (new_ts - pred_ts).num_seconds().abs() as f64;
                    let weight = compute_temporal_weight(diff, processor.half_life_hours);
                    let reason = format_time_diff(diff);
                    self.add_edge(Edge::new(pred.clone(), node_id.clone(), weight, reason));
                    edge_count += 1;
                }
            }
        }

        // 新节点 → 后继
        if let Some(ref succ) = succ_id {
            if let Some(succ_node) = self.get_node(succ) {
                if let Some(succ_ts) = succ_node.features.timestamp {
                    let diff = (succ_ts - new_ts).num_seconds().abs() as f64;
                    let weight = compute_temporal_weight(diff, processor.half_life_hours);
                    let reason = format_time_diff(diff);
                    self.add_edge(Edge::new(node_id.clone(), succ.clone(), weight, reason));
                    edge_count += 1;
                }
            }
        }

        Ok(edge_count)
    }

    /// 内部方法：找时间上的前驱和后继ID
    fn find_temporal_neighbors_internal(
        &self,
        ts: &DateTime<Utc>,
    ) -> (Option<String>, Option<String>) {
        let mut pred: Option<(String, DateTime<Utc>)> = None;
        let mut succ: Option<(String, DateTime<Utc>)> = None;

        for node in &self.nodes {
            if let Some(node_ts) = node.features.timestamp {
                if node_ts < *ts {
                    if pred.is_none() || node_ts > pred.as_ref().unwrap().1 {
                        pred = Some((node.id.clone(), node_ts));
                    }
                } else if node_ts > *ts {
                    if succ.is_none() || node_ts < succ.as_ref().unwrap().1 {
                        succ = Some((node.id.clone(), node_ts));
                    }
                }
            }
        }

        (pred.map(|p| p.0), succ.map(|s| s.0))
    }
}