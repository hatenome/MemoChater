<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useAssistantStore, useAppStore } from '@/stores'
import { chatApi, assistantsApi } from '@/api'
import ChatMessage from '@/components/ChatMessage.vue'
import ChatInput from '@/components/ChatInput.vue'
import type { ThinkingEntry, ShortTermMemoryEntry, ConversationTurn } from '@/types'

const route = useRoute()
const store = useAssistantStore()
const app = useAppStore()

const messagesContainer = ref<HTMLElement>()
const isStreaming = ref(false)
const streamingContent = ref('')

// 上一次请求的 messages（从 packet 获取，用于调试查看）
const lastRequestMessages = ref<Array<{role: string, content: string}>>([])
const showRequestModal = ref(false)

// 记忆池状态
const thinkingPool = ref<ThinkingEntry[]>([])
const shortTermMemory = ref<ShortTermMemoryEntry[]>([])
const conversationTurns = ref<ConversationTurn[]>([])
const isLoadingMemory = ref(false)
const isSavingThinking = ref(false)
const isSavingShortTerm = ref(false)

// 编辑状态
const editingThinkingIndex = ref<number | null>(null)
const editingShortTermIndex = ref<number | null>(null)
const editThinkingContent = ref('')
// 短期记忆编辑字段
const editShortTermSummary = ref('')
const editShortTermContent = ref('')
const editShortTermType = ref('')
const editShortTermConfidence = ref(1.0)
const editShortTermTimestamp = ref('')

const hasContext = computed(() => store.currentAssistantId && store.currentTopicId)

// 判断当前话题是否为记忆话题
const isMemoryTopic = computed(() => store.currentTopic?.topic_type === 'memory')

// 截断文本（用于预览）
function truncateText(text: string, maxLength: number = 100): string {
  if (!text || text.length <= maxLength) return text
  return text.slice(0, maxLength) + '...'
}

// 格式化时间显示
function formatTime(timestamp: string): string {
  try {
    const date = new Date(timestamp)
    return date.toLocaleString('zh-CN', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit'
    })
  } catch {
    return timestamp
  }
}

// 滚动到底部
function scrollToBottom() {
  nextTick(() => {
    if (messagesContainer.value) {
      messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight
    }
  })
}

// 监听消息变化自动滚动
watch(() => store.messages.length, scrollToBottom)

// 监听话题变化，加载记忆池
watch(
  () => [store.currentAssistantId, store.currentTopicId],
  async ([assistantId, topicId]) => {
    if (assistantId && topicId) {
      await loadPacketMemory()
    } else {
      thinkingPool.value = []
      shortTermMemory.value = []
      conversationTurns.value = []
    }
  },
  { immediate: true }
)

// 加载记忆池数据（记忆话题同时更新消息列表）
async function loadPacketMemory() {
  if (!store.currentAssistantId || !store.currentTopicId) return
  
  isLoadingMemory.value = true
  try {
    const data = await assistantsApi.getPacketMemory(
      store.currentAssistantId,
      store.currentTopicId
    )
    thinkingPool.value = data.thinking_pool || []
    shortTermMemory.value = data.short_term_memory || []
    conversationTurns.value = data.conversation_turns || []
    lastRequestMessages.value = data.last_request_messages || []
    console.log('[loadPacketMemory] 加载完成，last_request_messages:', data.last_request_messages?.length || 0)
    
    // 记忆话题：同步更新消息列表（因为后处理器可能清空了 messages）
    if (isMemoryTopic.value && data.messages) {
      store.messages.splice(0, store.messages.length, ...data.messages)
    }
  } catch (e) {
    console.error('加载记忆池失败:', e)
  } finally {
    isLoadingMemory.value = false
  }
}

// 保存思考池
async function saveThinkingPool() {
  if (!store.currentAssistantId || !store.currentTopicId) return
  
  isSavingThinking.value = true
  try {
    const data = await assistantsApi.updateThinkingPool(
      store.currentAssistantId,
      store.currentTopicId,
      thinkingPool.value
    )
    thinkingPool.value = data.thinking_pool || []
    app.showToast('思考池已保存', 'success')
  } catch (e) {
    app.showToast('保存思考池失败', 'error')
  } finally {
    isSavingThinking.value = false
  }
}

// 保存短期记忆
async function saveShortTermMemory() {
  if (!store.currentAssistantId || !store.currentTopicId) return
  
  isSavingShortTerm.value = true
  try {
    const data = await assistantsApi.updateShortTermMemory(
      store.currentAssistantId,
      store.currentTopicId,
      shortTermMemory.value
    )
    shortTermMemory.value = data.short_term_memory || []
    app.showToast('短期记忆已保存', 'success')
  } catch (e) {
    app.showToast('保存短期记忆失败', 'error')
  } finally {
    isSavingShortTerm.value = false
  }
}

// 开始编辑思考条目
function startEditThinking(index: number) {
  editingThinkingIndex.value = index
  editThinkingContent.value = thinkingPool.value[index].content
}

// 保存编辑的思考条目
function saveEditThinking() {
  if (editingThinkingIndex.value !== null) {
    thinkingPool.value[editingThinkingIndex.value].content = editThinkingContent.value
    editingThinkingIndex.value = null
    saveThinkingPool()
  }
}

// 取消编辑思考条目
function cancelEditThinking() {
  editingThinkingIndex.value = null
  editThinkingContent.value = ''
}

// 删除思考条目
function deleteThinking(index: number) {
  thinkingPool.value.splice(index, 1)
  saveThinkingPool()
}

// 添加新思考条目
function addThinking() {
  thinkingPool.value.push({
    content: '新的思考...',
    source: 'UserAnalysis',
    timestamp: new Date().toISOString()
  })
  startEditThinking(thinkingPool.value.length - 1)
}

// 开始编辑短期记忆
function startEditShortTerm(index: number) {
  editingShortTermIndex.value = index
  const mem = shortTermMemory.value[index]
  editShortTermSummary.value = mem.summary
  editShortTermContent.value = mem.content
  editShortTermType.value = mem.memory_type
  editShortTermConfidence.value = mem.confidence
  // 转换为 datetime-local 格式 (YYYY-MM-DDTHH:mm)
  editShortTermTimestamp.value = mem.timestamp.slice(0, 16)
}

// 保存编辑的短期记忆
function saveEditShortTerm() {
  if (editingShortTermIndex.value !== null) {
    const mem = shortTermMemory.value[editingShortTermIndex.value]
    mem.summary = editShortTermSummary.value
    mem.content = editShortTermContent.value
    mem.memory_type = editShortTermType.value
    mem.confidence = editShortTermConfidence.value
    mem.timestamp = editShortTermTimestamp.value
    editingShortTermIndex.value = null
    saveShortTermMemory()
  }
}

// 取消编辑短期记忆
function cancelEditShortTerm() {
  editingShortTermIndex.value = null
  editShortTermSummary.value = ''
  editShortTermContent.value = ''
  editShortTermType.value = ''
}

// 删除短期记忆
function deleteShortTerm(index: number) {
  shortTermMemory.value.splice(index, 1)
  saveShortTermMemory()
}

// 添加新短期记忆
function addShortTerm() {
  shortTermMemory.value.push({
    id: `mem_${Date.now()}`,
    summary: '新记忆',
    content: '详细内容...',
    memory_type: 'other',
    relevance: 0.5,
    confidence: 1.0,
    should_expand: false,
    source: 'CurrentConversation',
    timestamp: new Date().toISOString()
  })
  startEditShortTerm(shortTermMemory.value.length - 1)
}

// 切换短期记忆的展开标记
function toggleShouldExpand(index: number) {
  shortTermMemory.value[index].should_expand = !shortTermMemory.value[index].should_expand
  saveShortTermMemory()
}

async function sendMessage(content: string) {
  if (!store.currentAssistantConfig || !store.currentTopicId) {
    app.showToast('请先选择助手和话题', 'error')
    return
  }

  // 添加用户消息
  store.addMessage({ role: 'user', content })
  
  // 准备请求
  const messages = store.messages.map(m => ({
    role: m.role,
    content: m.content,
  }))

  // 如果有system_prompt，添加到开头
  if (store.currentAssistantConfig.system_prompt) {
    messages.unshift({
      role: 'system',
      content: store.currentAssistantConfig.system_prompt,
    })
  }

  // 添加空的助手消息用于流式显示
  store.addMessage({ role: 'assistant', content: '' })
  isStreaming.value = true
  streamingContent.value = ''

  try {
    const stream = chatApi.stream({
      model: store.currentAssistantConfig.model.main_model,
      messages,
      temperature: store.currentAssistantConfig.model.temperature,
      max_tokens: store.currentAssistantConfig.model.max_tokens,
      assistant_id: store.currentAssistantId!,
      topic_id: store.currentTopicId!,
    })

    for await (const chunk of stream) {
      // 检查是否是预处理完成标记
      if (typeof chunk === 'object' && chunk.type === 'pre_process_done') {
        // 预处理完成，可以获取 last_request_messages 了
        console.log('[ChatView] 收到 pre_process_done，开始加载 packet')
        await loadPacketMemory()
        console.log('[ChatView] packet 加载完成，lastRequestMessages:', lastRequestMessages.value.length)
        continue
      }
      
      // 检查是否是后处理完成标记
      if (typeof chunk === 'object' && chunk.type === 'post_process_done') {
        // 后处理完成，刷新数据
        await loadPacketMemory()
        continue
      }
      
      // 普通内容
      streamingContent.value += chunk
      store.updateLastMessage(streamingContent.value)
      scrollToBottom()
    }
  } catch (e) {
    const error = e as Error
    app.showToast(`发送失败: ${error.message}`, 'error')
    // 移除失败的助手消息
    store.messages.pop()
  } finally {
    isStreaming.value = false
    streamingContent.value = ''
  }
}

// 编辑消息
async function handleEditMessage(index: number, content: string) {
  await store.editMessage(index, content)
}

// 删除消息
async function handleDeleteMessage(index: number) {
  await store.removeMessage(index)
}

// 从消息创建分支
async function handleBranchFromMessage(index: number) {
  const title = prompt('请输入分支话题标题（留空自动生成）：')
  const topicId = await store.createBranchFromMessage(index, title || undefined)
  if (topicId) {
    // 可选：自动切换到新话题
    // await store.selectTopic(topicId)
  }
}

// 重新生成助手回复
async function handleRegenerate(index: number) {
  if (!store.currentAssistantConfig || !store.currentTopicId) {
    app.showToast('请先选择助手和话题', 'error')
    return
  }

  // 找到这条助手消息之前的用户消息
  // 删除当前助手消息，然后重新生成
  const messages = store.messages.slice(0, index)
  
  if (messages.length === 0) {
    app.showToast('没有可用的上下文', 'error')
    return
  }

  // 删除当前及之后的所有消息
  while (store.messages.length > index) {
    store.messages.pop()
  }

  // 准备请求消息
  const requestMessages = messages.map(m => ({
    role: m.role,
    content: m.content,
  }))

  // 如果有system_prompt，添加到开头
  if (store.currentAssistantConfig.system_prompt) {
    requestMessages.unshift({
      role: 'system',
      content: store.currentAssistantConfig.system_prompt,
    })
  }

  // 添加空的助手消息用于流式显示
  store.addMessage({ role: 'assistant', content: '' })
  isStreaming.value = true
  streamingContent.value = ''

  try {
    const stream = chatApi.stream({
      model: store.currentAssistantConfig.model.main_model,
      messages: requestMessages,
      temperature: store.currentAssistantConfig.model.temperature,
      max_tokens: store.currentAssistantConfig.model.max_tokens,
      assistant_id: store.currentAssistantId!,
      topic_id: store.currentTopicId!,
    })

    for await (const chunk of stream) {
      // 检查是否是预处理完成标记
      if (typeof chunk === 'object' && chunk.type === 'pre_process_done') {
        // 预处理完成，可以获取 last_request_messages 了
        await loadPacketMemory()
        continue
      }
      
      // 检查是否是后处理完成标记
      if (typeof chunk === 'object' && chunk.type === 'post_process_done') {
        // 后处理完成，刷新数据
        await loadPacketMemory()
        continue
      }
      
      // 普通内容
      streamingContent.value += chunk
      store.updateLastMessage(streamingContent.value)
      scrollToBottom()
    }
  } catch (e) {
    const error = e as Error
    app.showToast(`重新生成失败: ${error.message}`, 'error')
    // 移除失败的助手消息
    store.messages.pop()
  } finally {
    isStreaming.value = false
    streamingContent.value = ''
  }
}
</script>

<template>
  <div class="flex h-full">
    <!-- 主对话区域 -->
    <div class="flex flex-col flex-1 min-w-0">
      <!-- 头部 -->
      <header class="flex items-center justify-between px-6 py-4 border-b border-dark-700 bg-dark-900">
        <div>
          <h1 class="text-lg font-semibold">
            {{ store.currentAssistant?.name || '选择助手开始对话' }}
          </h1>
          <p v-if="store.currentTopic" class="text-sm text-dark-400">
            {{ store.currentTopic.title }}
          </p>
        </div>
        <div class="flex items-center gap-3">
          <button
            v-if="lastRequestMessages.length > 0"
            @click="showRequestModal = true"
            class="text-xs px-2 py-1 bg-dark-700 hover:bg-dark-600 rounded text-dark-400 hover:text-dark-200 transition-colors"
            title="查看上次请求"
          >
            📋 请求体
          </button>
          <div v-if="store.currentAssistantConfig" class="text-sm text-dark-500">
            模型: {{ store.currentAssistantConfig.model.main_model }}
          </div>
        </div>
      </header>

      <!-- 消息区域 -->
      <div 
        ref="messagesContainer"
        class="flex-1 overflow-y-auto"
      >
        <!-- 空状态 -->
        <div 
          v-if="!hasContext" 
          class="flex flex-col items-center justify-center h-full text-dark-500"
        >
          <div class="text-6xl mb-4">🧠</div>
          <h2 class="text-xl font-medium mb-2">欢迎使用 MemoChater</h2>
          <p class="text-sm">从左侧选择一个助手和话题开始对话</p>
        </div>

        <!-- 无消息状态 -->
        <div 
          v-else-if="store.messages.length === 0" 
          class="flex flex-col items-center justify-center h-full text-dark-500"
        >
          <div class="text-4xl mb-4">💬</div>
          <p>开始新的对话吧</p>
        </div>

        <!-- 消息列表 -->
        <div v-else class="max-w-4xl mx-auto">
          <ChatMessage
            v-for="(msg, index) in store.messages"
            :key="index"
            :message="msg"
            :index="index"
            :assistant-name="store.currentAssistantConfig?.roles.assistant_name"
            :user-name="store.currentAssistantConfig?.roles.user_name"
            @edit="handleEditMessage"
            @delete="handleDeleteMessage"
            @branch="handleBranchFromMessage"
            @regenerate="handleRegenerate"
          />

          <!-- 上一轮对话（仅记忆话题显示） -->
          <div 
            v-if="isMemoryTopic && conversationTurns.length > 0"
            class="mt-6 border border-dark-600 rounded-lg p-4 bg-dark-800/50"
          >
            <div class="text-xs text-dark-400 mb-3 flex items-center gap-2">
              <span class="w-2 h-2 bg-green-500 rounded-full"></span>
              上一轮对话 ({{ formatTime(conversationTurns[conversationTurns.length - 1].timestamp) }})
            </div>
            <!-- 用户消息 -->
            <div class="mb-3">
              <div class="text-xs text-blue-400 mb-1">{{ store.currentAssistantConfig?.roles.user_name || '用户' }}</div>
              <div class="text-sm text-dark-200 whitespace-pre-wrap">{{ conversationTurns[conversationTurns.length - 1].user_message }}</div>
            </div>
            <!-- AI回复 -->
            <div>
              <div class="text-xs text-purple-400 mb-1">{{ store.currentAssistantConfig?.roles.assistant_name || '助手' }}</div>
              <div class="text-sm text-dark-300 whitespace-pre-wrap">{{ conversationTurns[conversationTurns.length - 1].assistant_message }}</div>
            </div>
          </div>
        </div>
      </div>

      <!-- 输入区域 -->
      <ChatInput
        v-if="hasContext"
        :disabled="!hasContext"
        :loading="isStreaming"
        @send="sendMessage"
      />
    </div>

    <!-- 右侧记忆池面板（仅记忆话题显示） -->
    <div 
      v-if="hasContext && isMemoryTopic"
      class="w-80 border-l border-dark-700 bg-dark-900 flex flex-col"
    >
      <!-- 思考池 -->
      <div class="flex-1 flex flex-col border-b border-dark-700 min-h-0">
        <div class="flex items-center justify-between px-4 py-3 border-b border-dark-700">
          <h3 class="text-sm font-medium text-primary-400">💭 思考池</h3>
          <button 
            @click="addThinking"
            class="text-xs px-2 py-1 bg-dark-700 hover:bg-dark-600 rounded"
          >
            + 添加
          </button>
        </div>
        <div class="flex-1 overflow-y-auto p-3 space-y-2">
          <div v-if="isLoadingMemory" class="text-center text-dark-500 text-sm py-4">
            加载中...
          </div>
          <div v-else-if="thinkingPool.length === 0" class="text-center text-dark-500 text-sm py-4">
            暂无思考内容
          </div>
          <div 
            v-else
            v-for="(entry, index) in thinkingPool" 
            :key="index"
            class="bg-dark-800 rounded p-2 text-sm group relative"
          >
            <!-- 编辑模式 -->
            <div v-if="editingThinkingIndex === index">
              <textarea
                v-model="editThinkingContent"
                class="w-full bg-dark-700 border border-dark-600 rounded p-2 text-sm resize-none"
                rows="3"
                @keydown.ctrl.enter="saveEditThinking"
                @keydown.escape="cancelEditThinking"
              ></textarea>
              <div class="flex justify-end gap-2 mt-2">
                <button 
                  @click="cancelEditThinking"
                  class="text-xs px-2 py-1 bg-dark-700 hover:bg-dark-600 rounded"
                >
                  取消
                </button>
                <button 
                  @click="saveEditThinking"
                  class="text-xs px-2 py-1 bg-primary-600 hover:bg-primary-700 rounded"
                >
                  保存
                </button>
              </div>
            </div>
            <!-- 显示模式 -->
            <div v-else>
              <p class="text-dark-300 whitespace-pre-wrap">{{ entry.content }}</p>
              <div class="flex items-center justify-between mt-1">
                <span class="text-xs text-dark-500">{{ entry.source }}</span>
                <div class="opacity-0 group-hover:opacity-100 transition-opacity flex gap-1">
                  <button 
                    @click="startEditThinking(index)"
                    class="text-xs px-1.5 py-0.5 bg-dark-700 hover:bg-dark-600 rounded"
                  >
                    编辑
                  </button>
                  <button 
                    @click="deleteThinking(index)"
                    class="text-xs px-1.5 py-0.5 bg-red-600/20 hover:bg-red-600/40 text-red-400 rounded"
                  >
                    删除
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 短期记忆池 -->
      <div class="flex-1 flex flex-col min-h-0">
        <div class="flex items-center justify-between px-4 py-3 border-b border-dark-700">
          <h3 class="text-sm font-medium text-green-400">🧠 短期记忆</h3>
          <button 
            @click="addShortTerm"
            class="text-xs px-2 py-1 bg-dark-700 hover:bg-dark-600 rounded"
          >
            + 添加
          </button>
        </div>
        <div class="flex-1 overflow-y-auto p-3 space-y-2">
          <div v-if="isLoadingMemory" class="text-center text-dark-500 text-sm py-4">
            加载中...
          </div>
          <div v-else-if="shortTermMemory.length === 0" class="text-center text-dark-500 text-sm py-4">
            暂无短期记忆
          </div>
          <div 
            v-else
            v-for="(mem, index) in shortTermMemory" 
            :key="mem.id"
            class="bg-dark-800 rounded p-2 text-sm group relative"
          >
            <!-- 编辑模式 -->
            <div v-if="editingShortTermIndex === index" class="space-y-2">
              <!-- 概述 -->
              <input
                v-model="editShortTermSummary"
                class="w-full bg-dark-700 border border-dark-600 rounded px-2 py-1 text-sm"
                placeholder="概述/标题"
                @keydown.escape="cancelEditShortTerm"
              />
              <!-- 类型 -->
              <select
                v-model="editShortTermType"
                class="w-full bg-dark-700 border border-dark-600 rounded px-2 py-1 text-sm"
              >
                <option value="fact">事实 (fact)</option>
                <option value="event">事件 (event)</option>
                <option value="preference">偏好 (preference)</option>
                <option value="knowledge">知识 (knowledge)</option>
                <option value="task">任务 (task)</option>
                <option value="other">其他 (other)</option>
              </select>
              <!-- 内容 -->
              <textarea
                v-model="editShortTermContent"
                class="w-full bg-dark-700 border border-dark-600 rounded p-2 text-sm resize-none"
                rows="3"
                placeholder="详细内容"
                @keydown.ctrl.enter="saveEditShortTerm"
                @keydown.escape="cancelEditShortTerm"
              ></textarea>
              <!-- 置信度和时间 -->
              <div class="flex gap-2">
                <div class="flex-1">
                  <label class="text-xs text-dark-400 mb-1 block">置信度</label>
                  <input
                    v-model.number="editShortTermConfidence"
                    type="number"
                    min="0"
                    max="1"
                    step="0.1"
                    class="w-full bg-dark-700 border border-dark-600 rounded px-2 py-1 text-sm"
                  />
                </div>
                <div class="flex-1">
                  <label class="text-xs text-dark-400 mb-1 block">时间</label>
                  <input
                    v-model="editShortTermTimestamp"
                    type="datetime-local"
                    class="w-full bg-dark-700 border border-dark-600 rounded px-2 py-1 text-sm"
                  />
                </div>
              </div>
              <div class="flex justify-end gap-2">
                <button 
                  @click="cancelEditShortTerm"
                  class="text-xs px-2 py-1 bg-dark-700 hover:bg-dark-600 rounded"
                >
                  取消
                </button>
                <button 
                  @click="saveEditShortTerm"
                  class="text-xs px-2 py-1 bg-primary-600 hover:bg-primary-700 rounded"
                >
                  保存
                </button>
              </div>
            </div>
            <!-- 显示模式 -->
            <div v-else>
              <!-- 标题和类型 -->
              <div class="flex items-center gap-2 mb-1">
                <span class="font-medium text-dark-200">{{ mem.summary }}</span>
                <span class="text-xs px-1.5 py-0.5 bg-primary-600/20 text-primary-400 rounded">
                  {{ mem.memory_type }}
                </span>
                <!-- 展开标记 -->
                <button
                  @click="toggleShouldExpand(index)"
                  class="text-xs px-1.5 py-0.5 rounded transition-colors"
                  :class="mem.should_expand 
                    ? 'bg-green-600/30 text-green-400 hover:bg-green-600/50' 
                    : 'bg-dark-700 text-dark-400 hover:bg-dark-600'"
                  :title="mem.should_expand ? '点击取消展开' : '点击标记为展开'"
                >
                  {{ mem.should_expand ? '📖 展开' : '📕 折叠' }}
                </button>
              </div>
              <!-- 内容（预览，限100字符） -->
              <p class="text-dark-400 text-xs whitespace-pre-wrap">{{ truncateText(mem.content, 100) }}</p>
              <!-- 底部信息 -->
              <div class="flex items-center justify-between mt-2">
                <div class="flex items-center gap-3 text-xs text-dark-500">
                  <span>相关性: {{ (mem.relevance * 100).toFixed(0) }}%</span>
                  <span>置信度: {{ (mem.confidence * 100).toFixed(0) }}%</span>
                  <span>{{ formatTime(mem.timestamp) }}</span>
                </div>
                <div class="opacity-0 group-hover:opacity-100 transition-opacity flex gap-1">
                  <button 
                    @click="startEditShortTerm(index)"
                    class="text-xs px-1.5 py-0.5 bg-dark-700 hover:bg-dark-600 rounded"
                  >
                    编辑
                  </button>
                  <button 
                    @click="deleteShortTerm(index)"
                    class="text-xs px-1.5 py-0.5 bg-red-600/20 hover:bg-red-600/40 text-red-400 rounded"
                  >
                    删除
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 请求体查看模态框 -->
    <Teleport to="body">
      <div 
        v-if="showRequestModal"
        class="fixed inset-0 bg-black/60 flex items-center justify-center z-50"
        @click.self="showRequestModal = false"
      >
        <div class="bg-dark-800 rounded-lg w-[800px] max-h-[80vh] flex flex-col shadow-xl">
          <!-- 头部 -->
          <div class="flex items-center justify-between px-6 py-4 border-b border-dark-700">
            <h2 class="text-lg font-semibold">上次请求的 Messages</h2>
            <button 
              @click="showRequestModal = false"
              class="text-dark-400 hover:text-white text-xl"
            >
              ×
            </button>
          </div>
          
          <!-- 内容 -->
          <div class="flex-1 overflow-y-auto p-6 space-y-4">
            <div v-if="lastRequestMessages.length === 0" class="text-center text-dark-500 py-8">
              暂无请求记录
            </div>
            <div 
              v-else
              v-for="(msg, index) in lastRequestMessages" 
              :key="index"
              class="rounded-lg p-4"
              :class="{
                'bg-blue-900/30 border border-blue-700/50': msg.role === 'system',
                'bg-green-900/30 border border-green-700/50': msg.role === 'user',
                'bg-purple-900/30 border border-purple-700/50': msg.role === 'assistant'
              }"
            >
              <!-- 角色标签 -->
              <div 
                class="text-xs font-medium mb-2 px-2 py-0.5 rounded inline-block"
                :class="{
                  'bg-blue-600 text-blue-100': msg.role === 'system',
                  'bg-green-600 text-green-100': msg.role === 'user',
                  'bg-purple-600 text-purple-100': msg.role === 'assistant'
                }"
              >
                {{ msg.role.toUpperCase() }}
              </div>
              <!-- 内容 -->
              <pre class="text-sm whitespace-pre-wrap break-words font-mono"
                :class="{
                  'text-blue-200': msg.role === 'system',
                  'text-green-200': msg.role === 'user',
                  'text-purple-200': msg.role === 'assistant'
                }"
              >{{ msg.content }}</pre>
            </div>
          </div>
          
          <!-- 底部 -->
          <div class="px-6 py-4 border-t border-dark-700 flex justify-between items-center">
            <span class="text-sm text-dark-500">
              共 {{ lastRequestMessages.length }} 条消息
            </span>
            <button 
              @click="showRequestModal = false"
              class="px-4 py-2 bg-dark-700 hover:bg-dark-600 rounded"
            >
              关闭
            </button>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>