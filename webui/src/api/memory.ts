import { api } from './client'
import type { Memory, MemorySearchResult, PendingStatus } from '@/types'

export const memoryApi = {
  // 记忆搜索（按助手隔离）
  search: (assistantId: string, query: string, category?: string, limit = 100) => {
    const params = new URLSearchParams()
    if (query) params.set('query', query)
    if (category) params.set('category', category)
    params.set('limit', limit.toString())
    return api.get<{ memories: MemorySearchResult[]; total: number }>(
      `/assistants/${assistantId}/memories?${params}`
    )
  },

  // 获取单条记忆
  get: (assistantId: string, id: string) => api.get<Memory>(`/assistants/${assistantId}/memories/${id}`),

  // 创建记忆
  create: (assistantId: string, data: {
    content: string
    category: string
    importance: number
    tags: string[]
  }) => api.post<{ id: string }>(`/assistants/${assistantId}/memories`, data),

  // 删除记忆
  delete: (assistantId: string, id: string) => api.delete<void>(`/assistants/${assistantId}/memories/${id}`),

  // 待处理池
  getPending: (assistantId: string) => api.get<PendingStatus>(`/assistants/${assistantId}/pending`),
  
  processPending: (assistantId: string) => api.post<{
    processed: number
    failed: number
    remaining: number
  }>(`/assistants/${assistantId}/pending/process`, {}),
  
  clearPending: (assistantId: string) => api.delete<string>(`/assistants/${assistantId}/pending`),

  // 记忆提取（保持全局，因为可能需要指定助手）
  extract: (messages: { role: string; content: string }[], autoStore = false) =>
    api.post<{
      extracted_memories: string[]
      stored_count: number | null
      raw_response: string
    }>('/admin/api/extract', { messages, auto_store: autoStore }),
}