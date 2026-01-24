<template>
  <div class="tool-call-block my-2">
    <div 
      @click="expanded = !expanded"
      class="flex items-center gap-2 px-3 py-2 bg-purple-900/30 border border-purple-700/50 rounded-lg cursor-pointer hover:bg-purple-900/40 transition-colors"
    >
      <!-- 展开/收起图标 -->
      <svg 
        class="w-4 h-4 text-purple-400 transition-transform"
        :class="{ 'rotate-90': expanded }"
        viewBox="0 0 24 24" 
        fill="none" 
        stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
      </svg>
      
      <!-- 工具图标 -->
      <svg class="w-4 h-4 text-purple-400" viewBox="0 0 24 24" fill="none" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"/>
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"/>
      </svg>
      
      <!-- 标题 -->
      <span class="text-purple-300 text-sm font-medium">
        {{ displayTitle }}
      </span>
      
      <!-- 待处理指示器 -->
      <span v-if="pending" class="text-xs text-yellow-400 animate-pulse">
        处理中...
      </span>
    </div>
    
    <!-- 展开内容 -->
    <div 
      v-if="expanded"
      class="mt-1 px-3 py-2 bg-dark-800 border border-dark-700 rounded-lg text-sm"
    >
      <!-- 解析后的参数 -->
      <div v-if="parsed && parsed.toolName" class="space-y-1">
        <div v-if="parsed.maid" class="flex">
          <span class="text-dark-500 w-20">女仆:</span>
          <span class="text-dark-200">{{ parsed.maid }}</span>
        </div>
        <div class="flex">
          <span class="text-dark-500 w-20">工具:</span>
          <span class="text-purple-400">{{ parsed.toolName }}</span>
        </div>
        <div v-if="parsed.command" class="flex">
          <span class="text-dark-500 w-20">命令:</span>
          <span class="text-blue-400">{{ parsed.command }}</span>
        </div>
        <div v-if="Object.keys(parsed.params || {}).length > 0" class="mt-2">
          <span class="text-dark-500">参数:</span>
          <div class="mt-1 pl-2 border-l-2 border-dark-700">
            <div v-for="(value, key) in parsed.params" :key="key" class="flex">
              <span class="text-dark-500 w-32 flex-shrink-0">{{ key }}:</span>
              <span class="text-dark-200 break-all">{{ truncateValue(value) }}</span>
            </div>
          </div>
        </div>
      </div>
      
      <!-- 原始内容（如果解析失败） -->
      <div v-else>
        <pre class="text-xs text-dark-500 whitespace-pre-wrap break-all">{{ content }}</pre>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import type { ToolCallParsed } from '@/utils/vcpParser'

const props = defineProps<{
  content?: string
  parsed?: ToolCallParsed
  pending?: boolean
}>()

const expanded = ref(false)

const displayTitle = computed(() => {
  if (props.parsed?.toolName) {
    const cmd = props.parsed.command ? `.${props.parsed.command}` : ''
    return `调用工具: ${props.parsed.toolName}${cmd}`
  }
  return '工具调用'
})

function truncateValue(value: string, maxLength = 100): string {
  if (!value) return ''
  const str = String(value)
  if (str.length <= maxLength) return str
  return str.substring(0, maxLength) + '...'
}
</script>