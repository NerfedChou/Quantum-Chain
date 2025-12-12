import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

// Get API target from environment (for Docker) or default to localhost
const apiTarget = process.env.VITE_API_URL || 'http://localhost:8545'
const wsTarget = process.env.VITE_WS_URL || 'ws://localhost:8546'

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    vue(),
    tailwindcss()
  ],
  server: {
    port: 8765,
    proxy: {
      // Proxy API requests to Quantum-Chain node
      // This handles CORS and allows the frontend to connect to the backend
      '/api/rpc': {
        target: apiTarget,
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api\/rpc/, ''),
        configure: (proxy) => {
          proxy.on('error', (err) => {
            console.log('[Proxy] RPC connection error:', err.message)
          })
        }
      },
      '/api/health': {
        target: apiTarget.replace(':8545', ':8080'),
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api\/health/, '/health')
      },
      // WebSocket proxy for real-time updates (future)
      '/api/ws': {
        target: wsTarget,
        ws: true,
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api\/ws/, '')
      }
    }
  }
})

