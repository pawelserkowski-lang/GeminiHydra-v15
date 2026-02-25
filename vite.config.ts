import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import { resolve } from 'path';

const backendUrl = process.env.VITE_BACKEND_URL || 'http://localhost:8081';
const partnerBackendUrl = process.env.VITE_PARTNER_BACKEND_URL || 'http://localhost:8082';

export default defineConfig({
  plugins: [react(), tailwindcss()],
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
  },
});
