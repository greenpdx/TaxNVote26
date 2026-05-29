// tests/format.test.ts — formatDollars treats values as thousands of dollars,
// producing T/B/M/K. Verified end-to-end: the header showed $1.8T when the raw
// root total was 1,828,897,000 (thousands).

import { beforeEach, describe, expect, it } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { useBudgetStore } from '../src/stores/budget'

describe('formatDollars (input is thousands of dollars)', () => {
  let fmt: (v: number) => string
  beforeEach(() => {
    setActivePinia(createPinia())
    fmt = useBudgetStore().formatDollars
  })

  it('formats sub-million-dollar values in K', () => {
    expect(fmt(0)).toBe('$0K')
    expect(fmt(500)).toBe('$500K')
    expect(fmt(999)).toBe('$999K')
  })

  it('formats millions in M', () => {
    expect(fmt(1_000)).toBe('$1.0M')
    expect(fmt(1_500)).toBe('$1.5M')
    expect(fmt(999_999)).toBe('$1000.0M')
  })

  it('formats billions in B', () => {
    expect(fmt(1_000_000)).toBe('$1.0B')
    expect(fmt(82_600_000)).toBe('$82.6B')
  })

  it('formats trillions in T (matches the live header)', () => {
    expect(fmt(1_000_000_000)).toBe('$1.0T')
    expect(fmt(1_828_897_000)).toBe('$1.8T')
    expect(fmt(1_100_000_000)).toBe('$1.1T')
  })
})
