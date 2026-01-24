<template>
  <div class="tool-result-block my-2">
    <div 
      @click="expanded = !expanded"
      class="flex items-center gap-2 px-3 py-2 rounded-lg cursor-pointer transition-colors"
      :class="statusClasses"
    >
      <!-- 展开/收起图标 -->
      <svg 
        class="w-4 h-4 transition-transform"
        :class="{ 'rotate-90': expanded }"
        viewBox="0 0 24 24" 
        fill="none" 
        stroke="currentColor"
      >
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
      </svg>
      
      <!-- 状态图标 -->
      <svg v-if="isSuccess" class="w-4 h-4 text-green-400" viewBox="0 0 24 24" fill="none" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"/>
      </svg>
      <svg v-else class="w-4 h-4 text-red-400" viewBox="0 0 24 24" fill="none" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
      </svg>
      
      <!-- 标题 -->
      <span class="text-sm font-medium" :class="isSuccess ? 'text-green-300' : 'text-red-300'">
        {{ displayTitle }}
      </span>
      
      <!-- 待处理指示器 -->
      <span v-if="pending" class="text-xs text-yellow-400 animate-pulse">
        接收中...
      </span>
    </div>
    
    <!-- 展开内容 -->
    <div 
      v-if="expanded"
      class="mt-1 px-3 py-2 bg-dark-800 border border-dark-700 rounded-lg text-sm overflow-hidden"
    >
      <!-- 格式化的JSON内容 -->
      <div v-if="parsed?.parsedContent" class="max-h-96 overflow-auto">
        <pre class="text-xs text-dark-200 whitespace-pre-wrap break-all">{{ formattedContent }}</pre>
      </div>
      
      <!-- 原始文本内容 -->
      <div v-else-if="parsed?.content">
        <pre class="text-xs text-dark-400 whitespace-pre-wrap break-all max-h-96 overflow-auto">{{ parsed.content }}</pre>
      </div>
      
      <!-- 未解析的原始内容 -->
      <div v-else>
        <pre class="text-xs text-dark-500 whitespace-pre-wrap break-all">{{ content }}</pre>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import type { ToolResultParsed } from '@/utils/vcpParser'

const props = defineProps<{
  content?: string
  parsed?: ToolResultParsed
  pending?: boolean
}>()

const expanded = ref(false)

const isSuccess = computed(() => {
  return props.parsed?.status === 'success'
})

const statusClasses = computed(() => {
  if (props.pending) {
    return 'bg-yellow-900/30 border border-yellow-700/50 hover:bg-yellow-900/40'
  }
  return isSuccess.value 
    ? 'bg-green-900/30 border border-green-700/50 hover:bg-green-900/40'
    : 'bg-red-900/30 border border-red-700/50 hover:bg-red-900/40'
})

const displayTitle = computed(() => {
  if (props.parsed?.toolName) {
    const status = isSuccess.value ? '✅' : '❌'
    return `${status} ${props.parsed.toolName} 返回`
  }
  return '工具返回'
})

const formattedContent = computed(() => {
  if (props.parsed?.parsedContent) {
    try {
      return JSON.stringify(props.parsed.parsedContent, null, 2)
    } catch {
      return props.parsed.content
    }
  }
  return props.parsed?.content || props.content
})
</script>