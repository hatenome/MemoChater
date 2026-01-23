<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useAssistantStore, useAppStore } from '@/stores'
import { modelsApi, assistantsApi, processorsApi } from '@/api'
import type { AssistantConfig, PipelineConfig, ProcessorEntry } from '@/types'

const router = useRouter()
const store = useAssistantStore()
const app = useAppStore()

const showCreateModal = ref(false)
const editingId = ref<string | null>(null)
const formData = ref({
  name: '',
  description: '',
  system_prompt: '',
  main_model: 'gpt-4o-mini',
  processor_model: 'gpt-4o-mini',
  embedding_model: 'text-embedding-3-small',
  extractor_model: 'gpt-4o-mini',
  temperature: 0.7,
  max_tokens: 4096,
  user_name: '用户',
  assistant_name: '助手',
  memory_enabled: true,
  retrieval_count: 5,
  relevance_threshold: 0.6,
})

// 动态模型列表
const modelOptions = ref<string[]>([])
const embeddingModelOptions = ref<string[]>([])
const loadingModels = ref(false)

// 流水线配置
const showPipelineModal = ref(false)
const pipelineAssistantId = ref<string | null>(null)
const pipelineAssistantName = ref('')
const pipelineConfig = ref<PipelineConfig>({
  on_user_message: [],
  before_ai_call: [],
  on_stream_start: [],
  on_stream_chunk: [],
  after_ai_response: [],
  background_process: [],
})
const savingPipeline = ref(false)

// 可用的处理器列表（从后端动态加载）
const availableProcessors = ref<{ name: string; requires_memory: boolean }[]>([])
const loadingProcessors = ref(false)

// 时机配置
const pipelineTimings = [
  { key: 'on_user_message', label: '用户发言后', description: '用户消息追加到对话后执行' },
  { key: 'before_ai_call', label: 'AI调用前', description: '发送给AI API前执行' },
  { key: 'on_stream_start', label: '流式开始', description: '开始收到AI响应时执行（预留）' },
  { key: 'on_stream_chunk', label: '流式块', description: '收到每个chunk时执行（预留）' },
  { key: 'after_ai_response', label: 'AI响应后', description: 'AI响应完整接收后执行（同步，阻塞下一次对话）' },
  { key: 'background_process', label: '后台处理', description: '异步执行，不阻塞下一次对话' },
] as const

// 加载模型列表
async function loadModels() {
  loadingModels.value = true
  console.log('[loadModels] 开始加载模型列表...')
  try {
    const models = await modelsApi.list()
    console.log('[loadModels] 获取到模型:', models)
    // 分类：embedding 模型和普通模型
    const allModels = models.map(m => m.id)
    console.log('[loadModels] 所有模型ID:', allModels)
    embeddingModelOptions.value = allModels.filter(m => 
      m.includes('embedding') || m.includes('embed')
    )
    modelOptions.value = allModels.filter(m => 
      !m.includes('embedding') && !m.includes('embed')
    )
    
    // 如果没有分出 embedding 模型，提供默认值
    if (embeddingModelOptions.value.length === 0) {
      embeddingModelOptions.value = ['text-embedding-3-small', 'text-embedding-3-large']
    }
  } catch (e) {
    console.error('加载模型列表失败:', e)
    // 使用默认值
    modelOptions.value = ['gpt-4o', 'gpt-4o-mini', 'gpt-3.5-turbo']
    embeddingModelOptions.value = ['text-embedding-3-small', 'text-embedding-3-large']
  } finally {
    loadingModels.value = false
  }
}

// 加载处理器列表
async function loadProcessors() {
  loadingProcessors.value = true
  try {
    const processors = await processorsApi.list()
    availableProcessors.value = processors
    console.log('[loadProcessors] 获取到处理器:', processors)
  } catch (e) {
    console.error('加载处理器列表失败:', e)
    // 使用默认值
    availableProcessors.value = [
      { name: 'HistorySimplifier', requires_memory: true },
      { name: 'MemoryAssembler', requires_memory: true },
      { name: 'SubconsciousProcessor', requires_memory: true },
      { name: 'ContentChunker', requires_memory: true },
      { name: 'MemoryCommitter', requires_memory: true },
    ]
  } finally {
    loadingProcessors.value = false
  }
}

onMounted(() => {
  store.loadAssistants()
  loadModels()
  loadProcessors()
})

function openCreateModal() {
  editingId.value = null
  formData.value = {
    name: '',
    description: '',
    system_prompt: '',
    main_model: 'gpt-4o-mini',
    processor_model: 'gpt-4o-mini',
    embedding_model: 'text-embedding-3-small',
    extractor_model: 'gpt-4o-mini',
    temperature: 0.7,
    max_tokens: 4096,
    user_name: '用户',
    assistant_name: '助手',
    memory_enabled: true,
    retrieval_count: 5,
    relevance_threshold: 0.6,
  }
  showCreateModal.value = true
}

async function openEditModal(id: string) {
  try {
    const config = await store.selectAssistant(id)
    editingId.value = id
    if (store.currentAssistantConfig) {
      const c = store.currentAssistantConfig
      formData.value = {
        name: c.name,
        description: c.description,
        system_prompt: c.system_prompt,
        main_model: c.model.main_model,
        processor_model: c.model.processor_model,
        embedding_model: c.model.embedding_model,
        extractor_model: c.model.extractor_model,
        temperature: c.model.temperature,
        max_tokens: c.model.max_tokens,
        user_name: c.roles.user_name,
        assistant_name: c.roles.assistant_name,
        memory_enabled: c.memory.enabled,
        retrieval_count: c.memory.retrieval_count,
        relevance_threshold: c.memory.relevance_threshold,
      }
    }
    showCreateModal.value = true
  } catch {
    // 错误已处理
  }
}

async function saveAssistant() {
  const data: Partial<AssistantConfig> = {
    name: formData.value.name,
    description: formData.value.description,
    system_prompt: formData.value.system_prompt,
    model: {
      main_model: formData.value.main_model,
      processor_model: formData.value.processor_model,
      embedding_model: formData.value.embedding_model,
      extractor_model: formData.value.extractor_model,
      temperature: formData.value.temperature,
      max_tokens: formData.value.max_tokens,
    },
    roles: {
      user_name: formData.value.user_name,
      assistant_name: formData.value.assistant_name,
    },
    memory: {
      enabled: formData.value.memory_enabled,
      retrieval_count: formData.value.retrieval_count,
      relevance_threshold: formData.value.relevance_threshold,
    },
  }

  try {
    if (editingId.value) {
      await store.updateAssistant(editingId.value, data)
    } else {
      await store.createAssistant(data)
    }
    showCreateModal.value = false
  } catch {
    // 错误已处理
  }
}

async function deleteAssistant(id: string) {
  if (!confirm('确定要删除这个助手吗？所有相关话题和对话历史都将被删除。')) return
  await store.deleteAssistant(id)
}

function goToChat(id: string) {
  router.push(`/chat/${id}`)
}

function formatDate(dateStr: string) {
  return new Date(dateStr).toLocaleDateString('zh-CN')
}

// 流水线配置相关方法
function getDefaultProcessors(names: string[]): ProcessorEntry[] {
  return names.map(name => {
    return {
      name,
      description: ''
    }
  })
}

async function openPipelineModal(id: string) {
  try {
    const config = await store.selectAssistant(id)
    pipelineAssistantId.value = id
    pipelineAssistantName.value = store.currentAssistantConfig?.name || '助手'
    
    // 加载流水线配置，如果没有则使用默认值
    const pipeline = store.currentAssistantConfig?.pipeline
    pipelineConfig.value = {
      on_user_message: pipeline?.on_user_message || getDefaultProcessors(['HistorySimplifier', 'MemoryAssembler']),
      before_ai_call: pipeline?.before_ai_call || [],
      on_stream_start: pipeline?.on_stream_start || [],
      on_stream_chunk: pipeline?.on_stream_chunk || [],
      after_ai_response: pipeline?.after_ai_response || getDefaultProcessors(['SubconsciousProcessor', 'ContentChunker', 'MemoryCommitter']),
      background_process: pipeline?.background_process || [],
    }
    showPipelineModal.value = true
  } catch {
    app.showToast('加载流水线配置失败', 'error')
  }
}

function addProcessor(timingKey: keyof PipelineConfig) {
  const processors = pipelineConfig.value[timingKey]
  // 找一个还没添加的处理器
  const usedNames = processors.map(p => p.name)
  const available = availableProcessors.value.filter(p => !usedNames.includes(p.name))
  if (available.length > 0) {
    processors.push({
      name: available[0].name,
      description: ''
    })
  }
}

function removeProcessor(timingKey: keyof PipelineConfig, index: number) {
  pipelineConfig.value[timingKey].splice(index, 1)
}

function moveProcessor(timingKey: keyof PipelineConfig, index: number, direction: 'up' | 'down') {
  const processors = pipelineConfig.value[timingKey]
  const newIndex = direction === 'up' ? index - 1 : index + 1
  if (newIndex < 0 || newIndex >= processors.length) return
  
  const temp = processors[index]
  processors[index] = processors[newIndex]
  processors[newIndex] = temp
}

function updateProcessorName(timingKey: keyof PipelineConfig, index: number, newName: string) {
  const entry = pipelineConfig.value[timingKey][index]
  entry.name = newName
}

function updateProcessorDescription(timingKey: keyof PipelineConfig, index: number, description: string) {
  pipelineConfig.value[timingKey][index].description = description
}

async function savePipelineConfig() {
  if (!pipelineAssistantId.value) return
  
  savingPipeline.value = true
  try {
    await store.updateAssistant(pipelineAssistantId.value, {
      pipeline: pipelineConfig.value
    })
    app.showToast('流水线配置已保存', 'success')
    showPipelineModal.value = false
  } catch {
    app.showToast('保存失败', 'error')
  } finally {
    savingPipeline.value = false
  }
}
</script>

<template>
  <div class="h-full overflow-y-auto p-6">
    <div class="max-w-4xl mx-auto">
      <!-- 头部 -->
      <div class="flex items-center justify-between mb-6">
        <h1 class="text-2xl font-bold">助手管理</h1>
        <button
          @click="openCreateModal"
          class="px-4 py-2 bg-primary-600 hover:bg-primary-700 rounded-lg transition-colors flex items-center gap-2"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
          </svg>
          新建助手
        </button>
      </div>

      <!-- 助手列表 -->
      <div class="grid gap-4">
        <div
          v-for="assistant in store.assistants"
          :key="assistant.id"
          class="bg-dark-800 border border-dark-700 rounded-xl p-5 hover:border-primary-600/50 transition-colors"
        >
          <div class="flex items-start justify-between">
            <div class="flex items-center gap-4">
              <div class="w-12 h-12 bg-primary-600/20 rounded-xl flex items-center justify-center text-2xl">
                🤖
              </div>
              <div>
                <h3 class="font-semibold text-lg">{{ assistant.name }}</h3>
                <p class="text-sm text-dark-400">{{ assistant.description || '暂无描述' }}</p>
              </div>
            </div>
            <div class="flex items-center gap-2">
              <button
                @click="goToChat(assistant.id)"
                class="px-3 py-1.5 bg-primary-600/20 text-primary-400 hover:bg-primary-600/30 rounded-lg text-sm transition-colors"
              >
                开始对话
              </button>
              <button
                @click="openEditModal(assistant.id)"
                class="px-3 py-1.5 bg-dark-700 hover:bg-dark-600 rounded-lg text-sm transition-colors"
              >
                编辑
              </button>
              <button
                @click="openPipelineModal(assistant.id)"
                class="px-3 py-1.5 bg-emerald-600/20 text-emerald-400 hover:bg-emerald-600/30 rounded-lg text-sm transition-colors"
              >
                流水线
              </button>
              <button
                @click="deleteAssistant(assistant.id)"
                class="px-3 py-1.5 bg-red-600/20 text-red-400 hover:bg-red-600/30 rounded-lg text-sm transition-colors"
              >
                删除
              </button>
            </div>
          </div>
          <div class="mt-4 flex items-center gap-6 text-sm text-dark-500">
            <span>话题数: {{ assistant.topic_count }}</span>
            <span>创建于: {{ formatDate(assistant.created_at) }}</span>
          </div>
        </div>

        <!-- 空状态 -->
        <div
          v-if="store.assistants.length === 0"
          class="text-center py-12 text-dark-500"
        >
          <div class="text-4xl mb-4">🤖</div>
          <p>还没有助手，点击上方按钮创建一个吧</p>
        </div>
      </div>
    </div>

    <!-- 创建/编辑模态框 -->
    <Teleport to="body">
      <div
        v-if="showCreateModal"
        class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4"
        @mousedown.self="showCreateModal = false"
      >
        <div class="bg-dark-900 border border-dark-700 rounded-2xl w-full max-w-3xl max-h-[90vh] overflow-y-auto">
          <div class="p-6 border-b border-dark-700 flex items-center justify-between sticky top-0 bg-dark-900">
            <h2 class="text-xl font-semibold">
              {{ editingId ? '编辑助手' : '新建助手' }}
            </h2>
            <button
              @click="showCreateModal = false"
              class="p-2 hover:bg-dark-700 rounded-lg transition-colors"
            >
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>

          <form @submit.prevent="saveAssistant" class="p-6 space-y-6">
            <!-- 基本信息 -->
            <div class="space-y-4">
              <h3 class="text-sm font-medium text-dark-400 border-b border-dark-700 pb-2">基本信息</h3>
              
              <div>
                <label class="block text-sm text-dark-400 mb-1">名称 *</label>
                <input
                  v-model="formData.name"
                  required
                  class="w-full px-4 py-2 bg-dark-800 border border-dark-600 rounded-lg focus:outline-none focus:border-primary-500"
                  placeholder="助手名称"
                />
              </div>

              <div>
                <label class="block text-sm text-dark-400 mb-1">描述</label>
                <input
                  v-model="formData.description"
                  class="w-full px-4 py-2 bg-dark-800 border border-dark-600 rounded-lg focus:outline-none focus:border-primary-500"
                  placeholder="简短描述"
                />
              </div>

              <div>
                <label class="block text-sm text-dark-400 mb-1">系统提示词</label>
                <textarea
                  v-model="formData.system_prompt"
                  rows="4"
                  class="w-full px-4 py-2 bg-dark-800 border border-dark-600 rounded-lg focus:outline-none focus:border-primary-500 resize-none"
                  placeholder="定义助手的角色和行为..."
                />
              </div>
            </div>

            <!-- 模型配置 -->
            <div class="space-y-4">
              <h3 class="text-sm font-medium text-dark-400 border-b border-dark-700 pb-2">模型配置</h3>
              
              <div class="grid grid-cols-2 gap-4">
                <div>
                  <label class="block text-sm text-dark-400 mb-1">主模型 (对话)</label>
                  <select
                    v-model="formData.main_model"
                    :disabled="loadingModels"
                    class="w-full px-4 py-2 bg-dark-800 border border-dark-600 rounded-lg focus:outline-none focus:border-primary-500 disabled:opacity-50"
                  >
                    <option v-if="loadingModels" value="">加载中...</option>
                    <option v-for="m in modelOptions" :key="m" :value="m">{{ m }}</option>
                  </select>
                </div>
                <div>
                  <label class="block text-sm text-dark-400 mb-1">处理模型 (记忆处理)</label>
                  <select
                    v-model="formData.processor_model"
                    :disabled="loadingModels"
                    class="w-full px-4 py-2 bg-dark-800 border border-dark-600 rounded-lg focus:outline-none focus:border-primary-500 disabled:opacity-50"
                  >
                    <option v-if="loadingModels" value="">加载中...</option>
                    <option v-for="m in modelOptions" :key="m" :value="m">{{ m }}</option>
                  </select>
                </div>
              </div>

              <div class="grid grid-cols-2 gap-4">
                <div>
                  <label class="block text-sm text-dark-400 mb-1">提取模型 (信息提取)</label>
                  <select
                    v-model="formData.extractor_model"
                    :disabled="loadingModels"
                    class="w-full px-4 py-2 bg-dark-800 border border-dark-600 rounded-lg focus:outline-none focus:border-primary-500 disabled:opacity-50"
                  >
                    <option v-if="loadingModels" value="">加载中...</option>
                    <option v-for="m in modelOptions" :key="m" :value="m">{{ m }}</option>
                  </select>
                </div>
                <div>
                  <label class="block text-sm text-dark-400 mb-1">Embedding 模型</label>
                  <select
                    v-model="formData.embedding_model"
                    :disabled="loadingModels"
                    class="w-full px-4 py-2 bg-dark-800 border border-dark-600 rounded-lg focus:outline-none focus:border-primary-500 disabled:opacity-50"
                  >
                    <option v-if="loadingModels" value="">加载中...</option>
                    <option v-for="m in embeddingModelOptions" :key="m" :value="m">{{ m }}</option>
                  </select>
                </div>
              </div>

              <div class="grid grid-cols-2 gap-4">
                <div>
                  <label class="block text-sm text-dark-400 mb-1">温度 ({{ formData.temperature }})</label>
                  <input
                    v-model.number="formData.temperature"
                    type="range"
                    min="0"
                    max="2"
                    step="0.1"
                    class="w-full"
                  />
                </div>
                <div>
                  <label class="block text-sm text-dark-400 mb-1">最大输出 Token</label>
                  <input
                    v-model.number="formData.max_tokens"
                    type="number"
                    min="256"
                    max="128000"
                    class="w-full px-4 py-2 bg-dark-800 border border-dark-600 rounded-lg focus:outline-none focus:border-primary-500"
                  />
                </div>
              </div>
            </div>

            <!-- 角色配置 -->
            <div class="space-y-4">
              <h3 class="text-sm font-medium text-dark-400 border-b border-dark-700 pb-2">角色名称</h3>
              
              <div class="grid grid-cols-2 gap-4">
                <div>
                  <label class="block text-sm text-dark-400 mb-1">用户名称</label>
                  <input
                    v-model="formData.user_name"
                    class="w-full px-4 py-2 bg-dark-800 border border-dark-600 rounded-lg focus:outline-none focus:border-primary-500"
                    placeholder="用户"
                  />
                </div>
                <div>
                  <label class="block text-sm text-dark-400 mb-1">助手名称</label>
                  <input
                    v-model="formData.assistant_name"
                    class="w-full px-4 py-2 bg-dark-800 border border-dark-600 rounded-lg focus:outline-none focus:border-primary-500"
                    placeholder="助手"
                  />
                </div>
              </div>
            </div>

            <!-- 记忆配置 -->
            <div class="space-y-4">
              <h3 class="text-sm font-medium text-dark-400 border-b border-dark-700 pb-2">记忆配置</h3>
              
              <div class="flex items-center gap-2">
                <input
                  v-model="formData.memory_enabled"
                  type="checkbox"
                  id="memory_enabled"
                  class="w-4 h-4 rounded"
                />
                <label for="memory_enabled" class="text-sm">启用长期记忆</label>
              </div>

              <div v-if="formData.memory_enabled" class="grid grid-cols-2 gap-4">
                <div>
                  <label class="block text-sm text-dark-400 mb-1">检索数量</label>
                  <input
                    v-model.number="formData.retrieval_count"
                    type="number"
                    min="1"
                    max="20"
                    class="w-full px-4 py-2 bg-dark-800 border border-dark-600 rounded-lg focus:outline-none focus:border-primary-500"
                  />
                </div>
                <div>
                  <label class="block text-sm text-dark-400 mb-1">相关性阈值 ({{ formData.relevance_threshold }})</label>
                  <input
                    v-model.number="formData.relevance_threshold"
                    type="range"
                    min="0"
                    max="1"
                    step="0.05"
                    class="w-full"
                  />
                </div>
              </div>
            </div>

            <div class="flex justify-end gap-3 pt-4 border-t border-dark-700">
              <button
                type="button"
                @click="showCreateModal = false"
                class="px-4 py-2 bg-dark-700 hover:bg-dark-600 rounded-lg transition-colors"
              >
                取消
              </button>
              <button
                type="submit"
                class="px-4 py-2 bg-primary-600 hover:bg-primary-700 rounded-lg transition-colors"
              >
                保存
              </button>
            </div>
          </form>
        </div>
      </div>
    </Teleport>

    <!-- 流水线配置模态框 -->
    <Teleport to="body">
      <div
        v-if="showPipelineModal"
        class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4"
        @mousedown.self="showPipelineModal = false"
      >
        <div class="bg-dark-900 border border-dark-700 rounded-2xl w-full max-w-4xl max-h-[90vh] overflow-y-auto">
          <div class="p-6 border-b border-dark-700 flex items-center justify-between sticky top-0 bg-dark-900">
            <div>
              <h2 class="text-xl font-semibold">流水线配置</h2>
              <p class="text-sm text-dark-400 mt-1">{{ pipelineAssistantName }}</p>
            </div>
            <button
              @click="showPipelineModal = false"
              class="p-2 hover:bg-dark-700 rounded-lg transition-colors"
            >
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>

          <div class="p-6 space-y-6">
            <!-- 各时机的处理器配置 -->
            <div
              v-for="timing in pipelineTimings"
              :key="timing.key"
              class="border border-dark-700 rounded-xl p-4"
            >
              <div class="flex items-center justify-between mb-3">
                <div>
                  <h3 class="font-medium">{{ timing.label }}</h3>
                  <p class="text-xs text-dark-500">{{ timing.description }}</p>
                </div>
                <button
                  @click="addProcessor(timing.key)"
                  class="p-1.5 bg-emerald-600/20 text-emerald-400 hover:bg-emerald-600/30 rounded-lg transition-colors"
                  title="添加处理器"
                >
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
                  </svg>
                </button>
              </div>

              <!-- 处理器列表 -->
              <div v-if="pipelineConfig[timing.key].length > 0" class="space-y-2">
                <div
                  v-for="(processor, index) in pipelineConfig[timing.key]"
                  :key="index"
                  class="bg-dark-800 rounded-lg p-3"
                >
                  <div class="flex items-center gap-2">
                    <span class="text-xs text-dark-500 w-6">{{ index + 1 }}</span>
                    <select
                      :value="processor.name"
                      @change="updateProcessorName(timing.key, index, ($event.target as HTMLSelectElement).value)"
                      class="w-48 px-3 py-1.5 bg-dark-700 border border-dark-600 rounded-lg text-sm focus:outline-none focus:border-primary-500"
                    >
                      <!-- 如果当前值不在列表中，显示为无效选项 -->
                      <option v-if="!availableProcessors.some(p => p.name === processor.name)" :value="processor.name" class="text-red-400">
                        {{ processor.name }} (无效)
                      </option>
                      <option v-for="p in availableProcessors" :key="p.name" :value="p.name">{{ p.name }}</option>
                    </select>
                    <input
                      :value="processor.description"
                      @input="updateProcessorDescription(timing.key, index, ($event.target as HTMLInputElement).value)"
                      class="flex-1 px-3 py-1.5 bg-dark-700 border border-dark-600 rounded-lg text-sm focus:outline-none focus:border-primary-500"
                      placeholder="处理器描述..."
                    />
                    <button
                      @click="moveProcessor(timing.key, index, 'up')"
                      :disabled="index === 0"
                      class="p-1.5 hover:bg-dark-600 rounded transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
                      title="上移"
                    >
                      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 15l7-7 7 7" />
                      </svg>
                    </button>
                    <button
                      @click="moveProcessor(timing.key, index, 'down')"
                      :disabled="index === pipelineConfig[timing.key].length - 1"
                      class="p-1.5 hover:bg-dark-600 rounded transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
                      title="下移"
                    >
                      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                      </svg>
                    </button>
                    <button
                      @click="removeProcessor(timing.key, index)"
                      class="p-1.5 text-red-400 hover:bg-red-600/20 rounded transition-colors"
                      title="删除"
                    >
                      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                      </svg>
                    </button>
                  </div>
                </div>
              </div>
              <div v-else class="text-sm text-dark-500 text-center py-3">
                暂无处理器，点击右上角 + 添加
              </div>
            </div>

            <!-- 保存按钮 -->
            <div class="flex justify-end gap-3 pt-4 border-t border-dark-700">
              <button
                @click="showPipelineModal = false"
                class="px-4 py-2 bg-dark-700 hover:bg-dark-600 rounded-lg transition-colors"
              >
                取消
              </button>
              <button
                @click="savePipelineConfig"
                :disabled="savingPipeline"
                class="px-4 py-2 bg-emerald-600 hover:bg-emerald-700 rounded-lg transition-colors disabled:opacity-50"
              >
                {{ savingPipeline ? '保存中...' : '保存' }}
              </button>
            </div>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>