import { defineStore } from 'pinia'
import { ref } from 'vue'

export interface ToastMessage {
  id: number
  type: 'success' | 'error' | 'info'
  message: string
}

export const useAppStore = defineStore('app', () => {
  const toasts = ref<ToastMessage[]>([])
  const sidebarCollapsed = ref(false)
  let toastId = 0

  function showToast(message: string, type: ToastMessage['type'] = 'info') {
    const id = ++toastId
    toasts.value.push({ id, type, message })
    setTimeout(() => {
      toasts.value = toasts.value.filter(t => t.id !== id)
    }, 3000)
  }

  function toggleSidebar() {
    sidebarCollapsed.value = !sidebarCollapsed.value
  }

  return {
    toasts,
    sidebarCollapsed,
    showToast,
    toggleSidebar,
  }
})