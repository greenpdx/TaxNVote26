<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { useSessionStore } from '../stores/session'
import * as api from '../api'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ (e: 'close'): void; (e: 'success'): void }>()

const session = useSessionStore()

type Mode = 'signin' | 'signup' | 'verify' | 'demo'
const mode = ref<Mode>('signin')

const email = ref('')
const password = ref('')
const username = ref('')
const code = ref('')
const demoName = ref('')
const demoPin = ref('')

const localErr = ref<string | null>(null)
const note = ref<string | null>(null)
const signupBusy = ref(false)
const powMsg = ref('')

const msg = (e: unknown) => (e instanceof Error ? e.message : String(e))
const err = computed(() => localErr.value || session.error)
const busy = computed(() => session.busy || signupBusy.value)

watch(() => props.open, (o) => {
  if (o) {
    mode.value = 'signin'
    email.value = ''; password.value = ''; username.value = ''; code.value = ''
    demoName.value = ''; demoPin.value = ''
    localErr.value = null; note.value = null; powMsg.value = ''
    session.error = null
  }
})
function setMode(m: Mode) {
  mode.value = m
  localErr.value = null; note.value = null; session.error = null
}

async function doSignin() {
  if (await session.loginEmail(email.value.trim(), password.value)) done()
}

async function doSignup() {
  localErr.value = null; note.value = null
  if (username.value.trim().length < 3) { localErr.value = 'Username must be at least 3 characters.'; return }
  if (!email.value.includes('@')) { localErr.value = 'Enter a valid email.'; return }
  if (password.value.length < 8) { localErr.value = 'Password must be at least 8 characters.'; return }
  signupBusy.value = true
  powMsg.value = 'Preparing…'
  try {
    const ch = await api.getChallenge()
    powMsg.value = 'Solving proof-of-work…'
    const nonce = await api.solvePow(ch.challenge, ch.difficulty, (tried) => {
      powMsg.value = `Solving proof-of-work… ${tried.toLocaleString()} tries`
    })
    powMsg.value = 'Creating your account…'
    await api.register(username.value.trim(), email.value.trim(), password.value, ch.challenge, nonce)
    setMode('verify')
    note.value = 'We emailed you a 6-digit code. Enter it to finish.'
  } catch (e) {
    localErr.value = msg(e)
  } finally {
    signupBusy.value = false; powMsg.value = ''
  }
}

async function doVerify() {
  if (await session.verifyEmail(email.value.trim(), code.value.trim())) done()
}

async function doDemo() {
  if (await session.identify(demoName.value.trim(), demoPin.value.trim())) done()
}

function done() { emit('success'); emit('close') }
</script>

<template>
  <div v-if="open" class="overlay" @click.self="emit('close')">
    <div class="dialog">
      <!-- Tabs (hidden during the verify step) -->
      <div v-if="mode !== 'verify'" class="auth-tabs">
        <button :class="{ on: mode === 'signin' }" @click="setMode('signin')">Sign in</button>
        <button :class="{ on: mode === 'signup' }" @click="setMode('signup')">Sign up</button>
      </div>

      <!-- Sign in -->
      <template v-if="mode === 'signin'">
        <p class="d-hint">Sign in with your email and password.</p>
        <input class="d-in" v-model="email" type="email" placeholder="Email" autocomplete="username" @keyup.enter="doSignin" />
        <input class="d-in" v-model="password" type="password" placeholder="Password" autocomplete="current-password" @keyup.enter="doSignin" />
        <div v-if="err" class="d-err">{{ err }}</div>
        <div class="d-actions">
          <button class="d-cancel" @click="emit('close')">Cancel</button>
          <button class="d-login" :disabled="busy" @click="doSignin">{{ busy ? '…' : 'Sign in' }}</button>
        </div>
      </template>

      <!-- Sign up -->
      <template v-else-if="mode === 'signup'">
        <p class="d-hint">Create an account. We'll email you a code to confirm.</p>
        <input class="d-in" v-model="username" placeholder="Username" maxlength="32" autocomplete="username" />
        <input class="d-in" v-model="email" type="email" placeholder="Email" maxlength="254" autocomplete="email" />
        <input class="d-in" v-model="password" type="password" placeholder="Password (8+ chars)" maxlength="128" autocomplete="new-password" />
        <div v-if="powMsg" class="d-busy">{{ powMsg }}</div>
        <div v-if="err" class="d-err">{{ err }}</div>
        <div class="d-actions">
          <button class="d-cancel" @click="emit('close')">Cancel</button>
          <button class="d-login" :disabled="busy" @click="doSignup">{{ signupBusy ? '…' : 'Create account' }}</button>
        </div>
      </template>

      <!-- Verify code -->
      <template v-else-if="mode === 'verify'">
        <h3 class="d-title">Confirm your email</h3>
        <p class="d-hint">{{ note || 'Enter the 6-digit code we emailed you.' }}</p>
        <input class="d-in" v-model="code" placeholder="6-digit code" inputmode="numeric" maxlength="6" @keyup.enter="doVerify" />
        <div v-if="err" class="d-err">{{ err }}</div>
        <div class="d-actions">
          <button class="d-cancel" @click="setMode('signup')">Back</button>
          <button class="d-login" :disabled="busy" @click="doVerify">{{ busy ? '…' : 'Confirm' }}</button>
        </div>
      </template>

      <!-- Demo identity -->
      <template v-else>
        <h3 class="d-title">Quick demo identity</h3>
        <p class="d-hint">No account — a name + 4-digit number. Reuse the same pair to find your data.</p>
        <input class="d-in" v-model="demoName" placeholder="Name" maxlength="64" @keyup.enter="doDemo" />
        <input class="d-in" v-model="demoPin" placeholder="4-digit number" inputmode="numeric" maxlength="4" @keyup.enter="doDemo" />
        <div v-if="err" class="d-err">{{ err }}</div>
        <div class="d-actions">
          <button class="d-cancel" @click="setMode('signin')">Back</button>
          <button class="d-login" :disabled="busy" @click="doDemo">{{ busy ? '…' : 'Continue' }}</button>
        </div>
      </template>

      <div v-if="mode === 'signin' || mode === 'signup'" class="d-foot">
        <a href="#" @click.prevent="setMode('demo')">Use a quick demo identity instead</a>
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
.auth-tabs { display: flex; gap: 4px; margin-bottom: 12px; }
.auth-tabs button { flex: 1; background: #1e293b; border: 1px solid #334155; color: #94a3b8; padding: 7px; border-radius: 8px; font-size: 14px; cursor: pointer; }
.auth-tabs button.on { background: #3b82f6; border-color: #3b82f6; color: #fff; font-weight: 600; }
.d-title { font-size: 18px; color: #f59e0b; margin-bottom: 4px; }
.d-hint { font-size: 12px; color: #64748b; margin-bottom: 12px; }
.d-in {
  width: 100%; background: #1e293b; border: 1px solid #334155; color: #e2e8f0;
  padding: 10px 12px; border-radius: 8px; font-size: 15px; outline: none; margin-bottom: 8px;
}
.d-in:focus { border-color: #3b82f6; }
.d-err { color: #f87171; font-size: 13px; margin-bottom: 8px; }
.d-busy { color: #60a5fa; font-size: 13px; margin-bottom: 8px; }
.d-actions { display: flex; gap: 8px; justify-content: flex-end; margin-top: 4px; }
.d-cancel { background: #1e293b; border: 1px solid #334155; color: #94a3b8; padding: 8px 14px; border-radius: 8px; cursor: pointer; }
.d-cancel:hover { background: #334155; color: #e2e8f0; }
.d-login { background: #2563eb; border: 1px solid #2563eb; color: #fff; padding: 8px 16px; border-radius: 8px; font-weight: 600; cursor: pointer; }
.d-login:hover { background: #1d4ed8; }
.d-login:disabled { opacity: 0.5; cursor: not-allowed; }
.d-foot { margin-top: 12px; text-align: center; }
.d-foot a { color: #64748b; font-size: 12px; text-decoration: none; }
.d-foot a:hover { color: #94a3b8; text-decoration: underline; }
</style>
