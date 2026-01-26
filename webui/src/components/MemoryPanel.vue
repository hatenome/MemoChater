<script setup lang="ts">
import { ref, provide, computed } from 'vue'

export interface TabDefinition {
  id: string
  icon: string
  label: string
}

const props = defineProps<{
  tabs: TabDefinition[]
}>()

const activeTab = ref(props.tabs[0]?.id || '')

// 提供给子面板使用
provide('activeTab', activeTab)

function selectTab(tabId: string) {
  activeTab.value = tabId
}
</script>

<template>
  <div class="flex flex-col h-full">
    <!-- 标签栏 -->
    <div class="flex border-b border-dark-600 bg-dark-900 overflow-x-auto">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        @click="selectTab(tab.id)"
        class="flex items-center gap-1.5 px-4 py-2.5 text-sm whitespace-nowrap border-b-2 transition-colors"
        :class="activeTab === tab.id 
          ? 'border-primary-500 text-primary-300 bg-dark-800' 
          : 'border-transparent text-dark-500 hover:text-dark-300 hover:bg-dark-800/50'"
      >
        <span>{{ tab.icon }}</span>
        <span>{{ tab.label }}</span>
      </button>
    </div>
    
    <!-- 内容区域 -->
    <div class="flex-1 overflow-hidden">
      <slot :active-tab="activeTab"></slot>
    </div>
  </div>
</template>

