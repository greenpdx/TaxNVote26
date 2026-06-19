<script setup lang="ts">
// Shared top bar for both the landing page and the budget tool: brand,
// admin-configurable subtitles, and the login/logout widget. Pages drop
// page-specific rows (e.g. the tool's tabs) into the default slot.
import { ref, onMounted } from 'vue'
import { useSessionStore } from '../stores/session'
import { getPublicConfig } from '../api'

defineEmits<{ (e: 'login'): void }>()
const session = useSessionStore()

// Admin-configurable header subtitles (fetched publicly; defaults until loaded).
const subtitle1 = ref('Your Tax Dollar, Your Voice')
const subtitle2 = ref('')
onMounted(async () => {
  try {
    const cfg = await getPublicConfig()
    subtitle1.value = cfg.subtitle_1
    subtitle2.value = cfg.subtitle_2
  } catch { /* keep defaults if the config endpoint is unreachable */ }
})
</script>

<template>
  <header class="header">
    <div class="header-top">
      <div class="title-group">
        <RouterLink to="/" class="title-link"><h1 class="title">Tax N Vote</h1></RouterLink>
        <div class="subtitle-row">
          <span class="subtitle">{{ subtitle1 }}</span>
          <span v-if="subtitle2" class="subtitle subtitle-2">{{ subtitle2 }}</span>
        </div>
      </div>
      <div class="id-widget">
        <template v-if="session.isIdentified">
          <span class="id-who">👤 {{ session.name }}</span>
          <button class="abtn" @click="session.logout()">Logout</button>
        </template>
        <button v-else class="abtn login" @click="$emit('login')">Login</button>
      </div>
    </div>
    <slot />
  </header>
</template>
