<script setup lang="ts">
// What's new, once after an update (whatsNew.ts): the release's note in a dialog over the window.
import { computed, nextTick, ref, watch } from "vue";
import { status } from "../store";
import { windowStorage } from "../tabs";
import { closeNews, groups, openNote } from "../whatsNew";

const shown = computed(() => openNote.value);
const list = computed(() =>
  shown.value ? groups(shown.value.note, status.value?.cores.map((c) => c.id) ?? []) : [],
);
/** A new mod the note introduces: where to turn it on is said under the list. */
const hasNewMod = computed(() => list.value.some((g) => g.label.endsWith("new mod")));
const done = ref<HTMLButtonElement | null>(null);

function close(): void {
  closeNews(windowStorage());
}

// The button takes the focus when the dialog opens, so Enter and Escape both close it.
watch(shown, async (now) => {
  if (!now) return;
  await nextTick();
  done.value?.focus();
});
</script>

<template>
  <div v-if="shown" class="news-shade" @click.self="close" @keydown.esc="close">
    <section class="news" role="dialog" aria-modal="true" aria-labelledby="news-title">
      <h2 id="news-title">What's new in Ryna {{ shown.version }}</h2>
      <p class="muted">{{ shown.note.headline }}</p>
      <div class="news-list">
        <div v-for="g in list" :key="g.where" class="news-group">
          <span class="news-tag">{{ g.label }}</span>
          <ul>
            <li v-for="line in g.lines" :key="line">{{ line }}</li>
          </ul>
        </div>
      </div>
      <div class="news-foot">
        <span class="muted">
          <template v-if="hasNewMod">Turn a new mod on under World of Warcraft, Mods.</template>
        </span>
        <button ref="done" class="primary" type="button" @click="close">Got it</button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.news-shade {
  position: fixed;
  inset: 0;
  z-index: 50;
  background: rgba(15, 16, 32, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
}
.news {
  width: min(480px, 100%);
  max-height: 100%;
  display: flex;
  flex-direction: column;
  gap: 10px;
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 18px 20px 16px;
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.3);
}
.news h2 {
  margin: 0;
  font-size: 1.15rem;
}
.news p {
  margin: 0;
}
.news-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow-y: auto;
  min-height: 0;
}
.news-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.news-tag {
  align-self: flex-start;
  background: var(--busy-bg);
  color: var(--busy-text);
  font-size: 0.78rem;
  font-weight: 600;
  padding: 1px 9px;
  border-radius: 999px;
}
.news ul {
  margin: 0;
  padding-left: 18px;
  display: flex;
  flex-direction: column;
  gap: 3px;
}
.news-foot {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 10px;
  margin-top: 4px;
}
</style>
