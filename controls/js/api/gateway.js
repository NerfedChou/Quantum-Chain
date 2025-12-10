/**
 * Gateway API Module
 * 
 * Single point of communication with QC-16 API Gateway.
 * All subsystem interactions flow through this module.
 * 
 * @module api/gateway
 */

const API_BASE = '/api/v1';

/**
 * Makes a fetch request to the API Gateway.
 * @param {string} endpoint - API endpoint path
 * @param {RequestInit} options - Fetch options
 * @returns {Promise<any>} Parsed JSON response
 */
async function request(endpoint, options = {}) {
    const url = `${API_BASE}${endpoint}`;

    const defaultHeaders = {
        'Content-Type': 'application/json',
    };

    try {
        const response = await fetch(url, {
            ...options,
            headers: {
                ...defaultHeaders,
                ...options.headers,
            },
        });

        if (!response.ok) {
            throw new Error(`API Error: ${response.status} ${response.statusText}`);
        }

        return response.json();
    } catch (error) {
        console.error(`[Gateway] Request failed: ${endpoint}`, error);
        throw error;
    }
}

// ============================================================================
// Subsystem Endpoints
// ============================================================================

/**
 * Get all subsystems status.
 * @returns {Promise<Array>} List of subsystem statuses
 */
export async function getSubsystems() {
    return request('/subsystems');
}

/**
 * Get detailed status for a specific subsystem.
 * @param {string} id - Subsystem ID (e.g., 'qc-08')
 * @returns {Promise<Object>} Subsystem details
 */
export async function getSubsystemDetail(id) {
    return request(`/subsystems/${id}`);
}

/**
 * Restart a specific subsystem.
 * @param {string} id - Subsystem ID
 * @returns {Promise<Object>} Restart result
 */
export async function restartSubsystem(id) {
    return request(`/subsystems/${id}/restart`, { method: 'POST' });
}

/**
 * Stop a specific subsystem.
 * @param {string} id - Subsystem ID
 * @returns {Promise<Object>} Stop result
 */
export async function stopSubsystem(id) {
    return request(`/subsystems/${id}/stop`, { method: 'POST' });
}

// ============================================================================
// Metrics Endpoints
// ============================================================================

/**
 * Get current system metrics (CPU, Memory, Disk).
 * @returns {Promise<Object>} System metrics
 */
export async function getMetrics() {
    return request('/metrics');
}

/**
 * Get blockchain statistics.
 * @returns {Promise<Object>} Blockchain stats (block height, peers, etc.)
 */
export async function getBlockchainStats() {
    return request('/stats');
}

// ============================================================================
// Anomalies Endpoints
// ============================================================================

/**
 * Get anomaly/error logs.
 * @param {Object} filters - Filter options
 * @param {string} filters.level - Log level (info, warn, error)
 * @param {string} filters.subsystem - Filter by subsystem ID
 * @param {number} filters.limit - Max entries to return
 * @returns {Promise<Array>} List of anomalies
 */
export async function getAnomalies(filters = {}) {
    const params = new URLSearchParams(filters);
    return request(`/anomalies?${params}`);
}

// ============================================================================
// Settings Endpoints
// ============================================================================

/**
 * Get current configuration.
 * @returns {Promise<Object>} Configuration object
 */
export async function getConfig() {
    return request('/config');
}

/**
 * Update configuration.
 * @param {Object} config - Configuration updates
 * @returns {Promise<Object>} Updated configuration
 */
export async function updateConfig(config) {
    return request('/config', {
        method: 'PATCH',
        body: JSON.stringify(config),
    });
}

// ============================================================================
// Health Check
// ============================================================================

/**
 * Check API Gateway health.
 * @returns {Promise<boolean>} True if connected
 */
export async function checkHealth() {
    try {
        await request('/health');
        return true;
    } catch {
        return false;
    }
}

/**
 * Set custom API base URL.
 * @param {string} url - New API base URL
 */
export function setApiBase(url) {
    // This would need to be implemented differently in a real app
    // For now, we'll store it in localStorage
    localStorage.setItem('apiBase', url);
}
