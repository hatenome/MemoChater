import { api } from './client'
import type { AssistantSummary, AssistantConfig, TopicSummary, TopicMeta, TopicType, ChatMessage, PacketMemoryData, ThinkingEntry, ShortTermMemoryEntry } from '@/types'

export const assistantsApi = {
  // 助手管理
  list: () => api.get<AssistantSummary[]>('/assistants'),
  
  get: (id: string) => api.get<AssistantConfig>(`/assistants/${id}`),
  
  create: (data: Partial<AssistantConfig>) => 
    api.post<{ id: string }>('/assistants', data),
  
  update: (id: string, data: Partial<AssistantConfig>) =>
    api.put<void>(`/assistants/${id}`, data),
  
  delete: (id: string) => api.delete<void>(`/assistants/${id}`),

  // 话题管理
  listTopics: (assistantId: string) =>
    api.get<TopicSummary[]>(`/assistants/${assistantId}/topics`),
  
  getTopic: (assistantId: string, topicId: string) =>
    api.get<TopicMeta>(`/assistants/${assistantId}/topics/${topicId}`),
  
  createTopic: (assistantId: string, title: string, topicType: TopicType = 'normal') =>
    api.post<TopicMeta>(`/assistants/${assistantId}/topics`, { title, topic_type: topicType }),
  
  updateTopic: (assistantId: string, topicId: string, data: { title?: string }) =>
    api.put<void>(`/assistants/${assistantId}/topics/${topicId}`, data),
  
  deleteTopic: (assistantId: string, topicId: string) =>
    api.delete<void>(`/assistants/${assistantId}/topics/${topicId}`),

  // 对话历史
  getHistory: (assistantId: string, topicId: string) =>
    api.get<ChatMessage[]>(`/assistants/${assistantId}/topics/${topicId}/history`),
  
  clearHistory: (assistantId: string, topicId: string) =>
    api.delete<void>(`/assistants/${assistantId}/topics/${topicId}/history`),

  // 消息操作
  updateMessage: (assistantId: string, topicId: string, index: number, content: string) =>
    api.put<void>(`/assistants/${assistantId}/topics/${topicId}/messages/${index}`, { content }),
  
  deleteMessage: (assistantId: string, topicId: string, index: number) =>
    api.delete<void>(`/assistants/${assistantId}/topics/${topicId}/messages/${index}`),
  
  createBranch: (assistantId: string, topicId: string, fromIndex: number, title?: string) =>
    api.post<TopicMeta>(`/assistants/${assistantId}/topics/${topicId}/branch`, { from_index: fromIndex, title }),

  // Packet 记忆池 API
  getPacketMemory: (assistantId: string, topicId: string) =>
    api.get<PacketMemoryData>(`/assistants/${assistantId}/topics/${topicId}/packet`),
  
  updateThinkingPool: (assistantId: string, topicId: string, thinkingPool: ThinkingEntry[]) =>
    api.put<PacketMemoryData>(`/assistants/${assistantId}/topics/${topicId}/packet/thinking`, { thinking_pool: thinkingPool }),
  
  updateShortTermMemory: (assistantId: string, topicId: string, shortTermMemory: ShortTermMemoryEntry[]) =>
    api.put<PacketMemoryData>(`/assistants/${assistantId}/topics/${topicId}/packet/short-term`, { short_term_memory: shortTermMemory }),
}