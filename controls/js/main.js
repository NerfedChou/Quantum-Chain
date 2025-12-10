/**
 * QC-Controls Main Entry Point
 * 
 * Initializes the application and sets up routing.
 * 
 * @module main
 */

import { $, $$, delegate, show, hide } from './utils/dom.js';
import { currentPage, subsystems, apiStatus } from './stores/state.js';
import * as gateway from './api/gateway.js';
import { initSubsystemsPage, renderSubsystemsGrid } from './pages/subsystems.js';
import { initTopology } from './components/topology.js';

// ============================================================================
// Navigation
// ============================================================================

/**
 * Initialize navigation sidebar.
 */
function initNavigation() {
    const navItems = $$('.sidebar__item[data-page]');

    navItems.forEach(item => {
        item.addEventListener('click', (e) => {
            e.preventDefault();
            const page = item.dataset.page;
            navigateTo(page);
        });
    });

    // Handle browser back/forward
    window.addEventListener('popstate', () => {
        const page = location.hash.replace('#', '') || 'dashboard';
        navigateTo(page, false);
    });

    // Initial route
    const initialPage = location.hash.replace('#', '') || 'dashboard';
    navigateTo(initialPage, false);
}

/**
 * Navigate to a page.
 * @param {string} page - Page name
 * @param {boolean} pushState - Whether to push to history
 */
function navigateTo(page, pushState = true) {
    // Update state
    currentPage.set(page);

    // Update URL
    if (pushState) {
        history.pushState(null, '', `#${page}`);
    }

    // Update nav active state
    $$('.sidebar__item').forEach(item => {
        item.classList.toggle('sidebar__item--active', item.dataset.page === page);
        item.setAttribute('aria-current', item.dataset.page === page ? 'page' : 'false');
    });

    // Show/hide pages
    $$('.page').forEach(pageEl => {
        const pageId = pageEl.id.replace('page-', '');
        if (pageId === page) {
            show(pageEl);
        } else {
            hide(pageEl);
        }
    });

    // Page-specific initialization
    if (page === 'subsystems') {
        loadSubsystems();
    } else if (page === 'dashboard') {
        // Initialize network topology visualization
        setTimeout(() => initTopology(), 100);
    }

    console.log(`[Router] Navigated to: ${page}`);
}

// ============================================================================
// Data Loading
// ============================================================================

/**
 * Load subsystems data from API.
 */
async function loadSubsystems() {
    try {
        // For now, use mock data since API Gateway isn't built yet
        const mockSubsystems = [
            { id: 'qc-01', name: 'Peer Discovery', status: 'healthy', cpu: 5, memory: 48 },
            { id: 'qc-02', name: 'Block Storage', status: 'healthy', cpu: 12, memory: 256 },
            { id: 'qc-03', name: 'Transaction Indexing', status: 'healthy', cpu: 8, memory: 128 },
            { id: 'qc-04', name: 'State Management', status: 'healthy', cpu: 15, memory: 512 },
            { id: 'qc-05', name: 'Block Propagation', status: 'healthy', cpu: 3, memory: 64 },
            { id: 'qc-06', name: 'Mempool', status: 'healthy', cpu: 10, memory: 192 },
            { id: 'qc-07', name: 'Bloom Filters', status: 'healthy', cpu: 2, memory: 32 },
            { id: 'qc-08', name: 'Consensus', status: 'healthy', cpu: 25, memory: 384 },
            { id: 'qc-09', name: 'Finality', status: 'healthy', cpu: 8, memory: 128 },
            { id: 'qc-10', name: 'Signature Verification', status: 'healthy', cpu: 18, memory: 96 },
            { id: 'qc-11', name: 'Smart Contracts', status: 'warning', cpu: 35, memory: 768 },
            { id: 'qc-12', name: 'Transaction Ordering', status: 'healthy', cpu: 7, memory: 64 },
            { id: 'qc-13', name: 'Light Client Sync', status: 'healthy', cpu: 4, memory: 48 },
            { id: 'qc-14', name: 'Sharding', status: 'healthy', cpu: 6, memory: 128 },
            { id: 'qc-15', name: 'Cross-Chain', status: 'healthy', cpu: 3, memory: 64 },
            { id: 'qc-16', name: 'API Gateway', status: 'healthy', cpu: 12, memory: 96 },
            { id: 'qc-17', name: 'Block Production', status: 'healthy', cpu: 20, memory: 256 },
        ];

        subsystems.set(mockSubsystems);
        renderSubsystemsGrid(mockSubsystems);
    } catch (error) {
        console.error('[App] Failed to load subsystems:', error);
    }
}

/**
 * Check API Gateway connection.
 */
async function checkConnection() {
    try {
        const healthy = await gateway.checkHealth();
        apiStatus.set(healthy ? 'connected' : 'disconnected');

        const statusEl = $('#api-status');
        if (statusEl) {
            statusEl.textContent = healthy ? 'Connected' : 'Disconnected';
            statusEl.className = healthy ? 'text-success' : 'text-error';
        }
    } catch {
        apiStatus.set('disconnected');
    }
}

// ============================================================================
// Initialization
// ============================================================================

/**
 * Initialize the application.
 */
function init() {
    console.log('[App] QC-Controls initializing...');

    // Initialize navigation
    initNavigation();

    // Initialize pages
    initSubsystemsPage();

    // Check API connection
    checkConnection();

    // Periodic refresh (every 5 seconds)
    setInterval(() => {
        checkConnection();
        if (currentPage.get() === 'subsystems') {
            // loadSubsystems(); // Uncomment when API is ready
        }
    }, 5000);

    console.log('[App] QC-Controls ready');
}

// Start app when DOM is ready
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
} else {
    init();
}
