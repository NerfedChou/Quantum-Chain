<script setup>
defineProps({
  subsystem: {
    type: Object,
    required: true
  }
})

const getStatusColor = (status) => {
  const colors = {
    good: 'bg-[var(--color-success)]',
    average: 'bg-[var(--color-warning)]',
    overloaded: 'bg-[var(--color-error)]'
  }
  return colors[status] || colors.good
}

const getStatusLabel = (status) => {
  const labels = {
    good: 'Good',
    average: 'Average',
    overloaded: 'Overloaded'
  }
  return labels[status] || 'Good'
}

const getStatusBg = (status) => {
  const bgs = {
    good: 'bg-[var(--color-success)]/10 text-[var(--color-success)]',
    average: 'bg-[var(--color-warning)]/10 text-[var(--color-warning)]',
    overloaded: 'bg-[var(--color-error)]/10 text-[var(--color-error)]'
  }
  return bgs[status] || bgs.good
}
</script>

<template>
  <div
    class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg p-4 cursor-pointer transition-all hover:border-[var(--color-border)] hover:bg-[var(--color-surface)]"
  >
    <!-- Header -->
    <div class="flex items-center justify-between mb-3">
      <div class="flex items-center gap-3">
        <span class="text-xs font-mono px-2 py-0.5 bg-[var(--color-surface)] rounded text-[var(--color-text-muted)]">
          {{ subsystem.id.toUpperCase() }}
        </span>
        <h3 class="text-sm font-semibold text-[var(--color-text-primary)]">{{ subsystem.name }}</h3>
      </div>
      <span :class="['text-[10px] font-medium uppercase px-2 py-0.5 rounded', getStatusBg(subsystem.status)]">
        {{ getStatusLabel(subsystem.status) }}
      </span>
    </div>

    <!-- Metrics -->
    <div class="grid grid-cols-2 gap-3">
      <!-- CPU -->
      <div>
        <div class="flex justify-between text-xs mb-1">
          <span class="text-[var(--color-text-muted)]">CPU</span>
          <span class="text-[var(--color-text-primary)]">{{ subsystem.cpu }}%</span>
        </div>
        <div class="h-1.5 bg-[var(--color-surface)] rounded-full overflow-hidden">
          <div
            class="h-full bg-[var(--color-accent)] rounded-full transition-all"
            :style="{ width: `${subsystem.cpu}%` }"
          ></div>
        </div>
      </div>

      <!-- Memory -->
      <div>
        <div class="flex justify-between text-xs mb-1">
          <span class="text-[var(--color-text-muted)]">Memory</span>
          <span class="text-[var(--color-text-primary)]">{{ subsystem.memory }}%</span>
        </div>
        <div class="h-1.5 bg-[var(--color-surface)] rounded-full overflow-hidden">
          <div
            class="h-full bg-[var(--color-info)] rounded-full transition-all"
            :style="{ width: `${subsystem.memory}%` }"
          ></div>
        </div>
      </div>
    </div>

    <!-- Footer Metrics -->
    <div class="flex justify-between mt-4 pt-3 border-t border-[var(--color-border-subtle)]">
      <div class="text-xs">
        <span class="text-[var(--color-text-muted)]">Network: </span>
        <span class="text-[var(--color-text-primary)]">{{ subsystem.network }}</span>
      </div>
      <div class="text-xs">
        <span class="text-[var(--color-text-muted)]">Uptime: </span>
        <span class="text-[var(--color-text-primary)]">{{ subsystem.uptime }}</span>
      </div>
    </div>
  </div>
</template>
