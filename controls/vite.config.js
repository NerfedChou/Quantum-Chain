import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

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
        target: 'http://localhost:8545',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api\/rpc/, ''),
        configure: (proxy) => {
          proxy.on('error', (err) => {
            console.log('[Proxy] RPC connection error:', err.message)
          })
        }
      },
      '/api/health': {
        target: 'http://localhost:8081',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api\/health/, '/health')
      },
      // WebSocket proxy for real-time updates (future)
      '/api/ws': {
        target: 'ws://localhost:8546',
        ws: true,
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api\/ws/, '')
      }
    }
  }
})
