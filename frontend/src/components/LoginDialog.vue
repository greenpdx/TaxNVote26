<script setup lang="ts">
import { ref, watch } from 'vue'
import { useSessionStore } from '../stores/session'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ (e: 'close'): void; (e: 'success'): void }>()

const session = useSessionStore()
const name = ref('')
const pin = ref('')

watch(() => props.open, (o) => {
  if (o) { name.value = ''; pin.value = ''; session.error = null }
})

async function doLogin() {
  if (await session.identify(name.value.trim(), pin.value.trim())) {
    emit('success')
    emit('close')
  }
}
</script>

<template>
  <div v-if="open" class="overlay" @click.self="emit('close')">
    <div class="dialog">
      <h3 class="d-title">Sign in</h3>
      <p class="d-hint">Enter a name and a 4-digit number. Use the same pair again to find your data — like a receipt.</p>
      <input class="d-in" v-model="name" placeholder="Name" maxlength="64" @keyup.enter="doLogin" />
      <input class="d-in" v-model="pin" placeholder="4-digit number" inputmode="numeric"
             maxlength="4" @keyup.enter="doLogin" />
      <div v-if="session.error" class="d-err">{{ session.error }}</div>
      <div class="d-actions">
        <button class="d-cancel" @click="emit('close')">Cancel</button>
        <button class="d-login" :disabled="session.busy" @click="doLogin">
          {{ session.busy ? '…' : 'Login' }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed; inset: 0; z-index: 100;
  background: rgba(2, 6, 23, 0.7); backdrop-filter: blur(2px);
  display: flex; align-items: center; justify-content: center; padding: 16px;
}
.dialog {
  width: 100%; max-width: 360px; background: #0f172a;
  border: 1px solid #334155; border-radius: 12px; padding: 18px;
  box-shadow: 0 20px 50px rgba(0,0,0,0.5);
}
.d-title { font-size: 18px; color: #f59e0b; margin-bottom: 4px; }
.d-hint { font-size: 12px; color: #64748b; margin-bottom: 12px; }
.d-in {
  width: 100%; background: #1e293b; border: 1px solid #334155; color: #e2e8f0;
  padding: 10px 12px; border-radius: 8px; font-size: 15px; outline: none; margin-bottom: 8px;
}
.d-in:focus { border-color: #3b82f6; }
.d-err { color: #f87171; font-size: 13px; margin-bottom: 8px; }
.d-actions { display: flex; gap: 8px; justify-content: flex-end; margin-top: 4px; }
.d-cancel { background: #1e293b; border: 1px solid #334155; color: #94a3b8; padding: 8px 14px; border-radius: 8px; cursor: pointer; }
.d-cancel:hover { background: #334155; color: #e2e8f0; }
.d-login { background: #2563eb; border: 1px solid #2563eb; color: #fff; padding: 8px 16px; border-radius: 8px; font-weight: 600; cursor: pointer; }
.d-login:hover { background: #1d4ed8; }
.d-login:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
