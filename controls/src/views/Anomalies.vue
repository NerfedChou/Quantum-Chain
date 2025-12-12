<script setup>
import { ref } from 'vue'

const anomalies = ref([
  { id: 1, type: 'warning', title: 'Mempool Congestion', subsystem: 'QC-03', time: '2 min ago', description: 'Transaction queue size exceeds 80% capacity' },
  { id: 2, type: 'critical', title: 'Consensus Timeout', subsystem: 'QC-04', time: '15 min ago', description: 'Block proposal timeout exceeded 5 seconds' },
  { id: 3, type: 'info', title: 'Peer Reconnected', subsystem: 'QC-01', time: '22 min ago', description: 'Peer 192.168.1.50 reestablished connection' },
  { id: 4, type: 'warning', title: 'High CPU Usage', subsystem: 'QC-10', time: '45 min ago', description: 'Signature verification using 85% CPU' },
  { id: 5, type: 'resolved', title: 'Fork Detected', subsystem: 'QC-04', time: '1h ago', description: 'Minor fork resolved at block 1,284,720' }
])

const stats = ref([
  { label: 'Critical', value: 1, color: 'error' },
  { label: 'Warning', value: 2, color: 'warning' },
  { label: 'Info', value: 1, color: 'info' },
  { label: 'Resolved', value: 1, color: 'success' }
])

const getTypeClass = (type) => {
  const classes = {
    critical: 'bg-[var(--color-error)]/10 text-[var(--color-error)]',
    warning: 'bg-[var(--color-warning)]/10 text-[var(--color-warning)]',
    info: 'bg-[var(--color-info)]/10 text-[var(--color-info)]',
    resolved: 'bg-[var(--color-success)]/10 text-[var(--color-success)]'
  }
  return classes[type] || classes.info
}

const getDotClass = (type) => {
  const classes = {
    critical: 'bg-[var(--color-error)]',
    warning: 'bg-[var(--color-warning)]',
    info: 'bg-[var(--color-info)]',
    resolved: 'bg-[var(--color-success)]'
  }
  return classes[type] || classes.info
}
</script>

<template>
  <div class="space-y-6">
    <!-- Header -->
    <div>
      <h1 class="text-2xl font-bold text-[var(--color-text-primary)]">Anomalies</h1>
      <p class="text-sm text-[var(--color-text-secondary)]">System alerts, warnings, and events across all subsystems</p>
    </div>

    <!-- Stats -->
    <div class="grid grid-cols-4 gap-4">
      <div
        v-for="stat in stats"
        :key="stat.label"
        class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg p-4"
      >
        <p class="text-3xl font-bold" :class="`text-[var(--color-${stat.color})]`">{{ stat.value }}</p>
        <p class="text-sm text-[var(--color-text-muted)]">{{ stat.label }}</p>
      </div>
    </div>

    <!-- Anomaly List -->
    <div class="space-y-3">
      <div
        v-for="anomaly in anomalies"
        :key="anomaly.id"
        class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg p-4"
      >
        <div class="flex items-start gap-4">
          <span :class="['w-2 h-2 mt-2 rounded-full', getDotClass(anomaly.type)]"></span>
          <div class="flex-1">
            <div class="flex items-center gap-3">
              <h3 class="text-sm font-semibold text-[var(--color-text-primary)]">{{ anomaly.title }}</h3>
              <span :class="['text-[10px] font-medium uppercase px-2 py-0.5 rounded', getTypeClass(anomaly.type)]">
                {{ anomaly.type }}
              </span>
            </div>
            <p class="text-sm text-[var(--color-text-secondary)] mt-1">{{ anomaly.description }}</p>
            <div class="flex items-center gap-4 mt-2 text-xs text-[var(--color-text-muted)]">
              <span>{{ anomaly.subsystem }}</span>
              <span>{{ anomaly.time }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
