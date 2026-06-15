<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { useSessionStore } from '../stores/session'
import * as api from '../api'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ (e: 'close'): void; (e: 'success'): void }>()

const session = useSessionStore()

// Coupon that unlocks demo identity (programmable via build env).
const DEMO_COUPON = (import.meta.env.VITE_DEMO_COUPON as string) || 'ISDC2026'

type View = 'signin' | 'signup' | 'verify'
const view = ref<View>('signin')
const step = ref<1 | 2>(1)     // sign-up step
const isDemo = ref(false)      // chosen by the coupon at step 1

const email = ref('')
const username = ref('')
const coupon = ref('')
const secret = ref('')
const secret2 = ref('')
const showSecret = ref(false)
const showSecret2 = ref(false)
const code = ref('')

const localErr = ref<string | null>(null)
const note = ref<string | null>(null)
const signupBusy = ref(false)
const powMsg = ref('')

const msg = (e: unknown) => (e instanceof Error ? e.message : String(e))
const err = computed(() => localErr.value || session.error)
const busy = computed(() => session.busy || signupBusy.value)
const couponOk = computed(() => coupon.value.trim() === DEMO_COUPON)

watch(() => props.open, (o) => {
  if (o) reset()
})
function reset() {
  view.value = 'signin'; step.value = 1; isDemo.value = false
  email.value = ''; username.value = ''; coupon.value = ''
  secret.value = ''; secret2.value = ''; code.value = ''
  showSecret.value = false; showSecret2.value = false
  localErr.value = null; note.value = null; powMsg.value = ''
  session.error = null
}
function go(v: View) {
  view.value = v; step.value = 1
  secret.value = ''; secret2.value = ''; code.value = ''
  showSecret.value = false; showSecret2.value = false
  localErr.value = null; note.value = null; session.error = null
}

async function doSignin() {
  // With a valid coupon, sign in as a demo identity (name + PIN) instead.
  if (couponOk.value) {
    if (await session.identify(email.value.trim(), secret.value.trim())) done()
  } else {
    if (await session.loginEmail(email.value.trim(), secret.value)) done()
  }
}

// Step 1 → 2: validate identity fields, decide demo vs account by the coupon.
function nextStep() {
  localErr.value = null
  isDemo.value = couponOk.value
  if (isDemo.value) {
    if (username.value.trim().length < 1) { localErr.value = 'Enter a name.'; return }
  } else {
    if (username.value.trim().length < 3) { localErr.value = 'Username must be at least 3 characters.'; return }
    if (!email.value.includes('@')) { localErr.value = 'Enter a valid email.'; return }
  }
  secret.value = ''; secret2.value = ''
  step.value = 2
}

async function finishSignup() {
  localErr.value = null
  if (secret.value !== secret2.value) {
    localErr.value = isDemo.value ? 'PINs do not match.' : 'Passwords do not match.'
    return
  }
  if (isDemo.value) {
    if (!/^\d{4}$/.test(secret.value)) { localErr.value = 'PIN must be 4 digits.'; return }
    if (await session.identify(username.value.trim(), secret.value)) done()
    return
  }
  // Account: PoW + register, then email verification.
  if (secret.value.length < 8) { localErr.value = 'Password must be at least 8 characters.'; return }
  signupBusy.value = true
  powMsg.value = 'Preparing…'
  try {
    const ch = await api.getChallenge()
    powMsg.value = 'Solving proof-of-work…'
    const nonce = await api.solvePow(ch.challenge, ch.difficulty, (tried) => {
      powMsg.value = `Solving proof-of-work… ${tried.toLocaleString()} tries`
    })
    powMsg.value = 'Creating your account…'
    await api.register(username.value.trim(), email.value.trim(), secret.value, ch.challenge, nonce)
    view.value = 'verify'
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

function done() { emit('success'); emit('close') }
</script>

<template>
  <div v-if="open" class="overlay" @click.self="emit('close')">
    <div class="dialog">
      <!-- Tabs (hidden during sign-up step 2 and the verify step) -->
      <div v-if="view !== 'verify' && !(view === 'signup' && step === 2)" class="auth-tabs">
        <button :class="{ on: view === 'signin' }" @click="go('signin')">Sign in</button>
        <button :class="{ on: view === 'signup' }" @click="go('signup')">Sign up</button>
      </div>

      <!-- Sign in -->
      <template v-if="view === 'signin'">
        <p class="d-hint">Sign in with your email and password.</p>
        <input class="d-in" v-model="email" :type="couponOk ? 'text' : 'email'" :placeholder="couponOk ? 'Name' : 'Email'" autocomplete="username" @keyup.enter="doSignin" />
        <div class="pw-wrap">
          <input class="d-in" v-model="secret" :type="showSecret ? 'text' : 'password'"
                 :inputmode="couponOk ? 'numeric' : 'text'" :maxlength="couponOk ? 4 : 128"
                 :placeholder="couponOk ? '4-digit PIN' : 'Password'" autocomplete="current-password" @keyup.enter="doSignin" />
          <button class="eye" type="button" @click="showSecret = !showSecret" :aria-label="showSecret ? 'Hide' : 'Show'">{{ showSecret ? '🙈' : '👁' }}</button>
        </div>
        <input class="d-in" v-model="coupon" placeholder="Optional coupon" maxlength="32" @keyup.enter="doSignin" />
        <div v-if="couponOk" class="d-note">Demo identity sign-in — name + 4-digit PIN.</div>
        <div v-if="err" class="d-err">{{ err }}</div>
        <div class="d-actions">
          <button class="d-cancel" @click="emit('close')">Cancel</button>
          <button class="d-login" :disabled="busy" @click="doSignin">{{ busy ? '…' : 'Sign in' }}</button>
        </div>
      </template>

      <!-- Sign up · step 1: coupon + username + email -->
      <template v-else-if="view === 'signup' && step === 1">
        <p class="d-hint">Create an account. Have a coupon? Enter it to use a demo identity.</p>
        <input class="d-in" v-model="coupon" placeholder="Optional coupon" maxlength="32" @keyup.enter="nextStep" />
        <div v-if="couponOk" class="d-note">Demo identity unlocked — no email needed.</div>
        <input class="d-in" v-model="username" :placeholder="couponOk ? 'Name' : 'Username'" maxlength="64" autocomplete="username" @keyup.enter="nextStep" />
        <input class="d-in" v-model="email" type="email" :placeholder="couponOk ? 'Email (optional)' : 'Email'" maxlength="254" autocomplete="email" @keyup.enter="nextStep" />
        <div v-if="err" class="d-err">{{ err }}</div>
        <div class="d-actions">
          <button class="d-cancel" @click="emit('close')">Cancel</button>
          <button class="d-login" @click="nextStep">Next</button>
        </div>
      </template>

      <!-- Sign up · step 2: two secrets with eye toggles -->
      <template v-else-if="view === 'signup' && step === 2">
        <h3 class="d-title">{{ isDemo ? 'Choose a 4-digit PIN' : 'Choose a password' }}</h3>
        <p class="d-hint">{{ isDemo ? 'Reuse your name + PIN later to find your data.' : 'At least 8 characters.' }}</p>
        <div class="pw-wrap">
          <input class="d-in" v-model="secret" :type="showSecret ? 'text' : 'password'"
                 :inputmode="isDemo ? 'numeric' : 'text'" :maxlength="isDemo ? 4 : 128"
                 :placeholder="isDemo ? '4-digit PIN' : 'Password'" autocomplete="new-password" />
          <button class="eye" type="button" @click="showSecret = !showSecret">{{ showSecret ? '🙈' : '👁' }}</button>
        </div>
        <div class="pw-wrap">
          <input class="d-in" v-model="secret2" :type="showSecret2 ? 'text' : 'password'"
                 :inputmode="isDemo ? 'numeric' : 'text'" :maxlength="isDemo ? 4 : 128"
                 :placeholder="isDemo ? 'Confirm PIN' : 'Confirm password'" autocomplete="new-password"
                 @keyup.enter="finishSignup" />
          <button class="eye" type="button" @click="showSecret2 = !showSecret2">{{ showSecret2 ? '🙈' : '👁' }}</button>
        </div>
        <div v-if="powMsg" class="d-busy">{{ powMsg }}</div>
        <div v-if="err" class="d-err">{{ err }}</div>
        <div class="d-actions">
          <button class="d-cancel" @click="step = 1">Back</button>
          <button class="d-login" :disabled="busy" @click="finishSignup">
            {{ signupBusy ? '…' : (isDemo ? 'Continue' : 'Create account') }}
          </button>
        </div>
      </template>

      <!-- Verify code (account sign-up) -->
      <template v-else>
        <h3 class="d-title">Confirm your email</h3>
        <p class="d-hint">{{ note || 'Enter the 6-digit code we emailed you.' }}</p>
        <input class="d-in" v-model="code" placeholder="6-digit code" inputmode="numeric" maxlength="6" @keyup.enter="doVerify" />
        <div v-if="err" class="d-err">{{ err }}</div>
        <div class="d-actions">
          <button class="d-cancel" @click="view = 'signup'; step = 2">Back</button>
          <button class="d-login" :disabled="busy" @click="doVerify">{{ busy ? '…' : 'Confirm' }}</button>
        </div>
      </template>
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
.d-note { font-size: 12px; color: #34d399; margin: -2px 0 8px; }
.d-in {
  width: 100%; background: #1e293b; border: 1px solid #334155; color: #e2e8f0;
  padding: 10px 12px; border-radius: 8px; font-size: 15px; outline: none; margin-bottom: 8px;
}
.d-in:focus { border-color: #3b82f6; }
.pw-wrap { position: relative; }
.pw-wrap .d-in { padding-right: 40px; }
.eye {
  position: absolute; right: 8px; top: 9px; background: none; border: none;
  cursor: pointer; font-size: 15px; line-height: 1; padding: 2px; opacity: 0.8;
}
.eye:hover { opacity: 1; }
.d-err { color: #f87171; font-size: 13px; margin-bottom: 8px; }
.d-busy { color: #60a5fa; font-size: 13px; margin-bottom: 8px; }
.d-actions { display: flex; gap: 8px; justify-content: flex-end; margin-top: 4px; }
.d-cancel { background: #1e293b; border: 1px solid #334155; color: #94a3b8; padding: 8px 14px; border-radius: 8px; cursor: pointer; }
.d-cancel:hover { background: #334155; color: #e2e8f0; }
.d-login { background: #2563eb; border: 1px solid #2563eb; color: #fff; padding: 8px 16px; border-radius: 8px; font-weight: 600; cursor: pointer; }
.d-login:hover { background: #1d4ed8; }
.d-login:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
