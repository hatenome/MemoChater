const API_BASE = '/api'

export interface ChatCompletionRequest {
  model: string
  messages: { role: string; content: string }[]
  stream?: boolean
  temperature?: number
  max_tokens?: number
  /** 助手ID（用于记忆隔离） */
  assistant_id?: string
  /** 话题ID（用于会话隔离） */
  topic_id?: string
}

export interface ChatCompletionChoice {
  index: number
  message: { role: string; content: string }
  finish_reason: string
}

export interface ChatCompletionResponse {
  id: string
  object: string
  created: number
  model: string
  choices: ChatCompletionChoice[]
}

export const chatApi = {
  // 非流式对话
  complete: async (request: ChatCompletionRequest): Promise<ChatCompletionResponse> => {
    const response = await fetch(`${API_BASE}/v1/chat/completions`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ ...request, stream: false }),
    })
    
    if (!response.ok) {
      const error = await response.json()
      throw new Error(error.error?.message || 'Chat request failed')
    }
    
    return response.json()
  },

  // 流式对话（返回特殊标记表示预处理/后处理完成）
  stream: async function* (
    request: ChatCompletionRequest
  ): AsyncGenerator<string | { type: 'pre_process_done' } | { type: 'post_process_done' }, void, unknown> {
    const response = await fetch(`${API_BASE}/v1/chat/completions`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ ...request, stream: true }),
    })

    if (!response.ok) {
      const error = await response.json()
      throw new Error(error.error?.message || 'Chat request failed')
    }

    const reader = response.body?.getReader()
    if (!reader) throw new Error('No response body')

    const decoder = new TextDecoder()
    let buffer = ''

    while (true) {
      const { done, value } = await reader.read()
      if (done) break

      buffer += decoder.decode(value, { stream: true })
      const lines = buffer.split('\n')
      buffer = lines.pop() || ''

      for (const line of lines) {
        const trimmed = line.trim()
        if (!trimmed || !trimmed.startsWith('data: ')) continue
        
        const data = trimmed.slice(6)
        
        if (data === '[DONE]') continue  // 继续等待后处理完成
        
        if (data === '[PRE_PROCESS_DONE]') {
          // 预处理完成标记（可以获取 last_request_messages 了）
          yield { type: 'pre_process_done' }
          continue
        }

        if (data === '[POST_PROCESS_DONE]') {
          // 后处理完成标记
          yield { type: 'post_process_done' }
          return
        }

        try {
          const parsed = JSON.parse(data)
          const content = parsed.choices?.[0]?.delta?.content
          if (content) yield content
        } catch {
          // 忽略解析错误
        }
      }
    }
  },
}