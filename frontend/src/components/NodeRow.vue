<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { BudgetNode } from '../types/budget'
import { useBudgetStore } from '../stores/budget'

const props = defineProps<{ node: BudgetNode }>()
const store = useBudgetStore()

const hasChildren = computed(() => props.node.children.length > 0)
// Drill-down is only available in full mode; simple mode shows the 9 flat topics.
const canExpand = computed(() => store.mode === 'full' && hasChildren.value)
const isExpanded = computed(() => store.isExpanded(props.node.idx))
const isSelected = computed(() => store.selected === props.node.idx)
const pct = computed(() => store.pctOfTotal(props.node.value))
const dollars = computed(() => store.formatDollars(props.node.value))
const isChanged = computed(() => Math.abs(props.node.value - props.node.defaultValue) > 1)

const childNodes = computed(() =>
  props.node.children.map(i => store.nodes[i]).sort((a, b) => b.value - a.value)
)

const sliderVal = ref(props.node.value)
const sliderMax = computed(() => store.adjustMax(props.node))
// Baseline tick position along the slider, aligned to the thumb's travel
// (thumb is 24px wide, so its centre spans [12px, 100%-12px]).
const baselineSliderLeft = computed(() => {
  const max = sliderMax.value
  const frac = max > 0 ? Math.min(1, Math.max(0, props.node.defaultValue / max)) : 0
  return `calc(${frac} * (100% - 24px) + 12px)`
})

watch(() => props.node.value, (v) => { sliderVal.value = v })

function onSliderInput(e: Event) {
  const val = parseFloat((e.target as HTMLInputElement).value)
  sliderVal.value = val
  store.adjust(props.node.id, val)
}

const barColor = computed(() => {
  if (props.node.locked) return '#475569'
  if (isChanged.value) return '#f59e0b'
  const colors = ['', '#60a5fa', '#818cf8', '#a78bfa']
  return colors[props.node.level] || '#60a5fa'
})
</script>

<template>
  <div
    class="node-row"
    :class="{
      'node-locked': node.locked,
      'node-changed': isChanged,
      'node-selected': isSelected,
      [`node-level-${node.level}`]: true,
    }"
    :style="{ paddingLeft: `${node.level * 14 + 4}px` }"
  >
    <!-- Header row -->
    <div class="node-header" @click="store.selectNode(node.idx)">
      <button
        v-if="canExpand"
        class="expand-btn"
        @click.stop="store.toggleExpand(node.idx)"
      >{{ isExpanded ? '▾' : '▸' }}</button>
      <span v-else class="expand-spacer" />

      <div class="node-info">
        <span class="node-name">{{ node.name }}</span>
        <span class="node-vals">
          <span class="node-dollars">{{ dollars }}</span>
          <span class="node-pct">{{ pct.toFixed(1) }}%</span>
        </span>
      </div>

      <button
        class="lock-btn"
        :class="{ locked: node.locked }"
        :title="node.locked ? 'Locked' : 'Unlocked'"
        @click.stop="store.toggleLock(node.id)"
      >
        <!-- Locked: shackle closed over the body -->
        <svg v-if="node.locked" viewBox="0 0 24 24" width="17" height="17"
             fill="none" stroke="currentColor" stroke-width="2"
             stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <rect x="5" y="11" width="14" height="10" rx="2" />
          <path d="M8 11V7a4 4 0 0 1 8 0v4" />
        </svg>
        <!-- Unlocked: shackle (top) swung up to the left, away from the body -->
        <svg v-else viewBox="0 0 24 24" width="17" height="17"
             fill="none" stroke="currentColor" stroke-width="2"
             stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <rect x="8" y="11" width="12" height="10" rx="2" />
          <path d="M11 11V7a4 4 0 0 0-8 0" />
        </svg>
      </button>
    </div>

    <!-- Percentage bar -->
    <div class="bar-track" @click="store.selectNode(node.idx)">
      <div class="bar-fill" :style="{ width: store.barPct(node) + '%', background: barColor }" />
      <div
        v-if="node.defaultValue > 0"
        class="bar-baseline"
        :style="{ left: store.baselinePct(node) + '%' }"
        :title="`Baseline: ${store.formatDollars(node.defaultValue)}`"
      />
    </div>

    <!-- Slider (on select, not root) -->
    <div v-if="isSelected && node.parent >= 0" class="slider-area">
      <div class="slider-wrap">
        <input
          type="range"
          class="slider"
          :min="0"
          :max="sliderMax"
          :step="100"
          :value="sliderVal"
          :disabled="node.locked"
          @input="onSliderInput"
        />
        <div
          v-if="node.defaultValue > 0"
          class="slider-baseline"
          :style="{ left: baselineSliderLeft }"
          :title="`Baseline: ${store.formatDollars(node.defaultValue)}`"
        />
      </div>
      <div class="slider-actions">
        <button class="act-btn" @click="store.resetNode(node.id)" title="Reset">↺</button>
        <button class="act-btn" @click="store.adjust(node.id, 0)" title="Zero">0</button>
      </div>
    </div>

    <!-- Recursive children -->
    <div v-if="canExpand && isExpanded" class="children">
      <NodeRow v-for="child in childNodes" :key="child.idx" :node="child" />
    </div>
  </div>
</template>

<style scoped>
.node-row { border-bottom: 1px solid #1e293b; }

.node-header {
  display: flex; align-items: center; gap: 6px;
  padding: 10px 8px 4px 0; cursor: pointer; min-height: 40px;
  -webkit-tap-highlight-color: transparent;
}

.expand-btn {
  background: none; border: none; color: #94a3b8; font-size: 16px;
  width: 28px; height: 28px; display: flex; align-items: center; justify-content: center;
  cursor: pointer; flex-shrink: 0; border-radius: 4px;
}
.expand-btn:hover { background: #1e293b; }
.expand-btn:active { background: #334155; }
.expand-spacer { width: 28px; flex-shrink: 0; }

.node-info {
  flex: 1; min-width: 0; display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px 12px;
}

.node-name {
  font-weight: 500; color: #e2e8f0;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-width: 0;
}

.node-level-1 .node-name { font-weight: 700; font-size: 15px; }
.node-level-2 .node-name { font-size: 14px; color: #cbd5e1; }
.node-level-3 .node-name { font-size: 13px; color: #94a3b8; }
.node-level-4 .node-name { font-size: 13px; color: #94a3b8; }

.node-vals { display: flex; gap: 8px; flex-shrink: 0; font-variant-numeric: tabular-nums; }
.node-dollars { color: #60a5fa; font-weight: 600; font-size: 13px; }
.node-pct { color: #64748b; font-size: 12px; min-width: 40px; text-align: right; }

.lock-btn {
  background: none; border: none; cursor: pointer;
  padding: 4px; flex-shrink: 0; border-radius: 4px;
  display: flex; align-items: center; justify-content: center;
  color: #64748b;
}
.lock-btn svg { display: block; }
.lock-btn.locked { color: #f59e0b; }
.lock-btn:hover { background: #1e293b; }

.bar-track {
  position: relative;
  height: 4px; background: #1e293b; border-radius: 2px;
  margin: 0 8px 6px 36px; cursor: pointer;
}
.bar-fill { height: 100%; border-radius: 2px; transition: width 0.12s ease-out, background 0.12s; }
/* Baseline (default value) marker — vertical tick the fill grows past/short of. */
.bar-baseline {
  position: absolute; top: -3px; height: 10px; width: 2px;
  margin-left: -1px; background: #e2e8f0; border-radius: 1px;
  pointer-events: none;
}

.slider-area { padding: 6px 8px 10px 36px; display: flex; align-items: center; gap: 8px; }
.slider-wrap { position: relative; flex: 1; display: flex; align-items: center; }
/* Baseline (default value) marker on the slider track. */
.slider-baseline {
  position: absolute; top: 50%; transform: translateY(-50%);
  width: 2px; height: 16px; margin-left: -1px;
  background: #e2e8f0; border-radius: 1px; pointer-events: none;
}

.slider {
  flex: 1; height: 6px; -webkit-appearance: none; appearance: none;
  background: #334155; border-radius: 3px; outline: none;
}
.slider::-webkit-slider-thumb {
  -webkit-appearance: none; appearance: none; width: 24px; height: 24px;
  border-radius: 50%; background: #3b82f6; cursor: grab; border: 2px solid #0f172a;
}
.slider::-moz-range-thumb {
  width: 24px; height: 24px; border-radius: 50%; background: #3b82f6;
  cursor: grab; border: 2px solid #0f172a;
}
.slider:disabled { opacity: 0.35; cursor: not-allowed; }

.slider-actions { display: flex; gap: 4px; flex-shrink: 0; }
.act-btn {
  background: #1e293b; border: 1px solid #334155; color: #94a3b8;
  width: 34px; height: 34px; border-radius: 6px; font-size: 14px;
  cursor: pointer; display: flex; align-items: center; justify-content: center;
}
.act-btn:hover { background: #334155; color: #e2e8f0; }
.act-btn:active { background: #475569; }

.node-locked .node-name { color: #64748b; }
.node-locked .node-dollars { color: #475569; }
.node-changed .node-name { color: #fbbf24; }
.node-selected > .node-header { background: #0f172a; border-radius: 6px; }

@media (max-width: 480px) {
  .node-header { padding: 8px 4px 3px 0; min-height: 36px; }
  .node-name { font-size: 13px !important; }
  .node-dollars { font-size: 12px; }
  .node-pct { font-size: 11px; }
  .bar-track { margin-left: 28px; }
  .slider-area { padding-left: 28px; }
  .slider::-webkit-slider-thumb { width: 28px; height: 28px; }
  .slider::-moz-range-thumb { width: 28px; height: 28px; }
  .act-btn { width: 38px; height: 38px; font-size: 16px; }
}
</style>
