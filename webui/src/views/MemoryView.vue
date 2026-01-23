<script setup lang="ts">
import { ref, onMounted, watch, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { memoryApi } from '@/api'
import { useAppStore, useAssistantStore } from '@/stores'
import type { MemorySearchResult, PendingStatus } from '@/types'

const route = useRoute()
const router = useRouter()
const app = useAppStore()
const assistantStore = useAssistantStore()

// 从路由获取 assistantId
const assistantId = computed(() => route.params.assistantId as string)

// 获取当前助手名称
const currentAssistantName = computed(() => {
  const list = assistantStore.assistants || []
  const assistant = Array.isArray(list) ? list.find(a => a.id === assistantId.value) : null
  return assistant?.name || '未知助手'
})

const searchQuery = ref('')
const categoryFilter = ref('')
const memories = ref<MemorySearchResult[]>([])
const totalCount = ref(0)
const pendingStatus = ref<PendingStatus | null>(null)
const loading = ref(false)

const showCreateModal = ref(false)
const showPendingModal = ref(false)

const newMemory = ref({
  content: '',
  category: 'fact',
  importance: 0.5,
  tags: '',
})

const categories = [
  { value: '', label: '全部分类' },
  { value: 'fact', label: '事实' },
  { value: 'preference', label: '偏好' },
  { value: 'event', label: '事件' },
  { value: 'knowledge', label: '知识' },
]

onMounted(async () => {
  // 加载助手列表
  await assistantStore.loadAssistants()
  
  if (assistantId.value) {
    loadMemories()
    loadPendingStatus()
  }
})

// 监听 assistantId 变化
watch(assistantId, (newId) => {
  if (newId) {
    loadMemories()
    loadPendingStatus()
  }
})

async function loadMemories() {
  if (!assistantId.value) return
  
  loading.value = true
  try {
    const result = await memoryApi.search(assistantId.value, searchQuery.value, categoryFilter.value || undefined)
    memories.value = result.memories
    totalCount.value = result.total
  } catch {
    app.showToast('加载记忆失败', 'error')
  } finally {
    loading.value = false
  }
}

async function loadPendingStatus() {
  if (!assistantId.value) return
  
  try {
    pendingStatus.value = await memoryApi.getPending(assistantId.value)
  } catch {
    // 静默失败
  }
}

async function createMemory() {
  if (!assistantId.value) return
  
  try {
    const tags = newMemory.value.tags.split(',').map(t => t.trim()).filter(t => t)
    await memoryApi.create(assistantId.value, {
      content: newMemory.value.content,
      category: newMemory.value.category,
      importance: newMemory.value.importance,
      tags,
    })
    app.showToast('记忆创建成功', 'success')
    showCreateModal.value = false
    newMemory.value = { content: '', category: 'fact', importance: 0.5, tags: '' }
    loadMemories()
  } catch {
    app.showToast('创建失败', 'error')
  }
}

async function deleteMemory(id: string) {
  if (!assistantId.value) return
  if (!confirm('确定要删除这条记忆吗？')) return
  
  try {
    await memoryApi.delete(assistantId.value, id)
    app.showToast('删除成功', 'success')
    loadMemories()
  } catch {
    app.showToast('删除失败', 'error')
  }
}

async function processPending() {
  if (!assistantId.value) return
  
  try {
    const result = await memoryApi.processPending(assistantId.value)
    app.showToast(`处理完成: ${result.processed}条成功`, 'success')
    loadPendingStatus()
    loadMemories()
  } catch {
    app.showToast('处理失败', 'error')
  }
}

async function clearPending() {
  if (!assistantId.value) return
  if (!confirm('确定要清空所有待处理记忆吗？')) return
  
  try {
    await memoryApi.clearPending(assistantId.value)
    app.showToast('已清空', 'success')
    loadPendingStatus()
  } catch {
    app.showToast('清空失败', 'error')
  }
}

function getImportanceClass(importance: number) {
  if (importance >= 0.7) return 'text-yellow-400'
  if (importance >= 0.4) return 'text-green-400'
  return 'text-dark-500'
}

function formatDate(dateStr: string) {
  const d = new Date(dateStr)
  return d.toLocaleDateString('zh-CN') + ' ' + d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
}

function goBack() {
  router.push(`/chat/${assistantId.value}`)
}
</script>

<template>
  <div class="h-full overflow-y-auto p-6">
    <div class="max-w-5xl mx-auto">
      <div class="flex items-center justify-between mb-6">
        <div>
          <div class="flex items-center gap-3 mb-1">
            <button @click="goBack" class="text-dark-400 hover:text-dark-200 transition-colors">
              ← 返回
            </button>
            <h1 class="text-2xl font-bold">🧠 {{ currentAssistantName }} 的记忆</h1>
          </div>
          <p class="text-sm text-dark-500 mt-1">
            记忆总数: <span class="text-primary-400">{{ totalCount }}</span>
            <span v-if="pendingStatus" class="ml-4">
              待处理: <span class="text-yellow-400">{{ pendingStatus.pending_count }}</span>
              <button @click="showPendingModal = true" class="ml-2 text-xs text-primary-400 hover:underline">管理</button>
            </span>
          </p>
        </div>
        <button @click="showCreateModal = true" class="px-4 py-2 bg-primary-600 hover:bg-primary-700 rounded-lg transition-colors">
          + 新建记忆
        </button>
      </div>

      <div class="flex gap-3 mb-6">
        <input v-model="searchQuery" @keyup.enter="loadMemories" placeholder="搜索记忆内容..." 
          class="flex-1 px-4 py-2 bg-dark-800 border border-dark-600 rounded-lg focus:outline-none focus:border-primary-500" />
        <select v-model="categoryFilter" @change="loadMemories" 
          class="px-4 py-2 bg-dark-800 border border-dark-600 rounded-lg">
          <option v-for="cat in categories" :key="cat.value" :value="cat.value">{{ cat.label }}</option>
        </select>
        <button @click="loadMemories" class="px-4 py-2 bg-dark-700 hover:bg-dark-600 rounded-lg">搜索</button>
      </div>

      <div class="space-y-3">
        <div v-for="item in memories" :key="item.memory.id" 
          class="bg-dark-800 border border-dark-700 rounded-xl p-4 hover:border-primary-600/30 transition-colors">
          <div class="flex items-start justify-between mb-2">
            <div class="flex items-center gap-3">
              <span class="px-2 py-0.5 bg-primary-600/20 text-primary-400 rounded text-xs">{{ item.memory.category }}</span>
              <span :class="getImportanceClass(item.memory.importance)" class="text-xs">重要性: {{ item.memory.importance }}</span>
              <span v-if="item.score" class="text-xs text-dark-500 bg-dark-700 px-2 py-0.5 rounded">
                相关度: {{ (item.score * 100).toFixed(1) }}%
              </span>
            </div>
            <button @click="deleteMemory(item.memory.id)" class="text-red-400 hover:text-red-300 text-sm">删除</button>
          </div>
          <p class="text-dark-200 whitespace-pre-wrap mb-3">{{ item.memory.content }}</p>
          <div class="flex items-center justify-between text-xs text-dark-500">
            <div class="flex gap-2">
              <span v-for="tag in item.memory.tags" :key="tag" class="bg-dark-700 px-2 py-0.5 rounded">{{ tag }}</span>
            </div>
            <span>访问 {{ item.memory.access_count }} 次 · {{ formatDate(item.memory.created_at) }}</span>
          </div>
        </div>
        <div v-if="memories.length === 0 && !loading" class="text-center py-12 text-dark-500">
          <div class="text-4xl mb-4">📭</div>
          <p>暂无记忆</p>
        </div>
      </div>
    </div>

    <!-- 新建记忆模态框 -->
    <Teleport to="body">
      <div v-if="showCreateModal" class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4" @click.self="showCreateModal = false">
        <div class="bg-dark-900 border border-dark-700 rounded-2xl w-full max-w-lg">
          <div class="p-6 border-b border-dark-700"><h2 class="text-xl font-semibold">新建记忆</h2></div>
          <form @submit.prevent="createMemory" class="p-6 space-y-4">
            <div>
              <label class="block text-sm text-dark-400 mb-1">内容 *</label>
              <textarea v-model="newMemory.content" required rows="4" 
                class="w-full px-4 py-2 bg-dark-800 border border-dark-600 rounded-lg focus:outline-none focus:border-primary-500 resize-none" />
            </div>
            <div class="grid grid-cols-2 gap-4">
              <div>
                <label class="block text-sm text-dark-400 mb-1">分类</label>
                <select v-model="newMemory.category" class="w-full px-4 py-2 bg-dark-800 border border-dark-600 rounded-lg">
                  <option value="fact">事实</option>
                  <option value="preference">偏好</option>
                  <option value="event">事件</option>
                  <option value="knowledge">知识</option>
                </select>
              </div>
              <div>
                <label class="block text-sm text-dark-400 mb-1">重要性</label>
                <input v-model.number="newMemory.importance" type="number" min="0" max="1" step="0.1" 
                  class="w-full px-4 py-2 bg-dark-800 border border-dark-600 rounded-lg" />
              </div>
            </div>
            <div>
              <label class="block text-sm text-dark-400 mb-1">标签 (逗号分隔)</label>
              <input v-model="newMemory.tags" class="w-full px-4 py-2 bg-dark-800 border border-dark-600 rounded-lg" placeholder="标签1, 标签2" />
            </div>
            <div class="flex justify-end gap-3 pt-4">
              <button type="button" @click="showCreateModal = false" class="px-4 py-2 bg-dark-700 hover:bg-dark-600 rounded-lg">取消</button>
              <button type="submit" class="px-4 py-2 bg-primary-600 hover:bg-primary-700 rounded-lg">保存</button>
            </div>
          </form>
        </div>
      </div>
    </Teleport>

    <!-- 待处理池模态框 -->
    <Teleport to="body">
      <div v-if="showPendingModal" class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4" @click.self="showPendingModal = false">
        <div class="bg-dark-900 border border-dark-700 rounded-2xl w-full max-w-2xl">
          <div class="p-6 border-b border-dark-700 flex items-center justify-between">
            <h2 class="text-xl font-semibold">📋 待处理池管理</h2>
            <button @click="showPendingModal = false" class="p-2 hover:bg-dark-700 rounded-lg">✕</button>
          </div>
          <div class="p-6">
            <div class="flex items-center justify-between mb-4 p-4 bg-dark-800 rounded-lg">
              <div>
                <span class="text-dark-400">待处理数量:</span>
                <span class="text-2xl text-yellow-400 ml-2">{{ pendingStatus?.pending_count || 0 }}</span>
              </div>
              <div class="flex gap-2">
                <button @click="processPending" class="px-4 py-2 bg-green-600 hover:bg-green-700 rounded-lg text-sm">✅ 全部处理</button>
                <button @click="clearPending" class="px-4 py-2 bg-red-600 hover:bg-red-700 rounded-lg text-sm">🗑️ 全部清空</button>
              </div>
            </div>
            <div v-if="pendingStatus?.preview.length" class="space-y-2 max-h-80 overflow-y-auto">
              <div v-for="(m, i) in pendingStatus.preview" :key="i" class="p-3 bg-dark-800 rounded-lg border-l-2 border-yellow-500">
                <div class="flex justify-between text-xs text-dark-500 mb-1">
                  <span>#{{ i + 1 }} · {{ m.category }}</span>
                  <span>重要性: {{ m.importance }}</span>
                </div>
                <p class="text-dark-200 text-sm">{{ m.content }}</p>
              </div>
            </div>
            <div v-else class="text-center py-8 text-dark-500">待处理池为空</div>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>