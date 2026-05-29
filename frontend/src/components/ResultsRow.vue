<script setup lang="ts">
import { ref, computed } from 'vue'
import type { BudgetNode } from '../types/budget'
import type { NodeStat } from '../api'
import { useBudgetStore } from '../stores/budget'

const props = defineProps<{ node: BudgetNode; stats: Record<string, NodeStat> }>()
const store = useBudgetStore()
const open = ref(false)

const st = computed<NodeStat | undefined>(() => props.stats[props.node.id])
const hasChildren = computed(() => props.node.children.length > 0)
const children = computed(() =>
  props.node.children
    .map(i => store.nodes[i])
    .sort((a, b) => (props.stats[b.id]?.trimmed_mean ?? 0) - (props.stats[a.id]?.trimmed_mean ?? 0)),
)
const pct = (x?: number) => (x ?? 0) * 100
</script>

<template>
  <div class="rrow" :style="{ paddingLeft: node.level * 12 + 'px' }">
    <div class="rhead" @click="hasChildren && (open = !open)">
      <span class="rcaret">{{ hasChildren ? (open ? '▾' : '▸') : '·' }}</span>
      <span class="rname">{{ node.name }}</span>
      <span class="rmean">{{ pct(st?.trimmed_mean).toFixed(2) }}%</span>
    </div>
    <div class="rbar"><div class="rfill" :style="{ width: Math.min(100, pct(st?.trimmed_mean)) + '%' }" /></div>
    <div class="rstats">
      mean {{ pct(st?.mean).toFixed(2) }}% · median {{ pct(st?.median).toFixed(2) }}% ·
      sd {{ pct(st?.std_dev).toFixed(2) }} · range {{ pct(st?.min).toFixed(1) }}–{{ pct(st?.max).toFixed(1) }}%
    </div>
    <div v-if="open && hasChildren" class="rkids">
      <ResultsRow v-for="c in children" :key="c.idx" :node="c" :stats="stats" />
    </div>
  </div>
</template>

<style scoped>
.rrow { border-bottom: 1px solid #1e293b; }
.rhead { display: flex; align-items: baseline; gap: 8px; padding: 8px 4px 2px; cursor: pointer; }
.rcaret { width: 14px; color: #64748b; flex-shrink: 0; }
.rname { flex: 1; color: #e2e8f0; font-size: 14px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.rmean { color: #60a5fa; font-weight: 700; font-variant-numeric: tabular-nums; }
.rbar { height: 4px; background: #1e293b; border-radius: 2px; margin: 0 4px 4px 22px; overflow: hidden; }
.rfill { height: 100%; background: #818cf8; border-radius: 2px; }
.rstats { font-size: 11px; color: #64748b; padding: 0 4px 6px 22px; font-variant-numeric: tabular-nums; }
.rkids { margin-left: 0; }
</style>
