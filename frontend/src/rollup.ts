// src/rollup.ts — roll up a record's per-node amounts into the budget hierarchy
// (topic → agency → bureau → account) for a concise, expandable display.
//
// Node ids: t:<topic>, a:<topic>:<agency>, b:<topic>:<agency>:<bureau>,
//           c:<topic>:<agency>:<bureau>:<account>

export interface RollupNode {
  id: string
  amount: number
  pct: number
  children: RollupNode[]
}

function ancestorsOf(id: string): string[] {
  const p = id.split(':')
  if (p[0] === 'c' && p.length === 5) return [`t:${p[1]}`, `a:${p[1]}:${p[2]}`, `b:${p[1]}:${p[2]}:${p[3]}`, id]
  if (p[0] === 'b' && p.length === 4) return [`t:${p[1]}`, `a:${p[1]}:${p[2]}`, id]
  if (p[0] === 'a' && p.length === 3) return [`t:${p[1]}`, id]
  return [id] // topic, or an unrecognized id
}

function parentOf(id: string): string | null {
  const p = id.split(':')
  if (p[0] === 'c') return `b:${p[1]}:${p[2]}:${p[3]}`
  if (p[0] === 'b') return `a:${p[1]}:${p[2]}`
  if (p[0] === 'a') return `t:${p[1]}`
  return null // topic → root
}

/** Build the rolled-up topic→…→account tree from leaf amounts. Percentages are
 *  relative to the sum of all topics. */
export function buildRollup(allocs: { node_id: string; amount: number }[]): RollupNode[] {
  const amt = new Map<string, number>()
  for (const a of allocs) {
    if (!(a.amount > 0)) continue
    for (const anc of ancestorsOf(a.node_id)) amt.set(anc, (amt.get(anc) || 0) + a.amount)
  }
  const total = [...amt].filter(([id]) => id.startsWith('t:')).reduce((s, [, v]) => s + v, 0) || 1

  const childrenOf = new Map<string, string[]>()
  const roots: string[] = []
  for (const id of amt.keys()) {
    const par = parentOf(id)
    if (par && amt.has(par)) {
      const list = childrenOf.get(par) || []
      list.push(id)
      childrenOf.set(par, list)
    } else if (id.startsWith('t:')) {
      roots.push(id)
    }
  }

  const build = (id: string): RollupNode => ({
    id,
    amount: amt.get(id)!,
    pct: amt.get(id)! / total,
    children: (childrenOf.get(id) || []).map(build).sort((a, b) => b.amount - a.amount),
  })
  return roots.map(build).sort((a, b) => b.amount - a.amount)
}
