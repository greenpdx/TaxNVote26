// src/stores/session.ts — demo identity (name + 4-digit PIN).
// In-memory only: the session is ephemeral. It is never persisted, so a reload
// starts signed out, and we auto-logout after submitting / viewing own data.
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import * as api from '../api'

export const useSessionStore = defineStore('session', () => {
  const token = ref<string | null>(null)
  const name = ref<string | null>(null)
  const error = ref<string | null>(null)
  const busy = ref(false)

  const isIdentified = computed(() => !!token.value)

  async function identify(n: string, secret: string) {
    error.value = null
    busy.value = true
    try {
      const res = await api.identify(n, secret)
      token.value = res.token
      name.value = res.username
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
  }

  return { token, name, error, busy, isIdentified, identify, logout }
})
