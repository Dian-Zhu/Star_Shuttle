import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [
    svelte({
      // 使用新的ES模块配置文件
      configFile: './svelte.config.js',
    }),
  ],
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    outDir: 'dist',
    sourcemap: false,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('node_modules/xterm') || id.includes('node_modules/xterm-addon-')) {
            return 'vendor-xterm';
          }
          if (id.includes('node_modules/@tauri-apps/')) {
            return 'vendor-tauri';
          }
          if (id.includes('node_modules/svelte')) {
            return 'vendor-svelte';
          }
          return undefined;
        },
      },
    },
  },
})
