<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useAssistantStore, useAppStore } from '@/stores'
import type { TopicType } from '@/types'

const router = useRouter()
const route = useRoute()
const store = useAssistantStore()
const app = useAppStore()

// 编辑话题相关状态
const editingTopicId = ref<string | null>(null)
const editingTopicTitle = ref('')
const editInputRef = ref<HTMLInputElement | null>(null)

// 计算属性：分离普通话题和记忆话题
const normalTopics = computed(() => 
  store.topics.filter(t => t.topic_type === 'normal' || !t.topic_type)
)
const memoryTopics = computed(() => 
  store.topics.filter(t => t.topic_type === 'memory')
)

onMounted(() => {
  store.loadAssistants()
})

// 路由变化时同步状态
watch(() => route.params, async (params) => {
  if (params.assistantId && params.assistantId !== store.currentAssistantId) {
    await store.selectAssistant(params.assistantId as string)
  }
  if (params.topicId && params.topicId !== store.currentTopicId) {
    await store.selectTopic(params.topicId as string)
  }
}, { immediate: true })

function selectAssistant(id: string) {
  router.push(`/chat/${id}`)
}

function selectTopic(topicId: string) {
  if (store.currentAssistantId) {
    router.push(`/chat/${store.currentAssistantId}/${topicId}`)
  }
}

// 生成时间标题
function generateTimeTitle(): string {
  const now = new Date()
  const month = String(now.getMonth() + 1).padStart(2, '0')
  const day = String(now.getDate()).padStart(2, '0')
  const hour = String(now.getHours()).padStart(2, '0')
  const minute = String(now.getMinutes()).padStart(2, '0')
  return `${month}-${day} ${hour}:${minute}`
}

// 创建话题（直接使用时间标题）
async function createTopic(topicType: TopicType) {
  try {
    const title = generateTimeTitle()
    const topicId = await store.createTopic(title, topicType)
    if (topicId && store.currentAssistantId) {
      router.push(`/chat/${store.currentAssistantId}/${topicId}`)
    }
  } catch {
    // 错误已在store中处理
  }
}

// 双击开始编辑话题标题
function startEditTopic(topic: { id: string; title: string }) {
  editingTopicId.value = topic.id
  editingTopicTitle.value = topic.title
  nextTick(() => {
    editInputRef.value?.focus()
    editInputRef.value?.select()
  })
}

// 保存话题标题
async function saveTopicTitle() {
  if (!editingTopicId.value || !editingTopicTitle.value.trim()) {
    cancelEditTopic()
    return
  }
  
  try {
    await store.updateTopicTitle(editingTopicId.value, editingTopicTitle.value.trim())
    cancelEditTopic()
  } catch {
    // 错误已在store中处理
  }
}

// 取消编辑
function cancelEditTopic() {
  editingTopicId.value = null
  editingTopicTitle.value = ''
}

async function deleteTopic(topicId: string) {
  if (!confirm('确定要删除这个话题吗？所有对话历史都将被删除。')) return
  
  try {
    await store.deleteTopic(topicId)
    // 如果删除的是当前话题，返回助手页面
    if (store.currentTopicId === null && store.currentAssistantId) {
      router.push(`/chat/${store.currentAssistantId}`)
    }
  } catch {
    // 错误已在store中处理
  }
}

function goToAssistants() {
  router.push('/assistants')
}

function goToMemory() {
  if (store.currentAssistantId) {
    router.push(`/memory/${store.currentAssistantId}`)
  } else {
    app.showToast('请先选择一个助手', 'info')
  }
}

function goToSettings() {
  router.push('/settings')
}
</script>

<template>
  <aside 
    class="flex flex-col bg-dark-900 border-r border-dark-700 transition-all duration-300"
    :class="app.sidebarCollapsed ? 'w-16' : 'w-64'"
  >
    <!-- Logo -->
    <div class="flex items-center justify-between p-4 border-b border-dark-700">
      <div v-if="!app.sidebarCollapsed" class="flex items-center gap-2">
        <span class="text-2xl">🧠</span>
        <span class="font-bold text-primary-400">MemoChater</span>
      </div>
      <button 
        @click="app.toggleSidebar"
        class="p-2 rounded-lg hover:bg-dark-700 text-dark-400 hover:text-white transition-colors"
      >
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
            :d="app.sidebarCollapsed ? 'M13 5l7 7-7 7M5 5l7 7-7 7' : 'M11 19l-7-7 7-7m8 14l-7-7 7-7'" />
        </svg>
      </button>
    </div>

    <!-- 助手列表 -->
    <div class="p-2">
      <div v-if="!app.sidebarCollapsed" class="mb-2 px-2 text-xs text-dark-500 uppercase tracking-wider">
        助手
      </div>
      
      <div class="space-y-1">
        <button
          v-for="assistant in store.assistants"
          :key="assistant.id"
          @click="selectAssistant(assistant.id)"
          class="w-full flex items-center gap-3 px-3 py-2 rounded-lg transition-colors text-left"
          :class="store.currentAssistantId === assistant.id 
            ? 'bg-primary-600/20 text-primary-400' 
            : 'hover:bg-dark-700 text-dark-300'"
        >
          <span class="text-lg">🤖</span>
          <span v-if="!app.sidebarCollapsed" class="truncate">{{ assistant.name }}</span>
        </button>
      </div>
    </div>

    <!-- 话题列表区域 -->
    <div v-if="store.currentAssistantId && !app.sidebarCollapsed" class="flex-1 flex flex-col overflow-hidden">
      
      <!-- 普通话题区域 -->
      <div class="flex-1 flex flex-col min-h-0 border-t border-dark-700">
        <div class="px-4 py-2 flex items-center justify-between bg-dark-900 sticky top-0">
          <span class="text-xs text-dark-500 uppercase tracking-wider flex items-center gap-1">
            <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                d="M8 10h.01M12 10h.01M16 10h.01M9 16H5a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v8a2 2 0 01-2 2h-5l-5 5v-5z" />
            </svg>
            普通话题
          </span>
          <button 
            @click="createTopic('normal')"
            class="p-1 rounded hover:bg-dark-700 text-dark-400 hover:text-white"
            title="新建普通话题"
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
            </svg>
          </button>
        </div>

        <div class="flex-1 overflow-y-auto px-2 pb-2 space-y-1">
          <div
            v-for="topic in normalTopics"
            :key="topic.id"
            class="group flex items-center gap-1 pr-1"
          >
            <!-- 编辑模式 -->
            <div v-if="editingTopicId === topic.id" class="flex-1 px-1">
              <input
                ref="editInputRef"
                v-model="editingTopicTitle"
                @keyup.enter="saveTopicTitle"
                @keyup.esc="cancelEditTopic"
                @blur="saveTopicTitle"
                class="w-full px-2 py-1.5 bg-dark-800 border border-primary-500 rounded text-sm focus:outline-none"
              />
            </div>
            <!-- 正常显示模式 -->
            <button
              v-else
              @click="selectTopic(topic.id)"
              @dblclick.stop="startEditTopic(topic)"
              class="flex-1 flex items-center gap-2 px-3 py-2 rounded-lg transition-colors text-left text-sm"
              :class="store.currentTopicId === topic.id 
                ? 'bg-dark-700 text-white' 
                : 'hover:bg-dark-800 text-dark-400'"
            >
              <svg class="w-4 h-4 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                  d="M8 10h.01M12 10h.01M16 10h.01M9 16H5a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v8a2 2 0 01-2 2h-5l-5 5v-5z" />
              </svg>
              <span class="truncate">{{ topic.title }}</span>
              <span class="ml-auto text-xs text-dark-500">{{ topic.message_count }}</span>
            </button>
            <button
              @click.stop="deleteTopic(topic.id)"
              class="p-1.5 rounded opacity-0 group-hover:opacity-100 hover:bg-red-600/20 text-dark-500 hover:text-red-400 transition-all"
              title="删除话题"
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            </button>
          </div>
          <div v-if="normalTopics.length === 0" class="px-3 py-2 text-xs text-dark-500 text-center">
            暂无普通话题
          </div>
        </div>
      </div>

      <!-- 记忆话题区域 -->
      <div class="flex-1 flex flex-col min-h-0 border-t border-dark-700">
        <div class="px-4 py-2 flex items-center justify-between bg-dark-900 sticky top-0">
          <span class="text-xs text-emerald-500 uppercase tracking-wider flex items-center gap-1">
            <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
            </svg>
            记忆话题
          </span>
          <button 
            @click="createTopic('memory')"
            class="p-1 rounded hover:bg-dark-700 text-emerald-400 hover:text-emerald-300"
            title="新建记忆话题"
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
            </svg>
          </button>
        </div>

        <div class="flex-1 overflow-y-auto px-2 pb-2 space-y-1">
          <div
            v-for="topic in memoryTopics"
            :key="topic.id"
            class="group flex items-center gap-1 pr-1"
          >
            <!-- 编辑模式 -->
            <div v-if="editingTopicId === topic.id" class="flex-1 px-1">
              <input
                ref="editInputRef"
                v-model="editingTopicTitle"
                @keyup.enter="saveTopicTitle"
                @keyup.esc="cancelEditTopic"
                @blur="saveTopicTitle"
                class="w-full px-2 py-1.5 bg-dark-800 border border-emerald-500 rounded text-sm focus:outline-none"
              />
            </div>
            <!-- 正常显示模式 -->
            <button
              v-else
              @click="selectTopic(topic.id)"
              @dblclick.stop="startEditTopic(topic)"
              class="flex-1 flex items-center gap-2 px-3 py-2 rounded-lg transition-colors text-left text-sm"
              :class="store.currentTopicId === topic.id 
                ? 'bg-emerald-900/30 text-emerald-300' 
                : 'hover:bg-dark-800 text-dark-400'"
            >
              <svg class="w-4 h-4 flex-shrink-0 text-emerald-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                  d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
              </svg>
              <span class="truncate">{{ topic.title }}</span>
              <span class="ml-auto text-xs text-dark-500">{{ topic.message_count }}</span>
            </button>
            <button
              @click.stop="deleteTopic(topic.id)"
              class="p-1.5 rounded opacity-0 group-hover:opacity-100 hover:bg-red-600/20 text-dark-500 hover:text-red-400 transition-all"
              title="删除话题"
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            </button>
          </div>
          <div v-if="memoryTopics.length === 0" class="px-3 py-2 text-xs text-dark-500 text-center">
            暂无记忆话题
          </div>
        </div>
      </div>
    </div>

    <!-- 底部导航 -->
    <div class="p-2 border-t border-dark-700 space-y-1">
      <button 
        @click="goToAssistants"
        class="w-full flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-dark-700 text-dark-400 hover:text-white transition-colors"
      >
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
            d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4" />
        </svg>
        <span v-if="!app.sidebarCollapsed">助手管理</span>
      </button>
      
      <button 
        @click="goToMemory"
        class="w-full flex items-center gap-3 px-3 py-2 rounded-lg transition-colors"
        :class="store.currentAssistantId 
          ? 'hover:bg-dark-700 text-dark-400 hover:text-white' 
          : 'text-dark-600 cursor-not-allowed'"
        :title="store.currentAssistantId ? '管理当前助手的记忆' : '请先选择一个助手'"
      >
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
            d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
        </svg>
        <span v-if="!app.sidebarCollapsed">记忆管理</span>
      </button>

      <button 
        @click="goToSettings"
        class="w-full flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-dark-700 text-dark-400 hover:text-white transition-colors"
      >
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
            d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
        </svg>
        <span v-if="!app.sidebarCollapsed">设置</span>
      </button>
    </div>
  </aside>
</template>