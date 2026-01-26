<script setup lang="ts">
import type { ThinkingEntry } from '@/types'

const props = defineProps<{
  entries: ThinkingEntry[]
  isLoading: boolean
}>()

const emit = defineEmits<{
  add: []
  edit: [index: number, content: string]
  delete: [index: number]
  save: []
}>()

// 编辑状态
import { ref } from 'vue'
const editingIndex = ref<number | null>(null)
const editContent = ref('')

function startEdit(index: number) {
  editingIndex.value = index
  editContent.value = props.entries[index].content
}

function saveEdit() {
  if (editingIndex.value !== null) {
    emit('edit', editingIndex.value, editContent.value)
    editingIndex.value = null
    editContent.value = ''
  }
}

function cancelEdit() {
  editingIndex.value = null
  editContent.value = ''
}

function handleDelete(index: number) {
  emit('delete', index)
}

function handleAdd() {
  emit('add')
}
</script>

<template>
  <div class="flex flex-col h-full">
    <!-- 头部操作栏 -->
    <div class="flex items-center justify-between px-4 py-2 border-b border-dark-700">
      <span class="text-xs text-dark-500">{{ entries.length }} 条</span>
      <button 
        @click="handleAdd"
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
        暂无思考内容
      </div>
      <div 
        v-else
        v-for="(entry, index) in entries" 
        :key="index"
        class="bg-dark-800 rounded p-2 text-sm group relative"
      >
        <!-- 编辑模式 -->
        <div v-if="editingIndex === index">
          <textarea
            v-model="editContent"
            class="w-full bg-dark-700 border border-dark-600 rounded p-2 text-sm resize-none"
            rows="3"
            @keydown.ctrl.enter="saveEdit"
            @keydown.escape="cancelEdit"
          ></textarea>
          <div class="flex justify-end gap-2 mt-2">
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
          <p class="text-dark-300 whitespace-pre-wrap">{{ entry.content }}</p>
          <div class="flex items-center justify-between mt-1">
            <span class="text-xs text-dark-500">{{ entry.source }}</span>
            <div class="opacity-0 group-hover:opacity-100 transition-opacity flex gap-1">
              <button 
                @click="startEdit(index)"
                class="text-xs px-1.5 py-0.5 bg-dark-700 hover:bg-dark-600 rounded"
              >
                编辑
              </button>
              <button 
                @click="handleDelete(index)"
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