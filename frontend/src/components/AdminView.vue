<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { useSessionStore } from '../stores/session'
import { useBudgetStore } from '../stores/budget'
import { buildRollup, type RollupNode } from '../rollup'
import RollupRow from './RollupRow.vue'
import * as api from '../api'

// Rendered only when the session is an admin (gated in App.vue) → single token.
const session = useSessionStore()
const budget = useBudgetStore()

type Tab = 'users' | 'submissions' | 'templates' | 'audit' | 'config'
const tab = ref<Tab>('submissions')
const note = ref<string | null>(null)
const noteErr = ref(false)

const users = ref<api.AdminUser[]>([])
const userQuery = ref('')
const submissions = ref<api.AdminTaxDollar[]>([])
const templates = ref<api.AdminTemplate[]>([])
const audit = ref<api.AuditEntry[]>([])
const config = ref<api.SettingItem[]>([])

// Expanded record rollup (one at a time).
const openKey = ref<string | null>(null)
const rollup = ref<RollupNode[]>([])
const rollupBusy = ref(false)

const tok = () => session.token!
function flash(t: string, err = false) { note.value = t; noteErr.value = err }
function fail(e: unknown) { flash(e instanceof Error ? e.message : String(e), true) }

// id → human name from the loaded budget tree (falls back to the raw id).
const nameMap = computed(() => {
  const m = new Map<string, string>()
  for (const n of budget.nodes) m.set(n.id, n.name)
  return m
})
function nameOf(id: string): string { return nameMap.value.get(id) || id }

async function load() {
  if (!session.isAdmin) return
  collapse()
  try {
    if (tab.value === 'users') users.value = await api.adminListUsers(tok(), userQuery.value.trim())
    else if (tab.value === 'submissions') submissions.value = await api.adminListTaxdollars(tok())
    else if (tab.value === 'templates') templates.value = await api.adminListTemplates(tok())
    else if (tab.value === 'audit') audit.value = await api.adminListAudit(tok())
    else if (tab.value === 'config') config.value = await api.adminGetConfig(tok())
  } catch (e) { fail(e) }
}
watch(tab, load)
onMounted(load)

function collapse() { openKey.value = null; rollup.value = [] }
async function openRollup(key: string, fetcher: () => Promise<api.NodeAmount[]>) {
  if (openKey.value === key) { collapse(); return }
  openKey.value = key; rollup.value = []; rollupBusy.value = true
  try { rollup.value = buildRollup(await fetcher()) } catch (e) { fail(e) } finally { rollupBusy.value = false }
}

async function toggleDisabled(u: api.AdminUser) {
  try { await api.adminSetUserDisabled(tok(), u.kind, u.id, !u.disabled); flash(`${u.name} ${u.disabled ? 'enabled' : 'disabled'}`); await load() } catch (e) { fail(e) }
}
async function toggleAdmin(u: api.AdminUser) {
  try { await api.adminSetRole(tok(), u.kind, u.id, u.tier >= 100 ? 0 : 100); flash(`Role updated for ${u.name}`); await load() } catch (e) { fail(e) }
}
async function toggleSubHidden(s: api.AdminTaxDollar) {
  try { await api.adminSetTaxdollarHidden(tok(), s.receipt_token, !s.hidden); flash(`${s.receipt_token} ${s.hidden ? 'shown' : 'hidden'}`); await load() } catch (e) { fail(e) }
}
async function toggleTplHidden(t: api.AdminTemplate) {
  try { await api.adminSetTemplateHidden(tok(), t.receipt_no, !t.hidden); flash(`${t.receipt_no} ${t.hidden ? 'shown' : 'hidden'}`); await load() } catch (e) { fail(e) }
}
async function saveSetting(s: api.SettingItem, value: string) {
  try { await api.adminSetConfig(tok(), s.key, value); flash(`${s.key} = ${value || '(unset)'}`); await load() } catch (e) { fail(e) }
}
// Long-form landing copy (lp_*) renders as a textarea; other free-text keys as a
// single-line input; everything else as a boolean toggle.
const STRING_KEYS = new Set(['subtitle_1', 'subtitle_2'])
const isLpKey = (key: string) => key.startsWith('lp_')
const isStringKey = (key: string) => STRING_KEYS.has(key) || isLpKey(key)
</script>

<template>
  <div class="admin" v-if="session.isAdmin">
    <div class="admin-bar"><span class="muted">Signed in as admin: <b>{{ session.name }}</b></span></div>

    <nav class="subtabs">
      <button :class="{ active: tab === 'users' }" @click="tab = 'users'">Users</button>
      <button :class="{ active: tab === 'submissions' }" @click="tab = 'submissions'">Submissions</button>
      <button :class="{ active: tab === 'templates' }" @click="tab = 'templates'">Templates</button>
      <button :class="{ active: tab === 'audit' }" @click="tab = 'audit'">Audit</button>
      <button :class="{ active: tab === 'config' }" @click="tab = 'config'">Config</button>
      <button class="abtn" @click="load">↻</button>
    </nav>

    <div v-if="note" class="flash" :class="{ err: noteErr }" @click="note = null">{{ note }}</div>

    <!-- Users -->
    <div v-if="tab === 'users'">
      <div class="search-wrap"><input v-model="userQuery" class="search" placeholder="Search name / username…" @keyup.enter="load" /></div>
      <table class="tbl">
        <thead><tr><th>Kind</th><th>Name</th><th>Tier</th><th>Status</th><th></th></tr></thead>
        <tbody>
          <tr v-for="u in users" :key="u.kind + u.id" :class="{ off: u.disabled }">
            <td>{{ u.kind }}</td><td>{{ u.name }}</td><td>{{ u.tier }}</td><td>{{ u.disabled ? 'disabled' : 'active' }}</td>
            <td class="row-actions">
              <button class="abtn" @click="toggleDisabled(u)">{{ u.disabled ? 'Enable' : 'Disable' }}</button>
              <button v-if="u.kind === 'account'" class="abtn" @click="toggleAdmin(u)">{{ u.tier >= 100 ? 'Revoke admin' : 'Make admin' }}</button>
            </td>
          </tr>
          <tr v-if="users.length === 0"><td colspan="5" class="muted">No users.</td></tr>
        </tbody>
      </table>
    </div>

    <!-- Submissions -->
    <div v-else-if="tab === 'submissions'">
      <table class="tbl">
        <thead><tr><th>Receipt</th><th>By</th><th>FY</th><th>State</th><th></th></tr></thead>
        <tbody>
          <template v-for="s in submissions" :key="s.receipt_token">
            <tr :class="{ off: s.hidden }">
              <td class="mono">{{ s.receipt_token }}</td>
              <td>{{ s.subject_kind }}#{{ s.subject_id }}</td>
              <td>{{ s.fiscal_year }}</td>
              <td>{{ s.hidden ? 'hidden' : 'visible' }}</td>
              <td class="row-actions">
                <button class="abtn" @click="openRollup(s.receipt_token, () => api.adminTaxdollarAllocations(tok(), s.receipt_token))">
                  {{ openKey === s.receipt_token ? 'Hide data' : 'View data' }}
                </button>
                <button class="abtn" @click="toggleSubHidden(s)">{{ s.hidden ? 'Unhide' : 'Hide' }}</button>
              </td>
            </tr>
            <tr v-if="openKey === s.receipt_token">
              <td colspan="5" class="rollup-cell">
                <div v-if="rollupBusy" class="muted">Loading…</div>
                <RollupRow v-else v-for="n in rollup" :key="n.id" :node="n" :name-of="nameOf" />
                <div v-if="!rollupBusy && rollup.length === 0" class="muted">No allocations.</div>
              </td>
            </tr>
          </template>
          <tr v-if="submissions.length === 0"><td colspan="5" class="muted">No submissions.</td></tr>
        </tbody>
      </table>
    </div>

    <!-- Templates -->
    <div v-else-if="tab === 'templates'">
      <table class="tbl">
        <thead><tr><th>Receipt</th><th>Name</th><th>FY</th><th>State</th><th></th></tr></thead>
        <tbody>
          <template v-for="t in templates" :key="t.receipt_no">
            <tr :class="{ off: t.hidden }">
              <td class="mono">{{ t.receipt_no }}</td>
              <td>{{ t.name }}</td>
              <td>{{ t.fiscal_year }}</td>
              <td>{{ t.hidden ? 'hidden' : 'visible' }}</td>
              <td class="row-actions">
                <button class="abtn" @click="openRollup(t.receipt_no, () => api.adminTemplateEntries(tok(), t.receipt_no))">
                  {{ openKey === t.receipt_no ? 'Hide data' : 'View data' }}
                </button>
                <button class="abtn" @click="toggleTplHidden(t)">{{ t.hidden ? 'Unhide' : 'Hide' }}</button>
              </td>
            </tr>
            <tr v-if="openKey === t.receipt_no">
              <td colspan="5" class="rollup-cell">
                <div v-if="rollupBusy" class="muted">Loading…</div>
                <RollupRow v-else v-for="n in rollup" :key="n.id" :node="n" :name-of="nameOf" :fmt="(a: number) => budget.formatDollars(a * budget.totalValue)" />
                <div v-if="!rollupBusy && rollup.length === 0" class="muted">No entries.</div>
              </td>
            </tr>
          </template>
          <tr v-if="templates.length === 0"><td colspan="5" class="muted">No templates.</td></tr>
        </tbody>
      </table>
    </div>

    <!-- Audit -->
    <div v-else-if="tab === 'audit'">
      <table class="tbl">
        <thead><tr><th>When</th><th>Actor</th><th>Action</th><th>Target</th><th>IP</th></tr></thead>
        <tbody>
          <tr v-for="a in audit" :key="a.id">
            <td class="mono">{{ a.ts }}</td>
            <td>{{ a.actor_kind }}<span v-if="a.actor_id">#{{ a.actor_id }}</span></td>
            <td>{{ a.action }}</td>
            <td>{{ a.target_kind }}<span v-if="a.target_id">:{{ a.target_id }}</span></td>
            <td class="mono">{{ a.ip }}</td>
          </tr>
          <tr v-if="audit.length === 0"><td colspan="5" class="muted">No audit entries.</td></tr>
        </tbody>
      </table>
    </div>

    <!-- Config -->
    <div v-else>
      <table class="tbl">
        <thead><tr><th>Setting</th><th>Value</th><th></th></tr></thead>
        <tbody>
          <tr v-for="s in config" :key="s.key">
            <td>{{ s.key }}</td>
            <td>
              <textarea v-if="isLpKey(s.key)" class="cfg-text cfg-area" maxlength="4000" rows="4"
                        :value="s.value" placeholder="(default)"
                        @change="saveSetting(s, ($event.target as HTMLTextAreaElement).value)"></textarea>
              <input v-else-if="isStringKey(s.key)" class="cfg-text" type="text" maxlength="256"
                     :value="s.value" placeholder="(unset)"
                     @change="saveSetting(s, ($event.target as HTMLInputElement).value)" />
              <select v-else :value="s.value || 'false'" @change="saveSetting(s, ($event.target as HTMLSelectElement).value)">
                <option value="true">true</option><option value="false">false</option>
              </select>
            </td>
            <td class="mono muted">{{ s.updated_at }}</td>
          </tr>
          <tr v-if="config.length === 0"><td colspan="3" class="muted">No settings.</td></tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<style scoped>
.admin { padding: 8px 16px 24px; }
.muted { color: #64748b; font-size: 12px; padding: 6px 4px; }
.admin-bar { display: flex; justify-content: space-between; align-items: center; padding: 4px 0 8px; }
.subtabs { display: flex; gap: 4px; margin-bottom: 8px; flex-wrap: wrap; }
.subtabs button { background: #1e293b; border: 1px solid #334155; color: #94a3b8; padding: 5px 10px; border-radius: 6px; font-size: 13px; cursor: pointer; }
.subtabs button.active { background: #3b82f6; border-color: #3b82f6; color: #fff; font-weight: 600; }
.cfg-text { width: 22em; max-width: 100%; background: #1e293b; border: 1px solid #334155; color: #e2e8f0; padding: 5px 8px; border-radius: 6px; font-size: 13px; }
.cfg-text:focus { outline: none; border-color: #3b82f6; }
.cfg-area { width: 32em; resize: vertical; font-family: inherit; line-height: 1.5; }
.tbl { width: 100%; border-collapse: collapse; font-size: 13px; }
.tbl th { text-align: left; color: #64748b; font-weight: 600; padding: 6px 8px; border-bottom: 1px solid #1e293b; }
.tbl td { padding: 6px 8px; border-bottom: 1px solid #1e293b; vertical-align: middle; }
.tbl tr.off { opacity: 0.5; }
.row-actions { display: flex; gap: 4px; }
.mono { font-family: ui-monospace, monospace; font-size: 11px; color: #94a3b8; }
.rollup-cell { background: #0b1424; padding: 6px 8px 8px; }
.tbl select { background: #1e293b; border: 1px solid #334155; color: #e2e8f0; padding: 4px 6px; border-radius: 6px; }
</style>
