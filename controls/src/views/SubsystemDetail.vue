<script setup>
import { ref, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'

const route = useRoute()
const router = useRouter()

const subsystemId = computed(() => route.params.id)

// Helper to format bytes
const formatBytes = (bytes) => {
  if (bytes >= 1e12) return `${(bytes / 1e12).toFixed(1)} TB`
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB`
  return `${bytes} B`
}

// Mock data for all subsystems
const subsystemData = {
  // ═══════════════════════════════════════════════════════════════════════════
  // QC-01: PEER DISCOVERY
  // ═══════════════════════════════════════════════════════════════════════════
  'qc-01': {
    name: 'Peer Discovery',
    description: 'Kademlia DHT for peer discovery and routing in the Quantum-Chain network.',
    status: 'good',
    type: 'peer-discovery',
    routingTable: {
      totalPeers: 24,
      bucketsUsed: 12,
      maxBuckets: 256,
      pendingVerification: 3,
      maxPending: 100,
      oldestPeerAge: '47d 3h',
      bannedCount: 5
    },
    connections: {
      inbound: { current: 32, max: 40 },
      outbound: { current: 8, max: 10 },
      protected: 18,
      evictionQueue: 2
    },
    addressPool: {
      newCount: 142,
      triedCount: 87
    },
    feelerState: {
      activeProbes: 2,
      nextProbeIn: '45s',
      successRate: 94,
      failuresToday: 3
    },
    bannedPeers: [
      { id: '0x7f3a...d921', reason: 'MALFORMED_MSG', expiresIn: '58m' },
      { id: '0x8b2c...e112', reason: 'INVALID_SIG', expiresIn: '2h 15m' },
      { id: '0x9d4e...f003', reason: 'PROTOCOL_VIOLATION', expiresIn: '4h 30m' }
    ],
    recentEvents: [
      { type: 'connect', msg: 'Peer 192.168.1.15 connected (inbound)', time: '2 min ago' },
      { type: 'ban', msg: 'Peer 10.0.0.8 banned (INVALID_SIGNATURE)', time: '5 min ago' },
      { type: 'info', msg: 'Eviction: replaced 192.168.2.1 with 10.0.1.5', time: '15 min ago' }
    ]
  },

  // ═══════════════════════════════════════════════════════════════════════════
  // QC-02: BLOCK STORAGE
  // ═══════════════════════════════════════════════════════════════════════════
  'qc-02': {
    name: 'Block Storage',
    description: 'Authoritative persistence layer for blockchain data using V2.3 Stateful Assembler pattern.',
    status: 'good',
    type: 'block-storage',
    blockStatus: {
      latestHeight: 1284739,
      finalizedHeight: 1284730,
      totalBlocks: 1284740,
      genesisHash: '0x7f3a8b2c...5b6c7d8e',
      storageVersion: 1
    },
    diskStorage: {
      usedBytes: 153000000000,    // 153 GB
      capacityBytes: 500000000000, // 500 GB
      usagePercent: 30.6
    },
    ioPerformance: {
      readOps: 847293,
      writeOps: 1284740,
      bytesStored: 14200000000000  // 14.2 TB
    },
    compaction: {
      totalCompactions: 12847,
      bytesCompacted: 8400000000000, // 8.4 TB
      avgDurationMs: 245,
      inProgress: 0
    },
    pendingAssemblies: [
      { hash: '0x7f3a...d921', height: 1284740, hasBlock: true, hasMerkle: true, hasState: false, elapsedSecs: 2.3 },
      { hash: '0x8b2c...e112', height: 1284741, hasBlock: true, hasMerkle: false, hasState: false, elapsedSecs: 1.1 }
    ],
    assemblyConfig: {
      timeoutSecs: 30,
      maxPending: 1000
    },
    recentEvents: [
      { type: 'write', msg: 'Block #1,284,739 stored successfully', time: '2s ago' },
      { type: 'finalize', msg: 'Block #1,284,730 finalized', time: '15s ago' },
      { type: 'gc', msg: 'Expired assembly purged (timeout)', time: '1m ago' }
    ]
  }
}

const data = computed(() => {
  return subsystemData[subsystemId.value] || {
    name: 'Unknown Subsystem',
    description: 'Subsystem data not available. Check if the subsystem ID is correct.',
    status: 'unknown',
    type: 'unknown'
  }
})

// Helper to determine if current subsystem is QC-01 or QC-02
const isQc01 = computed(() => subsystemId.value === 'qc-01')
const isQc02 = computed(() => subsystemId.value === 'qc-02')

const goBack = () => router.push('/subsystems')

const getEventDotClass = (type) => {
  const classes = {
    connect: 'bg-[var(--color-success)]',
    disconnect: 'bg-[var(--color-warning)]',
    ban: 'bg-[var(--color-error)]',
    info: 'bg-[var(--color-info)]',
    write: 'bg-[var(--color-success)]',
    finalize: 'bg-[var(--color-accent)]',
    gc: 'bg-[var(--color-warning)]'
  }
  return classes[type] || classes.info
}

const getStatusColor = (status) => {
  const colors = {
    good: 'bg-[var(--color-success)]/10 text-[var(--color-success)]',
    average: 'bg-[var(--color-warning)]/10 text-[var(--color-warning)]',
    overloaded: 'bg-[var(--color-error)]/10 text-[var(--color-error)]',
    unknown: 'bg-[var(--color-text-muted)]/10 text-[var(--color-text-muted)]'
  }
  return colors[status] || colors.unknown
}

const getDiskUsageColor = (percent) => {
  if (percent >= 85) return 'bg-[var(--color-error)]'
  if (percent >= 70) return 'bg-[var(--color-warning)]'
  return 'bg-[var(--color-success)]'
}
</script>

<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center gap-4 pb-4 border-b border-[var(--color-border-subtle)]">
      <button
        @click="goBack"
        class="flex items-center gap-2 px-3 py-2 text-sm text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] hover:bg-[var(--color-surface)] rounded-md transition-colors"
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18" />
        </svg>
        Back
      </button>
      <div class="flex-1">
        <span class="text-xs font-mono px-2 py-0.5 bg-[var(--color-surface)] rounded text-[var(--color-text-muted)]">
          {{ subsystemId.toUpperCase() }}
        </span>
        <h1 class="text-xl font-semibold text-[var(--color-text-primary)] mt-1">{{ data.name }}</h1>
      </div>
      <span :class="['text-xs font-medium uppercase px-3 py-1 rounded', getStatusColor(data.status)]">
        {{ data.status }}
      </span>
    </div>

    <p class="text-sm text-[var(--color-text-secondary)]">{{ data.description }}</p>

    <!-- ═══════════════════════════════════════════════════════════════════════ -->
    <!-- QC-01: PEER DISCOVERY SPECIFIC -->
    <!-- ═══════════════════════════════════════════════════════════════════════ -->
    <template v-if="isQc01">
      <div class="grid grid-cols-2 gap-4">
        <!-- Routing Table -->
        <div class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg">
          <h2 class="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-muted)] px-4 py-3 border-b border-[var(--color-border-subtle)] bg-[var(--color-surface)]">
            Routing Table
          </h2>
          <div class="p-4 space-y-2">
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Total Peers</span>
              <span class="text-[var(--color-text-primary)]">{{ data.routingTable.totalPeers }}</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Buckets Used</span>
              <span class="text-[var(--color-text-primary)]">{{ data.routingTable.bucketsUsed }}/{{ data.routingTable.maxBuckets }}</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Pending Verification</span>
              <span class="text-[var(--color-text-primary)]">{{ data.routingTable.pendingVerification }}/{{ data.routingTable.maxPending }}</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Oldest Peer Age</span>
              <span class="text-[var(--color-text-primary)]">{{ data.routingTable.oldestPeerAge }}</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Banned Peers</span>
              <span class="text-[var(--color-text-primary)]">{{ data.routingTable.bannedCount }}</span>
            </div>
          </div>
        </div>

        <!-- Connections -->
        <div class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg">
          <h2 class="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-muted)] px-4 py-3 border-b border-[var(--color-border-subtle)] bg-[var(--color-surface)]">
            Connections
          </h2>
          <div class="p-4 space-y-3">
            <div>
              <div class="flex justify-between text-xs mb-1">
                <span class="text-[var(--color-text-secondary)]">Inbound</span>
                <span class="text-[var(--color-text-primary)]">{{ data.connections.inbound.current }}/{{ data.connections.inbound.max }}</span>
              </div>
              <div class="h-2 bg-[var(--color-surface)] rounded-full overflow-hidden">
                <div class="h-full bg-[var(--color-info)] rounded-full" :style="{ width: `${(data.connections.inbound.current / data.connections.inbound.max) * 100}%` }"></div>
              </div>
            </div>
            <div>
              <div class="flex justify-between text-xs mb-1">
                <span class="text-[var(--color-text-secondary)]">Outbound</span>
                <span class="text-[var(--color-text-primary)]">{{ data.connections.outbound.current }}/{{ data.connections.outbound.max }}</span>
              </div>
              <div class="h-2 bg-[var(--color-surface)] rounded-full overflow-hidden">
                <div class="h-full bg-[var(--color-accent)] rounded-full" :style="{ width: `${(data.connections.outbound.current / data.connections.outbound.max) * 100}%` }"></div>
              </div>
            </div>
            <div class="flex justify-between text-sm pt-2">
              <span class="text-[var(--color-text-secondary)]">Protected</span>
              <span class="text-[var(--color-text-primary)]">{{ data.connections.protected }}</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Eviction Queue</span>
              <span class="text-[var(--color-text-primary)]">{{ data.connections.evictionQueue }}</span>
            </div>
          </div>
        </div>

        <!-- Address Pool -->
        <div class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg">
          <h2 class="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-muted)] px-4 py-3 border-b border-[var(--color-border-subtle)] bg-[var(--color-surface)]">
            Address Pool
          </h2>
          <div class="p-4 space-y-2">
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">New Addresses</span>
              <span class="text-[var(--color-text-primary)]">{{ data.addressPool.newCount }}</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Tried Addresses</span>
              <span class="text-[var(--color-text-primary)]">{{ data.addressPool.triedCount }}</span>
            </div>
          </div>
        </div>

        <!-- Feeler State -->
        <div class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg">
          <h2 class="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-muted)] px-4 py-3 border-b border-[var(--color-border-subtle)] bg-[var(--color-surface)]">
            Feeler State
          </h2>
          <div class="p-4 space-y-2">
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Active Probes</span>
              <span class="text-[var(--color-text-primary)]">{{ data.feelerState.activeProbes }}</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Next Probe In</span>
              <span class="text-[var(--color-text-primary)]">{{ data.feelerState.nextProbeIn }}</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Success Rate</span>
              <span class="text-[var(--color-text-primary)]">{{ data.feelerState.successRate }}%</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Failures Today</span>
              <span class="text-[var(--color-text-primary)]">{{ data.feelerState.failuresToday }}</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Banned Peers Table -->
      <div class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg">
        <h2 class="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-muted)] px-4 py-3 border-b border-[var(--color-border-subtle)] bg-[var(--color-surface)]">
          Banned Peers
        </h2>
        <div class="p-4">
          <table class="w-full text-sm">
            <thead>
              <tr class="text-left text-xs text-[var(--color-text-muted)] uppercase">
                <th class="pb-2">Peer ID</th>
                <th class="pb-2">Reason</th>
                <th class="pb-2">Expires In</th>
                <th class="pb-2">Action</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="peer in data.bannedPeers" :key="peer.id" class="border-t border-[var(--color-border-subtle)]">
                <td class="py-2 font-mono text-[var(--color-text-primary)]">{{ peer.id }}</td>
                <td class="py-2">
                  <span class="text-xs px-2 py-0.5 rounded bg-[var(--color-warning)]/10 text-[var(--color-warning)]">{{ peer.reason }}</span>
                </td>
                <td class="py-2 text-[var(--color-text-secondary)]">{{ peer.expiresIn }}</td>
                <td class="py-2">
                  <button class="text-xs px-2 py-1 border border-[var(--color-border)] rounded hover:border-[var(--color-text-muted)] transition-colors">Unban</button>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </template>

    <!-- ═══════════════════════════════════════════════════════════════════════ -->
    <!-- QC-02: BLOCK STORAGE SPECIFIC -->
    <!-- ═══════════════════════════════════════════════════════════════════════ -->
    <template v-else-if="isQc02">
      <div class="grid grid-cols-2 gap-4">
        <!-- Block Status -->
        <div class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg">
          <h2 class="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-muted)] px-4 py-3 border-b border-[var(--color-border-subtle)] bg-[var(--color-surface)]">
            Block Status
          </h2>
          <div class="p-4 space-y-2">
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Latest Height</span>
              <span class="text-[var(--color-text-primary)] font-mono">{{ data.blockStatus.latestHeight.toLocaleString() }}</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Finalized Height</span>
              <span class="text-[var(--color-text-primary)] font-mono">{{ data.blockStatus.finalizedHeight.toLocaleString() }}</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Behind Finalization</span>
              <span class="text-[var(--color-warning)] font-mono">{{ data.blockStatus.latestHeight - data.blockStatus.finalizedHeight }}</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Total Blocks</span>
              <span class="text-[var(--color-text-primary)]">{{ data.blockStatus.totalBlocks.toLocaleString() }}</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Genesis Hash</span>
              <span class="text-[var(--color-text-primary)] font-mono text-xs">{{ data.blockStatus.genesisHash }}</span>
            </div>
          </div>
        </div>

        <!-- Disk Storage -->
        <div class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg">
          <h2 class="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-muted)] px-4 py-3 border-b border-[var(--color-border-subtle)] bg-[var(--color-surface)]">
            Disk Storage
          </h2>
          <div class="p-4 space-y-3">
            <div>
              <div class="flex justify-between text-xs mb-1">
                <span class="text-[var(--color-text-secondary)]">Usage</span>
                <span class="text-[var(--color-text-primary)]">{{ formatBytes(data.diskStorage.usedBytes) }} / {{ formatBytes(data.diskStorage.capacityBytes) }}</span>
              </div>
              <div class="h-3 bg-[var(--color-surface)] rounded-full overflow-hidden">
                <div :class="['h-full rounded-full', getDiskUsageColor(data.diskStorage.usagePercent)]" :style="{ width: `${data.diskStorage.usagePercent}%` }"></div>
              </div>
              <div class="flex justify-between text-xs mt-1">
                <span class="text-[var(--color-text-muted)]">{{ data.diskStorage.usagePercent.toFixed(1) }}% used</span>
                <span class="text-[var(--color-text-muted)]">{{ formatBytes(data.diskStorage.capacityBytes - data.diskStorage.usedBytes) }} free</span>
              </div>
            </div>
          </div>
        </div>

        <!-- I/O Performance -->
        <div class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg">
          <h2 class="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-muted)] px-4 py-3 border-b border-[var(--color-border-subtle)] bg-[var(--color-surface)]">
            I/O Performance
          </h2>
          <div class="p-4 space-y-2">
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Read Operations</span>
              <span class="text-[var(--color-text-primary)]">{{ data.ioPerformance.readOps.toLocaleString() }}</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Write Operations</span>
              <span class="text-[var(--color-text-primary)]">{{ data.ioPerformance.writeOps.toLocaleString() }}</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Total Bytes Stored</span>
              <span class="text-[var(--color-text-primary)]">{{ formatBytes(data.ioPerformance.bytesStored) }}</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">R/W Ratio</span>
              <span class="text-[var(--color-text-primary)]">{{ (data.ioPerformance.readOps / data.ioPerformance.writeOps).toFixed(2) }}</span>
            </div>
          </div>
        </div>

        <!-- Compaction Status -->
        <div class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg">
          <h2 class="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-muted)] px-4 py-3 border-b border-[var(--color-border-subtle)] bg-[var(--color-surface)]">
            Compaction Status
          </h2>
          <div class="p-4 space-y-2">
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Total Compactions</span>
              <span class="text-[var(--color-text-primary)]">{{ data.compaction.totalCompactions.toLocaleString() }}</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Bytes Compacted</span>
              <span class="text-[var(--color-text-primary)]">{{ formatBytes(data.compaction.bytesCompacted) }}</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">Avg Duration</span>
              <span class="text-[var(--color-text-primary)]">{{ data.compaction.avgDurationMs }} ms</span>
            </div>
            <div class="flex justify-between text-sm">
              <span class="text-[var(--color-text-secondary)]">In Progress</span>
              <span :class="data.compaction.inProgress > 0 ? 'text-[var(--color-warning)]' : 'text-[var(--color-text-primary)]'">{{ data.compaction.inProgress }}</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Pending Assemblies Table -->
      <div class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg">
        <div class="flex items-center justify-between px-4 py-3 border-b border-[var(--color-border-subtle)] bg-[var(--color-surface)]">
          <h2 class="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-muted)]">
            Assembly Buffer (V2.3 Choreography)
          </h2>
          <div class="text-xs text-[var(--color-text-muted)]">
            Timeout: {{ data.assemblyConfig.timeoutSecs }}s | Max: {{ data.assemblyConfig.maxPending }}
          </div>
        </div>
        <div class="p-4">
          <table class="w-full text-sm">
            <thead>
              <tr class="text-left text-xs text-[var(--color-text-muted)] uppercase">
                <th class="pb-2">Block Hash</th>
                <th class="pb-2">Height</th>
                <th class="pb-2 text-center">Block</th>
                <th class="pb-2 text-center">Merkle</th>
                <th class="pb-2 text-center">State</th>
                <th class="pb-2 text-right">Elapsed</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="assembly in data.pendingAssemblies" :key="assembly.hash" class="border-t border-[var(--color-border-subtle)]">
                <td class="py-2 font-mono text-[var(--color-text-primary)]">{{ assembly.hash }}</td>
                <td class="py-2 font-mono text-[var(--color-text-primary)]">{{ assembly.height.toLocaleString() }}</td>
                <td class="py-2 text-center">
                  <span v-if="assembly.hasBlock" class="text-[var(--color-success)]">✓</span>
                  <span v-else class="text-[var(--color-warning)]">⏳</span>
                </td>
                <td class="py-2 text-center">
                  <span v-if="assembly.hasMerkle" class="text-[var(--color-success)]">✓</span>
                  <span v-else class="text-[var(--color-warning)]">⏳</span>
                </td>
                <td class="py-2 text-center">
                  <span v-if="assembly.hasState" class="text-[var(--color-success)]">✓</span>
                  <span v-else class="text-[var(--color-warning)]">⏳</span>
                </td>
                <td class="py-2 text-right text-[var(--color-text-muted)]">{{ assembly.elapsedSecs }}s</td>
              </tr>
              <tr v-if="data.pendingAssemblies.length === 0">
                <td colspan="6" class="py-4 text-center text-[var(--color-text-muted)]">No pending assemblies</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </template>

    <!-- ═══════════════════════════════════════════════════════════════════════ -->
    <!-- UNKNOWN SUBSYSTEM -->
    <!-- ═══════════════════════════════════════════════════════════════════════ -->
    <template v-else>
      <div class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg p-8 text-center">
        <svg class="w-12 h-12 mx-auto text-[var(--color-text-muted)] mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9.75 9.75l4.5 4.5m0-4.5l-4.5 4.5M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <h3 class="text-lg font-medium text-[var(--color-text-primary)]">Subsystem Not Found</h3>
        <p class="text-sm text-[var(--color-text-muted)] mt-2">Metrics for {{ subsystemId }} are not yet implemented.</p>
      </div>
    </template>

    <!-- ═══════════════════════════════════════════════════════════════════════ -->
    <!-- RECENT EVENTS (Shared by QC-01 and QC-02) -->
    <!-- ═══════════════════════════════════════════════════════════════════════ -->
    <div v-if="data.recentEvents && data.recentEvents.length > 0" class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg">
      <h2 class="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-muted)] px-4 py-3 border-b border-[var(--color-border-subtle)] bg-[var(--color-surface)]">
        Recent Events
      </h2>
      <div class="p-4 space-y-3">
        <div v-for="(event, i) in data.recentEvents" :key="i" class="flex items-start gap-3">
          <span :class="['w-2 h-2 mt-1.5 rounded-full', getEventDotClass(event.type)]"></span>
          <div class="flex-1">
            <p class="text-sm text-[var(--color-text-primary)]">{{ event.msg }}</p>
            <p class="text-xs text-[var(--color-text-muted)]">{{ event.time }}</p>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
