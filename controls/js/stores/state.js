/**
 * Simple Reactive State Store
 * 
 * Manages application state with subscriber pattern.
 * No external dependencies.
 * 
 * @module stores/state
 */

/**
 * Creates a reactive store.
 * @param {any} initialValue - Initial state value
 * @returns {Object} Store with get, set, subscribe methods
 */
export function createStore(initialValue) {
    let value = initialValue;
    const subscribers = new Set();

    return {
        /**
         * Get current value.
         * @returns {any} Current state
         */
        get() {
            return value;
        },

        /**
         * Set new value and notify subscribers.
         * @param {any} newValue - New state value
         */
        set(newValue) {
            value = newValue;
            subscribers.forEach(callback => callback(value));
        },

        /**
         * Update value using a function.
         * @param {Function} updater - Function that receives current value and returns new value
         */
        update(updater) {
            this.set(updater(value));
        },

        /**
         * Subscribe to state changes.
         * @param {Function} callback - Function called on state change
         * @returns {Function} Unsubscribe function
         */
        subscribe(callback) {
            subscribers.add(callback);
            // Call immediately with current value
            callback(value);
            // Return unsubscribe function
            return () => subscribers.delete(callback);
        },
    };
}

// ============================================================================
// Application State
// ============================================================================

/**
 * Current page/route.
 */
export const currentPage = createStore('dashboard');

/**
 * List of subsystems with their status.
 */
export const subsystems = createStore([]);

/**
 * System metrics (CPU, Memory, Disk).
 */
export const metrics = createStore({
    cpu: 0,
    memory: 0,
    disk: 0,
});

/**
 * Blockchain statistics.
 */
export const blockchainStats = createStore({
    blockHeight: 0,
    peers: 0,
    mempool: 0,
    finalized: 0,
});

/**
 * Recent log entries.
 */
export const logs = createStore([]);

/**
 * API connection status.
 */
export const apiStatus = createStore('disconnected');

/**
 * Currently selected subsystem (for detail view).
 */
export const selectedSubsystem = createStore(null);

/**
 * User settings.
 */
export const settings = createStore({
    apiEndpoint: 'http://localhost:8080/api/v1',
    refreshInterval: 5000,
    theme: 'dark',
});
