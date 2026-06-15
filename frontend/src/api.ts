// src/api.ts — TNV demo API client + Tax Dollar / template CSV builders.

const BASE = '/api'

export interface TemplateSummary {
  receipt_no: string
  name: string
  entity_name: string | null
  description: string | null
  fiscal_year: string
  entry_count: number
  created_at: string
}

export interface TaxDollarReceipt {
  receipt_token: string
  fiscal_year: string
  created_at: string
  replaced: boolean
  access_code: string
}

export interface TaxDollarSummary {
  receipt_token: string
  fiscal_year: string
  template_receipt_no: string
  created_at: string
  raw_csv: string
}

export interface NodeStat {
  node_id: string
  count: number
  mean: number
  median: number
  trimmed_mean: number
  std_dev: number
  min: number
  max: number
}

export interface AggregateResponse {
  fiscal_year: string
  submission_count: number
  nodes: NodeStat[]
}

export interface Alloc { id: string; pct: number }
export interface Entry { id: string; value: number }

function authHeaders(token: string | null): Record<string, string> {
  return token ? { authorization: `Bearer ${token}` } : {}
}

async function asError(res: Response): Promise<never> {
  let msg = `HTTP ${res.status}`
  try {
    const body = await res.json()
    if (body?.error) msg = body.error
  } catch { /* non-JSON body */ }
  throw new Error(msg)
}

// ─── Auth (demo) ───────────────────────────────────────────────
export async function identify(name: string, secret: string): Promise<{ token: string; username: string }> {
  const res = await fetch(`${BASE}/identify`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ name, secret }),
  })
  if (!res.ok) return asError(res)
  return res.json()
}

export interface MeResponse { id: number; username: string; tier: number; created_at: string }

export async function login(email: string, password: string): Promise<{ token: string; username: string }> {
  const res = await fetch(`${BASE}/auth/login`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ email, password }),
  })
  if (!res.ok) return asError(res)
  return res.json()
}

export async function me(token: string): Promise<MeResponse> {
  const res = await fetch(`${BASE}/auth/me`, { headers: authHeaders(token) })
  if (!res.ok) return asError(res)
  return res.json()
}

export interface ChallengeResponse { challenge: string; difficulty: number; expires_in_secs: number }

export async function getChallenge(): Promise<ChallengeResponse> {
  const res = await fetch(`${BASE}/auth/challenge`)
  if (!res.ok) return asError(res)
  return res.json()
}

export async function register(
  username: string, email: string, password: string, challenge: string, nonce: string,
): Promise<{ message: string; email: string }> {
  const res = await fetch(`${BASE}/auth/register`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ username, email, password, challenge, nonce }),
  })
  if (!res.ok) return asError(res)
  return res.json()
}

export async function verifyEmail(email: string, code: string): Promise<{ token: string; username: string }> {
  const res = await fetch(`${BASE}/auth/verify`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ email, code }),
  })
  if (!res.ok) return asError(res)
  return res.json()
}

/** Solve the registration proof-of-work: find a nonce so that
 *  SHA-256(challenge + nonce) has `difficulty` leading zero bits. Uses the
 *  pure-JS SHA-256 in yield-friendly chunks (works on non-secure origins). */
export async function solvePow(
  challenge: string, difficulty: number, onProgress?: (tried: number) => void,
): Promise<string> {
  const fullZeros = Math.floor(difficulty / 4)
  const remBits = difficulty % 4
  const prefix = '0'.repeat(fullZeros)
  const maxNibble = remBits === 0 ? 16 : 1 << (4 - remBits)
  const ok = (h: string) =>
    h.startsWith(prefix) && (remBits === 0 || parseInt(h[fullZeros], 16) < maxNibble)

  let nonce = 0
  const CHUNK = 1000
  for (;;) {
    for (let i = 0; i < CHUNK; i++) {
      if (ok(sha256Fallback(challenge + nonce))) return String(nonce)
      nonce++
    }
    onProgress?.(nonce)
    await new Promise((r) => setTimeout(r)) // yield to keep the UI responsive
  }
}

// ─── Templates ─────────────────────────────────────────────────
export async function listTemplates(): Promise<TemplateSummary[]> {
  const res = await fetch(`${BASE}/templates`)
  if (!res.ok) return asError(res)
  return res.json()
}

export async function getTemplateCsv(receiptNo: string): Promise<string> {
  const res = await fetch(`${BASE}/templates/${encodeURIComponent(receiptNo)}`)
  if (!res.ok) return asError(res)
  return res.text()
}

export async function createTemplate(csv: string, token: string): Promise<{ receipt_no: string; name: string }> {
  const res = await fetch(`${BASE}/templates`, {
    method: 'POST',
    headers: { 'content-type': 'text/plain', ...authHeaders(token) },
    body: csv,
  })
  if (!res.ok) return asError(res)
  return res.json()
}

// ─── Tax Dollars ───────────────────────────────────────────────
export async function submitTaxDollar(csv: string, token: string): Promise<TaxDollarReceipt> {
  const res = await fetch(`${BASE}/taxdollar`, {
    method: 'POST',
    headers: { 'content-type': 'text/plain', ...authHeaders(token) },
    body: csv,
  })
  if (!res.ok) return asError(res)
  return res.json()
}

/** Fetch a submission via its public link. Throws an error with `codeRequired`
 *  set when the access code is needed (HTTP 401). */
export async function fetchSubmission(receiptToken: string, code = ''): Promise<string> {
  const qs = code ? `?code=${encodeURIComponent(code)}` : ''
  const res = await fetch(`${BASE}/taxdollar/${encodeURIComponent(receiptToken)}${qs}`)
  if (res.status === 401) {
    const e = new Error('access code required') as Error & { codeRequired?: boolean }
    e.codeRequired = true
    throw e
  }
  if (!res.ok) return asError(res)
  return res.text()
}

export async function myTaxDollars(token: string): Promise<TaxDollarSummary[]> {
  const res = await fetch(`${BASE}/taxdollar/mine`, { headers: authHeaders(token) })
  if (!res.ok) return asError(res)
  return res.json()
}

export async function getTaxDollarCsv(receiptToken: string): Promise<string> {
  const res = await fetch(`${BASE}/taxdollar/${encodeURIComponent(receiptToken)}`)
  if (!res.ok) return asError(res)
  return res.text()
}

// ─── Aggregate ─────────────────────────────────────────────────
export async function getAggregate(fiscalYear: string): Promise<AggregateResponse> {
  const res = await fetch(`${BASE}/aggregate?fiscal_year=${encodeURIComponent(fiscalYear)}`)
  if (!res.ok) return asError(res)
  return res.json()
}

// ─── Checksum (must byte-match server: sorted "id:pct.6f" join "," → sha256) ──
// Always use the pure-JS SHA-256: Web Crypto (crypto.subtle) only exists in
// secure contexts (https / localhost), so it's unavailable over a plain-HTTP
// LAN origin. The JS impl works everywhere and is plenty fast for our sizes.
function sha256Hex(s: string): string {
  return sha256Fallback(s)
}

/** Pure-JS SHA-256 over UTF-8 → lowercase hex. Used when crypto.subtle is
 *  unavailable (non-secure origin). Verified against the reference impl. */
function sha256Fallback(input: string): string {
  const K = new Uint32Array([
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
  ])
  let h0 = 0x6a09e667, h1 = 0xbb67ae85, h2 = 0x3c6ef372, h3 = 0xa54ff53a
  let h4 = 0x510e527f, h5 = 0x9b05688c, h6 = 0x1f83d9ab, h7 = 0x5be0cd19
  const data = new TextEncoder().encode(input)
  const l = data.length
  const bitLen = l * 8
  const withOne = l + 1
  const k = (56 - (withOne % 64) + 64) % 64
  const total = withOne + k + 8
  const m = new Uint8Array(total)
  m.set(data)
  m[l] = 0x80
  const dv = new DataView(m.buffer)
  dv.setUint32(total - 4, bitLen >>> 0, false)
  dv.setUint32(total - 8, Math.floor(bitLen / 0x100000000), false)
  const w = new Uint32Array(64)
  const rotr = (x: number, n: number) => (x >>> n) | (x << (32 - n))
  for (let i = 0; i < total; i += 64) {
    for (let j = 0; j < 16; j++) w[j] = dv.getUint32(i + j * 4, false)
    for (let j = 16; j < 64; j++) {
      const s0 = rotr(w[j - 15], 7) ^ rotr(w[j - 15], 18) ^ (w[j - 15] >>> 3)
      const s1 = rotr(w[j - 2], 17) ^ rotr(w[j - 2], 19) ^ (w[j - 2] >>> 10)
      w[j] = (w[j - 16] + s0 + w[j - 7] + s1) | 0
    }
    let a = h0, b = h1, c = h2, d = h3, e = h4, f = h5, g = h6, h = h7
    for (let j = 0; j < 64; j++) {
      const S1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25)
      const ch = (e & f) ^ (~e & g)
      const t1 = (h + S1 + ch + K[j] + w[j]) | 0
      const S0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22)
      const maj = (a & b) ^ (a & c) ^ (b & c)
      const t2 = (S0 + maj) | 0
      h = g; g = f; f = e; e = (d + t1) | 0; d = c; c = b; b = a; a = (t1 + t2) | 0
    }
    h0 = (h0 + a) | 0; h1 = (h1 + b) | 0; h2 = (h2 + c) | 0; h3 = (h3 + d) | 0
    h4 = (h4 + e) | 0; h5 = (h5 + f) | 0; h6 = (h6 + g) | 0; h7 = (h7 + h) | 0
  }
  const hx = (x: number) => (x >>> 0).toString(16).padStart(8, '0')
  return hx(h0) + hx(h1) + hx(h2) + hx(h3) + hx(h4) + hx(h5) + hx(h6) + hx(h7)
}

// ─── CSV builders ──────────────────────────────────────────────

/** Build a #TNV-TAXDOLLAR CSV from allocations. Rounds pct to 6 decimals,
 *  drops zeros, fixes rounding residual into the largest so Σ=1.0, and
 *  computes the checksum over exactly the emitted values. */
export async function buildTaxDollarCsv(
  allocs: Alloc[], fiscalYear: string, templateId: string,
): Promise<string> {
  let rounded = allocs
    .map(a => ({ id: a.id, pct: Math.round(a.pct * 1e6) / 1e6 }))
    .filter(a => a.pct > 0)
  if (rounded.length === 0) throw new Error('nothing allocated')

  const sum = rounded.reduce((s, a) => s + a.pct, 0)
  const residual = Math.round((1 - sum) * 1e6) / 1e6
  if (Math.abs(residual) > 0) {
    const largest = rounded.reduce((m, a) => (a.pct > m.pct ? a : m), rounded[0])
    largest.pct = Math.round((largest.pct + residual) * 1e6) / 1e6
  }

  const sorted = rounded.slice().sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))
  const canonical = sorted.map(a => `${a.id}:${a.pct.toFixed(6)}`).join(',')
  const checksum = `sha256:${sha256Hex(canonical)}`

  const ts = new Date().toISOString().replace(/\.\d+Z$/, 'Z')
  const lines = [
    '#TNV-TAXDOLLAR',
    '#version,1',
    `#fiscal_year,${fiscalYear}`,
    `#template_id,${templateId}`,
    `#timestamp,${ts}`,
    `#checksum,${checksum}`,
    'id,pct',
    ...sorted.map(a => `${a.id},${a.pct.toFixed(6)}`),
  ]
  return lines.join('\n') + '\n'
}

/** Build a #TNV-TEMPLATE CSV. Entries are stored as PERCENTAGES (fractions of
 *  the total); the dollar amount is derived later as pct × total, so a template
 *  is a fiscal-year-independent plan of proportions. */
export function buildTemplateCsv(
  allocs: Alloc[],
  meta: { name: string; entity: string; description: string; fiscalYear: string },
): string {
  const lines = ['#TNV-TEMPLATE', `#name,${meta.name}`]
  if (meta.entity) lines.push(`#entity,${meta.entity}`)
  if (meta.description) lines.push(`#description,${meta.description}`)
  lines.push(`#fiscal_year,${meta.fiscalYear}`)
  lines.push('id,pct')
  for (const a of allocs) if (a.pct > 0) lines.push(`${a.id},${a.pct.toFixed(6)}`)
  return lines.join('\n') + '\n'
}

// ─── Admin ─────────────────────────────────────────────────────

export interface AdminUser {
  kind: string; id: number; name: string; tier: number; disabled: boolean; created_at: string
}
export interface AdminTemplate {
  receipt_no: string; name: string; subject_kind: string; subject_id: number
  fiscal_year: string; hidden: boolean; created_at: string
}
export interface AuditEntry {
  id: number; ts: string; actor_kind: string; actor_id: number | null; action: string
  target_kind: string | null; target_id: string | null; detail: string | null; ip: string | null
}
export interface SettingItem { key: string; value: string; updated_at: string }

async function adminReq(path: string, token: string, init?: RequestInit): Promise<Response> {
  const res = await fetch(`${BASE}/admin${path}`, {
    ...init,
    headers: { 'content-type': 'application/json', ...authHeaders(token), ...(init?.headers || {}) },
  })
  if (!res.ok) return asError(res)
  return res
}

export async function adminListUsers(token: string, q = ''): Promise<AdminUser[]> {
  const qs = q ? `?q=${encodeURIComponent(q)}` : ''
  return (await adminReq(`/users${qs}`, token)).json()
}
export async function adminSetUserDisabled(token: string, kind: string, id: number, disabled: boolean): Promise<void> {
  const action = disabled ? 'disable' : 'enable'
  await adminReq(`/users/${kind}/${id}/${action}`, token, { method: 'POST' })
}
export async function adminSetRole(token: string, kind: string, id: number, tier: number): Promise<void> {
  await adminReq(`/users/${kind}/${id}/role`, token, { method: 'POST', body: JSON.stringify({ tier }) })
}
export async function adminListTemplates(token: string): Promise<AdminTemplate[]> {
  return (await adminReq('/templates', token)).json()
}
export async function adminSetTemplateHidden(token: string, receiptNo: string, hidden: boolean): Promise<void> {
  const action = hidden ? 'hide' : 'unhide'
  await adminReq(`/templates/${encodeURIComponent(receiptNo)}/${action}`, token, { method: 'POST' })
}
export interface AdminTaxDollar {
  receipt_token: string; subject_kind: string; subject_id: number
  fiscal_year: string; template_receipt_no: string; hidden: boolean; created_at: string
}
export interface NodeAmount { node_id: string; amount: number }

export async function adminListTaxdollars(token: string): Promise<AdminTaxDollar[]> {
  return (await adminReq('/taxdollars', token)).json()
}
export async function adminTaxdollarAllocations(token: string, receiptToken: string): Promise<NodeAmount[]> {
  return (await adminReq(`/taxdollars/${encodeURIComponent(receiptToken)}/allocations`, token)).json()
}
export async function adminSetTaxdollarHidden(token: string, receiptToken: string, hidden: boolean): Promise<void> {
  const action = hidden ? 'hide' : 'unhide'
  await adminReq(`/taxdollars/${encodeURIComponent(receiptToken)}/${action}`, token, { method: 'POST' })
}
export async function adminTemplateEntries(token: string, receiptNo: string): Promise<NodeAmount[]> {
  return (await adminReq(`/templates/${encodeURIComponent(receiptNo)}/entries`, token)).json()
}

export async function adminListAudit(token: string, action = ''): Promise<AuditEntry[]> {
  const qs = action ? `?action=${encodeURIComponent(action)}` : ''
  return (await adminReq(`/audit${qs}`, token)).json()
}
export async function adminGetConfig(token: string): Promise<SettingItem[]> {
  return (await adminReq('/config', token)).json()
}
export async function adminSetConfig(token: string, key: string, value: string): Promise<void> {
  await adminReq(`/config/${encodeURIComponent(key)}`, token, { method: 'PUT', body: JSON.stringify({ value }) })
}

/** Parse the data rows of a #TNV-TEMPLATE (`id,value`) or #TNV-TAXDOLLAR
 *  (`id,pct`) CSV into [{id, value}] pairs (value = the second column as-is). */
export function parseTemplateEntries(csv: string): Entry[] {
  const out: Entry[] = []
  let inData = false
  for (const raw of csv.split(/\r?\n/)) {
    const line = raw.trim()
    if (!line || line.startsWith('#')) continue
    if (!inData) {
      const h = line.toLowerCase().replace(/\s/g, '')
      if (h === 'id,value' || h === 'id,pct') inData = true
      continue
    }
    const i = line.indexOf(',')
    if (i < 0) continue
    const id = line.slice(0, i).trim()
    const value = parseFloat(line.slice(i + 1).trim().replace(/,/g, ''))
    if (id && Number.isFinite(value)) out.push({ id, value })
  }
  return out
}
