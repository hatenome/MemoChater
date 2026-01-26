<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useAssistantStore, useAppStore } from '@/stores'
import { chatApi, assistantsApi } from '@/api'
import ChatMessage from '@/components/ChatMessage.vue'
import ChatInput from '@/components/ChatInput.vue'
import ToolCallBlock from '@/components/ToolCallBlock.vue'
import ToolResultBlock from '@/components/ToolResultBlock.vue'
import MemoryPanel from '@/components/MemoryPanel.vue'
import ThinkingPanel from '@/components/panels/ThinkingPanel.vue'
import ShortTermPanel from '@/components/panels/ShortTermPanel.vue'
import ConversationMemoryPanel from '@/components/panels/ConversationMemoryPanel.vue'
import { parseVCPContent, hasVCPBlocks } from '@/utils/vcpParser'
import type { ThinkingEntry, ShortTermMemoryEntry, ConversationTurn, VectorMemoryEntry } from '@/types'

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

// 对话记忆库状态
const conversationMemory = ref<VectorMemoryEntry[]>([])
const isConversationMemoryLoading = ref(false)
const conversationMemorySearchResults = ref<{ memory: VectorMemoryEntry, score: number }[] | null>(null)

// 对话记忆库面板引用
const conversationMemoryPanelRef = ref<InstanceType<typeof ConversationMemoryPanel> | null>(null)

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

// 标签页配置
const memoryTabs = [
  { id: 'thinking', icon: '💭', label: '思考池' },
  { id: 'shortTerm', icon: '🧠', label: '短期记忆' },
  { id: 'conversationMemory', icon: '📚', label: '对话记忆库' },
]

// 过滤系统消息
const displayMessages = computed(() => store.messages.filter(m => m.role !== 'system'))

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
      // 同时加载对话记忆库
      await loadConversationMemory()
    } else {
      thinkingPool.value = []
      shortTermMemory.value = []
      conversationTurns.value = []
      conversationMemory.value = []
      conversationMemorySearchResults.value = null
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
    should_expand: true,
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

// 处理思考池编辑（来自子组件）
function handleThinkingEdit(index: number, content: string) {
  thinkingPool.value[index].content = content
  saveThinkingPool()
}

// 处理短期记忆编辑（来自子组件）
function handleShortTermEdit(index: number, data: Partial<typeof shortTermMemory.value[0]>) {
  const mem = shortTermMemory.value[index]
  if (data.summary !== undefined) mem.summary = data.summary
  if (data.content !== undefined) mem.content = data.content
  if (data.memory_type !== undefined) mem.memory_type = data.memory_type
  if (data.confidence !== undefined) mem.confidence = data.confidence
  if (data.timestamp !== undefined) mem.timestamp = data.timestamp
  saveShortTermMemory()
}

// ============ 对话记忆库操作 ============

// 加载对话记忆库
async function loadConversationMemory() {
  if (!store.currentAssistantId || !store.currentTopicId) return
  
  isConversationMemoryLoading.value = true
  try {
    const data = await assistantsApi.listConversationMemory(
      store.currentAssistantId,
      store.currentTopicId
    )
    conversationMemory.value = data.memories || []
  } catch (e) {
    console.error('加载对话记忆库失败:', e)
    app.showToast('加载对话记忆库失败', 'error')
  } finally {
    isConversationMemoryLoading.value = false
  }
}

// 搜索对话记忆库
async function handleConversationMemorySearch(query: string) {
  if (!store.currentAssistantId || !store.currentTopicId) return
  
  isConversationMemoryLoading.value = true
  try {
    const results = await assistantsApi.searchConversationMemory(
      store.currentAssistantId,
      store.currentTopicId,
      query,
      10
    )
    conversationMemorySearchResults.value = results
  } catch (e) {
    console.error('搜索对话记忆库失败:', e)
    app.showToast('搜索失败', 'error')
  } finally {
    isConversationMemoryLoading.value = false
  }
}

// 清除搜索结果
function clearConversationMemorySearch() {
  conversationMemorySearchResults.value = null
}

// 编辑对话记忆
async function handleConversationMemoryEdit(id: string, data: { summary?: string, content?: string, memory_type?: string }) {
  if (!store.currentAssistantId || !store.currentTopicId) return
  
  try {
    await assistantsApi.updateConversationMemory(
      store.currentAssistantId,
      store.currentTopicId,
      id,
      data
    )
    app.showToast('记忆已更新', 'success')
    await loadConversationMemory()
  } catch (e) {
    console.error('更新对话记忆失败:', e)
    app.showToast('更新失败', 'error')
  }
}

// 删除对话记忆
async function handleConversationMemoryDelete(id: string) {
  if (!store.currentAssistantId || !store.currentTopicId) return
  
  if (!confirm('确定要删除这条记忆吗？')) return
  
  try {
    await assistantsApi.deleteConversationMemory(
      store.currentAssistantId,
      store.currentTopicId,
      id
    )
    app.showToast('记忆已删除', 'success')
    await loadConversationMemory()
  } catch (e) {
    console.error('删除对话记忆失败:', e)
    app.showToast('删除失败', 'error')
  }
}

// 重建对话向量库
async function handleConversationMemoryRebuild() {
  if (!store.currentAssistantId || !store.currentTopicId) return
  
  // 设置重建状态
  conversationMemoryPanelRef.value?.setRebuilding(true)
  conversationMemoryPanelRef.value?.setRebuildResult(null)
  
  try {
    const result = await assistantsApi.rebuildConversationMemory(
      store.currentAssistantId,
      store.currentTopicId
    )
    
    conversationMemoryPanelRef.value?.setRebuildResult({
      rebuilt: result.rebuilt,
      total: result.total
    })
    
    app.showToast(`重建完成: ${result.rebuilt}/${result.total} 条`, 'success')
    
    // 刷新列表
    await loadConversationMemory()
  } catch (e) {
    console.error('重建对话向量库失败:', e)
    app.showToast('重建失败', 'error')
  } finally {
    conversationMemoryPanelRef.value?.setRebuilding(false)
  }
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
            v-for="(msg, index) in displayMessages"
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
              <div class="text-sm text-dark-300">
                <template v-if="hasVCPBlocks(conversationTurns[conversationTurns.length - 1].assistant_message)">
                  <template v-for="(segment, idx) in parseVCPContent(conversationTurns[conversationTurns.length - 1].assistant_message)" :key="idx">
                    <div v-if="segment.type === 'text'" class="whitespace-pre-wrap">{{ segment.content }}</div>
                    <ToolCallBlock v-else-if="segment.type === 'tool_call'" :content="segment.content" :parsed="segment.parsed" />
                    <ToolResultBlock v-else-if="segment.type === 'tool_result'" :content="segment.content" :parsed="segment.parsed" />
                  </template>
                </template>
                <div v-else class="whitespace-pre-wrap">{{ conversationTurns[conversationTurns.length - 1].assistant_message }}</div>
              </div>
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
      <MemoryPanel :tabs="memoryTabs" v-slot="{ activeTab }">
        <!-- 思考池面板 -->
        <ThinkingPanel
          v-show="activeTab === 'thinking'"
          :entries="thinkingPool"
          :is-loading="isLoadingMemory"
          @add="addThinking"
          @edit="handleThinkingEdit"
          @delete="deleteThinking"
        />
        
        <!-- 短期记忆面板 -->
        <ShortTermPanel
          v-show="activeTab === 'shortTerm'"
          :entries="shortTermMemory"
          :is-loading="isLoadingMemory"
          @add="addShortTerm"
          @edit="handleShortTermEdit"
          @delete="deleteShortTerm"
          @toggle-expand="toggleShouldExpand"
        />
        
        <!-- 对话记忆库面板 -->
        <ConversationMemoryPanel
          ref="conversationMemoryPanelRef"
          v-show="activeTab === 'conversationMemory'"
          :entries="conversationMemory"
          :is-loading="isConversationMemoryLoading"
          :search-results="conversationMemorySearchResults"
          @search="handleConversationMemorySearch"
          @clear-search="clearConversationMemorySearch"
          @edit="handleConversationMemoryEdit"
          @delete="handleConversationMemoryDelete"
          @refresh="loadConversationMemory"
          @rebuild="handleConversationMemoryRebuild"
        />
      </MemoryPanel>
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