<script setup lang="ts">
import { RouterView } from 'vue-router'
import Sidebar from './components/Sidebar.vue'
import Toast from './components/Toast.vue'
</script>

<template>
  <div class="flex h-screen bg-dark-950">
    <Sidebar />
    <main class="flex-1 overflow-hidden">
      <RouterView />
    </main>
    <Toast />
  </div>
</template>