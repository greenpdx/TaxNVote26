import { defineConfig } from '@playwright/test'

// E2E assumes both servers are already running, built as the DEMO variant
// (the specs sign in with name + 4-digit PIN):
//   * Vite frontend on :5173 — VITE_AUTH_MODE=demo npm run dev
//   * Axum API on :3000 — cargo run -p tnv-server --no-default-features --features demo
//     (proxied via Vite /api)
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
