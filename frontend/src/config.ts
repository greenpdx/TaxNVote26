// src/config.ts — build-time variant config.
//
// AUTH_MODE selects which sign-in UI the SPA presents, and MUST match the
// server build it talks to:
//   full — email/password registration + login (the permanent public site)
//   demo — name + 4-digit PIN identity (conference build; admin email login
//          is still reachable via the "Admin sign-in" toggle)
// Set VITE_AUTH_MODE=demo at build time for the demo bundle; defaults to full.
export const AUTH_MODE: 'full' | 'demo' =
  (import.meta.env.VITE_AUTH_MODE as string) === 'demo' ? 'demo' : 'full'

export const IS_DEMO = AUTH_MODE === 'demo'
