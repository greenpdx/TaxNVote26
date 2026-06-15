<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { getTaxDollarCsv } from '../api'

const props = defineProps<{ open: boolean; receipt: string }>()
const emit = defineEmits<{ (e: 'close'): void }>()

const copied = ref(false)
const inputEl = ref<HTMLInputElement | null>(null)

// Full shareable link to the public submission view (hash route).
const url = computed(() => `${window.location.origin}/#/s/${props.receipt}`)

async function download() {
  try {
    const csv = await getTaxDollarCsv(props.receipt)
    const blob = new Blob([csv], { type: 'text/csv' })
    const a = document.createElement('a')
    a.href = URL.createObjectURL(blob)
    a.download = `tax-dollar-${props.receipt}.csv`
    document.body.appendChild(a)
    a.click()
    a.remove()
    URL.revokeObjectURL(a.href)
  } catch { /* network error — the link still works */ }
}

watch(() => props.open, (o) => { if (o) copied.value = false })

async function copy() {
  const text = url.value
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(text)
    } else {
      // Non-secure origin (plain-HTTP LAN): clipboard API is unavailable.
      inputEl.value?.select()
      document.execCommand('copy')
    }
    copied.value = true
    setTimeout(() => { copied.value = false }, 2500)
  } catch {
    // As a last resort, the field is selected so the user can copy manually.
    inputEl.value?.select()
  }
}
</script>

<template>
  <div v-if="open" class="overlay" @click.self="emit('close')">
    <div class="dialog">
      <h3 class="r-title">Submission saved ✓</h3>
      <p class="r-hint">
        This link is the only way to find your submission later — copy and keep
        it somewhere safe. Anyone with the link can view this submission.
      </p>
      <div class="r-row">
        <input ref="inputEl" class="r-in" :value="url" readonly @focus="inputEl?.select()" />
        <button class="r-copy" @click="copy">{{ copied ? 'Copied ✓' : 'Copy' }}</button>
      </div>
      <p class="r-token">Receipt: <span class="r-mono">{{ receipt }}</span></p>
      <div class="r-actions">
        <button class="r-dl" @click="download">⬇ Download CSV</button>
        <button class="r-ok" @click="emit('close')">Done</button>
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
  width: 100%; max-width: 400px; background: #0f172a;
  border: 1px solid #334155; border-radius: 12px; padding: 18px;
  box-shadow: 0 20px 50px rgba(0,0,0,0.5);
}
.r-title { font-size: 18px; color: #34d399; margin-bottom: 6px; }
.r-hint { font-size: 13px; color: #94a3b8; line-height: 1.5; margin-bottom: 12px; }
.r-row { display: flex; gap: 6px; }
.r-in {
  flex: 1; min-width: 0; background: #1e293b; border: 1px solid #334155; color: #e2e8f0;
  padding: 10px 12px; border-radius: 8px; font-size: 14px; font-family: ui-monospace, monospace; outline: none;
}
.r-in:focus { border-color: #3b82f6; }
.r-copy { background: #2563eb; border: 1px solid #2563eb; color: #fff; padding: 8px 14px; border-radius: 8px; font-weight: 600; cursor: pointer; white-space: nowrap; }
.r-copy:hover { background: #1d4ed8; }
.r-token { font-size: 11px; color: #64748b; margin-top: 8px; }
.r-mono { font-family: ui-monospace, monospace; color: #94a3b8; }
.r-actions { display: flex; justify-content: space-between; gap: 8px; margin-top: 12px; }
.r-dl { background: #1e293b; border: 1px solid #334155; color: #cbd5e1; padding: 8px 14px; border-radius: 8px; cursor: pointer; }
.r-dl:hover { background: #334155; color: #e2e8f0; }
.r-ok { background: #1e293b; border: 1px solid #334155; color: #cbd5e1; padding: 8px 16px; border-radius: 8px; cursor: pointer; }
.r-ok:hover { background: #334155; }
</style>
