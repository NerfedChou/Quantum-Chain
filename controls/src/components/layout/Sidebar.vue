<script setup>
import { RouterLink, useRoute } from 'vue-router'
import { computed, onMounted, onUnmounted } from 'vue'
import { useApi } from '../../composables/useApi'

const route = useRoute()
const { connected, connecting, latencyMs, checkHealth } = useApi()

const navItems = [
  { path: '/', name: 'Dashboard', icon: 'dashboard' },
  { path: '/subsystems', name: 'Subsystems', icon: 'subsystems' },
  { path: '/anomalies', name: 'Anomalies', icon: 'anomalies' },
  { path: '/settings', name: 'Settings', icon: 'settings' }
]

const isActive = (path) => {
  if (path === '/') return route.path === '/'
  return route.path.startsWith(path)
}

// Connection status text
const statusText = computed(() => {
  if (connecting.value) return 'Connecting...'
  if (connected.value) return `Connected (${latencyMs.value}ms)`
  return 'Disconnected'
})

const statusColor = computed(() => {
  if (connecting.value) return 'bg-[var(--color-warning)]'
  if (connected.value) return 'bg-[var(--color-success)]'
  return 'bg-[var(--color-error)]'
})

// Check health on mount and periodically
let healthInterval = null

onMounted(() => {
  checkHealth()
  // Check every 15 seconds (more stable)
  healthInterval = setInterval(checkHealth, 15000)
})

onUnmounted(() => {
  if (healthInterval) clearInterval(healthInterval)
})
</script>

<template>
  <aside class="w-60 bg-[var(--color-bg-secondary)] border-r border-[var(--color-border-subtle)] flex flex-col">
    <!-- Logo -->
    <div class="p-4 border-b border-[var(--color-border-subtle)]">
      <div class="flex items-center gap-3">
        <div class="w-8 h-8 rounded-lg bg-[var(--color-accent)] flex items-center justify-center">
          <span class="text-white font-bold text-sm">QC</span>
        </div>
        <div>
          <h1 class="text-sm font-semibold text-[var(--color-text-primary)]">Quantum-Chain</h1>
          <p class="text-xs text-[var(--color-text-muted)]">Node Controls</p>
        </div>
      </div>
    </div>

    <!-- Navigation -->
    <nav class="flex-1 p-3">
      <ul class="space-y-1">
        <li v-for="item in navItems" :key="item.path">
          <RouterLink
            :to="item.path"
            :class="[
              'flex items-center gap-3 px-3 py-2 rounded-md text-sm transition-colors',
              isActive(item.path)
                ? 'bg-[var(--color-surface)] text-[var(--color-text-primary)]'
                : 'text-[var(--color-text-secondary)] hover:bg-[var(--color-surface)] hover:text-[var(--color-text-primary)]'
            ]"
          >
            <!-- Icons -->
            <svg v-if="item.icon === 'dashboard'" class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zM14 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V6zM4 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zM14 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z" />
            </svg>
            <svg v-else-if="item.icon === 'subsystems'" class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m18-6h-2m2 6h-2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2zM9 9h6v6H9V9z" />
            </svg>
            <svg v-else-if="item.icon === 'anomalies'" class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
            <svg v-else-if="item.icon === 'settings'" class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
            </svg>
            <span>{{ item.name }}</span>
          </RouterLink>
        </li>
      </ul>
    </nav>

    <!-- Status -->
    <div class="p-4 border-t border-[var(--color-border-subtle)]">
      <div class="flex items-center gap-2">
        <span :class="['w-2 h-2 rounded-full transition-colors', statusColor]"></span>
        <span class="text-xs text-[var(--color-text-secondary)]">{{ statusText }}</span>
      </div>
    </div>
  </aside>
</template>
