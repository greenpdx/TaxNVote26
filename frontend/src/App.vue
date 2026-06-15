<script setup lang="ts">
import { onMounted } from 'vue'
import { useBudgetStore } from './stores/budget'

// Shell: initialise the budget tree once for every route (the home app and the
// public submission page both need it), then render the matched route.
const store = useBudgetStore()
onMounted(async () => { await store.init() })
</script>

<template>
  <router-view />
</template>

<style>
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, 'Helvetica Neue', Arial, sans-serif;
  background: #0f172a;
  color: #e2e8f0;
  -webkit-font-smoothing: antialiased;
}

.app { max-width: 720px; margin: 0 auto; min-height: 100vh; display: flex; flex-direction: column; }

.header {
  position: sticky; top: 0; z-index: 10;
  background: #0f172aee; backdrop-filter: blur(12px); -webkit-backdrop-filter: blur(12px);
  border-bottom: 1px solid #1e293b; padding: 12px 16px 8px;
}
.header-top { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; margin-bottom: 8px; }
.title-group { min-width: 0; }
.title { font-size: 20px; font-weight: 800; color: #f59e0b; letter-spacing: -0.02em; line-height: 1.2; }
.subtitle-row { display: flex; align-items: baseline; flex-wrap: wrap; }
.subtitle { font-size: 12px; color: #64748b; }
/* Second subtitle sits ~10em to the right of the first, on the same row. */
.subtitle-2 { margin-left: 10em; }

.tabs { display: flex; gap: 4px; }
.tabs button {
  flex: 1; background: #1e293b; border: 1px solid #334155; color: #94a3b8;
  padding: 6px 10px; border-radius: 6px; font-size: 13px; cursor: pointer;
}
.tabs button.active { background: #3b82f6; border-color: #3b82f6; color: #fff; font-weight: 600; }

.id-widget { display: flex; align-items: center; gap: 6px; flex-shrink: 0; }
.id-who { color: #34d399; font-size: 13px; font-weight: 600; }
.abtn.login { background: #2563eb; border-color: #2563eb; color: #fff; }
.abtn.login:hover { background: #1d4ed8; }

.flash { margin-top: 8px; padding: 8px 10px; border-radius: 6px; font-size: 13px; cursor: pointer;
  background: #064e3b; color: #d1fae5; border: 1px solid #065f46; word-break: break-all; }
.flash.err { background: #4c0519; color: #fecaca; border-color: #7f1d1d; }

.subbar { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 10px 16px 4px; }
.summary { display: flex; align-items: baseline; gap: 8px; }
.summary-label { color: #64748b; font-size: 13px; }
.summary-value { color: #60a5fa; font-size: 18px; font-weight: 700; font-variant-numeric: tabular-nums; }
.actions { display: flex; gap: 6px; }
.abtn { background: #1e293b; border: 1px solid #334155; color: #94a3b8; padding: 5px 9px; border-radius: 6px; font-size: 12px; cursor: pointer; white-space: nowrap; }
.abtn:hover { background: #334155; color: #e2e8f0; }

.actbar { display: flex; gap: 8px; padding: 4px 16px 8px; flex-wrap: wrap; }
.primary { background: #2563eb; border: 1px solid #2563eb; color: #fff; padding: 8px 14px; border-radius: 8px; font-size: 14px; font-weight: 600; cursor: pointer; }
.primary:hover { background: #1d4ed8; }
.primary:disabled { opacity: 0.5; cursor: not-allowed; }
.ghost { background: #1e293b; border: 1px solid #334155; color: #cbd5e1; padding: 8px 12px; border-radius: 8px; font-size: 14px; cursor: pointer; }
.ghost:hover { background: #334155; }

.saveform { display: flex; gap: 6px; flex-wrap: wrap; padding: 0 16px 8px; }
.sf-in { background: #1e293b; border: 1px solid #334155; color: #e2e8f0; padding: 7px 10px; border-radius: 6px; font-size: 13px; outline: none; flex: 1; min-width: 140px; }
.sf-wide { flex-basis: 100%; }
.sf-in:focus { border-color: #3b82f6; }

.search-wrap { position: relative; padding: 0 16px 8px; }
.search { width: 100%; background: #1e293b; border: 1px solid #334155; color: #e2e8f0; padding: 8px 32px 8px 12px; border-radius: 8px; font-size: 14px; outline: none; }
.search::placeholder { color: #475569; }
.search:focus { border-color: #3b82f6; }
.search-clear { position: absolute; right: 24px; top: 50%; transform: translateY(-50%); background: none; border: none; color: #64748b; font-size: 14px; cursor: pointer; padding: 4px; }
.search-clear:hover { color: #e2e8f0; }

.tree-container { flex: 1; overflow-y: auto; padding: 0 4px; -webkit-overflow-scrolling: touch; }
.loading { padding: 40px; text-align: center; color: #64748b; }

.footer { border-top: 1px solid #1e293b; padding: 8px 16px; text-align: center; }
.footer-stats { display: flex; gap: 8px; justify-content: center; font-size: 11px; color: #475569; }

@media (max-width: 480px) {
  .header { padding: 8px 10px 6px; }
  .title { font-size: 18px; }
  .summary-value { font-size: 16px; }
  .search { font-size: 16px; }
}
</style>
