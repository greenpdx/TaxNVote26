// src/stores/admin.ts — admin session (email/password → JWT, requires admin tier).
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import * as api from '../api'

const msg = (e: unknown) => (e instanceof Error ? e.message : String(e))

export const useAdminStore = defineStore('admin', () => {
  const token = ref<string | null>(null)
  const username = ref<string | null>(null)
  const tier = ref(0)
  const error = ref<string | null>(null)
  const busy = ref(false)

  const isAdmin = computed(() => !!token.value && tier.value >= 100)

  async function login(email: string, password: string) {
    error.value = null
    busy.value = true
    try {
      const r = await api.login(email, password)
      const m = await api.me(r.token)
      if (m.tier < 100) {
        error.value = 'This account is not an administrator.'
        return false
      }
      token.value = r.token
      username.value = m.username
      tier.value = m.tier
      return true
    } catch (e) {
      error.value = msg(e)
      return false
    } finally {
      busy.value = false
    }
  }

  function logout() {
    token.value = null
    username.value = null
    tier.value = 0
    error.value = null
  }

  return { token, username, tier, error, busy, isAdmin, login, logout }
})
