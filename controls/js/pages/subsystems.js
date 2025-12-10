/**
 * Subsystems Page Module
 * 
 * Handles rendering and interaction for the subsystems list.
 * 
 * @module pages/subsystems
 */

import { $, createElement, delegate } from '../utils/dom.js';
import { selectedSubsystem } from '../stores/state.js';

/**
 * Initialize the subsystems page.
 */
export function initSubsystemsPage() {
  const grid = $('#subsystems-grid');
  if (!grid) return;

  // Event delegation for card clicks
  delegate(grid, 'click', '.card--clickable', (event, card) => {
    const id = card.dataset.id;
    openSubsystemDetail(id);
  });

  // Event delegation for action buttons
  delegate(grid, 'click', '.button[data-action]', (event, button) => {
    event.stopPropagation(); // Prevent card click
    const action = button.dataset.action;
    const id = button.dataset.id;
    handleAction(action, id);
  });
}

/**
 * Render the subsystems grid.
 * @param {Array} subsystemsList - List of subsystem objects
 */
export function renderSubsystemsGrid(subsystemsList) {
  const grid = $('#subsystems-grid');
  if (!grid) return;

  grid.innerHTML = '';

  subsystemsList.forEach(sub => {
    const card = createSubsystemCard(sub);
    grid.appendChild(card);
  });

  // Start live metric updates for a "feel alive" effect
  startLiveMetricUpdates(grid);
}

// Store interval ID for cleanup
let liveUpdateInterval = null;

/**
 * Simulate live metric updates on cards.
 * @param {Element} grid - The subsystems grid element
 */
function startLiveMetricUpdates(grid) {
  // Clear any previous interval
  if (liveUpdateInterval) clearInterval(liveUpdateInterval);

  liveUpdateInterval = setInterval(() => {
    const cards = grid.querySelectorAll('.subsystem-card');
    cards.forEach(card => {
      // Randomize CPU slightly (+/- 5%)
      const cpuValueEl = card.querySelector('.subsystem-card__metric-row:first-child .subsystem-card__value');
      const cpuBarEl = card.querySelector('.subsystem-card__metric-row:first-child .subsystem-card__bar-fill');
      if (cpuValueEl && cpuBarEl) {
        const currentCpu = parseInt(cpuValueEl.textContent) || 10;
        const newCpu = Math.max(1, Math.min(100, currentCpu + Math.floor(Math.random() * 11) - 5));
        cpuValueEl.textContent = `${newCpu}%`;
        cpuBarEl.style.width = `${newCpu}%`;
      }

      // Randomize Network activity
      const networkValueEl = card.querySelector('.subsystem-card__mini-metric:first-child .subsystem-card__value--sm');
      if (networkValueEl) {
        const newNetwork = Math.floor(Math.random() * 500 + 10);
        networkValueEl.textContent = `${newNetwork} msg/s`;
      }
    });
  }, 3000); // Update every 3 seconds
}

/**
 * Create a subsystem card element.
 * @param {Object} sub - Subsystem data
 * @returns {Element} Card element
 */
function createSubsystemCard(sub) {
  const statusClass = {
    healthy: 'status-dot--healthy',
    warning: 'status-dot--warning',
    error: 'status-dot--error',
    offline: 'status-dot--offline',
  }[sub.status] || '';

  // Simulate extra metrics for a richer UI
  const uptime = Math.floor(Math.random() * 24 + 1) + 'd ' + Math.floor(Math.random() * 23) + 'h';
  const network = Math.floor(Math.random() * 500 + 10);

  // Calculate load status based on CPU
  let loadStatus, loadClass;
  if (sub.cpu <= 50) {
    loadStatus = 'Good';
    loadClass = 'subsystem-card__load--good';
  } else if (sub.cpu <= 80) {
    loadStatus = 'Average';
    loadClass = 'subsystem-card__load--average';
  } else {
    loadStatus = 'Overloaded';
    loadClass = 'subsystem-card__load--overloaded';
  }

  const card = createElement('article', {
    className: 'card card--clickable subsystem-card',
    dataset: { id: sub.id },
    'aria-labelledby': `subsystem-${sub.id}`,
  });

  card.innerHTML = `
    <header class="card__header subsystem-card__header">
      <div class="subsystem-card__title-row">
        <div class="subsystem-card__name-group">
            <span class="subsystem-card__id">${sub.id.toUpperCase()}</span>
            <h3 id="subsystem-${sub.id}" class="subsystem-card__name">${sub.name}</h3>
        </div>
        <span class="subsystem-card__load ${loadClass}">${loadStatus}</span>
      </div>
    </header>
    
    <div class="card__body subsystem-card__body">
      <!-- CPU Metric -->
      <div class="subsystem-card__metric-row">
        <div class="subsystem-card__metric-info">
            <span class="subsystem-card__label">CPU Usage</span>
            <span class="subsystem-card__value">${sub.cpu}%</span>
        </div>
        <div class="subsystem-card__bar-track">
            <div class="subsystem-card__bar-fill" style="width: ${sub.cpu}%; background-color: var(--color-accent);"></div>
        </div>
      </div>

      <!-- Memory Metric -->
      <div class="subsystem-card__metric-row">
        <div class="subsystem-card__metric-info">
            <span class="subsystem-card__label">Memory</span>
            <span class="subsystem-card__value">${sub.memory} MB</span>
        </div>
        <div class="subsystem-card__bar-track">
            <div class="subsystem-card__bar-fill" style="width: ${Math.min(100, (sub.memory / 1024) * 100)}%; background-color: var(--color-info);"></div>
        </div>
      </div>

      <!-- Secondary Metrics Row -->
      <div class="subsystem-card__secondary-metrics">
        <div class="subsystem-card__stat-row">
            <span class="subsystem-card__label">Network</span>
            <span class="subsystem-card__value--sm">${network} msg/s</span>
        </div>
        <div class="subsystem-card__stat-row">
            <span class="subsystem-card__label">Uptime</span>
            <span class="subsystem-card__value--sm">${uptime}</span>
        </div>
      </div>
    </div>
  `;

  return card;
}

/**
 * Open subsystem detail panel.
 * @param {string} id - Subsystem ID
 */
function openSubsystemDetail(id) {
  console.log(`[Subsystems] Opening detail for: ${id}`);
  selectedSubsystem.set(id);
  // TODO: Implement slide-in detail panel
}

/**
 * Handle action button clicks.
 * @param {string} action - Action name
 * @param {string} id - Subsystem ID
 */
async function handleAction(action, id) {
  console.log(`[Subsystems] Action "${action}" for: ${id}`);

  switch (action) {
    case 'restart':
      // TODO: Call gateway.restartSubsystem(id)
      alert(`Restarting ${id}...`);
      break;
    case 'stop':
      // TODO: Call gateway.stopSubsystem(id)
      break;
  }
}
