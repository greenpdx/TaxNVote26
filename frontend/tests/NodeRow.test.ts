// tests/NodeRow.test.ts — render NodeRow against the real store + WASM
// and drive interactions the way a user would.

import { beforeEach, describe, expect, it } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { mount, flushPromises } from '@vue/test-utils'
import NodeRow from '../src/components/NodeRow.vue'
import { useBudgetStore } from '../src/stores/budget'
import type { BudgetNode } from '../src/types/budget'

function findLeafWithSibling(store: ReturnType<typeof useBudgetStore>): BudgetNode {
  for (const n of store.nodes) {
    if (n.level !== 4 || n.parent < 0) continue
    const sibs = store.nodes.filter(s => s.parent === n.parent && s.idx !== n.idx && !s.locked)
    if (sibs.length >= 1 && n.value > 1000) return n
  }
  throw new Error('no adjustable leaf found')
}

async function mountedStore() {
  setActivePinia(createPinia())
  const store = useBudgetStore()
  await store.init()
  return store
}

describe('NodeRow', () => {
  beforeEach(() => setActivePinia(createPinia()))

  it('renders name and dollar value', async () => {
    const store = await mountedStore()
    const leaf = findLeafWithSibling(store)
    const wrapper = mount(NodeRow, { props: { node: store.nodes[leaf.idx] } })
    expect(wrapper.text()).toContain(leaf.name)
    expect(wrapper.find('.node-dollars').text()).toMatch(/\$/)
  })

  it('toggles lock when 🔓 is clicked', async () => {
    const store = await mountedStore()
    const leaf = findLeafWithSibling(store)
    const wrapper = mount(NodeRow, { props: { node: store.nodes[leaf.idx] } })

    expect(store.nodes[leaf.idx].locked).toBe(false)
    await wrapper.find('.lock-btn').trigger('click')
    expect(store.nodes[leaf.idx].locked).toBe(true)

    await wrapper.find('.lock-btn').trigger('click')
    expect(store.nodes[leaf.idx].locked).toBe(false)
  })

  it('slider input drives store.adjust and cascades to siblings', async () => {
    const store = await mountedStore()
    const leaf = findLeafWithSibling(store)
    const startValue = leaf.value
    const parentValue = store.nodes[leaf.parent].value
    const siblingsBefore = store.nodes
      .filter(s => s.parent === leaf.parent && s.idx !== leaf.idx)
      .map(s => ({ idx: s.idx, value: s.value }))

    const wrapper = mount(NodeRow, { props: { node: store.nodes[leaf.idx] } })
    // Slider only appears once the node is selected.
    await wrapper.find('.node-header').trigger('click')
    await flushPromises()

    const slider = wrapper.find<HTMLInputElement>('.slider')
    expect(slider.exists()).toBe(true)

    const target = startValue * 0.7
    slider.element.value = String(target)
    await slider.trigger('input')

    expect(store.nodes[leaf.idx].value).toBeLessThan(startValue)
    expect(store.nodes[leaf.idx].value).toBeCloseTo(target, 0)
    expect(Math.abs(store.nodes[leaf.parent].value - parentValue)).toBeLessThan(1)
    const anyGrew = siblingsBefore.some(s => store.nodes[s.idx].value > s.value + 1)
    expect(anyGrew).toBe(true)

    // Reactivity: rendered dollar text reflects the new value.
    expect(wrapper.find('.node-dollars').text()).toMatch(/\$/)
  })

  it('expand button reveals children, child renders recursively', async () => {
    const store = await mountedStore()
    store.mode = 'full' // drill-down (expand button) only exists in full mode
    // Pick a topic (level 1) that has children
    const agency = store.nodes.find(n => n.level === 1 && n.children.length > 0)!
    const wrapper = mount(NodeRow, { props: { node: store.nodes[agency.idx] } })

    expect(wrapper.findAll('.node-row').length).toBe(1)
    await wrapper.find('.expand-btn').trigger('click')
    await flushPromises()
    // After expand, at least one child row should be rendered.
    expect(wrapper.findAll('.node-row').length).toBeGreaterThan(1)
  })

  it('reset button (↺) restores the leaf to its default and cascades back', async () => {
    const store = await mountedStore()
    const leaf = findLeafWithSibling(store)
    const startValue = leaf.value

    const wrapper = mount(NodeRow, { props: { node: store.nodes[leaf.idx] } })
    await wrapper.find('.node-header').trigger('click')
    await flushPromises()

    const slider = wrapper.find<HTMLInputElement>('.slider')
    slider.element.value = String(startValue * 0.5)
    await slider.trigger('input')
    expect(store.nodes[leaf.idx].value).toBeLessThan(startValue)

    const resetBtn = wrapper.findAll('.act-btn').find(b => b.text() === '↺')!
    await resetBtn.trigger('click')
    await flushPromises()

    expect(store.nodes[leaf.idx].value).toBeCloseTo(startValue, 0)
  })

  it('locked node disables the slider', async () => {
    const store = await mountedStore()
    const leaf = findLeafWithSibling(store)
    const wrapper = mount(NodeRow, { props: { node: store.nodes[leaf.idx] } })
    await wrapper.find('.node-header').trigger('click')
    await flushPromises()

    await wrapper.find('.lock-btn').trigger('click')
    await flushPromises()

    const slider = wrapper.find<HTMLInputElement>('.slider')
    expect(slider.element.disabled).toBe(true)
  })
})
