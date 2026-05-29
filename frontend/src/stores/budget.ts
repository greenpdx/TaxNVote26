// src/stores/budget.ts
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { BudgetNode, DisplayMode, IWasmBudgetTree } from '../types/budget'
import init, { WasmBudgetTree } from '../wasm/tnv_budget_tree.js'

// Defaults must match server-side FISCAL_YEAR (.env). VITE_FISCAL_YEAR
// overrides from frontend/.env at build time.
const FISCAL_YEAR  = import.meta.env.VITE_FISCAL_YEAR ?? '2027'
const BEA_FILTER   = import.meta.env.VITE_BEA_FILTER ?? 'Discretionary'
const ON_BUDGET    = (import.meta.env.VITE_ON_BUDGET_ONLY ?? 'true') !== 'false'

export const useBudgetStore = defineStore('budget', () => {
  const nodes = ref<BudgetNode[]>([])
  const tree = ref<IWasmBudgetTree | null>(null)
  const mode = ref<DisplayMode>('simple')
  // How the proportion bar is scaled. 'linear' = true share of total (honest,
  // with a min sliver); 'log' = log-compressed so small categories are more
  // visible (distorts true proportion). Linear is the default.
  const barScale = ref<'linear' | 'log'>('linear')
  const expanded = ref<Set<number>>(new Set())
  const selected = ref<number | null>(null)
  const searchQuery = ref('')
  const ready = ref(false)
  const initError = ref<string | null>(null)

  const rootNode = computed(() => nodes.value[0] ?? null)
  const totalValue = computed(() => rootNode.value?.value ?? 0)

  // Top-level rows are the 9 topics (root's children), kept in fixed tree order
  // (Defense … Other). Search keeps a topic visible if any descendant matches.
  const filteredTopics = computed(() => {
    if (!rootNode.value) return []
    let topics = rootNode.value.children.map(i => nodes.value[i])
    if (searchQuery.value.trim()) {
      const q = searchQuery.value.toLowerCase()
      topics = topics.filter(t => matchesSearch(t, q))
    }
    return topics.sort((a, b) => a.idx - b.idx)
  })

  function matchesSearch(node: BudgetNode, q: string): boolean {
    if (node.name.toLowerCase().includes(q)) return true
    for (const ci of node.children) {
      if (matchesSearch(nodes.value[ci], q)) return true
    }
    return false
  }

  async function initStore() {
    try {
      await init()
      const t = new WasmBudgetTree(0, FISCAL_YEAR, BEA_FILTER, ON_BUDGET) as unknown as IWasmBudgetTree
      tree.value = t
      const raw: BudgetNode[] = JSON.parse(t.all_nodes_json())
      // WasmBudgetTree.all_nodes_json() omits children and defaultValue;
      // reconstruct them from the parent links so the UI can walk the tree.
      for (const n of raw) {
        n.children = []
        n.defaultValue = n.defaultValue ?? n.value
        n.templateValue = n.defaultValue
      }
      for (const n of raw) {
        if (n.parent >= 0 && raw[n.parent]) raw[n.parent].children.push(n.idx)
      }
      nodes.value = raw
      ready.value = true
    } catch (e) {
      initError.value = e instanceof Error ? e.message : String(e)
      console.error('budget tree init failed:', e)
    }
  }

  function applyChangeset(packed: Float64Array) {
    if (packed.length === 1 && isNaN(packed[0])) return // error
    for (let i = 0; i < packed.length; i += 2) {
      const idx = packed[i]
      const val = packed[i + 1]
      if (nodes.value[idx]) {
        nodes.value[idx].value = val
      }
    }
  }

  function adjust(id: string, newValue: number) {
    if (!tree.value) return
    const cs = tree.value.adjust(id, newValue)
    applyChangeset(cs)
  }

  function lock(id: string) {
    if (!tree.value) return
    tree.value.lock(id)
    const idx = nodes.value.findIndex(n => n.id === id)
    if (idx >= 0) nodes.value[idx].locked = true
  }

  function unlock(id: string) {
    if (!tree.value) return
    tree.value.unlock(id)
    const idx = nodes.value.findIndex(n => n.id === id)
    if (idx >= 0) nodes.value[idx].locked = false
  }

  function toggleLock(id: string) {
    const idx = nodes.value.findIndex(n => n.id === id)
    if (idx < 0) return
    nodes.value[idx].locked ? unlock(id) : lock(id)
  }

  function resetNode(id: string) {
    if (!tree.value) return
    const cs = tree.value.reset_node(id)
    applyChangeset(cs)
    const idx = nodes.value.findIndex(n => n.id === id)
    if (idx >= 0) nodes.value[idx].locked = false
  }

  function resetAll() {
    if (!tree.value) return
    const cs = tree.value.reset_to_default()
    applyChangeset(cs)
    for (const n of nodes.value) n.locked = false
    expanded.value.clear()
    selected.value = null
  }

  function toggleExpand(idx: number) {
    if (expanded.value.has(idx)) {
      expanded.value.delete(idx)
    } else {
      expanded.value.add(idx)
    }
  }

  function selectNode(idx: number | null) {
    selected.value = selected.value === idx ? null : idx
  }

  function isExpanded(idx: number): boolean {
    return expanded.value.has(idx)
  }

  function pctOfTotal(value: number): number {
    return totalValue.value > 0 ? (value / totalValue.value) * 100 : 0
  }

  // Upper bound for a node's slider. Normally its parent's value, but for a
  // sole child (which the engine redirects to its parent) walk up to the
  // nearest ancestor that has siblings and use that ancestor's parent value —
  // so the thumb isn't pinned at full and the node can grow.
  function adjustMax(node: BudgetNode): number {
    let n = node
    while (n.parent >= 0) {
      const parent = nodes.value[n.parent]
      if (parent.children.length > 1) return parent.value
      n = parent
    }
    return node.value
  }

  // Width (0–100) of a node's proportion bar, based on its true share of the
  // total budget (so the bar matches the % label: 62.7% → 62.7% of the track).
  // Min sliver keeps tiny categories visible/tappable. 'log' compresses the
  // range so small categories grow (distorts true proportion).
  const MIN_BAR = 3
  // Bar width (0–100) for an arbitrary value, honoring the current scale.
  // 'log' maps share% on a 3-decade axis: 0.1% → 0, 100% → 100.
  function scaleWidth(value: number): number {
    const pct = pctOfTotal(value)
    if (pct <= 0) return 0
    const w = barScale.value === 'log' ? ((Math.log10(pct) + 1) / 3) * 100 : pct
    return Math.min(100, Math.max(0, w))
  }
  function barPct(node: BudgetNode): number {
    if (node.value <= 0) return 0
    return Math.max(MIN_BAR, scaleWidth(node.value))
  }
  // Position (0–100) of a node's baseline (default) value on its bar, so the
  // user can see how far the current value has moved from the original.
  function baselinePct(node: BudgetNode): number {
    return scaleWidth(node.defaultValue)
  }

  // `value` is in THOUSANDS of dollars (as stored in the budget tree).
  function applyTemplateEntries(entries: { id: string; value: number }[]) {
    if (!tree.value) return
    const cs = tree.value.apply_template(JSON.stringify(entries))
    applyChangeset(cs)
  }

  // Leaf (account) nodes with a positive value — the full-detail allocation.
  function leafNodes(): BudgetNode[] {
    return nodes.value.filter(n => n.children.length === 0 && n.value > 0)
  }
  function leafAllocations(): { id: string; pct: number }[] {
    const total = totalValue.value
    if (total <= 0) return []
    return leafNodes().map(n => ({ id: n.id, pct: n.value / total }))
  }
  function leafEntries(): { id: string; value: number }[] {
    return leafNodes().map(n => ({ id: n.id, value: n.value }))
  }

  function formatDollars(value: number): string {
    const abs = Math.abs(value)
    if (abs >= 1e9) return `$${(value / 1e9).toFixed(1)}T`
    if (abs >= 1e6) return `$${(value / 1e6).toFixed(1)}B`
    if (abs >= 1e3) return `$${(value / 1e3).toFixed(1)}M`
    return `$${value.toFixed(0)}K`
  }

  return {
    nodes, tree, mode, barScale, expanded, selected, searchQuery, ready, initError,
    rootNode, totalValue, filteredTopics, fiscalYear: FISCAL_YEAR,
    init: initStore,
    adjust, lock, unlock, toggleLock, resetNode, resetAll,
    toggleExpand, selectNode, isExpanded, pctOfTotal, barPct, baselinePct, adjustMax, formatDollars,
    applyTemplateEntries, leafAllocations, leafEntries,
  }
})
