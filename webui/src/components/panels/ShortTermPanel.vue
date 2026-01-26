<script setup lang="ts">
import { ref } from 'vue'
import type { ShortTermMemoryEntry } from '@/types'

const props = defineProps<{
  entries: ShortTermMemoryEntry[]
  isLoading: boolean
}>()

const emit = defineEmits<{
  add: []
  edit: [index: number, data: Partial<ShortTermMemoryEntry>]
  delete: [index: number]
  toggleExpand: [index: number]
  save: []
}>()

// 编辑状态
const editingIndex = ref<number | null>(null)
const editSummary = ref('')
const editContent = ref('')
const editType = ref('')
const editConfidence = ref(1.0)
const editTimestamp = ref('')

function startEdit(index: number) {
  editingIndex.value = index
  const mem = props.entries[index]
  editSummary.value = mem.summary
  editContent.value = mem.content
  editType.value = mem.memory_type
  editConfidence.value = mem.confidence
  editTimestamp.value = mem.timestamp.slice(0, 16)
}

function saveEdit() {
  if (editingIndex.value !== null) {
    emit('edit', editingIndex.value, {
      summary: editSummary.value,
      content: editContent.value,
      memory_type: editType.value,
      confidence: editConfidence.value,
      timestamp: editTimestamp.value
    })
    editingIndex.value = null
  }
}

function cancelEdit() {
  editingIndex.value = null
}

function truncateText(text: string, maxLength: number = 100): string {
  if (!text || text.length <= maxLength) return text
  return text.slice(0, maxLength) + '...'
}

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
</script>

<template>
  <div class="flex flex-col h-full">
    <!-- 头部操作栏 -->
    <div class="flex items-center justify-between px-4 py-2 border-b border-dark-700">
      <span class="text-xs text-dark-500">{{ entries.length }} 条</span>
      <button 
        @click="emit('add')"
        class="text-xs px-2 py-1 bg-dark-700 hover:bg-dark-600 rounded transition-colors"
      >
        + 添加
      </button>
    </div>
    
    <!-- 内容列表 -->
    <div class="flex-1 overflow-y-auto p-3 space-y-2">
      <div v-if="isLoading" class="text-center text-dark-500 text-sm py-4">
        加载中...
      </div>
      <div v-else-if="entries.length === 0" class="text-center text-dark-500 text-sm py-4">
        暂无短期记忆
      </div>
      <div 
        v-else
        v-for="(mem, index) in entries" 
        :key="mem.id"
        class="bg-dark-800 rounded p-2 text-sm group relative"
      >
        <!-- 编辑模式 -->
        <div v-if="editingIndex === index" class="space-y-2">
          <input
            v-model="editSummary"
            class="w-full bg-dark-700 border border-dark-600 rounded px-2 py-1 text-sm"
            placeholder="概述/标题"
            @keydown.escape="cancelEdit"
          />
          <select
            v-model="editType"
            class="w-full bg-dark-700 border border-dark-600 rounded px-2 py-1 text-sm"
          >
            <option value="fact">事实 (fact)</option>
            <option value="event">事件 (event)</option>
            <option value="preference">偏好 (preference)</option>
            <option value="knowledge">知识 (knowledge)</option>
            <option value="task">任务 (task)</option>
            <option value="other">其他 (other)</option>
          </select>
          <textarea
            v-model="editContent"
            class="w-full bg-dark-700 border border-dark-600 rounded p-2 text-sm resize-none"
            rows="3"
            placeholder="详细内容"
            @keydown.ctrl.enter="saveEdit"
            @keydown.escape="cancelEdit"
          ></textarea>
          <div class="flex gap-2">
            <div class="flex-1">
              <label class="text-xs text-dark-400 mb-1 block">置信度</label>
              <input
                v-model.number="editConfidence"
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
                v-model="editTimestamp"
                type="datetime-local"
                class="w-full bg-dark-700 border border-dark-600 rounded px-2 py-1 text-sm"
              />
            </div>
          </div>
          <div class="flex justify-end gap-2">
            <button 
              @click="cancelEdit"
              class="text-xs px-2 py-1 bg-dark-700 hover:bg-dark-600 rounded"
            >
              取消
            </button>
            <button 
              @click="saveEdit"
              class="text-xs px-2 py-1 bg-primary-600 hover:bg-primary-700 rounded"
            >
              保存
            </button>
          </div>
        </div>
        <!-- 显示模式 -->
        <div v-else>
          <div class="flex items-center gap-2 mb-1">
            <span class="font-medium text-dark-200">{{ mem.summary }}</span>
            <span class="text-xs px-1.5 py-0.5 bg-primary-600/20 text-primary-400 rounded">
              {{ mem.memory_type }}
            </span>
            <button
              @click="emit('toggleExpand', index)"
              class="text-xs px-1.5 py-0.5 rounded transition-colors"
              :class="mem.should_expand 
                ? 'bg-green-600/30 text-green-400 hover:bg-green-600/50' 
                : 'bg-dark-700 text-dark-400 hover:bg-dark-600'"
              :title="mem.should_expand ? '点击取消展开' : '点击标记为展开'"
            >
              {{ mem.should_expand ? '📖 展开' : '📕 折叠' }}
            </button>
          </div>
          <p class="text-dark-400 text-xs whitespace-pre-wrap">{{ truncateText(mem.content, 100) }}</p>
          <div class="flex items-center justify-between mt-2">
            <div class="flex items-center gap-3 text-xs text-dark-500">
              <span>相关性: {{ (mem.relevance * 100).toFixed(0) }}%</span>
              <span>置信度: {{ (mem.confidence * 100).toFixed(0) }}%</span>
              <span>{{ formatTime(mem.timestamp) }}</span>
            </div>
            <div class="opacity-0 group-hover:opacity-100 transition-opacity flex gap-1">
              <button 
                @click="startEdit(index)"
                class="text-xs px-1.5 py-0.5 bg-dark-700 hover:bg-dark-600 rounded"
              >
                编辑
              </button>
              <button 
                @click="emit('delete', index)"
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
</template>