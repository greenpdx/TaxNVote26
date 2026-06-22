<script setup lang="ts">
// Landing page at `/`. The budget tool lives at `/app`.
// Section copy is admin-editable: it comes from /api/config/public (lp_* keys),
// falling back to these in-code defaults until the fetch resolves / if it fails.
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { IS_DEMO } from '../config'
import { getPublicConfig } from '../api'
import AppHeader from '../components/AppHeader.vue'
import AuthDialog from '../components/AuthDialog.vue'

const router = useRouter()
const showLogin = ref(false)
// Logging in from the landing page sends you straight into the tool.
function onLoginSuccess() {
  showLogin.value = false
  router.push('/app')
}

// Editable landing copy (defaults mirror the server's DEFAULT_LP_* constants).
const copy = ref({
  lp_kicker: 'When you file your taxes, you vote for the U.S. federal budget.',
  lp_headline: 'Decide the federal budget yourself.',
  lp_pitch:
    'Tax N Vote lets every taxpayer allocate their Tax Dollar across the federal ' +
    'budget — and turns the total into direct feedback to Congress. One taxpayer, one vote.',
  lp_why:
    "Right now, political parties are the gatekeepers to government — and it takes " +
    "money to play. Lobbyists, donors, and PACs shape what gets funded long before " +
    "the public is ever asked.\n\n" +
    "Tax N Vote routes around the gate. It's direct budget feedback to Congress: you " +
    "set the numbers, and the aggregate — the People's Budget — shows what the public " +
    "actually wants funded. No party platform, no donor class.\n\n" +
    "And in the process, people learn the budget. To spend your dollar you have to see " +
    "where it really goes — the agencies, the programs, the trade-offs. An informed " +
    "public is half the point.",
  lp_pb_intro:
    "When thousands of taxpayers each set their own priorities, the total is a budget " +
    "written by the public — not by parties or donors.",
  lp_pillars:
    "Bypasses political parties — no platform, no caucus, no whip; just your allocation.\n" +
    "Bypasses money — no lobbyists, PACs, or ad budgets move a single dollar.\n" +
    "Hard data — every submission is one taxpayer's real numbers; measured, not polled.",
  lp_pb_footer:
    "At scale, that's a credible, data-driven picture of what citizens actually want " +
    "funded — a counterweight to the appropriations process.",
})

onMounted(async () => {
  try {
    const cfg = await getPublicConfig()
    // Overlay any non-empty server values onto the defaults.
    for (const k of Object.keys(copy.value) as (keyof typeof copy.value)[]) {
      if (cfg[k]) copy.value[k] = cfg[k]
    }
  } catch { /* keep defaults if the config endpoint is unreachable */ }
})

// lp_why is multi-paragraph (blank-line separated); lp_pillars is one
// "Bold — rest" bullet per line. Rendered as text (no HTML from the DB).
const whyParagraphs = computed(() =>
  copy.value.lp_why.split(/\n\s*\n/).map(s => s.trim()).filter(Boolean))
const pillars = computed(() =>
  copy.value.lp_pillars.split('\n').map(s => s.trim()).filter(Boolean).map(line => {
    const i = line.indexOf(' — ')
    return i === -1 ? { lead: '', rest: line } : { lead: line.slice(0, i), rest: line.slice(i + 3) }
  }))

// How sign-in works differs per build variant.
const signInStep = IS_DEMO
  ? 'Pick a name and a 4-digit PIN — no email required — and submit your Tax Dollar.'
  : 'Create a free account, then submit your Tax Dollar.'

const faqs = [
  {
    q: 'What is a "Tax Dollar"?',
    a: 'A single dollar that stands in for the whole discretionary federal budget. ' +
       'You decide how to split it across agencies and programs; we scale your ' +
       'percentages back up to real budget figures.',
  },
  {
    q: 'How do you identify me?',
    a: IS_DEMO
      ? 'By a name and a 4-digit PIN you choose — there are no email accounts in ' +
        'this build. Reuse the same name + PIN later to find and update your submission.'
      : 'By an email-and-password account. Your email is only ever stored as a ' +
        'one-way hash, never in the clear.',
  },
  {
    q: 'Is my submission public?',
    a: 'Submissions are shown only in aggregate as the People’s Budget. An ' +
       'individual submission is viewable solely through its own unguessable ' +
       'receipt link, which you choose whether to share.',
  },
  {
    q: 'One person, how many votes?',
    a: 'One. Resubmitting replaces your previous Tax Dollar rather than adding a ' +
       'second — one taxpayer, one vote.',
  },
]
</script>

<template>
  <div class="app">
    <AppHeader @login="showLogin = true" />
    <AuthDialog :open="showLogin" @close="showLogin = false" @success="onLoginSuccess" />

    <main class="landing">
      <!-- Hero -->
      <section class="hero">
        <p class="kicker">{{ copy.lp_kicker }}</p>
        <h1 class="lead">{{ copy.lp_headline }}</h1>
        <p class="pitch">{{ copy.lp_pitch }}</p>
        <button class="cta" @click="router.push('/app')">Get started →</button>
      </section>

      <!-- Why -->
      <section class="section">
        <h2 class="h2">Why Tax N Vote</h2>
        <p v-for="(p, i) in whyParagraphs" :key="i" class="prose">{{ p }}</p>
      </section>

      <!-- The People's Budget -->
      <section class="section pb">
        <h2 class="h2">The People's Budget</h2>
        <p class="prose">{{ copy.lp_pb_intro }}</p>
        <ul class="pillars">
          <li v-for="(p, i) in pillars" :key="i">
            <strong v-if="p.lead">{{ p.lead }}</strong><template v-if="p.lead"> — </template>{{ p.rest }}
          </li>
        </ul>
        <p class="prose">{{ copy.lp_pb_footer }}</p>
      </section>

      <!-- How it works -->
      <section class="section">
        <h2 class="h2">How it works</h2>
        <ol class="steps">
          <li>
            <span class="step-n">1</span>
            <div>
              <h3 class="step-t">Allocate</h3>
              <p class="step-d">Drag the sliders to divide your Tax Dollar across the
                discretionary federal budget — start simple or open the full tree.</p>
            </div>
          </li>
          <li>
            <span class="step-n">2</span>
            <div>
              <h3 class="step-t">Submit</h3>
              <p class="step-d">{{ signInStep }}</p>
            </div>
          </li>
          <li>
            <span class="step-n">3</span>
            <div>
              <h3 class="step-t">See the People's Budget</h3>
              <p class="step-d">Your allocation joins everyone else's. Open Results to
                watch a public budget emerge — hard data, not a poll.</p>
            </div>
          </li>
        </ol>
      </section>

      <!-- FAQ -->
      <section class="section">
        <h2 class="h2">FAQ</h2>
        <details v-for="(f, i) in faqs" :key="i" class="faq">
          <summary>{{ f.q }}</summary>
          <p>{{ f.a }}</p>
        </details>
      </section>

      <div class="cta-foot">
        <button class="cta" @click="router.push('/app')">Allocate your Tax Dollar →</button>
      </div>
    </main>
  </div>
</template>

<style scoped>
.landing { flex: 1; padding: 8px 16px 64px; }

.hero { text-align: center; padding: 32px 0 36px; border-bottom: 1px solid #1e293b; }
.kicker { font-size: 13px; font-weight: 600; color: #f59e0b; letter-spacing: 0.02em; margin-bottom: 12px; }
.lead { font-size: 30px; font-weight: 800; color: #e2e8f0; letter-spacing: -0.02em; line-height: 1.15; }
.pitch { margin: 16px auto 0; max-width: 52ch; font-size: 15px; line-height: 1.6; color: #cbd5e1; }
.pitch strong { color: #f59e0b; }

.cta {
  display: inline-block; margin-top: 24px; background: #2563eb; border: 1px solid #2563eb;
  color: #fff; padding: 11px 22px; border-radius: 10px; font-size: 15px; font-weight: 600;
  cursor: pointer;
}
.cta:hover { background: #1d4ed8; }

.section { padding: 28px 0; border-bottom: 1px solid #1e293b; }
.h2 { font-size: 13px; text-transform: uppercase; letter-spacing: 0.08em; color: #64748b; margin-bottom: 16px; }
.prose { font-size: 15px; line-height: 1.65; color: #cbd5e1; margin-bottom: 12px; max-width: 60ch; }
.prose strong { color: #e2e8f0; }

.pb { background: #0b1220; border-radius: 12px; padding: 24px 18px; border: 1px solid #1e293b; }
.pillars { list-style: none; display: flex; flex-direction: column; gap: 10px; margin: 4px 0 14px; }
.pillars li { font-size: 14px; line-height: 1.55; color: #94a3b8; padding-left: 18px; position: relative; }
.pillars li::before { content: '▸'; position: absolute; left: 0; color: #f59e0b; }
.pillars strong { color: #e2e8f0; }
.pillars em { color: #60a5fa; font-style: normal; }

.steps { list-style: none; display: flex; flex-direction: column; gap: 18px; }
.steps li { display: flex; gap: 14px; align-items: flex-start; }
.step-n {
  flex: none; width: 28px; height: 28px; border-radius: 50%; background: #1e293b;
  border: 1px solid #334155; color: #f59e0b; font-weight: 700; font-size: 14px;
  display: flex; align-items: center; justify-content: center;
}
.step-t { font-size: 15px; color: #e2e8f0; margin-bottom: 2px; }
.step-d { font-size: 14px; line-height: 1.55; color: #94a3b8; }

.faq { border: 1px solid #1e293b; border-radius: 8px; padding: 0 14px; margin-bottom: 8px; background: #0b1220; }
.faq summary { cursor: pointer; padding: 12px 0; font-size: 14px; color: #e2e8f0; list-style: none; }
.faq summary::-webkit-details-marker { display: none; }
.faq summary::before { content: '＋'; color: #64748b; margin-right: 8px; }
.faq[open] summary::before { content: '－'; }
.faq p { padding: 0 0 14px 22px; font-size: 14px; line-height: 1.6; color: #94a3b8; }

.cta-foot { text-align: center; padding-top: 32px; }
</style>
