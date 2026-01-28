import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      redirect: '/chat'
    },
    {
      path: '/chat',
      name: 'chat',
      component: () => import('./views/ChatView.vue')
    },
    {
      path: '/chat/:assistantId',
      name: 'assistant-chat',
      component: () => import('./views/ChatView.vue')
    },
    {
      path: '/chat/:assistantId/:topicId',
      name: 'topic-chat',
      component: () => import('./views/ChatView.vue')
    },
    {
      path: '/graph/:assistantId/:topicId',
      name: 'topic-graph',
      component: () => import('./views/GraphView.vue')
    },
    {
      path: '/assistants',
      name: 'assistants',
      component: () => import('./views/AssistantsView.vue')
    },
    {
      path: '/memory/:assistantId',
      name: 'memory',
      component: () => import('./views/MemoryView.vue')
    },
    {
      path: '/settings',
      name: 'settings',
      component: () => import('./views/SettingsView.vue')
    }
  ]
})

export default router