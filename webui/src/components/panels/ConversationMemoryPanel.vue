<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import type { VectorMemoryEntry } from '@/types'

const props = defineProps<{
  entries: VectorMemoryEntry[]
  isLoading: boolean
  searchResults: { memory: VectorMemoryEntry, score: number }[] | null
}>()

const emit = defineEmits<{
  search: [query: string]
  clearSearch: []
  edit: [id: string, data: { summary?: string, content?: string, memory_type?: string }]
  delete: [id: string]
  refresh: []
}>()

// 搜索状态
const searchQuery = ref('')
const isSearching = ref(false)

// 编辑状态
const editingId = ref<string | null>(null)
const editSummary = ref('')
const editContent = ref('')
const editType = ref('')

// 排序方式
const sortBy = ref<'time' | 'type'>('time')

// 排序后的条目
const sortedEntries = computed(() => {
  const list = [...props.entries]
  if (sortBy.value === 'time') {
    return list.sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime())
  } else {
    return list.sort((a, b) => a.memory_type.localeCompare(b.memory_type))
  }
})

// 显示的条目（搜索结果或全部）
const displayEntries = computed(() => {
  if (props.searchResults) {
    return props.searchResults.map(r => ({ ...r.memory, score: r.score }))
  }
  return sortedEntries.value.map(e => ({ ...e, score: undefined }))
})

function handleSearch() {
  if (!searchQuery.value.trim()) {
    emit('clearSearch')
    return
  }
  isSearching.value = true
  emit('search', searchQuery.value.trim())
}

function clearSearch() {
  searchQuery.value = ''
  emit('clearSearch')
}

function startEdit(entry: VectorMemoryEntry) {
  editingId.value = entry.id
  editSummary.value = entry.summary
  editContent.value = entry.content
  editType.value = entry.memory_type
}

function saveEdit() {
  if (editingId.value) {
    emit('edit', editingId.value, {
      summary: editSummary.value,
      content: editContent.value,
      memory_type: editType.value
    })
    editingId.value = null
  }
}

function cancelEdit() {
  editingId.value = null
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

function truncateText(text: string, maxLength = 80): string {
  if (!text || text.length <= maxLength) return text
  return text.slice(0, maxLength) + '...'
}

// 监听搜索结果变化
watch(() => props.searchResults, () => {
  isSearching.value = false
})
</script>

<template>
  <div class="flex flex-col h-full">
    <!-- 头部操作栏 -->
    <div class="px-3 py-2 border-b border-dark-700 space-y-2">
      <!-- 搜索框 -->
      <div class="flex gap-2">
        <input
          v-model="searchQuery"
          type="text"
          placeholder="向量搜索..."
          class="flex-1 bg-dark-700 border border-dark-600 rounded px-2 py-1 text-sm"
          @keydown.enter="handleSearch"
        />
        <button
          @click="handleSearch"
          :disabled="isSearching"
          class="px-2 py-1 bg-primary-600 hover:bg-primary-700 rounded text-xs disabled:opacity-50"
        >
          {{ isSearching ? '...' : '搜索' }}
        </button>
        <button
          v-if="searchResults"
          @click="clearSearch"
          class="px-2 py-1 bg-dark-700 hover:bg-dark-600 rounded text-xs"
        >
          清除
        </button>
      </div>
      
      <!-- 统计和排序 -->
      <div class="flex items-center justify-between text-xs text-dark-500">
        <span>
          {{ searchResults ? `${searchResults.length} 条结果` : `${entries.length} 条记忆` }}
        </span>
        <div class="flex items-center gap-2">
          <button
            @click="emit('refresh')"
            class="hover:text-dark-300"
            title="刷新"
          >
            🔄
          </button>
          <select
            v-model="sortBy"
            class="bg-dark-700 border border-dark-600 rounded px-1 py-0.5 text-xs"
          >
            <option value="time">按时间</option>
            <option value="type">按类型</option>
          </select>
        </div>
      </div>
    </div>
    
    <!-- 内容列表 -->
    <div class="flex-1 overflow-y-auto p-3 space-y-2">
      <div v-if="isLoading" class="text-center text-dark-500 text-sm py-4">
        加载中...
      </div>
      <div v-else-if="displayEntries.length === 0" class="text-center text-dark-500 text-sm py-4">
        {{ searchResults ? '无搜索结果' : '暂无对话记忆' }}
      </div>
      <div 
        v-else
        v-for="entry in displayEntries" 
        :key="entry.id"
        class="bg-dark-800 rounded p-2 text-sm group relative"
      >
        <!-- 编辑模式 -->
        <div v-if="editingId === entry.id" class="space-y-2">
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
            <span class="font-medium text-dark-200 flex-1 truncate">{{ entry.summary }}</span>
            <span class="text-xs px-1.5 py-0.5 bg-primary-600/20 text-primary-400 rounded shrink-0">
              {{ entry.memory_type }}
            </span>
            <!-- 相似度分数（搜索结果时显示） -->
            <span 
              v-if="(entry as any).score !== undefined"
              class="text-xs px-1.5 py-0.5 bg-green-600/20 text-green-400 rounded shrink-0"
            >
              {{ ((entry as any).score * 100).toFixed(0) }}%
            </span>
          </div>
          <p class="text-dark-400 text-xs whitespace-pre-wrap mb-2">{{ truncateText(entry.content) }}</p>
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2 text-xs text-dark-500">
              <span>{{ formatTime(entry.timestamp) }}</span>
              <span class="text-dark-600">|</span>
              <span>{{ entry.source }}</span>
            </div>
            <div class="opacity-0 group-hover:opacity-100 transition-opacity flex gap-1">
              <button 
                @click="startEdit(entry)"
                class="text-xs px-1.5 py-0.5 bg-dark-700 hover:bg-dark-600 rounded"
              >
                编辑
              </button>
              <button 
                @click="emit('delete', entry.id)"
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