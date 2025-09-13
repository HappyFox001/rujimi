import { fileURLToPath, URL } from 'node:url'
import { resolve, dirname } from 'node:path'

import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import vueDevTools from 'vite-plugin-vue-devtools'

// 获取 __dirname 等效值
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// https://vite.dev/config/
export default defineConfig({
  base: '/assets/', 
  plugins: [
    vue(),
    vueDevTools(),
  ],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url))
    },
  },
  build: {
    // 输出目录设置为 rujimi/assets
    outDir: resolve(__dirname, '../assets'),
    // 不生成 HTML 文件，我们将在 build.js 中手动创建
    emptyOutDir: true,
    // 禁用自动添加哈希值到文件名
    rollupOptions: {
      output: {
        // 使用固定文件名，不添加哈希值
        entryFileNames: 'main.js',
        chunkFileNames: '[name].js',
        assetFileNames: (assetInfo) => {
          if (assetInfo.name === 'style.css') {
            return 'index.css';
          }
          // 对于其他资源，使用原始文件名
          return '[name].[ext]';
        }
      }
    }
  },
})
