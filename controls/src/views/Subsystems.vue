<script setup>
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import SubsystemCard from '../components/subsystems/SubsystemCard.vue'

const router = useRouter()

const subsystems = ref([
  { id: 'qc-01', name: 'Peer Discovery', cpu: 32, memory: 45, network: '1.2 MB/s', uptime: '47d 3h', status: 'good' },
  { id: 'qc-02', name: 'Block Storage', cpu: 28, memory: 62, network: '3.8 MB/s', uptime: '47d 3h', status: 'good' },
  { id: 'qc-03', name: 'Mempool Manager', cpu: 67, memory: 78, network: '2.1 MB/s', uptime: '47d 3h', status: 'average' },
  { id: 'qc-04', name: 'Consensus Engine', cpu: 45, memory: 55, network: '0.8 MB/s', uptime: '47d 3h', status: 'good' },
  { id: 'qc-05', name: 'Network Layer', cpu: 38, memory: 42, network: '5.2 MB/s', uptime: '47d 3h', status: 'good' },
  { id: 'qc-06', name: 'State Sync', cpu: 52, memory: 48, network: '4.1 MB/s', uptime: '47d 3h', status: 'average' },
  { id: 'qc-07', name: 'Block Validation', cpu: 41, memory: 38, network: '0.5 MB/s', uptime: '47d 3h', status: 'good' },
  { id: 'qc-08', name: 'RPC Server', cpu: 22, memory: 35, network: '1.8 MB/s', uptime: '47d 3h', status: 'good' },
  { id: 'qc-09', name: 'Light Client', cpu: 18, memory: 28, network: '0.9 MB/s', uptime: '47d 3h', status: 'good' },
  { id: 'qc-10', name: 'Sig Verification', cpu: 85, memory: 72, network: '0.3 MB/s', uptime: '47d 3h', status: 'overloaded' }
])

const openDetail = (id) => {
  router.push(`/subsystems/${id}`)
}
</script>

<template>
  <div class="space-y-6">
    <!-- Header -->
    <div>
      <h1 class="text-2xl font-bold text-[var(--color-text-primary)]">Subsystems</h1>
      <p class="text-sm text-[var(--color-text-secondary)]">Monitor individual subsystem performance and health</p>
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
