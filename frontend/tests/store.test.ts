// tests/store.test.ts — exercise the Pinia store through real WASM.
//
// Vue reactive proxies update in place: capturing `node.value` as a number
// before mutation is required, otherwise comparisons resolve to the
// post-mutation value vs itself.

import { beforeEach, describe, expect, it } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { useBudgetStore } from '../src/stores/budget'
import type { BudgetNode } from '../src/types/budget'

interface Snapshot { idx: number; id: string; value: number; level: number; parent: number; children: number[] }

function snap(n: BudgetNode): Snapshot {
  return { idx: n.idx, id: n.id, value: n.value, level: n.level, parent: n.parent, children: [...n.children] }
}

function findLeafWithUnlockedSiblings(store: ReturnType<typeof useBudgetStore>, minSiblings = 1): Snapshot {
  for (const n of store.nodes) {
    if (n.level !== 3 || n.parent < 0) continue
    const siblings = store.nodes.filter(s => s.parent === n.parent && s.idx !== n.idx && !s.locked)
    if (siblings.length >= minSiblings && n.value > 1000 && !n.locked) return snap(n)
  }
  throw new Error(`no adjustable leaf with >=${minSiblings} siblings found`)
}

function findBureau(store: ReturnType<typeof useBudgetStore>): Snapshot {
  for (const n of store.nodes) {
    if (n.level !== 2 || n.parent < 0) continue
    const siblings = store.nodes.filter(s => s.parent === n.parent && s.idx !== n.idx)
    const children = store.nodes.filter(c => c.parent === n.idx)
    if (siblings.length >= 1 && children.length >= 1 && n.value > 1000) return snap(n)
  }
  throw new Error('no adjustable bureau found')
}

describe('budget store ↔ WASM', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('initialises and exposes a non-empty tree', async () => {
    const store = useBudgetStore()
    await store.init()
    expect(store.initError).toBeNull()
    expect(store.ready).toBe(true)
    expect(store.nodes.length).toBeGreaterThan(50)
    expect(store.nodes[0].id).toBe('root')
    expect(store.totalValue).toBeGreaterThan(0)
  })

  it('leaf increase: siblings drop, parent stays constant', async () => {
    const store = useBudgetStore()
    await store.init()

    const leaf = findLeafWithUnlockedSiblings(store)
    const parentBefore = store.nodes[leaf.parent].value
    const siblingsBefore = store.nodes
      .filter(s => s.parent === leaf.parent && s.idx !== leaf.idx)
      .map(s => ({ idx: s.idx, value: s.value }))

    store.adjust(leaf.id, leaf.value * 1.10)

    expect(store.nodes[leaf.idx].value).toBeGreaterThan(leaf.value)
    expect(Math.abs(store.nodes[leaf.parent].value - parentBefore)).toBeLessThan(1)

    const anyDropped = siblingsBefore.some(s => store.nodes[s.idx].value < s.value - 1)
    expect(anyDropped).toBe(true)
  })

  it('leaf decrease: siblings rise, parent stays constant', async () => {
    const store = useBudgetStore()
    await store.init()

    const leaf = findLeafWithUnlockedSiblings(store)
    const parentBefore = store.nodes[leaf.parent].value
    const siblingsBefore = store.nodes
      .filter(s => s.parent === leaf.parent && s.idx !== leaf.idx)
      .map(s => ({ idx: s.idx, value: s.value }))
    const target = leaf.value * 0.5

    store.adjust(leaf.id, target)

    expect(store.nodes[leaf.idx].value).toBeLessThan(leaf.value)
    expect(store.nodes[leaf.idx].value).toBeCloseTo(target, 0)
    expect(Math.abs(store.nodes[leaf.parent].value - parentBefore)).toBeLessThan(1)

    const anyGrew = siblingsBefore.some(s => store.nodes[s.idx].value > s.value + 1)
    expect(anyGrew).toBe(true)
  })

  it('bureau change: rescales children + sibling bureaus absorb delta', async () => {
    const store = useBudgetStore()
    await store.init()

    const bureau = findBureau(store)
    const childrenBefore = store.nodes
      .filter(c => c.parent === bureau.idx)
      .map(c => ({ idx: c.idx, value: c.value }))
    const siblingsBefore = store.nodes
      .filter(s => s.parent === bureau.parent && s.idx !== bureau.idx)
      .map(s => ({ idx: s.idx, value: s.value }))
    const parentBefore = store.nodes[bureau.parent].value
    const siblingSumBefore = siblingsBefore.reduce((a, s) => a + s.value, 0)

    store.adjust(bureau.id, bureau.value * 1.10)

    // Agency total preserved.
    expect(Math.abs(store.nodes[bureau.parent].value - parentBefore)).toBeLessThan(1)

    // Children rescaled to new bureau value.
    const bureauAfter = store.nodes[bureau.idx].value
    const childSum = childrenBefore.reduce((a, c) => a + store.nodes[c.idx].value, 0)
    expect(Math.abs(childSum - bureauAfter)).toBeLessThan(1)

    const someChildChanged = childrenBefore.some(c => Math.abs(store.nodes[c.idx].value - c.value) > 1)
    expect(someChildChanged).toBe(true)

    // Sibling pool absorbs the bureau's net delta. Test only the magnitude,
    // since clamping can cap the increase below the requested target.
    const siblingSumAfter = siblingsBefore.reduce((a, s) => a + store.nodes[s.idx].value, 0)
    const absorbed = siblingSumBefore - siblingSumAfter
    const leafDelta = bureauAfter - bureau.value
    expect(absorbed).toBeCloseTo(leafDelta, 0)
  })

  it('locked sibling is not touched by adjustment', async () => {
    const store = useBudgetStore()
    await store.init()

    const leaf = findLeafWithUnlockedSiblings(store, 2)
    const sibling = store.nodes.find(s => s.parent === leaf.parent && s.idx !== leaf.idx)!
    const lockedSnap = snap(sibling)
    store.lock(lockedSnap.id)

    store.adjust(leaf.id, leaf.value * 1.05)

    expect(Math.abs(store.nodes[lockedSnap.idx].value - lockedSnap.value)).toBeLessThan(0.01)
    expect(store.nodes[lockedSnap.idx].locked).toBe(true)
  })

  it('subtree sum invariant: every non-leaf == sum(children)', async () => {
    const store = useBudgetStore()
    await store.init()

    const leaf = findLeafWithUnlockedSiblings(store)
    store.adjust(leaf.id, leaf.value * 0.7)

    for (const n of store.nodes) {
      if (!n || n.children.length === 0) continue
      const sum = n.children.reduce((s, ci) => s + store.nodes[ci].value, 0)
      expect(Math.abs(n.value - sum)).toBeLessThan(1)
    }
  })

  it('resetAll restores defaults and clears locks', async () => {
    const store = useBudgetStore()
    await store.init()

    const leaf = findLeafWithUnlockedSiblings(store)
    store.lock(leaf.id)
    expect(store.nodes[leaf.idx].locked).toBe(true)
    store.unlock(leaf.id)

    // Mutate via a different leaf so we have something to reset.
    const other = (() => {
      for (const n of store.nodes) {
        if (n.idx === leaf.idx || n.level !== 3 || n.parent < 0 || n.value <= 1000) continue
        const sibs = store.nodes.filter(s => s.parent === n.parent && s.idx !== n.idx && !s.locked)
        if (sibs.length >= 1) return snap(n)
      }
      throw new Error('no second adjustable leaf')
    })()

    store.adjust(other.id, other.value * 0.6)
    expect(store.nodes[other.idx].value).toBeLessThan(other.value)

    store.resetAll()
    expect(Math.abs(store.nodes[other.idx].value - other.value)).toBeLessThan(1)
    expect(store.nodes.every(n => !n.locked)).toBe(true)
  })
})
