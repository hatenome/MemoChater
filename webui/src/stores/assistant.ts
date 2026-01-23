import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { assistantsApi } from '@/api'
import type { AssistantSummary, AssistantConfig, TopicSummary, TopicType, ChatMessage } from '@/types'
import { useAppStore } from './app'

export const useAssistantStore = defineStore('assistant', () => {
  const app = useAppStore()
  
  // 状态
  const assistants = ref<AssistantSummary[]>([])
  const currentAssistantId = ref<string | null>(null)
  const currentAssistantConfig = ref<AssistantConfig | null>(null)
  const topics = ref<TopicSummary[]>([])
  const currentTopicId = ref<string | null>(null)
  const messages = ref<ChatMessage[]>([])
  const loading = ref(false)

  // 计算属性
  const currentAssistant = computed(() =>
    assistants.value.find(a => a.id === currentAssistantId.value)
  )

  const currentTopic = computed(() =>
    topics.value.find(t => t.id === currentTopicId.value)
  )

  // 助手操作
  async function loadAssistants() {
    try {
      assistants.value = await assistantsApi.list()
    } catch (e) {
      app.showToast('加载助手列表失败', 'error')
    }
  }

  async function selectAssistant(id: string) {
    if (currentAssistantId.value === id) return
    
    currentAssistantId.value = id
    currentTopicId.value = null
    messages.value = []
    
    try {
      loading.value = true
      const [config, topicList] = await Promise.all([
        assistantsApi.get(id),
        assistantsApi.listTopics(id),
      ])
      currentAssistantConfig.value = config
      topics.value = topicList
    } catch (e) {
      app.showToast('加载助手信息失败', 'error')
    } finally {
      loading.value = false
    }
  }

  async function createAssistant(data: Partial<AssistantConfig>) {
    try {
      const result = await assistantsApi.create(data)
      await loadAssistants()
      app.showToast('助手创建成功', 'success')
      return result.id
    } catch (e) {
      app.showToast('创建助手失败', 'error')
      throw e
    }
  }

  async function updateAssistant(id: string, data: Partial<AssistantConfig>) {
    try {
      await assistantsApi.update(id, data)
      await loadAssistants()
      if (currentAssistantId.value === id) {
        currentAssistantConfig.value = await assistantsApi.get(id)
      }
      app.showToast('助手更新成功', 'success')
    } catch (e) {
      app.showToast('更新助手失败', 'error')
      throw e
    }
  }

  async function deleteAssistant(id: string) {
    try {
      await assistantsApi.delete(id)
      if (currentAssistantId.value === id) {
        currentAssistantId.value = null
        currentAssistantConfig.value = null
        topics.value = []
        currentTopicId.value = null
        messages.value = []
      }
      await loadAssistants()
      app.showToast('助手删除成功', 'success')
    } catch (e) {
      app.showToast('删除助手失败', 'error')
      throw e
    }
  }

  // 话题操作
  async function selectTopic(topicId: string) {
    if (!currentAssistantId.value) return
    if (currentTopicId.value === topicId) return
    
    currentTopicId.value = topicId
    
    try {
      loading.value = true
      
      // 获取话题信息以判断类型
      const topic = topics.value.find(t => t.id === topicId)
      
      if (topic?.topic_type === 'memory') {
        // 记忆话题：从 packet 获取 messages
        const packetData = await assistantsApi.getPacketMemory(
          currentAssistantId.value,
          topicId
        )
        messages.value = packetData.messages || []
      } else {
        // 普通话题：从 history.json 获取
        messages.value = await assistantsApi.getHistory(
          currentAssistantId.value,
          topicId
        )
      }
    } catch (e) {
      app.showToast('加载对话历史失败', 'error')
      messages.value = []
    } finally {
      loading.value = false
    }
  }

  async function createTopic(title: string, topicType: TopicType = 'normal') {
    if (!currentAssistantId.value) return
    
    try {
      const topic = await assistantsApi.createTopic(currentAssistantId.value, title, topicType)
      topics.value.unshift({
        id: topic.id,
        assistant_id: currentAssistantId.value,
        title: topic.title,
        topic_type: topic.topic_type,
        message_count: 0,
        created_at: topic.created_at,
        updated_at: topic.updated_at,
      })
      app.showToast('话题创建成功', 'success')
      return topic.id
    } catch (e) {
      app.showToast('创建话题失败', 'error')
      throw e
    }
  }

  async function deleteTopic(topicId: string) {
    if (!currentAssistantId.value) return
    
    try {
      await assistantsApi.deleteTopic(currentAssistantId.value, topicId)
      topics.value = topics.value.filter(t => t.id !== topicId)
      if (currentTopicId.value === topicId) {
        currentTopicId.value = null
        messages.value = []
      }
      app.showToast('话题删除成功', 'success')
    } catch (e) {
      app.showToast('删除话题失败', 'error')
      throw e
    }
  }

  async function updateTopicTitle(topicId: string, title: string) {
    if (!currentAssistantId.value) return
    
    try {
      await assistantsApi.updateTopic(currentAssistantId.value, topicId, { title })
      const topic = topics.value.find(t => t.id === topicId)
      if (topic) {
        topic.title = title
      }
      app.showToast('话题标题已更新', 'success')
    } catch (e) {
      app.showToast('更新话题标题失败', 'error')
      throw e
    }
  }

  // 消息操作
  function addMessage(message: ChatMessage) {
    messages.value.push(message)
  }

  function updateLastMessage(content: string) {
    if (messages.value.length > 0) {
      const last = messages.value[messages.value.length - 1]
      if (last.role === 'assistant') {
        last.content = content
      }
    }
  }

  // 编辑指定消息
  async function editMessage(index: number, content: string) {
    if (!currentAssistantId.value || !currentTopicId.value) return
    
    try {
      await assistantsApi.updateMessage(
        currentAssistantId.value,
        currentTopicId.value,
        index,
        content
      )
      messages.value[index].content = content
      app.showToast('消息已更新', 'success')
    } catch (e) {
      app.showToast('更新消息失败', 'error')
      throw e
    }
  }

  // 删除指定消息
  async function removeMessage(index: number) {
    if (!currentAssistantId.value || !currentTopicId.value) return
    
    try {
      await assistantsApi.deleteMessage(
        currentAssistantId.value,
        currentTopicId.value,
        index
      )
      messages.value.splice(index, 1)
      app.showToast('消息已删除', 'success')
    } catch (e) {
      app.showToast('删除消息失败', 'error')
      throw e
    }
  }

  // 从指定位置创建分支话题
  async function createBranchFromMessage(index: number, title?: string) {
    if (!currentAssistantId.value || !currentTopicId.value) return
    
    try {
      const topic = await assistantsApi.createBranch(
        currentAssistantId.value,
        currentTopicId.value,
        index,
        title
      )
      // 添加到话题列表（继承原话题的类型）
      topics.value.unshift({
        id: topic.id,
        assistant_id: currentAssistantId.value,
        title: topic.title,
        topic_type: topic.topic_type,
        message_count: index,
        created_at: topic.created_at,
        updated_at: topic.updated_at,
      })
      app.showToast('分支话题已创建', 'success')
      return topic.id
    } catch (e) {
      app.showToast('创建分支话题失败', 'error')
      throw e
    }
  }

  return {
    // 状态
    assistants,
    currentAssistantId,
    currentAssistantConfig,
    topics,
    currentTopicId,
    messages,
    loading,
    // 计算属性
    currentAssistant,
    currentTopic,
    // 方法
    loadAssistants,
    selectAssistant,
    createAssistant,
    updateAssistant,
    deleteAssistant,
    selectTopic,
    createTopic,
    deleteTopic,
    updateTopicTitle,
    addMessage,
    updateLastMessage,
    editMessage,
    removeMessage,
    createBranchFromMessage,
  }
})