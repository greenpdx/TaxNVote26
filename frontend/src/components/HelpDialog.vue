<script setup lang="ts">
defineProps<{ open: boolean }>()
const emit = defineEmits<{ (e: 'close'): void }>()
</script>

<template>
  <div v-if="open" class="overlay" @click.self="emit('close')">
    <div class="dialog">
      <div class="h-head">
        <h3 class="h-title">How to build your budget</h3>
        <button class="h-x" @click="emit('close')" aria-label="Close">✕</button>
      </div>

      <section class="h-sec">
        <h4>Adjusting — biggest first, then lock</h4>
        <p>
          Move a slider to set a category's share. The rest of the budget is
          redistributed proportionally across the other <em>unlocked</em>
          categories, so the total always stays at 100%.
        </p>
        <p>
          Work from most important to least: set the biggest category first —
          usually <b>Defense</b>, the largest — then press <b>🔒 Lock</b> so later
          changes don't move it. Set the next most important, lock it, and so on.
          Locked categories are never touched when you adjust the others.
        </p>
      </section>

      <section class="h-sec">
        <h4>Simple ◇ / Full ◈</h4>
        <p>
          <b>Simple</b> shows the top-level topics (Defense, Health, …). <b>Full</b>
          lets you expand into agencies, bureaus, and accounts for fine-grained
          control. Switch any time — your allocations carry over.
        </p>
      </section>

      <section class="h-sec">
        <h4>Linear 📊 / Log</h4>
        <p>
          This only changes how the bars are <em>drawn</em>, not the numbers.
          <b>Linear</b> makes bar length proportional to dollars, so large
          categories dominate. <b>Log</b> compresses the scale so small
          categories are still visible next to big ones.
        </p>
      </section>

      <section class="h-sec">
        <h4>Reset ↺</h4>
        <p>Returns every category to the current federal budget baseline.</p>
      </section>

      <div class="h-actions">
        <button class="h-ok" @click="emit('close')">Got it</button>
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
  width: 100%; max-width: 460px; max-height: 86vh; overflow-y: auto;
  background: #0f172a; border: 1px solid #334155; border-radius: 12px; padding: 18px;
  box-shadow: 0 20px 50px rgba(0,0,0,0.5);
}
.h-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; }
.h-title { font-size: 18px; color: #f59e0b; }
.h-x { background: none; border: none; color: #64748b; font-size: 16px; cursor: pointer; padding: 4px; }
.h-x:hover { color: #e2e8f0; }
.h-sec { margin: 12px 0; }
.h-sec h4 { font-size: 14px; color: #93c5fd; margin-bottom: 4px; }
.h-sec p { font-size: 13px; color: #cbd5e1; line-height: 1.5; margin-bottom: 6px; }
.h-sec b { color: #e2e8f0; }
.h-actions { display: flex; justify-content: flex-end; margin-top: 8px; }
.h-ok { background: #2563eb; border: 1px solid #2563eb; color: #fff; padding: 8px 16px; border-radius: 8px; font-weight: 600; cursor: pointer; }
.h-ok:hover { background: #1d4ed8; }
</style>
