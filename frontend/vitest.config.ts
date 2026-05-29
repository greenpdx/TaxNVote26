import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  test: {
    environment: 'happy-dom',
    globals: true,
    setupFiles: ['./tests/setup.ts'],
    // Drop the wasm shim under node_modules out of scope so vitest doesn't
    // touch the file outside our explicit init().
    server: { deps: { inline: [/wasm/] } },
  },
})
