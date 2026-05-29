<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { listTemplates, getTemplateCsv, parseTemplateEntries, type TemplateSummary } from '../api'
import { useBudgetStore } from '../stores/budget'

const store = useBudgetStore()
const emit = defineEmits<{ (e: 'loaded'): void }>()

const items = ref<TemplateSummary[]>([])
const loading = ref(false)
const err = ref<string | null>(null)
const msg = ref<string | null>(null)

async function load() {
  loading.value = true
  err.value = null
  try {
    items.value = await listTemplates()
  } catch (e) {
    err.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

async function loadTemplate(t: TemplateSummary) {
  err.value = null
  msg.value = null
  try {
    const csv = await getTemplateCsv(t.receipt_no)
    store.applyTemplateEntries(parseTemplateEntries(csv))
    msg.value = `Loaded "${t.name}" into the budget.`
    emit('loaded')
  } catch (e) {
    err.value = e instanceof Error ? e.message : String(e)
  }
}
onMounted(load)
</script>

<template>
  <div class="tpl">
    <div class="ttop">
      <h2 class="ttitle">Template Registry</h2>
      <button class="tbtn" :disabled="loading" @click="load">↻ Refresh</button>
    </div>
    <div v-if="msg" class="tmsg">{{ msg }}</div>
    <div v-if="err" class="terr">{{ err }}</div>
    <div v-if="!loading && items.length === 0" class="tinfo">
      No templates yet. On the Budget tab, set an allocation and "Save as template".
    </div>
    <ul class="tlist">
      <li v-for="t in items" :key="t.receipt_no" class="titem">
        <div class="tinfo-row">
          <span class="tname">{{ t.name }}</span>
          <span v-if="t.entity_name" class="tentity">{{ t.entity_name }}</span>
          <span class="tfy">FY{{ t.fiscal_year }}</span>
        </div>
        <div v-if="t.description" class="tdesc">{{ t.description }}</div>
        <div class="tmeta">
          <span>{{ t.entry_count }} entries · {{ t.receipt_no }}</span>
          <button class="tload" @click="loadTemplate(t)">Load →</button>
        </div>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.tpl { padding: 12px 8px 24px; }
.ttop { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
.ttitle { font-size: 18px; color: #f59e0b; }
.tbtn, .tload { background: #1e293b; border: 1px solid #334155; color: #94a3b8; border-radius: 6px; cursor: pointer; padding: 4px 10px; font-size: 13px; }
.tbtn:hover, .tload:hover { background: #334155; color: #e2e8f0; }
.tmsg { color: #34d399; padding: 6px 4px; font-size: 13px; }
.terr { color: #f87171; padding: 6px 4px; font-size: 13px; }
.tinfo { color: #64748b; padding: 24px 8px; text-align: center; }
.tlist { list-style: none; display: flex; flex-direction: column; gap: 8px; }
.titem { border: 1px solid #1e293b; border-radius: 8px; padding: 10px; background: #0f172a; }
.tinfo-row { display: flex; align-items: baseline; gap: 8px; flex-wrap: wrap; }
.tname { font-weight: 700; color: #e2e8f0; }
.tentity { color: #818cf8; font-size: 12px; }
.tfy { color: #64748b; font-size: 12px; margin-left: auto; }
.tdesc { color: #94a3b8; font-size: 13px; margin: 4px 0; }
.tmeta { display: flex; align-items: center; justify-content: space-between; font-size: 11px; color: #64748b; margin-top: 6px; }
</style>
