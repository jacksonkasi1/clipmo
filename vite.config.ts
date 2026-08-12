import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { fileURLToPath } from 'node:url'

// Tauri expects a fixed port and must not fall back to another one.
const DEV_PORT = 1420

export default defineConfig({
  plugins: [react(), tailwindcss()],

  test: {
    include: ['src/**/*.test.{ts,tsx}'],
  },

  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },

  // Prevent Vite from obscuring Rust errors.
  clearScreen: false,

  server: {
    port: DEV_PORT,
    strictPort: true,
    host: false,
    watch: {
      // Rust sources are rebuilt by the Tauri CLI, not Vite.
      ignored: ['**/src-tauri/**'],
    },
  },

  // Produce output the WebView2 runtime can consume without polyfills.
  build: {
    target: 'esnext',
    minify: 'esbuild',
    sourcemap: false,
    cssCodeSplit: false,
    rollupOptions: {
      input: {
        main: fileURLToPath(new URL('./index.html', import.meta.url)),
        settings: fileURLToPath(new URL('./settings.html', import.meta.url)),
      },
    },
  },
})
