/// <reference types="vitest/config" />
import { defineConfig, loadEnv } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import { visualizer } from 'rollup-plugin-visualizer';
import { resolve } from 'path';

export default defineConfig(({ mode }) => {
  // Load ALL env vars (empty prefix = no VITE_ filter)
  const env = loadEnv(mode, process.cwd(), '');
  const backendUrl = env.VITE_BACKEND_URL || 'http://localhost:8081';
  const partnerBackendUrl = env.VITE_PARTNER_BACKEND_URL || 'http://localhost:8082';
  const adkUrl = env.VITE_ADK_URL || 'http://localhost:8000';

  return {
    plugins: [
      react({
        babel: {
          plugins: [['babel-plugin-react-compiler', { target: '19' }]],
        },
      }),
      tailwindcss(),
      // Bundle size tracking: always generate stats.html on build, auto-open in analyze mode
      ...(mode === 'production'
        ? [visualizer({ open: false, filename: 'dist/stats.html', gzipSize: true, brotliSize: true })]
        : mode === 'analyze'
          ? [visualizer({ open: true, filename: 'dist/stats.html', gzipSize: true, brotliSize: true })]
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
        target: backendUrl.replace(/^http/, 'ws'),
        changeOrigin: true,
        ws: true,
      },
      '/partner-api': {
        target: partnerBackendUrl,
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/partner-api/, '/api'),
      },
      '/adk-api': {
        target: adkUrl,
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/adk-api/, ''),
      },
    },
  },
  preview: {
    port: 4176,
    proxy: {
      '/api': {
        target: backendUrl,
        changeOrigin: true,
        secure: backendUrl.startsWith('https'),
      },
      '/ws': {
        target: backendUrl.replace(/^http/, 'ws'),
        changeOrigin: true,
        ws: true,
      },
      '/partner-api': {
        target: partnerBackendUrl,
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/partner-api/, '/api'),
      },
      '/adk-api': {
        target: adkUrl,
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/adk-api/, ''),
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
          'vendor-ui': ['sonner', 'tailwind-merge', 'clsx', 'dompurify'],
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
  };
});
