//! 待处理记忆存储

use crate::types::PendingMemory;
use std::collections::VecDeque;

/// 待处理记忆存储（等待写入长期记忆）
#[derive(Debug, Default)]
pub struct PendingMemoryStore {
    queue: VecDeque<PendingMemory>,
}

impl PendingMemoryStore {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    /// 添加待处理记忆
    pub async fn add(&mut self, memory: PendingMemory) {
        self.queue.push_back(memory);
    }

    /// 取出一批待处理记忆
    pub async fn take_batch(&mut self, count: usize) -> Vec<PendingMemory> {
        let mut batch = Vec::with_capacity(count);
        for _ in 0..count {
            if let Some(memory) = self.queue.pop_front() {
                batch.push(memory);
            } else {
                break;
            }
        }
        batch
    }

    /// 队列长度
    pub async fn len(&self) -> usize {
        self.queue.len()
    }

    /// 是否为空
    pub async fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// 预览队列中的记忆（不取出）
    pub async fn peek(&self, count: usize) -> Vec<&PendingMemory> {
        self.queue.iter().take(count).collect()
    }
}