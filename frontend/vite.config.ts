import tailwindcss from '@tailwindcss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [
    tanstackRouter({
      target: 'react',
      autoCodeSplitting: true,
    }),
    react(),
    tailwindcss(),
  ],
  optimizeDeps: {
    // パッケージのソース経由では Base UI の依存を事前スキャンできない
    include: [
      '@base-ui/react/button',
      '@base-ui/react/dialog',
      '@base-ui/react/input',
      '@base-ui/react/select',
      '@base-ui/react/tooltip',
    ],
  },
  server: {
    proxy: {
      // バックエンド API へのプロキシ (開発時のみ)
      '/api': {
        target: process.env.VITE_API_URL ?? 'http://localhost:3000',
        changeOrigin: true,
      },
    },
  },
})
