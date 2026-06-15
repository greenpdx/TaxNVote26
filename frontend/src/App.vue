<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { useBudgetStore } from './stores/budget'
import { useSessionStore } from './stores/session'
import NodeRow from './components/NodeRow.vue'
import AuthDialog from './components/AuthDialog.vue'
import TemplatesView from './components/TemplatesView.vue'
import ResultsView from './components/ResultsView.vue'
import AdminView from './components/AdminView.vue'
import {
  buildTaxDollarCsv, submitTaxDollar, buildTemplateCsv, createTemplate,
  myTaxDollars, getTaxDollarCsv, parseTemplateEntries,
} from './api'

const store = useBudgetStore()
const session = useSessionStore()

type View = 'budget' | 'templates' | 'results' | 'admin'
const view = ref<View>('budget')
const msg = ref<string | null>(null)
const msgErr = ref(false)
const busy = ref(false)

// Login is modal + ephemeral: log in to perform one action, then auto-logout.
const showLogin = ref(false)
const afterLogin = ref<null | (() => void)>(null)
function requireLogin(fn: () => void) {
  if (session.isIdentified) fn()
  else { afterLogin.value = fn; showLogin.value = true }
}
function onLoginSuccess() {
  showLogin.value = false
  const fn = afterLogin.value
  afterLogin.value = null
  if (fn) fn()
}
// Opening the "save as template" form requires login.
function toggleSave() {
  if (saveOpen.value) { saveOpen.value = false; return }
  requireLogin(() => { saveOpen.value = true })
}

// save-as-template form
const saveOpen = ref(false)
const saveName = ref('')
const saveEntity = ref('')
const saveDesc = ref('')

onMounted(async () => { await store.init() })

// Logout (manual, or auto after submit) returns the budget to defaults —
// a clean slate for the next person.
watch(() => session.isIdentified, (now, was) => {
  if (was && !now) { store.resetAll(); msg.value = null }
})

// Leave the admin view when the session is no longer an admin.
watch(() => session.isAdmin, (admin) => {
  if (!admin && view.value === 'admin') view.value = 'budget'
})

function toggleMode() { store.mode = store.mode === 'simple' ? 'full' : 'simple' }
function toggleBarScale() { store.barScale = store.barScale === 'linear' ? 'log' : 'linear' }
function flash(text: string, isErr = false) { msg.value = text; msgErr.value = isErr }

async function submit() {
  if (!session.isIdentified) { flash('Sign in first.', true); return }
  busy.value = true
  try {
    const csv = await buildTaxDollarCsv(store.leafAllocations(), store.fiscalYear, 'default')
    const r = await submitTaxDollar(csv, session.token!)
    flash(`Submitted ✓ as ${session.name} · receipt ${r.receipt_token}${r.replaced ? ' (replaced your prior one)' : ''}`)
    // Stay signed in after submitting (you can change and re-submit; it upserts).
  } catch (e) {
    flash('Submit failed: ' + (e instanceof Error ? e.message : String(e)), true)
  } finally { busy.value = false }
}

async function saveTemplate() {
  if (!session.isIdentified) { flash('Sign in first.', true); return }
  if (saveName.value.trim().length < 3) { flash('Template name must be ≥3 chars.', true); return }
  busy.value = true
  try {
    const csv = buildTemplateCsv(store.leafAllocations(), {
      name: saveName.value.trim(), entity: saveEntity.value.trim(),
      description: saveDesc.value.trim(), fiscalYear: store.fiscalYear,
    })
    const r = await createTemplate(csv, session.token!)
    flash(`Saved template ${r.receipt_no}`)
    saveOpen.value = false; saveName.value = ''; saveEntity.value = ''; saveDesc.value = ''
  } catch (e) {
    flash('Save failed: ' + (e instanceof Error ? e.message : String(e)), true)
  } finally { busy.value = false }
}

async function loadMine() {
  if (!session.isIdentified) { flash('Sign in first.', true); return }
  busy.value = true
  try {
    const mine = await myTaxDollars(session.token!)
    if (mine.length === 0) { flash('No saved submission yet for you.', true); return }
    const csv = await getTaxDollarCsv(mine[0].receipt_token)
    const total = store.totalValue
    // tax-dollar rows are id,pct → convert to dollar values for the tree
    const entries = parseTemplateEntries(csv).map(e => ({ id: e.id, value: e.value * total }))
    store.applyTemplateEntries(entries)
    view.value = 'budget'
    flash(`Loaded ${session.name}'s submission ${mine[0].receipt_token} — Logout to clear.`)
    // No auto-logout here: logout resets the tree, which would wipe the view.
  } catch (e) {
    flash('Load failed: ' + (e instanceof Error ? e.message : String(e)), true)
  } finally { busy.value = false }
}
</script>

<template>
  <div class="app">
    <AuthDialog :open="showLogin" @close="showLogin = false" @success="onLoginSuccess" />
    <header class="header">
      <div class="header-top">
        <div class="title-group">
          <h1 class="title">Tax N Vote</h1>
          <span class="subtitle">Your Tax Dollar, Your Voice</span>
        </div>
        <div class="id-widget">
          <template v-if="session.isIdentified">
            <span class="id-who">👤 {{ session.name }}</span>
            <button class="abtn" @click="session.logout()">Logout</button>
          </template>
          <button v-else class="abtn login" @click="showLogin = true">Login</button>
        </div>
      </div>

      <nav class="tabs">
        <button :class="{ active: view === 'budget' }" @click="view = 'budget'">Budget</button>
        <button :class="{ active: view === 'templates' }" @click="view = 'templates'">Templates</button>
        <button :class="{ active: view === 'results' }" @click="view = 'results'">Results</button>
        <button v-if="session.isAdmin" :class="{ active: view === 'admin' }" @click="view = 'admin'">Admin</button>
      </nav>

      <div v-if="msg" class="flash" :class="{ err: msgErr }" @click="msg = null">{{ msg }}</div>
    </header>

    <!-- ─── Budget view ─── -->
    <template v-if="view === 'budget'">
      <div class="subbar">
        <div class="summary">
          <span class="summary-label">Federal Budget (discretionary)</span>
          <span class="summary-value">{{ store.formatDollars(store.totalValue) }}</span>
        </div>
        <div class="actions">
          <button class="abtn" @click="toggleBarScale">📊 {{ store.barScale === 'linear' ? 'Linear' : 'Log' }}</button>
          <button class="abtn" @click="toggleMode">{{ store.mode === 'simple' ? '◈ Full' : '◇ Simple' }}</button>
          <button class="abtn" @click="store.resetAll()">↺ Reset</button>
        </div>
      </div>

      <div v-if="session.isIdentified" class="actbar">
        <button class="primary" :disabled="busy" @click="submit">Submit my Tax Dollar</button>
        <button class="ghost" :disabled="busy" @click="toggleSave">Save as template</button>
        <button class="ghost" :disabled="busy" @click="loadMine">View my submission</button>
      </div>

      <div v-if="saveOpen" class="saveform">
        <input v-model="saveName" class="sf-in" placeholder="Template name (≥3)" maxlength="128" />
        <input v-model="saveEntity" class="sf-in" placeholder="Entity (org) name" maxlength="128" />
        <input v-model="saveDesc" class="sf-in sf-wide" placeholder="Description (optional)" maxlength="512" />
        <button class="primary" :disabled="busy" @click="requireLogin(saveTemplate)">Save</button>
      </div>

      <div class="search-wrap">
        <input type="text" class="search" placeholder="Search agencies, bureaus, accounts…" v-model="store.searchQuery" />
        <button v-if="store.searchQuery" class="search-clear" @click="store.searchQuery = ''">✕</button>
      </div>

      <main class="tree-container">
        <div v-if="store.initError" class="loading" style="color:#f87171">{{ store.initError }}</div>
        <div v-else-if="store.nodes.length === 0" class="loading">Loading budget…</div>
        <div v-else class="tree">
          <NodeRow v-for="topic in store.filteredTopics" :key="topic.idx" :node="topic" />
        </div>
      </main>
    </template>

    <!-- ─── Templates view ─── -->
    <main v-else-if="view === 'templates'" class="tree-container">
      <TemplatesView @loaded="view = 'budget'" />
    </main>

    <!-- ─── Results view ─── -->
    <main v-else-if="view === 'results'" class="tree-container">
      <ResultsView />
    </main>

    <!-- ─── Admin view ─── -->
    <main v-else class="tree-container">
      <AdminView />
    </main>

    <footer class="footer">
      <div class="footer-stats">
        <span>{{ store.nodes.length }} nodes</span>
        <span>·</span>
        <span>{{ store.mode }} mode</span>
        <span>·</span>
        <span>FY{{ store.fiscalYear }}</span>
      </div>
    </footer>
  </div>
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
.subtitle { font-size: 12px; color: #64748b; }

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
