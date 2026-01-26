// 助手相关类型
export interface AssistantConfig {
  name: string
  description: string
  system_prompt: string
  model: ModelConfig
  roles: RolesConfig
  memory: MemoryConfig
  pipeline?: PipelineConfig
  created_at: string
  updated_at: string
}

export interface ModelConfig {
  main_model: string
  processor_model: string
  embedding_model: string
  extractor_model: string
  temperature: number
  max_tokens: number
}

export interface RolesConfig {
  user_name: string
  assistant_name: string
}

export interface MemoryConfig {
  enabled: boolean
  retrieval_count: number
  relevance_threshold: number
}

// 处理器条目
export interface ProcessorEntry {
  name: string
  description: string
}

// 流水线配置
export interface PipelineConfig {
  on_user_message: ProcessorEntry[]
  before_ai_call: ProcessorEntry[]
  on_stream_start: ProcessorEntry[]
  on_stream_chunk: ProcessorEntry[]
  after_ai_response: ProcessorEntry[]
  background_process: ProcessorEntry[]
}

export interface AssistantSummary {
  id: string
  name: string
  description: string
  topic_count: number
  created_at: string
  updated_at: string
}

// 话题类型
export type TopicType = 'normal' | 'memory'

// 话题相关类型
export interface TopicMeta {
  id: string
  title: string
  topic_type: TopicType
  created_at: string
  updated_at: string
  message_count?: number
}

export interface TopicSummary {
  id: string
  assistant_id: string
  title: string
  topic_type: TopicType
  message_count: number
  created_at: string
  updated_at: string
}

// 消息类型
export interface ChatMessage {
  role: 'user' | 'assistant' | 'system'
  content: string
}

// 记忆类型
export interface Memory {
  id: string
  content: string
  category: string
  importance: number
  tags: string[]
  access_count: number
  created_at: string
}

export interface MemorySearchResult {
  memory: Memory
  score: number
}

// API响应类型
export interface ApiResponse<T> {
  success: boolean
  data?: T
  error?: string
}

// 待处理记忆
export interface PendingMemory {
  content: string
  category: string
  importance: number
  source_session: string
  created_at: string
}

export interface PendingStatus {
  pending_count: number
  preview: PendingMemory[]
}

// 思考池条目
export interface ThinkingEntry {
  content: string
  source: string
  timestamp: string
}

// 短期记忆条目
export interface ShortTermMemoryEntry {
  id: string
  /** 概述/标题 */
  summary: string
  /** 详细内容 */
  content: string
  /** 记忆类型（如 fact/event/preference/knowledge/task/other） */
  memory_type: string
  relevance: number
  /** 置信度 0.0 - 1.0（预留字段） */
  confidence: number
  /** 是否需要展开（前端控制） */
  should_expand: boolean
  source: string
  /** 创建时间 */
  timestamp: string
}

// 对话轮次
export interface ConversationTurn {
  user_message: string
  assistant_message: string
  timestamp: string
}

// Packet 记忆数据
export interface PacketMemoryData {
  messages: ChatMessage[]
  thinking_pool: ThinkingEntry[]
  short_term_memory: ShortTermMemoryEntry[]
  conversation_turns: ConversationTurn[]
  /** 最终发送给 AI 的 messages（调试用） */
  last_request_messages: ChatMessage[]
}

// ============ 对话记忆库（向量库）相关类型 ============

/** 向量记忆条目 - 对应 short_term_vectors.json 中的数据 */
export interface VectorMemoryEntry {
  /** 唯一标识符 */
  id: string
  /** 概述/标题 */
  summary: string
  /** 详细内容 */
  content: string
  /** 记忆类型 */
  memory_type: string
  /** 置信度 0.0 - 1.0 */
  confidence: number
  /** 来源 */
  source: string
  /** 创建时间 */
  timestamp: string
  /** 向量嵌入（可选，前端通常不需要） */
  embedding?: number[]
}

/** 向量搜索结果 */
export interface VectorSearchResult {
  /** 记忆条目 */
  entry: VectorMemoryEntry
  /** 相似度分数 0.0 - 1.0 */
  score: number
}

/** 向量记忆库列表响应 */
export interface VectorMemoryListResponse {
  /** 记忆条目列表 */
  entries: VectorMemoryEntry[]
  /** 总数 */
  total: number
}

/** 向量搜索响应 */
export interface VectorSearchResponse {
  /** 搜索结果 */
  results: VectorSearchResult[]
  /** 查询文本 */
  query: string
}