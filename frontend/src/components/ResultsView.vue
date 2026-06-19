<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useBudgetStore } from '../stores/budget'
import { getAggregate, type NodeStat } from '../api'
import ResultsRow from './ResultsRow.vue'

const store = useBudgetStore()
const stats = ref<Record<string, NodeStat>>({})
const count = ref(0)
const loading = ref(false)
const err = ref<string | null>(null)

const topics = computed(() => (store.rootNode ? store.rootNode.children.map(i => store.nodes[i]) : []))

async function load() {
  loading.value = true
  err.value = null
  try {
    const res = await getAggregate(store.fiscalYear)
    count.value = res.submission_count
    const m: Record<string, NodeStat> = {}
    for (const ns of res.nodes) m[ns.node_id] = ns
    stats.value = m
  } catch (e) {
    err.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}
onMounted(load)
</script>

<template>
  <div class="results">
    <div class="rtop">
      <div>
        <h2 class="rtitle">People's Budget</h2>
        <span class="rsub">FY{{ store.fiscalYear }} · trimmed mean (extremes excluded)</span>
      </div>
      <div class="rmeta">
        {{ count }} submission{{ count === 1 ? '' : 's' }}
        <button class="rbtn" :disabled="loading" @click="load">↻</button>
      </div>
    </div>

    <p class="rlead">
      The aggregate of everyone's allocations — priorities set directly by taxpayers,
      with no party line and no lobbying. Hard data, not a poll.
    </p>

    <div v-if="err" class="rerr">{{ err }}</div>
    <div v-else-if="loading && count === 0" class="rinfo">Loading…</div>
    <div v-else-if="count === 0" class="rinfo">
      No submissions yet — set your budget on the Budget tab and submit to seed the People's Budget.
    </div>
    <div v-else class="rtree">
      <ResultsRow v-for="t in topics" :key="t.idx" :node="t" :stats="stats" />
    </div>
  </div>
</template>

<style scoped>
.results { padding: 12px 8px 24px; }
.rtop { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; margin-bottom: 10px; }
.rtitle { font-size: 18px; color: #f59e0b; }
.rsub { font-size: 12px; color: #64748b; }
.rlead { font-size: 13px; line-height: 1.55; color: #94a3b8; margin: 0 0 14px; max-width: 60ch; }
.rmeta { color: #94a3b8; font-size: 13px; white-space: nowrap; }
.rbtn { background: #1e293b; border: 1px solid #334155; color: #94a3b8; border-radius: 6px; cursor: pointer; padding: 2px 8px; }
.rbtn:hover { background: #334155; color: #e2e8f0; }
.rerr { color: #f87171; padding: 16px; }
.rinfo { color: #64748b; padding: 24px 8px; text-align: center; }
</style>
