<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useBudgetStore } from '../stores/budget'
import { fetchSubmission, parseTemplateEntries } from '../api'
import { buildRollup, type RollupNode } from '../rollup'
import RollupRow from './RollupRow.vue'

const props = defineProps<{ token: string }>()
const budget = useBudgetStore()

const rollup = ref<RollupNode[]>([])
const fiscalYear = ref('')
const loading = ref(true)
const needCode = ref(false)
const codeInput = ref('')
const err = ref<string | null>(null)

const nameMap = computed(() => {
  const m = new Map<string, string>()
  for (const n of budget.nodes) m.set(n.id, n.name)
  return m
})
function nameOf(id: string): string { return nameMap.value.get(id) || id }

async function load(code = '') {
  err.value = null
  loading.value = true
  try {
    const csv = await fetchSubmission(props.token, code)
    needCode.value = false
    fiscalYear.value = (csv.match(/^#fiscal_year,(.+)$/m)?.[1] || '').trim()
    rollup.value = buildRollup(parseTemplateEntries(csv).map(e => ({ node_id: e.id, amount: e.value })))
  } catch (e) {
    if ((e as { codeRequired?: boolean })?.codeRequired) needCode.value = true
    else err.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

function submitCode() {
  if (codeInput.value.trim().length < 4) { err.value = 'Enter the access code.'; return }
  load(codeInput.value.trim())
}

onMounted(() => load())
</script>

<template>
  <div class="pub">
    <header class="pub-head">
      <h1 class="pub-title">Tax N Vote</h1>
      <span class="pub-sub">A submitted Tax Dollar<span v-if="fiscalYear"> · FY {{ fiscalYear }}</span></span>
    </header>

    <main class="pub-main">
      <div v-if="loading" class="pub-msg">Loading submission…</div>

      <!-- Access-code gate -->
      <div v-else-if="needCode" class="pub-pin">
        <p class="pub-msg">This submission is private until the data is released. Enter the access code from the receipt to view it.</p>
        <div class="pin-row">
          <input v-model="codeInput" maxlength="12" autocapitalize="characters" spellcheck="false"
                 placeholder="Access code" @keyup.enter="submitCode" />
          <button @click="submitCode">View</button>
        </div>
        <div v-if="err" class="pub-msg err">{{ err }}</div>
      </div>

      <div v-else-if="err" class="pub-msg err">Submission not found.</div>

      <template v-else>
        <p class="pub-receipt">Receipt <span class="mono">{{ token }}</span></p>
        <div class="pub-tree">
          <RollupRow v-for="n in rollup" :key="n.id" :node="n" :name-of="nameOf" />
        </div>
        <p v-if="rollup.length === 0" class="pub-msg">No allocations.</p>
      </template>
    </main>

    <footer class="pub-foot">
      <RouterLink to="/app">← Build your own at Tax N Vote</RouterLink>
    </footer>
  </div>
</template>

<style scoped>
.pub { max-width: 640px; margin: 0 auto; min-height: 100vh; display: flex; flex-direction: column; padding: 0 12px; }
.pub-head { padding: 24px 4px 12px; border-bottom: 1px solid #1e293b; }
.pub-title { font-size: 22px; font-weight: 800; color: #f59e0b; letter-spacing: -0.02em; }
.pub-sub { font-size: 13px; color: #64748b; }
.pub-main { flex: 1; padding: 12px 4px; }
.pub-receipt { font-size: 12px; color: #64748b; margin-bottom: 12px; }
.mono { font-family: ui-monospace, monospace; color: #94a3b8; }
.pub-tree { border-top: 1px solid #1e293b; }
.pub-msg { padding: 16px; text-align: center; color: #64748b; }
.pub-msg.err { color: #f87171; }
.pub-pin { max-width: 280px; margin: 24px auto; text-align: center; }
.pin-row { display: flex; gap: 6px; margin-top: 8px; }
.pin-row input { flex: 1; background: #1e293b; border: 1px solid #334155; color: #e2e8f0; padding: 10px 12px; border-radius: 8px; font-size: 15px; outline: none; }
.pin-row input:focus { border-color: #3b82f6; }
.pin-row button { background: #2563eb; border: 1px solid #2563eb; color: #fff; padding: 8px 16px; border-radius: 8px; font-weight: 600; cursor: pointer; }
.pin-row button:hover { background: #1d4ed8; }
.pub-foot { padding: 16px 4px; border-top: 1px solid #1e293b; text-align: center; }
.pub-foot a { color: #60a5fa; font-size: 13px; text-decoration: none; }
.pub-foot a:hover { text-decoration: underline; }
</style>
