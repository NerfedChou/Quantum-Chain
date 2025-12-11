<script setup>
import { ref } from 'vue'

const settings = ref({
  rpcEnabled: true,
  debugMode: false,
  maxPeers: 50,
  blockTimeout: 5,
  mempoolSize: 5000
})
</script>

<template>
  <div class="space-y-6">
    <!-- Header -->
    <div>
      <h1 class="text-2xl font-bold text-[var(--color-text-primary)]">Settings</h1>
      <p class="text-sm text-[var(--color-text-secondary)]">Configure node parameters and preferences</p>
    </div>

    <!-- Settings Groups -->
    <div class="space-y-6">
      <!-- Network Settings -->
      <div class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg">
        <h2 class="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-muted)] px-4 py-3 border-b border-[var(--color-border-subtle)] bg-[var(--color-surface)]">
          Network
        </h2>
        <div class="p-4 space-y-4">
          <div class="flex items-center justify-between">
            <div>
              <p class="text-sm text-[var(--color-text-primary)]">Max Peers</p>
              <p class="text-xs text-[var(--color-text-muted)]">Maximum number of peer connections</p>
            </div>
            <input
              v-model="settings.maxPeers"
              type="number"
              class="w-20 px-3 py-1.5 text-sm bg-[var(--color-surface)] border border-[var(--color-border)] rounded text-[var(--color-text-primary)] focus:outline-none focus:border-[var(--color-accent)]"
            />
          </div>
          <div class="flex items-center justify-between">
            <div>
              <p class="text-sm text-[var(--color-text-primary)]">RPC Enabled</p>
              <p class="text-xs text-[var(--color-text-muted)]">Enable JSON-RPC server</p>
            </div>
            <button
              @click="settings.rpcEnabled = !settings.rpcEnabled"
              :class="[
                'w-12 h-6 rounded-full transition-colors relative',
                settings.rpcEnabled ? 'bg-[var(--color-accent)]' : 'bg-[var(--color-surface)]'
              ]"
            >
              <span
                :class="[
                  'absolute top-0.5 w-5 h-5 bg-white rounded-full transition-transform',
                  settings.rpcEnabled ? 'left-6' : 'left-0.5'
                ]"
              ></span>
            </button>
          </div>
        </div>
      </div>

      <!-- Consensus Settings -->
      <div class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg">
        <h2 class="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-muted)] px-4 py-3 border-b border-[var(--color-border-subtle)] bg-[var(--color-surface)]">
          Consensus
        </h2>
        <div class="p-4 space-y-4">
          <div class="flex items-center justify-between">
            <div>
              <p class="text-sm text-[var(--color-text-primary)]">Block Timeout (seconds)</p>
              <p class="text-xs text-[var(--color-text-muted)]">Maximum time to wait for block proposal</p>
            </div>
            <input
              v-model="settings.blockTimeout"
              type="number"
              class="w-20 px-3 py-1.5 text-sm bg-[var(--color-surface)] border border-[var(--color-border)] rounded text-[var(--color-text-primary)] focus:outline-none focus:border-[var(--color-accent)]"
            />
          </div>
          <div class="flex items-center justify-between">
            <div>
              <p class="text-sm text-[var(--color-text-primary)]">Mempool Size</p>
              <p class="text-xs text-[var(--color-text-muted)]">Maximum transactions in mempool</p>
            </div>
            <input
              v-model="settings.mempoolSize"
              type="number"
              class="w-20 px-3 py-1.5 text-sm bg-[var(--color-surface)] border border-[var(--color-border)] rounded text-[var(--color-text-primary)] focus:outline-none focus:border-[var(--color-accent)]"
            />
          </div>
        </div>
      </div>

      <!-- Debug Settings -->
      <div class="bg-[var(--color-bg-secondary)] border border-[var(--color-border-subtle)] rounded-lg">
        <h2 class="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-muted)] px-4 py-3 border-b border-[var(--color-border-subtle)] bg-[var(--color-surface)]">
          Debug
        </h2>
        <div class="p-4">
          <div class="flex items-center justify-between">
            <div>
              <p class="text-sm text-[var(--color-text-primary)]">Debug Mode</p>
              <p class="text-xs text-[var(--color-text-muted)]">Enable verbose logging</p>
            </div>
            <button
              @click="settings.debugMode = !settings.debugMode"
              :class="[
                'w-12 h-6 rounded-full transition-colors relative',
                settings.debugMode ? 'bg-[var(--color-accent)]' : 'bg-[var(--color-surface)]'
              ]"
            >
              <span
                :class="[
                  'absolute top-0.5 w-5 h-5 bg-white rounded-full transition-transform',
                  settings.debugMode ? 'left-6' : 'left-0.5'
                ]"
              ></span>
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Save Button -->
    <div class="flex justify-end">
      <button class="px-4 py-2 bg-[var(--color-accent)] text-white text-sm font-medium rounded-md hover:bg-[var(--color-accent)]/90 transition-colors">
        Save Changes
      </button>
    </div>
  </div>
</template>
