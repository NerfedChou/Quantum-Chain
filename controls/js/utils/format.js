/**
 * Format Utility Functions
 * 
 * Formatting helpers for numbers, dates, and bytes.
 * 
 * @module utils/format
 */

/**
 * Format a number with thousand separators.
 * @param {number} num - Number to format
 * @returns {string} Formatted number
 */
export function formatNumber(num) {
    return num.toLocaleString('en-US');
}

/**
 * Format bytes to human readable string.
 * @param {number} bytes - Bytes to format
 * @param {number} decimals - Decimal places (default: 2)
 * @returns {string} Formatted string (e.g., "1.5 GB")
 */
export function formatBytes(bytes, decimals = 2) {
    if (bytes === 0) return '0 Bytes';

    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));

    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(decimals))} ${sizes[i]}`;
}

/**
 * Format a percentage.
 * @param {number} value - Value (0-100 or 0-1)
 * @param {boolean} isDecimal - If true, value is 0-1, else 0-100
 * @returns {string} Formatted percentage
 */
export function formatPercent(value, isDecimal = false) {
    const percent = isDecimal ? value * 100 : value;
    return `${percent.toFixed(1)}%`;
}

/**
 * Format a timestamp to time string.
 * @param {Date|number} timestamp - Date or Unix timestamp
 * @returns {string} Formatted time (HH:MM:SS)
 */
export function formatTime(timestamp) {
    const date = timestamp instanceof Date ? timestamp : new Date(timestamp);
    return date.toLocaleTimeString('en-US', {
        hour12: false,
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
    });
}

/**
 * Format a date to relative time (e.g., "5 minutes ago").
 * @param {Date|number} timestamp - Date or Unix timestamp
 * @returns {string} Relative time string
 */
export function formatRelativeTime(timestamp) {
    const date = timestamp instanceof Date ? timestamp : new Date(timestamp);
    const now = new Date();
    const diffMs = now - date;
    const diffSec = Math.floor(diffMs / 1000);
    const diffMin = Math.floor(diffSec / 60);
    const diffHour = Math.floor(diffMin / 60);

    if (diffSec < 60) return 'just now';
    if (diffMin < 60) return `${diffMin}m ago`;
    if (diffHour < 24) return `${diffHour}h ago`;
    return date.toLocaleDateString();
}

/**
 * Truncate a hex string for display.
 * @param {string} hex - Hex string (e.g., "0x1234567890abcdef")
 * @param {number} chars - Characters to show on each side (default: 4)
 * @returns {string} Truncated string (e.g., "0x1234...cdef")
 */
export function truncateHex(hex, chars = 4) {
    if (hex.length <= chars * 2 + 2) return hex;
    const prefix = hex.startsWith('0x') ? '0x' : '';
    const clean = hex.replace('0x', '');
    return `${prefix}${clean.slice(0, chars)}...${clean.slice(-chars)}`;
}
