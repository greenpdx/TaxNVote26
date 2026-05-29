// tests/csv-builders.test.ts — buildTemplateCsv and parseTemplateEntries
// are pure CSV helpers shared by the Templates and Tax Dollar flows.

import { describe, expect, it } from 'vitest'
import { buildTemplateCsv, parseTemplateEntries } from '../src/api'

describe('buildTemplateCsv', () => {
  it('emits the magic header and required metadata in order', () => {
    const csv = buildTemplateCsv(
      [{ id: 't:def', value: 1100000000 }, { id: 't:va', value: 145000000 }],
      { name: 'Tester Plan', entity: 'Sierra Club', description: 'demo', fiscalYear: '2027' },
    )
    const lines = csv.trim().split('\n')
    expect(lines[0]).toBe('#TNV-TEMPLATE')
    expect(lines).toContain('#name,Tester Plan')
    expect(lines).toContain('#entity,Sierra Club')
    expect(lines).toContain('#description,demo')
    expect(lines).toContain('#fiscal_year,2027')
    expect(lines).toContain('id,value')
    expect(lines).toContain('t:def,1100000000')
    expect(lines).toContain('t:va,145000000')
  })

  it('omits #entity and #description when empty', () => {
    const csv = buildTemplateCsv(
      [{ id: 't:def', value: 1 }],
      { name: 'No Entity', entity: '', description: '', fiscalYear: '2027' },
    )
    expect(csv).not.toContain('#entity')
    expect(csv).not.toContain('#description')
    expect(csv).toContain('#name,No Entity')
  })

  it('rounds values to integers in data rows', () => {
    const csv = buildTemplateCsv(
      [{ id: 't:def', value: 100.4 }, { id: 't:va', value: 99.6 }],
      { name: 'Round', entity: '', description: '', fiscalYear: '2027' },
    )
    expect(csv).toContain('t:def,100')
    expect(csv).toContain('t:va,100')
  })
})

describe('parseTemplateEntries', () => {
  it('parses #TNV-TEMPLATE (id,value)', () => {
    const csv = '#TNV-TEMPLATE\n#name,T\n#fiscal_year,2027\nid,value\nt:def,1100000000\nt:va,145000000\n'
    expect(parseTemplateEntries(csv)).toEqual([
      { id: 't:def', value: 1100000000 },
      { id: 't:va', value: 145000000 },
    ])
  })

  it('parses #TNV-TAXDOLLAR (id,pct) using the same shape', () => {
    const csv = '#TNV-TAXDOLLAR\n#version,1\nid,pct\nt:def,0.600000\nt:va,0.400000\n'
    expect(parseTemplateEntries(csv)).toEqual([
      { id: 't:def', value: 0.6 },
      { id: 't:va', value: 0.4 },
    ])
  })

  it('skips metadata, blank lines, and lines without a comma', () => {
    const csv = '#TNV-TEMPLATE\n#name,X\n\nid,value\nt:def,100\n\nbad-line\nt:va,50\n'
    expect(parseTemplateEntries(csv)).toEqual([
      { id: 't:def', value: 100 },
      { id: 't:va', value: 50 },
    ])
  })

  it('returns empty when no data section is present', () => {
    expect(parseTemplateEntries('#TNV-TEMPLATE\n#name,X\n')).toEqual([])
  })
})
