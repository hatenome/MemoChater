<script setup lang="ts">
import { ref, computed } from 'vue'
import type { ChatMessage } from '@/types'

const props = defineProps<{
  message: ChatMessage
  index: number
  assistantName?: string
  userName?: string
}>()

const emit = defineEmits<{
  edit: [index: number, content: string]
  delete: [index: number]
  branch: [index: number]
  regenerate: [index: number]
}>()

const isUser = computed(() => props.message.role === 'user')
const displayName = computed(() => {
  if (props.message.role === 'user') return props.userName || '用户'
  if (props.message.role === 'assistant') return props.assistantName || '助手'
  return 'System'
})

// 编辑状态
const isEditing = ref(false)
const editContent = ref('')
const showActions = ref(false)

function startEdit() {
  editContent.value = props.message.content
  isEditing.value = true
}

function cancelEdit() {
  isEditing.value = false
  editContent.value = ''
}

function confirmEdit() {
  if (editContent.value.trim()) {
    emit('edit', props.index, editContent.value.trim())
  }
  isEditing.value = false
}

function handleDelete() {
  if (confirm('确定要删除这条消息吗？')) {
    emit('delete', props.index)
  }
}

function handleBranch() {
  emit('branch', props.index)
}

function handleRegenerate() {
  emit('regenerate', props.index)
}
</script>

<template>
  <div 
    class="flex gap-4 p-4 group relative"
    :class="isUser ? 'bg-dark-900/50' : 'bg-dark-800/30'"
    @mouseenter="showActions = true"
    @mouseleave="showActions = false"
  >
    <!-- 头像 -->
    <div 
      class="w-10 h-10 rounded-full flex items-center justify-center flex-shrink-0 text-lg"
      :class="isUser ? 'bg-blue-600' : 'bg-primary-600'"
    >
      {{ isUser ? '👤' : '🤖' }}
    </div>
    
    <!-- 内容 -->
    <div class="flex-1 min-w-0">
      <div class="flex items-center gap-2 mb-1">
        <span class="font-medium" :class="isUser ? 'text-blue-400' : 'text-primary-400'">
          {{ displayName }}
        </span>
        <span class="text-xs text-dark-500">#{{ index }}</span>
      </div>
      
      <!-- 编辑模式 -->
      <div v-if="isEditing" class="space-y-2">
        <textarea
          v-model="editContent"
          class="w-full bg-dark-700 border border-dark-600 rounded-lg p-3 text-dark-200 resize-none focus:outline-none focus:border-primary-500"
          rows="4"
          @keydown.ctrl.enter="confirmEdit"
          @keydown.escape="cancelEdit"
        />
        <div class="flex gap-2">
          <button
            @click="confirmEdit"
            class="px-3 py-1 bg-primary-600 hover:bg-primary-500 rounded text-sm transition-colors"
          >
            保存
          </button>
          <button
            @click="cancelEdit"
            class="px-3 py-1 bg-dark-600 hover:bg-dark-500 rounded text-sm transition-colors"
          >
            取消
          </button>
        </div>
      </div>
      
      <!-- 显示模式 -->
      <div v-else class="text-dark-200 whitespace-pre-wrap break-words leading-relaxed">
        {{ message.content }}
      </div>
    </div>

    <!-- 操作按钮 -->
    <div 
      v-show="showActions && !isEditing"
      class="absolute right-4 top-4 flex gap-1 bg-dark-800 rounded-lg p-1 shadow-lg border border-dark-600"
    >
      <button
        @click="startEdit"
        class="p-1.5 hover:bg-dark-600 rounded transition-colors text-dark-400 hover:text-dark-200"
        title="编辑"
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
        </svg>
      </button>
      <button
        @click="handleBranch"
        class="p-1.5 hover:bg-dark-600 rounded transition-colors text-dark-400 hover:text-dark-200"
        title="从此处创建分支"
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7v8a2 2 0 002 2h6M8 7V5a2 2 0 012-2h4.586a1 1 0 01.707.293l4.414 4.414a1 1 0 01.293.707V15a2 2 0 01-2 2h-2M8 7H6a2 2 0 00-2 2v10a2 2 0 002 2h8a2 2 0 002-2v-2" />
        </svg>
      </button>
      <button
        v-if="!isUser"
        @click="handleRegenerate"
        class="p-1.5 hover:bg-green-600/20 rounded transition-colors text-dark-400 hover:text-green-400"
        title="重新生成"
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
      </button>
      <button
        @click="handleDelete"
        class="p-1.5 hover:bg-red-600/20 rounded transition-colors text-dark-400 hover:text-red-400"
        title="删除"
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
        </svg>
      </button>
    </div>
  </div>
</template>