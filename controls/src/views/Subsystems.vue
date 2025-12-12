<script setup>
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import SubsystemCard from '../components/subsystems/SubsystemCard.vue'
import { useAllSubsystems } from '../composables/useSubsystemData'

const router = useRouter()

// Get all subsystems from API Gateway (QC-16)
const { subsystems: apiSubsystems, loading, error, refresh } = useAllSubsystems()

// Transform API response to card format with fallback data
const subsystems = computed(() => {
  if (apiSubsystems.value.length > 0) {
    return apiSubsystems.value.map(s => ({
      id: s.id,
      name: s.name,
      cpu: 0, // Would come from specific_metrics
      memory: 0,
      network: '0 MB/s',
      uptime: s.uptime_ms ? formatUptime(s.uptime_ms) : '-',
      status: mapStatus(s.status)
    }))
  }
  
  // Fallback when API is not available - minimal static list
  return [
    { id: 'qc-01', name: 'Peer Discovery', cpu: 0, memory: 0, network: '-', uptime: '-', status: 'unknown' },
    { id: 'qc-02', name: 'Block Storage', cpu: 0, memory: 0, network: '-', uptime: '-', status: 'unknown' },
    { id: 'qc-03', name: 'Transaction Indexing', cpu: 0, memory: 0, network: '-', uptime: '-', status: 'unknown' },
    { id: 'qc-04', name: 'State Management', cpu: 0, memory: 0, network: '-', uptime: '-', status: 'unknown' },
    { id: 'qc-05', name: 'Block Propagation', cpu: 0, memory: 0, network: '-', uptime: '-', status: 'unknown' },
    { id: 'qc-06', name: 'Mempool', cpu: 0, memory: 0, network: '-', uptime: '-', status: 'unknown' },
    { id: 'qc-07', name: 'Bloom Filters', cpu: 0, memory: 0, network: '-', uptime: '-', status: 'unknown' },
    { id: 'qc-08', name: 'Consensus', cpu: 0, memory: 0, network: '-', uptime: '-', status: 'unknown' },
    { id: 'qc-09', name: 'Finality', cpu: 0, memory: 0, network: '-', uptime: '-', status: 'unknown' },
    { id: 'qc-10', name: 'Signature Verification', cpu: 0, memory: 0, network: '-', uptime: '-', status: 'unknown' },
    { id: 'qc-11', name: 'Smart Contracts', cpu: 0, memory: 0, network: '-', uptime: '-', status: 'unknown' },
    { id: 'qc-12', name: 'Transaction Ordering', cpu: 0, memory: 0, network: '-', uptime: '-', status: 'unknown' },
    { id: 'qc-13', name: 'Light Client Sync', cpu: 0, memory: 0, network: '-', uptime: '-', status: 'unknown' },
    { id: 'qc-14', name: 'Sharding', cpu: 0, memory: 0, network: '-', uptime: '-', status: 'unknown' },
    { id: 'qc-15', name: 'Cross-Chain', cpu: 0, memory: 0, network: '-', uptime: '-', status: 'unknown' },
    { id: 'qc-16', name: 'API Gateway', cpu: 0, memory: 0, network: '-', uptime: '-', status: 'unknown' },
    { id: 'qc-17', name: 'Block Production', cpu: 0, memory: 0, network: '-', uptime: '-', status: 'unknown' }
  ]
})

// Map API status to card status
function mapStatus(status) {
  const statusMap = {
    'running': 'good',
    'stopped': 'error',
    'degraded': 'average',
    'error': 'overloaded',
    'not_implemented': 'average',
    'unknown': 'unknown'
  }
  return statusMap[status] || 'unknown'
}

// Format uptime from milliseconds
function formatUptime(ms) {
  const days = Math.floor(ms / (1000 * 60 * 60 * 24))
  const hours = Math.floor((ms % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60))
  return `${days}d ${hours}h`
}

const openDetail = (id) => {
  router.push(`/subsystems/${id}`)
}
</script>

<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold text-[var(--color-text-primary)]">Subsystems</h1>
        <p class="text-sm text-[var(--color-text-secondary)]">Monitor individual subsystem performance and health</p>
      </div>
      <button 
        @click="refresh" 
        :disabled="loading"
        class="px-3 py-1.5 text-xs rounded bg-[var(--color-surface)] border border-[var(--color-border-subtle)] text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] disabled:opacity-50"
      >
        {{ loading ? 'Refreshing...' : 'Refresh' }}
      </button>
    </div>

    <!-- Error banner -->
    <div v-if="error" class="bg-[var(--color-error)]/10 border border-[var(--color-error)]/30 rounded-lg px-4 py-3 text-sm text-[var(--color-error)]">
      <span class="font-medium">Connection Error:</span> {{ error }}
    </div>

    <!-- Grid -->
    <div class="grid grid-cols-2 gap-4">
      <SubsystemCard
        v-for="sub in subsystems"
        :key="sub.id"
        :subsystem="sub"
        @click="openDetail(sub.id)"
      />
    </div>
  </div>
</template>
