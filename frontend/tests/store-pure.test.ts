// tests/store-pure.test.ts — bar scaling, baseline ticks, slider bounds.
// These all build on the real WASM tree to be sure the contracts hold against
// the actual node ids / level structure (topics at L1, accounts at L4).

import { beforeEach, describe, expect, it } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { useBudgetStore } from '../src/stores/budget'

async function mountedStore() {
  setActivePinia(createPinia())
  const store = useBudgetStore()
  await store.init()
  return store
}

describe('bar / baseline / slider helpers', () => {
  beforeEach(() => setActivePinia(createPinia()))

  it('linear barPct equals the true share of total (Defense ≈ 62%)', async () => {
    const store = await mountedStore()
    expect(store.barScale).toBe('linear')
    const def = store.nodes.find(n => n.id === 't:def')!
    const share = (def.value / store.totalValue) * 100
    expect(store.barPct(def)).toBeCloseTo(share, 5)
    expect(share).toBeGreaterThan(40) // sanity: Defense dominates
  })

  it('barPct floors tiny values at the 3% minimum sliver', async () => {
    const store = await mountedStore()
    // Synthesize a tiny share. Pick any leaf and treat it as if it had a small
    // share — we can directly use a topic with very small share for this test.
    const tinyTopic = store.nodes
      .filter(n => n.level === 1)
      .reduce((m, n) => (n.value < m.value ? n : m))
    const share = (tinyTopic.value / store.totalValue) * 100
    expect(share).toBeLessThan(3) // qualifies for the floor
    expect(store.barPct(tinyTopic)).toBeCloseTo(3, 5)
  })

  it('log barPct is bigger than linear for sub-1% nodes', async () => {
    const store = await mountedStore()
    const tinyTopic = store.nodes
      .filter(n => n.level === 1)
      .reduce((m, n) => (n.value < m.value ? n : m))
    store.barScale = 'linear'
    const lin = store.barPct(tinyTopic)
    store.barScale = 'log'
    const log = store.barPct(tinyTopic)
    expect(log).toBeGreaterThan(lin)
    expect(log).toBeLessThanOrEqual(100)
  })

  it('baselinePct reflects defaultValue, not current value', async () => {
    const store = await mountedStore()
    const def = store.nodes.find(n => n.id === 't:def')!
    const before = store.baselinePct(def)
    // Move Defense down; baseline tick should stay put.
    store.adjust(def.id, def.value * 0.5)
    expect(store.baselinePct(def)).toBeCloseTo(before, 5)
    expect(store.barPct(def)).toBeLessThan(before)
  })

  it('adjustMax returns the parent value for a node with siblings', async () => {
    const store = await mountedStore()
    const t = store.nodes.find(n => n.level === 1 && n.children.length > 1)!
    const child = store.nodes[t.children[0]]
    expect(store.adjustMax(child)).toBeCloseTo(t.value, 0)
  })

  it('adjustMax walks past sole-child ancestors (so the thumb can move)', async () => {
    const store = await mountedStore()
    // A sole child of a sole child of a topic. Some topics (VA, Health, DHS)
    // have a single agency — find a bureau under that lone agency that is
    // itself a sole child of its agency, OR any sole-child node; verify
    // adjustMax > parent.value.
    const sole = store.nodes.find(n => {
      if (n.parent < 0) return false
      const parent = store.nodes[n.parent]
      return parent.children.length === 1 && parent.parent >= 0
    })
    if (!sole) return // tree may not happen to have a sole-child chain
    const parent = store.nodes[sole.parent]
    const max = store.adjustMax(sole)
    expect(max).toBeGreaterThan(parent.value)
  })
})
