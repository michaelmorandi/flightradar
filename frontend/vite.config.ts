import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';
import { fileURLToPath, URL } from 'node:url';

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => {
  return {
    plugins: [vue()],
    resolve: {
      alias: {
        '@': fileURLToPath(new URL('./src', import.meta.url))
      }
    },
    // In prod mode, proxy /api to the production backend so cookies are same-origin
    // (SameSite=Lax cookies are blocked on cross-origin XHR requests)
    ...(mode === 'prod' && {
      server: {
        proxy: {
          '/api': {
            target: 'https://flights-api.example.com',
            changeOrigin: true,
            secure: true,
          }
        }
      }
    })
  }
});