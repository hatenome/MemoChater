import { api, ApiError } from './client'

export { api, ApiError }
export { assistantsApi } from './assistants'
export { memoryApi } from './memory'
export { chatApi } from './chat'

// 模型相关
export interface ModelInfo {
  id: string
  object?: string
  owned_by?: string
}

export const modelsApi = {
  // 通过 /api 代理访问后端
  list: () => api.get<ModelInfo[]>('/admin/api/models'),
}

// 处理器相关
export interface ProcessorInfo {
  name: string
  requires_memory: boolean
}

export const processorsApi = {
  // 获取已注册的处理器列表
  list: () => api.get<ProcessorInfo[]>('/admin/api/processors'),
}