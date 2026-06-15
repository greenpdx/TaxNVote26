<script setup lang="ts">
import { ref, watch, onMounted } from 'vue'
import { useSessionStore } from '../stores/session'
import * as api from '../api'

// This view is only rendered when the session is an admin (gated in App.vue),
// so it uses the single session token — no separate admin login.
const session = useSessionStore()

type Tab = 'users' | 'templates' | 'audit' | 'config'
const tab = ref<Tab>('users')
const note = ref<string | null>(null)
const noteErr = ref(false)

const users = ref<api.AdminUser[]>([])
const userQuery = ref('')
const templates = ref<api.AdminTemplate[]>([])
const audit = ref<api.AuditEntry[]>([])
const config = ref<api.SettingItem[]>([])

const tok = () => session.token!
function flash(t: string, err = false) { note.value = t; noteErr.value = err }
function fail(e: unknown) { flash(e instanceof Error ? e.message : String(e), true) }

async function load() {
  if (!session.isAdmin) return
  try {
    if (tab.value === 'users') users.value = await api.adminListUsers(tok(), userQuery.value.trim())
    else if (tab.value === 'templates') templates.value = await api.adminListTemplates(tok())
    else if (tab.value === 'audit') audit.value = await api.adminListAudit(tok())
    else if (tab.value === 'config') config.value = await api.adminGetConfig(tok())
  } catch (e) { fail(e) }
}
watch(tab, load)
onMounted(load)

async function toggleDisabled(u: api.AdminUser) {
  try { await api.adminSetUserDisabled(tok(), u.kind, u.id, !u.disabled); flash(`${u.name} ${u.disabled ? 'enabled' : 'disabled'}`); await load() } catch (e) { fail(e) }
}
async function toggleAdmin(u: api.AdminUser) {
  try { await api.adminSetRole(tok(), u.kind, u.id, u.tier >= 100 ? 0 : 100); flash(`Role updated for ${u.name}`); await load() } catch (e) { fail(e) }
}
async function toggleHidden(t: api.AdminTemplate) {
  try { await api.adminSetTemplateHidden(tok(), t.receipt_no, !t.hidden); flash(`${t.receipt_no} ${t.hidden ? 'shown' : 'hidden'}`); await load() } catch (e) { fail(e) }
}
async function saveSetting(s: api.SettingItem, value: string) {
  try { await api.adminSetConfig(tok(), s.key, value); flash(`${s.key} = ${value || '(unset)'}`); await load() } catch (e) { fail(e) }
}
</script>

<template>
  <div class="admin" v-if="session.isAdmin">
    <div class="admin-bar">
      <span class="muted">Signed in as admin: <b>{{ session.name }}</b></span>
    </div>

    <nav class="subtabs">
      <button :class="{ active: tab === 'users' }" @click="tab = 'users'">Users</button>
      <button :class="{ active: tab === 'templates' }" @click="tab = 'templates'">Templates</button>
      <button :class="{ active: tab === 'audit' }" @click="tab = 'audit'">Audit</button>
      <button :class="{ active: tab === 'config' }" @click="tab = 'config'">Config</button>
      <button class="abtn" @click="load">↻</button>
    </nav>

    <div v-if="note" class="flash" :class="{ err: noteErr }" @click="note = null">{{ note }}</div>

    <!-- Users -->
    <div v-if="tab === 'users'">
      <div class="search-wrap">
        <input v-model="userQuery" class="search" placeholder="Search name / username…" @keyup.enter="load" />
      </div>
      <table class="tbl">
        <thead><tr><th>Kind</th><th>Name</th><th>Tier</th><th>Status</th><th></th></tr></thead>
        <tbody>
          <tr v-for="u in users" :key="u.kind + u.id" :class="{ off: u.disabled }">
            <td>{{ u.kind }}</td>
            <td>{{ u.name }}</td>
            <td>{{ u.tier }}</td>
            <td>{{ u.disabled ? 'disabled' : 'active' }}</td>
            <td class="row-actions">
              <button class="abtn" @click="toggleDisabled(u)">{{ u.disabled ? 'Enable' : 'Disable' }}</button>
              <button v-if="u.kind === 'account'" class="abtn" @click="toggleAdmin(u)">
                {{ u.tier >= 100 ? 'Revoke admin' : 'Make admin' }}
              </button>
            </td>
          </tr>
          <tr v-if="users.length === 0"><td colspan="5" class="muted">No users.</td></tr>
        </tbody>
      </table>
    </div>

    <!-- Templates -->
    <div v-else-if="tab === 'templates'">
      <table class="tbl">
        <thead><tr><th>Receipt</th><th>Name</th><th>FY</th><th>Owner</th><th>State</th><th></th></tr></thead>
        <tbody>
          <tr v-for="t in templates" :key="t.receipt_no" :class="{ off: t.hidden }">
            <td>{{ t.receipt_no }}</td>
            <td>{{ t.name }}</td>
            <td>{{ t.fiscal_year }}</td>
            <td>{{ t.subject_kind }}#{{ t.subject_id }}</td>
            <td>{{ t.hidden ? 'hidden' : 'visible' }}</td>
            <td class="row-actions">
              <button class="abtn" @click="toggleHidden(t)">{{ t.hidden ? 'Unhide' : 'Hide' }}</button>
            </td>
          </tr>
          <tr v-if="templates.length === 0"><td colspan="6" class="muted">No templates.</td></tr>
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
              <select :value="s.value || 'false'" @change="saveSetting(s, ($event.target as HTMLSelectElement).value)">
                <option value="true">true</option>
                <option value="false">false</option>
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
.muted { color: #64748b; font-size: 12px; }
.admin-bar { display: flex; justify-content: space-between; align-items: center; padding: 4px 0 8px; }
.subtabs { display: flex; gap: 4px; margin-bottom: 8px; }
.subtabs button { background: #1e293b; border: 1px solid #334155; color: #94a3b8; padding: 5px 10px; border-radius: 6px; font-size: 13px; cursor: pointer; }
.subtabs button.active { background: #3b82f6; border-color: #3b82f6; color: #fff; font-weight: 600; }
.tbl { width: 100%; border-collapse: collapse; font-size: 13px; }
.tbl th { text-align: left; color: #64748b; font-weight: 600; padding: 6px 8px; border-bottom: 1px solid #1e293b; }
.tbl td { padding: 6px 8px; border-bottom: 1px solid #1e293b; vertical-align: middle; }
.tbl tr.off { opacity: 0.5; }
.row-actions { display: flex; gap: 4px; }
.mono { font-family: ui-monospace, monospace; font-size: 11px; color: #94a3b8; }
.tbl select { background: #1e293b; border: 1px solid #334155; color: #e2e8f0; padding: 4px 6px; border-radius: 6px; }
</style>
