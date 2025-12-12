/**
 * Subsystem Data Composable - Single source of truth for subsystem metrics
 * 
 * Uses QC-16 API Gateway as the ONLY door for all data.
 * No hard-wired mock data - everything flows through the gateway.
 */

import { ref, watch, onMounted, onUnmounted } from 'vue'
import { useApi } from './useApi'

/**
 * Get data for a single subsystem
 * @param {Ref<string>} subsystemId - Reactive subsystem ID (e.g., 'qc-01')
 * @returns {object} - Reactive data, loading state, error, and refresh function
 */
export function useSubsystemData(subsystemId) {
    const { rpc, connected } = useApi()

    const data = ref(null)
    const loading = ref(true)
    const error = ref(null)

    // Refresh interval handle
    let refreshInterval = null

    /**
     * Fetch subsystem data from gateway
     */
    async function refresh() {
        if (!subsystemId.value) return

        loading.value = true
        error.value = null

        try {
            // Use debug_subsystemStatus for detailed single-subsystem data
            const result = await rpc('debug_subsystemStatus', [subsystemId.value])
            data.value = result
        } catch (e) {
            error.value = e.message
            // Keep previous data on error for graceful degradation
        } finally {
            loading.value = false
        }
    }

    // Watch for subsystem ID changes and auto-refresh
    watch(subsystemId, (newId, oldId) => {
        if (newId !== oldId) {
            data.value = null // Clear old data
            refresh()
        }
    }, { immediate: true })

    // Watch connection status - refresh when reconnected
    watch(connected, (isConnected) => {
        if (isConnected && !data.value) {
            refresh()
        }
    })

    // Auto-refresh every 5 seconds when component is mounted
    onMounted(() => {
        refreshInterval = setInterval(() => {
            if (connected.value) {
                refresh()
            }
        }, 5000)
    })

    onUnmounted(() => {
        if (refreshInterval) {
            clearInterval(refreshInterval)
        }
    })

    return {
        data,
        loading,
        error,
        refresh
    }
}

/**
 * Get health/status for ALL subsystems (for grid view)
 * @returns {object} - Reactive subsystems array, loading, error, and refresh
 */
export function useAllSubsystems() {
    const { rpc, connected } = useApi()

    const subsystems = ref([])
    const loading = ref(true)
    const error = ref(null)

    let refreshInterval = null

    /**
     * Fetch all subsystem health from gateway
     */
    async function refresh() {
        loading.value = true
        error.value = null

        try {
            const result = await rpc('debug_subsystemHealth')
            subsystems.value = result.subsystems || []
        } catch (e) {
            error.value = e.message
            // Keep previous data on error
        } finally {
            loading.value = false
        }
    }

    // Initial fetch
    onMounted(() => {
        refresh()
        // Auto-refresh every 10 seconds
        refreshInterval = setInterval(() => {
            if (connected.value) {
                refresh()
            }
        }, 10000)
    })

    onUnmounted(() => {
        if (refreshInterval) {
            clearInterval(refreshInterval)
        }
    })

    // Refresh when connection is restored
    watch(connected, (isConnected) => {
        if (isConnected && subsystems.value.length === 0) {
            refresh()
        }
    })

    return {
        subsystems,
        loading,
        error,
        refresh
    }
}

/**
 * Get IPC metrics for the gateway
 * @returns {object} - Reactive metrics, loading, error, and refresh
 */
export function useIpcMetrics() {
    const { rpc, connected } = useApi()

    const metrics = ref(null)
    const loading = ref(true)
    const error = ref(null)

    async function refresh() {
        loading.value = true
        try {
            metrics.value = await rpc('debug_ipcMetrics')
            error.value = null
        } catch (e) {
            error.value = e.message
        } finally {
            loading.value = false
        }
    }

    onMounted(refresh)

    return {
        metrics,
        loading,
        error,
        refresh
    }
}
