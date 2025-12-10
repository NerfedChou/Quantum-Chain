/**
 * DOM Utility Functions
 * 
 * Helper functions for DOM manipulation.
 * 
 * @module utils/dom
 */

/**
 * Query selector shorthand.
 * @param {string} selector - CSS selector
 * @param {Element} parent - Parent element (default: document)
 * @returns {Element|null} Found element or null
 */
export function $(selector, parent = document) {
    return parent.querySelector(selector);
}

/**
 * Query selector all shorthand.
 * @param {string} selector - CSS selector
 * @param {Element} parent - Parent element (default: document)
 * @returns {NodeList} List of found elements
 */
export function $$(selector, parent = document) {
    return parent.querySelectorAll(selector);
}

/**
 * Create element with attributes and children.
 * @param {string} tag - HTML tag name
 * @param {Object} attrs - Element attributes
 * @param {Array|string} children - Child elements or text content
 * @returns {Element} Created element
 */
export function createElement(tag, attrs = {}, children = []) {
    const el = document.createElement(tag);

    Object.entries(attrs).forEach(([key, value]) => {
        if (key === 'className') {
            el.className = value;
        } else if (key === 'dataset') {
            Object.entries(value).forEach(([dataKey, dataValue]) => {
                el.dataset[dataKey] = dataValue;
            });
        } else if (key.startsWith('on') && typeof value === 'function') {
            el.addEventListener(key.slice(2).toLowerCase(), value);
        } else {
            el.setAttribute(key, value);
        }
    });

    if (typeof children === 'string') {
        el.textContent = children;
    } else if (Array.isArray(children)) {
        children.forEach(child => {
            if (typeof child === 'string') {
                el.appendChild(document.createTextNode(child));
            } else if (child instanceof Element) {
                el.appendChild(child);
            }
        });
    }

    return el;
}

/**
 * Show an element.
 * @param {Element} el - Element to show
 */
export function show(el) {
    el.classList.remove('hidden');
}

/**
 * Hide an element.
 * @param {Element} el - Element to hide
 */
export function hide(el) {
    el.classList.add('hidden');
}

/**
 * Toggle element visibility.
 * @param {Element} el - Element to toggle
 * @param {boolean} force - Force show/hide
 */
export function toggle(el, force) {
    el.classList.toggle('hidden', !force);
}

/**
 * Add event listener with delegation.
 * @param {Element} parent - Parent element
 * @param {string} eventType - Event type (e.g., 'click')
 * @param {string} selector - CSS selector for target elements
 * @param {Function} handler - Event handler
 */
export function delegate(parent, eventType, selector, handler) {
    parent.addEventListener(eventType, (event) => {
        const target = event.target.closest(selector);
        if (target && parent.contains(target)) {
            handler(event, target);
        }
    });
}
