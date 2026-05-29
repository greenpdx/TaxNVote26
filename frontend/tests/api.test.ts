// tests/api.test.ts — the Tax Dollar checksum is a cross-language contract:
// the browser must produce exactly the sha256 the Rust server recomputes
// (allocations sorted by id, joined as "id:pct(6dp)" with ",", then sha256).

import { describe, expect, it } from 'vitest'
import { createHash } from 'node:crypto'
import { buildTaxDollarCsv } from '../src/api'

type Alloc = { id: string; pct: number }

// Independent reference using Node's crypto over the canonical form.
function refChecksum(allocs: Alloc[]): string {
  const sorted = [...allocs].sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))
  const canonical = sorted.map(a => `${a.id}:${a.pct.toFixed(6)}`).join(',')
  return 'sha256:' + createHash('sha256').update(canonical).digest('hex')
}
function field(csv: string, key: string): string | undefined {
  return csv.split('\n').find(l => l.startsWith(`#${key},`))?.slice(key.length + 2)
}
function dataRows(csv: string): Alloc[] {
  const lines = csv.split('\n')
  const start = lines.findIndex(l => l.trim() === 'id,pct') + 1
  return lines.slice(start).filter(l => l.trim()).map(l => {
    const [id, pct] = l.split(',')
    return { id, pct: parseFloat(pct) }
  })
}

describe('buildTaxDollarCsv', () => {
  it('produces the exact checksum the Rust server accepted (regression lock)', async () => {
    const csv = await buildTaxDollarCsv(
      [{ id: 't:va', pct: 0.4 }, { id: 't:def', pct: 0.6 }], '2027', 'default')
    expect(field(csv, 'checksum')).toBe(
      'sha256:c616cde4a850b3c37fb94acc03ed4b01f52287d1a475feae5e2e266b38c58b00')
  })

  it('sorts allocations by id and formats pct to 6 decimals', async () => {
    const csv = await buildTaxDollarCsv(
      [{ id: 't:va', pct: 0.4 }, { id: 't:def', pct: 0.6 }], '2027', 'default')
    expect(dataRows(csv).map(r => r.id)).toEqual(['t:def', 't:va'])
    expect(csv).toContain('t:def,0.600000')
    expect(csv).toContain('t:va,0.400000')
  })

  it('emits the required header fields', async () => {
    const csv = await buildTaxDollarCsv([{ id: 't:def', pct: 1 }], '2027', 'TPL-2027-000001')
    expect(csv.startsWith('#TNV-TAXDOLLAR\n')).toBe(true)
    expect(field(csv, 'version')).toBe('1')
    expect(field(csv, 'fiscal_year')).toBe('2027')
    expect(field(csv, 'template_id')).toBe('TPL-2027-000001')
    expect(field(csv, 'checksum')).toMatch(/^sha256:[0-9a-f]{64}$/)
  })

  it('pure-JS checksum matches Node crypto over the canonical form', async () => {
    const allocs = [
      { id: 't:edu', pct: 0.25 }, { id: 't:health', pct: 0.25 }, { id: 't:infra', pct: 0.5 }]
    const csv = await buildTaxDollarCsv(allocs, '2027', 'default')
    expect(field(csv, 'checksum')).toBe(refChecksum(allocs))
  })

  it('fixes rounding so the allocations sum to exactly 1.0', async () => {
    const t = 1 / 3
    const csv = await buildTaxDollarCsv(
      [{ id: 't:aaa', pct: t }, { id: 't:bbb', pct: t }, { id: 't:ccc', pct: t }], '2027', 'default')
    const rows = dataRows(csv)
    expect(rows.reduce((s, r) => s + r.pct, 0)).toBeCloseTo(1.0, 6)
    expect(field(csv, 'checksum')).toBe(refChecksum(rows)) // checksum over emitted values
  })

  it('drops zero allocations', async () => {
    const csv = await buildTaxDollarCsv(
      [{ id: 't:def', pct: 0.7 }, { id: 't:va', pct: 0.3 }, { id: 't:oth', pct: 0 }], '2027', 'default')
    expect(csv).not.toContain('t:oth')
  })

  it('rejects an empty allocation', async () => {
    await expect(buildTaxDollarCsv([], '2027', 'default')).rejects.toThrow()
  })
})
