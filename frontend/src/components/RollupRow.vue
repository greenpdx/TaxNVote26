<script setup lang="ts">
import { ref } from 'vue'
import type { RollupNode } from '../rollup'

const props = defineProps<{
  node: RollupNode
  nameOf: (id: string) => string
  depth?: number
  fmt?: (n: number) => string   // when set, show this formatted amount (e.g. dollars)
}>()
const open = ref(false)
const depth = props.depth ?? 0
function toggle() { if (props.node.children.length) open.value = !open.value }
</script>

<template>
  <div class="rr" :class="{ clickable: node.children.length }" :style="{ paddingLeft: depth * 16 + 4 + 'px' }" @click="toggle">
    <span class="rr-tw">{{ node.children.length ? (open ? '▾' : '▸') : '·' }}</span>
    <span class="rr-name">{{ nameOf(node.id) }}</span>
    <span class="rr-bar"><span class="rr-fill" :style="{ width: Math.min(100, node.pct * 100) + '%' }"></span></span>
    <span v-if="fmt" class="rr-amt">{{ fmt(node.amount) }}</span>
    <span class="rr-pct">{{ (node.pct * 100).toFixed(1) }}%</span>
  </div>
  <template v-if="open">
    <RollupRow v-for="c in node.children" :key="c.id" :node="c" :name-of="nameOf" :fmt="fmt" :depth="depth + 1" />
  </template>
</template>

<style scoped>
.rr {
  display: flex; align-items: center; gap: 8px;
  padding: 3px 4px; font-size: 13px; border-bottom: 1px solid #111c30;
}
.rr.clickable { cursor: pointer; }
.rr.clickable:hover { background: #16223a; }
.rr-tw { width: 12px; color: #64748b; flex-shrink: 0; text-align: center; }
.rr-name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #cbd5e1; }
.rr-bar { width: 90px; height: 6px; background: #1e293b; border-radius: 3px; overflow: hidden; flex-shrink: 0; }
.rr-fill { display: block; height: 100%; background: #3b82f6; }
.rr-amt { width: 72px; text-align: right; color: #94a3b8; font-variant-numeric: tabular-nums; flex-shrink: 0; font-size: 12px; }
.rr-pct { width: 48px; text-align: right; color: #93c5fd; font-variant-numeric: tabular-nums; flex-shrink: 0; }
</style>
