<script setup lang="ts">
import { ref, computed } from 'vue'

const props = defineProps<{
  disabled?: boolean
  loading?: boolean
}>()

const emit = defineEmits<{
  send: [message: string]
}>()

const input = ref('')

const canSend = computed(() => input.value.trim() && !props.disabled && !props.loading)

function send() {
  if (!canSend.value) return
  emit('send', input.value.trim())
  input.value = ''
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    send()
  }
}
</script>

<template>
  <div class="border-t border-dark-700 p-4 bg-dark-900">
    <div class="flex gap-3 items-end max-w-4xl mx-auto">
      <div class="flex-1 relative">
        <textarea
          v-model="input"
          @keydown="handleKeydown"
          :disabled="disabled || loading"
          placeholder="输入消息... (Enter发送, Shift+Enter换行)"
          rows="1"
          class="w-full px-4 py-3 bg-dark-800 border border-dark-600 rounded-xl resize-none focus:outline-none focus:border-primary-500 disabled:opacity-50 disabled:cursor-not-allowed"
          style="min-height: 48px; max-height: 200px;"
        />
      </div>
      <button
        @click="send"
        :disabled="!canSend"
        class="px-6 py-3 bg-primary-600 hover:bg-primary-700 disabled:bg-dark-700 disabled:cursor-not-allowed rounded-xl transition-colors flex items-center gap-2"
      >
        <svg v-if="loading" class="w-5 h-5 animate-spin" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
        </svg>
        <svg v-else class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8" />
        </svg>
        <span>发送</span>
      </button>
    </div>
  </div>
</template>