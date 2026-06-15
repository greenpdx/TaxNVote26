// src/stores/session.ts — demo identity (name + 4-digit PIN).
// In-memory only: the session is ephemeral. It is never persisted, so a reload
// starts signed out, and we auto-logout after submitting / viewing own data.
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import * as api from '../api'

export const useSessionStore = defineStore('session', () => {
  const token = ref<string | null>(null)
  const name = ref<string | null>(null)
  const tier = ref(0)
  const error = ref<string | null>(null)
  const busy = ref(false)

  const isIdentified = computed(() => !!token.value)
  // Only email/password accounts can reach admin tier; demo identities are 0.
  const isAdmin = computed(() => tier.value >= 100)

  // Fetch the account tier after an email login/verify (demo stays 0).
  async function refreshTier() {
    try {
      tier.value = token.value ? (await api.me(token.value)).tier : 0
    } catch {
      tier.value = 0
    }
  }

  async function identify(n: string, secret: string) {
    error.value = null
    busy.value = true
    try {
      const res = await api.identify(n, secret)
      token.value = res.token
      name.value = res.username
      tier.value = 0
      return true
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      return false
    } finally {
      busy.value = false
    }
  }

  async function loginEmail(email: string, password: string) {
    error.value = null
    busy.value = true
    try {
      const res = await api.login(email, password)
      token.value = res.token
      name.value = res.username
      await refreshTier()
      return true
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      return false
    } finally {
      busy.value = false
    }
  }

  async function verifyEmail(email: string, code: string) {
    error.value = null
    busy.value = true
    try {
      const res = await api.verifyEmail(email, code)
      token.value = res.token
      name.value = res.username
      await refreshTier()
      return true
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      return false
    } finally {
      busy.value = false
    }
  }

  function logout() {
    token.value = null
    name.value = null
    tier.value = 0
  }

  return { token, name, tier, error, busy, isIdentified, isAdmin, identify, loginEmail, verifyEmail, logout }
})
