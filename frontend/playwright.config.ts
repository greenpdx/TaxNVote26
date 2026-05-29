import { defineConfig } from '@playwright/test'

// E2E assumes both servers are already running:
//   * Vite frontend on :5173 (npm run dev)
//   * Axum API server on :3000 (cargo run -p tnv-server, proxied via Vite /api)
export default defineConfig({
  testDir: './e2e',
  fullyParallel: false, // submissions hit the shared DB; serialise to keep counts predictable
  retries: 0,
  use: {
    baseURL: 'http://127.0.0.1:5173',
    headless: true,
    trace: 'retain-on-failure',
    actionTimeout: 10000,
  },
})
