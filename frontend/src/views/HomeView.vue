<script setup lang="ts">
import { ref, watch } from 'vue'
import { useBudgetStore } from '../stores/budget'
import { useSessionStore } from '../stores/session'
import NodeRow from '../components/NodeRow.vue'
import AppHeader from '../components/AppHeader.vue'
import AuthDialog from '../components/AuthDialog.vue'
import TemplatesView from '../components/TemplatesView.vue'
import ResultsView from '../components/ResultsView.vue'
import AdminView from '../components/AdminView.vue'
import HelpDialog from '../components/HelpDialog.vue'
import ReceiptDialog from '../components/ReceiptDialog.vue'
import {
  buildTaxDollarCsv, submitTaxDollar, buildTemplateCsv, createTemplate,
  myTaxDollars, parseTemplateEntries,
} from '../api'

const store = useBudgetStore()
const session = useSessionStore()

type View = 'budget' | 'templates' | 'results' | 'admin'
const view = ref<View>('budget')
const msg = ref<string | null>(null)
const msgErr = ref(false)
const busy = ref(false)

const showLogin = ref(false)
const showHelp = ref(false)
const showReceipt = ref(false)
const submittedReceipt = ref<string | null>(null)
const submittedCode = ref('')
const submittedCsv = ref('')
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
function toggleSave() {
  if (saveOpen.value) { saveOpen.value = false; return }
  requireLogin(() => { saveOpen.value = true })
}

// save-as-template form
const saveOpen = ref(false)
const saveName = ref('')
const saveEntity = ref('')
const saveDesc = ref('')

// Logout returns the budget to defaults — a clean slate for the next person.
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
    flash(`Submitted ✓${r.replaced ? ' (replaced your prior submission)' : ''}`)
    submittedReceipt.value = r.receipt_token
    submittedCode.value = r.access_code
    submittedCsv.value = csv
    showReceipt.value = true
    // Stay signed in after submitting (you can change and re-submit; it upserts).
  } catch (e) {
    flash('Submit failed: ' + (e instanceof Error ? e.message : String(e)), true)
  } finally {
    busy.value = false
  }
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
    const csv = mine[0].raw_csv
    const total = store.totalValue
    const entries = parseTemplateEntries(csv).map(e => ({ id: e.id, value: e.value * total }))
    store.applyTemplateEntries(entries)
    view.value = 'budget'
    flash(`Loaded ${session.name}'s submission ${mine[0].receipt_token} — Logout to clear.`)
  } catch (e) {
    flash('Load failed: ' + (e instanceof Error ? e.message : String(e)), true)
  } finally { busy.value = false }
}
</script>

<template>
  <div class="app">
    <AuthDialog :open="showLogin" @close="showLogin = false" @success="onLoginSuccess" />
    <HelpDialog :open="showHelp" @close="showHelp = false" />
    <ReceiptDialog :open="showReceipt" :receipt="submittedReceipt || ''" :code="submittedCode" :csv="submittedCsv" @close="showReceipt = false" />
    <AppHeader @login="showLogin = true">
      <nav class="tabs">
        <button :class="{ active: view === 'budget' }" @click="view = 'budget'">Budget</button>
        <button :class="{ active: view === 'templates' }" @click="view = 'templates'">Templates</button>
        <button :class="{ active: view === 'results' }" @click="view = 'results'">Results</button>
        <button v-if="session.isAdmin" :class="{ active: view === 'admin' }" @click="view = 'admin'">Admin</button>
      </nav>

      <div v-if="msg" class="flash" :class="{ err: msgErr }" @click="msg = null">{{ msg }}</div>
    </AppHeader>

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
          <button class="abtn" @click="showHelp = true" title="How to use">❓ Help</button>
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
