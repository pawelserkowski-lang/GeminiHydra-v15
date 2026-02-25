/// <reference types="vitest/config" />
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import { visualizer } from 'rollup-plugin-visualizer';
import { resolve } from 'path';

const backendUrl = process.env.VITE_BACKEND_URL || 'http://localhost:8081';
const partnerBackendUrl = process.env.VITE_PARTNER_BACKEND_URL || 'http://localhost:8082';

export default defineConfig(({ mode }) => ({
  plugins: [
    react(),
    tailwindcss(),
    ...(mode === 'analyze'
      ? [visualizer({ open: true, filename: 'dist/stats.html', gzipSize: true })]
      : []),
  ],
  resolve: {
    alias: {
      '@': resolve(__dirname, './src'),
    },
  },
  server: {
    port: 5176,
    proxy: {
      '/api': {
        target: backendUrl,
        changeOrigin: true,
        secure: backendUrl.startsWith('https'),
      },
      '/ws': {
        target: backendUrl,
        changeOrigin: true,
        ws: true,
      },
      '/partner-api': {
        target: partnerBackendUrl,
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/partner-api/, '/api'),
      },
    },
  },
  build: {
    target: 'esnext',
    sourcemap: true,
    rollupOptions: {
      output: {
        manualChunks: {
          'vendor-motion': ['motion'],
          'vendor-i18n': ['i18next', 'react-i18next'],
          'vendor-query': ['@tanstack/react-query'],
          'vendor-ui': ['sonner', 'tailwind-merge', 'clsx'],
          'vendor-zod': ['zod'],
        },
      },
    },
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.test.{ts,tsx}'],
  },
}));
