// tests/session.test.ts — the ephemeral name+PIN session store. Auth network
// calls are mocked; we just check the store's state transitions.

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'
import { useSessionStore } from '../src/stores/session'

function mockFetchOnce(payload: unknown, ok = true, status = 200) {
  vi.stubGlobal('fetch', vi.fn(async () => ({
    ok,
    status,
    async json() { return payload },
  })) as unknown as typeof fetch)
}

describe('session store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.unstubAllGlobals()
  })

  it('starts signed out', () => {
    const s = useSessionStore()
    expect(s.token).toBeNull()
    expect(s.name).toBeNull()
    expect(s.isIdentified).toBe(false)
  })

  it('identify(name, pin) stores token + name on success', async () => {
    mockFetchOnce({ token: 'jwt.test.token', username: 'Alice' })
    const s = useSessionStore()
    const ok = await s.identify('Alice', '1234')
    expect(ok).toBe(true)
    expect(s.isIdentified).toBe(true)
    expect(s.token).toBe('jwt.test.token')
    expect(s.name).toBe('Alice')
    expect(s.error).toBeNull()
  })

  it('identify surfaces the server error and stays signed out on failure', async () => {
    mockFetchOnce({ error: 'secret must be 4 digits' }, false, 400)
    const s = useSessionStore()
    const ok = await s.identify('Alice', '12')
    expect(ok).toBe(false)
    expect(s.isIdentified).toBe(false)
    expect(s.error).toContain('4 digits')
  })

  it('logout clears the in-memory token and name', async () => {
    mockFetchOnce({ token: 'tok', username: 'Bob' })
    const s = useSessionStore()
    await s.identify('Bob', '4321')
    expect(s.isIdentified).toBe(true)
    s.logout()
    expect(s.isIdentified).toBe(false)
    expect(s.token).toBeNull()
    expect(s.name).toBeNull()
  })

  it('does not persist anything to localStorage (ephemeral session)', async () => {
    mockFetchOnce({ token: 'tok', username: 'Carol' })
    const s = useSessionStore()
    await s.identify('Carol', '0000')
    expect(localStorage.getItem('tnv_token')).toBeNull()
    expect(localStorage.getItem('tnv_name')).toBeNull()
  })
})
