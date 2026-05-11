import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { VitePWA } from 'vite-plugin-pwa'

export default defineConfig({
  plugins: [
    react(),
    VitePWA({
      registerType: 'autoUpdate',
      manifest: false, // We use public/manifest.json
      workbox: {
        globPatterns: ['**/*.{js,css,html,ico,png,svg}'],
      },
    }),
  ],
  server: {
    port: 5173,
    proxy: {
      '/radio': 'http://localhost:8080',
      '/stream': 'http://localhost:8080',
      '/playback': 'http://localhost:8080',
      '/feedback': 'http://localhost:8080',
      '/auth': 'http://localhost:8080',
      '/health': 'http://localhost:8080',
    },
  },
})
