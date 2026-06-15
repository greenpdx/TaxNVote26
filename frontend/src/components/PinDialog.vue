<script setup lang="ts">
import { ref, watch } from 'vue'

const props = defineProps<{ open: boolean; busy?: boolean }>()
const emit = defineEmits<{ (e: 'submit', pin: string): void; (e: 'close'): void }>()

const pin = ref('')
const err = ref<string | null>(null)

watch(() => props.open, (o) => { if (o) { pin.value = ''; err.value = null } })

function go() {
  if (!/^\d{4}$/.test(pin.value)) { err.value = 'Enter a 4-digit PIN.'; return }
  emit('submit', pin.value)
}
</script>

<template>
  <div v-if="open" class="overlay" @click.self="emit('close')">
    <div class="dialog">
      <h3 class="d-title">Protect your submission</h3>
      <p class="d-hint">
        Set a 4-digit PIN. It's needed to open your submission link until the
        data is released. (In production this is the last 4 digits of your SSN.)
      </p>
      <input class="d-in" v-model="pin" type="password" inputmode="numeric" maxlength="4"
             placeholder="4-digit PIN" @keyup.enter="go" />
      <div v-if="err" class="d-err">{{ err }}</div>
      <div class="d-actions">
        <button class="d-cancel" @click="emit('close')">Cancel</button>
        <button class="d-login" :disabled="busy" @click="go">{{ busy ? '…' : 'Submit' }}</button>
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
.d-hint { font-size: 12px; color: #64748b; margin-bottom: 12px; line-height: 1.5; }
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
