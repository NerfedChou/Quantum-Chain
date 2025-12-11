<script setup>
import { ref } from 'vue'

const stats = ref([
  { label: 'Block Height', value: '1,284,739', change: '+12' },
  { label: 'TPS', value: '847', change: '+5.2%' },
  { label: 'Active Peers', value: '42', change: '-2' },
  { label: 'Mempool Size', value: '1,247', change: '+89' },
  { label: 'Avg Block Time', value: '2.4s', change: '-0.1s' },
  { label: 'Hash Rate', value: '48.2 EH/s', change: '+3.1%' }
])
</script>

<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex justify-between items-center">
      <div>
        <h1 class="text-2xl font-bold text-[var(--color-text-primary)]">Dashboard</h1>
        <p class="text-sm text-[var(--color-text-secondary)]">Quantum-Chain Node Overview</p>
      </div>
      <div class="flex items-center gap-2 px-4 py-2 bg-[var(--color-success)]/10 rounded-full">
        <span class="w-2 h-2 rounded-full bg-[var(--color-success)]"></span>
        <span class="text-sm text-[var(--color-success)]">All Systems Operational</span>
      </div>
    </div>

    <!-- Stats Grid -->
    <div class="grid grid-cols-6 gap-4">
      <div
        v-for="stat in stats"
        :key="stat.label"
        class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg p-4"
      >
        <p class="text-xs text-[var(--color-text-muted)] uppercase tracking-wide">{{ stat.label }}</p>
        <p class="text-2xl font-semibold text-[var(--color-text-primary)] mt-1">{{ stat.value }}</p>
        <p class="text-xs text-[var(--color-success)] mt-1">{{ stat.change }}</p>
      </div>
    </div>

    <!-- Main Content Grid -->
    <div class="grid grid-cols-3 gap-6">
      <!-- Subsystem Health -->
      <div class="col-span-2 bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg">
        <div class="p-4 border-b border-[var(--color-border-subtle)]">
          <h2 class="text-sm font-semibold text-[var(--color-text-primary)]">Subsystem Health</h2>
        </div>
        <div class="p-4">
          <div class="grid grid-cols-4 gap-3">
            <div
              v-for="i in 8"
              :key="i"
              class="p-3 bg-[var(--color-surface)] rounded-md"
            >
              <div class="flex items-center justify-between">
                <span class="text-xs font-mono text-[var(--color-text-muted)]">QC-{{ String(i).padStart(2, '0') }}</span>
                <span class="w-2 h-2 rounded-full bg-[var(--color-success)]"></span>
              </div>
              <p class="text-sm font-medium text-[var(--color-text-primary)] mt-1">
                {{ ['Peer Disc.', 'Block Store', 'Mempool', 'Consensus', 'Network', 'Sync', 'Validation', 'RPC'][i-1] }}
              </p>
              <p class="text-xs text-[var(--color-text-muted)] mt-1">{{ 15 + i * 5 }}% CPU</p>
            </div>
          </div>
        </div>
      </div>

      <!-- Recent Anomalies -->
      <div class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg">
        <div class="p-4 border-b border-[var(--color-border-subtle)]">
          <h2 class="text-sm font-semibold text-[var(--color-text-primary)]">Recent Anomalies</h2>
        </div>
        <div class="p-4 space-y-3">
          <div class="flex items-start gap-3">
            <span class="w-2 h-2 mt-1.5 rounded-full bg-[var(--color-warning)]"></span>
            <div>
              <p class="text-sm text-[var(--color-text-primary)]">Mempool Congestion</p>
              <p class="text-xs text-[var(--color-text-muted)]">2 min ago</p>
            </div>
          </div>
          <div class="flex items-start gap-3">
            <span class="w-2 h-2 mt-1.5 rounded-full bg-[var(--color-info)]"></span>
            <div>
              <p class="text-sm text-[var(--color-text-primary)]">Peer Reconnected</p>
              <p class="text-xs text-[var(--color-text-muted)]">8 min ago</p>
            </div>
          </div>
          <div class="flex items-start gap-3">
            <span class="w-2 h-2 mt-1.5 rounded-full bg-[var(--color-success)]"></span>
            <div>
              <p class="text-sm text-[var(--color-text-primary)]">Block Finalized</p>
              <p class="text-xs text-[var(--color-text-muted)]">15 min ago</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
