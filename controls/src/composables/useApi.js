/**
 * API composable for connecting to Quantum-Chain node
 * 
 * Uses JSON-RPC over HTTP to communicate with qc-16-api-gateway.
 * Falls back gracefully if node is not running.
 */

import { ref, readonly } from 'vue'

// API endpoints - proxied through Vite dev server
const RPC_ENDPOINT = '/api/rpc'
const HEALTH_ENDPOINT = '/api/health'

// Connection state (reactive)
const connected = ref(false)
const connecting = ref(false)
const lastError = ref(null)
const latencyMs = ref(0)

// Request counter for JSON-RPC id
let requestId = 1

/**
 * Make a JSON-RPC 2.0 call to the node
 * @param {string} method - RPC method name (e.g., 'eth_blockNumber')
 * @param {Array} params - Method parameters
 * @returns {Promise<any>} - Result from the node
 */
async function rpc(method, params = []) {
    const startTime = performance.now()

    try {
        const response = await fetch(RPC_ENDPOINT, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                jsonrpc: '2.0',
                id: requestId++,
                method,
                params
            })
        })

        latencyMs.value = Math.round(performance.now() - startTime)

        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`)
        }

        const data = await response.json()

        if (data.error) {
            throw new Error(data.error.message || 'RPC Error')
        }

        connected.value = true
        lastError.value = null
        return data.result
    } catch (error) {
        lastError.value = error.message
        // Don't set connected = false on single failure
        // Let health check handle connection status
        throw error
    }
}

/**
 * Check node health and update connection status
 * @returns {Promise<boolean>} - True if healthy
 */
async function checkHealth() {
    connecting.value = true
    const startTime = performance.now()

    try {
        // Try a simple RPC call (eth_chainId is always available)
        const response = await fetch(RPC_ENDPOINT, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                jsonrpc: '2.0',
                id: requestId++,
                method: 'eth_chainId',
                params: []
            }),
            signal: AbortSignal.timeout(3000) // 3 second timeout
        })

        latencyMs.value = Math.round(performance.now() - startTime)

        if (response.ok) {
            const data = await response.json()
            connected.value = !data.error
            lastError.value = data.error?.message || null
            return connected.value
        }

        connected.value = false
        lastError.value = `HTTP ${response.status}`
        return false
    } catch (error) {
        connected.value = false
        lastError.value = error.name === 'TimeoutError' ? 'Connection timeout' : error.message
        return false
    } finally {
        connecting.value = false
    }
}

/**
 * Get subsystem-specific metrics
 * @param {string} subsystemId - Subsystem ID (e.g., 'qc-01')
 * @returns {Promise<object>} - Subsystem metrics
 */
async function getSubsystemMetrics(subsystemId) {
    return rpc('debug_subsystemMetrics', [subsystemId])
}

/**
 * Get latest block number
 * @returns {Promise<number>} - Block height
 */
async function getBlockNumber() {
    const result = await rpc('eth_blockNumber')
    return parseInt(result, 16)
}

/**
 * Get peer count
 * @returns {Promise<number>} - Number of connected peers
 */
async function getPeerCount() {
    const result = await rpc('net_peerCount')
    return parseInt(result, 16)
}

/**
 * Get mempool status
 * @returns {Promise<object>} - { pending, queued }
 */
async function getMempoolStatus() {
    const result = await rpc('txpool_status')
    return {
        pending: parseInt(result.pending, 16),
        queued: parseInt(result.queued, 16)
    }
}

/**
 * Get node info
 * @returns {Promise<object>} - Node information
 */
async function getNodeInfo() {
    return rpc('admin_nodeInfo')
}

/**
 * Get connected peers list
 * @returns {Promise<Array>} - List of peers
 */
async function getPeers() {
    return rpc('admin_peers')
}

/**
 * Composable hook for API access
 */
export function useApi() {
    return {
        // State (readonly refs)
        connected: readonly(connected),
        connecting: readonly(connecting),
        lastError: readonly(lastError),
        latencyMs: readonly(latencyMs),

        // Core methods
        rpc,
        checkHealth,

        // Convenience methods
        getSubsystemMetrics,
        getBlockNumber,
        getPeerCount,
        getMempoolStatus,
        getNodeInfo,
        getPeers
    }
}
