<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useAppStore } from '@/stores'
import { api } from '@/api/client'

const app = useAppStore()

const apiBase = ref('http://localhost:7892')
const theme = ref('dark')
const disableGeminiThinking = ref(false)
const isLoading = ref(false)

// 加载设置
async function loadSettings() {
  // 从 localStorage 加载
  apiBase.value = localStorage.getItem('memochater_api_base') || 'http://localhost:7892'
  theme.value = localStorage.getItem('memochater_theme') || 'dark'
  
  // 从后端加载 Gemini 设置
  try {
    const response = await api.get<{ disable_gemini_thinking: boolean }>('/admin/api/settings')
    disableGeminiThinking.value = response.disable_gemini_thinking
  } catch (e) {
    console.error('加载设置失败:', e)
  }
}

async function saveSettings() {
  isLoading.value = true
  try {
    // 保存到 localStorage
    localStorage.setItem('memochater_api_base', apiBase.value)
    localStorage.setItem('memochater_theme', theme.value)
    
    // 保存 Gemini 设置到后端
    await api.put('/admin/api/settings', {
      disable_gemini_thinking: disableGeminiThinking.value
    })
    
    app.showToast('设置已保存', 'success')
  } catch (e) {
    console.error('保存设置失败:', e)
    app.showToast('保存失败', 'error')
  } finally {
    isLoading.value = false
  }
}

onMounted(() => {
  loadSettings()
})
</script>

<template>
  <div class="h-full overflow-y-auto p-6">
    <div class="max-w-2xl mx-auto">
      <h1 class="text-2xl font-bold mb-6">⚙️ 设置</h1>

      <div class="bg-dark-800 border border-dark-700 rounded-xl p-6 space-y-6">
        <div>
          <label class="block text-sm text-dark-400 mb-2">API 地址</label>
          <input
            v-model="apiBase"
            class="w-full px-4 py-2 bg-dark-900 border border-dark-600 rounded-lg focus:outline-none focus:border-primary-500"
            placeholder="http://localhost:7892"
          />
          <p class="text-xs text-dark-500 mt-1">MemoChater 后端服务地址</p>
        </div>

        <div>
          <label class="block text-sm text-dark-400 mb-2">主题</label>
          <select
            v-model="theme"
            class="w-full px-4 py-2 bg-dark-900 border border-dark-600 rounded-lg"
          >
            <option value="dark">深色</option>
            <option value="light" disabled>浅色 (开发中)</option>
          </select>
        </div>

        <!-- Gemini 思考设置 -->
        <div class="flex items-center justify-between">
          <div>
            <label class="block text-sm text-dark-300">禁用 Gemini 思考</label>
            <p class="text-xs text-dark-500 mt-1">
              检测到 Gemini 模型时自动添加 thinking_budget: 0
            </p>
          </div>
          <button
            @click="disableGeminiThinking = !disableGeminiThinking"
            :class="[
              'relative inline-flex h-6 w-11 items-center rounded-full transition-colors',
              disableGeminiThinking ? 'bg-primary-600' : 'bg-dark-600'
            ]"
          >
            <span
              :class="[
                'inline-block h-4 w-4 transform rounded-full bg-white transition-transform',
                disableGeminiThinking ? 'translate-x-6' : 'translate-x-1'
              ]"
            />
          </button>
        </div>

        <div class="pt-4 border-t border-dark-700">
          <button
            @click="saveSettings"
            :disabled="isLoading"
            class="px-6 py-2 bg-primary-600 hover:bg-primary-700 rounded-lg transition-colors disabled:opacity-50"
          >
            {{ isLoading ? '保存中...' : '保存设置' }}
          </button>
        </div>
      </div>

      <div class="mt-6 bg-dark-800 border border-dark-700 rounded-xl p-6">
        <h2 class="text-lg font-semibold mb-4">关于</h2>
        <div class="space-y-2 text-sm text-dark-400">
          <p><span class="text-dark-300">项目:</span> MemoChater WebUI</p>
          <p><span class="text-dark-300">版本:</span> 0.1.0</p>
          <p><span class="text-dark-300">描述:</span> AI 记忆增强框架的 Web 管理界面</p>
        </div>
      </div>
    </div>
  </div>
</template>