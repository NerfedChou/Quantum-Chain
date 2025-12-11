<script setup>
import { RouterView } from 'vue-router'
import Sidebar from './components/layout/Sidebar.vue'
</script>

<template>
  <div class="flex min-h-screen bg-[var(--color-bg-primary)]">
    <Sidebar />
    <main class="flex-1 p-6 overflow-y-auto">
      <RouterView />
    </main>
  </div>
</template>
