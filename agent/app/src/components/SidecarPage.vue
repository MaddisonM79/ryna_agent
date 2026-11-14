<script setup lang="ts">
// A fetched core's page (Palworld): the parts the agent already has and nothing else. Its one
// state panel (ready to turn on, Getting…, Retry, not on this computer yet, stopped, off) from
// coreWords,
// the status card's rows as the core lays them out, its activity lines, and its on/off switch.
// The panel is plain `banner` markup with a class per state, so it restyles without code.
import { computed, ref } from "vue";
import { api, errorText, type CoreView } from "../api";
import { coreLines, corePanel } from "../coreWords";
import { activity, log, refresh } from "../store";

const props = defineProps<{ core: CoreView }>();

const panel = computed(() => corePanel(props.core));
const lines = computed(() => coreLines(activity.value, props.core.id));
const busy = ref(false);

function time(at: number): string {
  return new Date(at * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

async function act(): Promise<void> {
  const does = panel.value?.action?.does;
  if (!does) return;
  busy.value = true;
  try {
    if (does === "retry") {
      await api.retryCoreFetch(props.core.id);
    } else {
      // "Turn on <name>" and "Start again" are the Settings switch turned on: it fetches, then starts.
      await api.setCoreEnabled(props.core.id, true);
      log(`${props.core.name} is on on this computer.`, "info", props.core.id);
    }
  } catch (e) {
    log(errorText(e), "error", props.core.id);
  } finally {
    busy.value = false;
    await refresh();
  }
}

async function toggle(e: Event): Promise<void> {
  const on = (e.target as HTMLInputElement).checked;
  try {
    await api.setCoreEnabled(props.core.id, on);
    log(on ? `${props.core.name} is on on this computer.` : `${props.core.name} is off on this computer.`, "info", props.core.id);
  } catch (err) {
    log(errorText(err), "error", props.core.id);
  }
  await refresh();
}
</script>

<template>
  <div class="tabpanel sidecar-page" :class="`core-${core.id}`">
    <section>
      <h2>{{ core.name }}</h2>
      <div v-if="panel" class="banner core-state" :class="[panel.tone, `state-${panel.state}`]" role="status">
        <p><strong>{{ panel.title }}</strong></p>
        <p>{{ panel.text }}</p>
        <progress v-if="panel.progress !== null" :value="panel.progress" max="1">{{ Math.round(panel.progress * 100) }}%</progress>
        <p v-if="panel.action">
          <button
            type="button"
            :class="panel.action.does === 'turnOn' ? 'primary' : 'quiet'"
            :disabled="busy"
            @click="act()"
          >
            {{ panel.action.label }}
          </button>
        </p>
      </div>
      <dl v-if="core.rows.length" class="glance">
        <div v-for="row in core.rows" :key="row.label">
          <dt>{{ row.label }}</dt>
          <dd>
            <span v-if="row.chip" class="chip" :class="row.chip">{{ row.value }}</span>
            <span v-else>{{ row.value }}</span>
          </dd>
        </div>
      </dl>
      <p v-else-if="core.state.is === 'running'" class="muted">Nothing to show yet.</p>
    </section>
    <section class="activity">
      <h2 class="sub">Activity</h2>
      <p v-if="!lines.length" class="muted">Nothing yet this session.</p>
      <ol v-else class="log">
        <li v-for="line in lines" :key="line.id" :class="{ 'error-text': line.level === 'error' }">
          <time class="muted">{{ time(line.at) }}</time>
          <span>{{ line.text }}</span>
        </li>
      </ol>
    </section>
    <section v-if="core.state.is !== 'notHere'">
      <h2 class="sub">Settings</h2>
      <label class="setting">
        <input type="checkbox" :checked="core.on" @change="toggle" />
        <span>
          Sync {{ core.name }} on this computer
          <span class="muted">Off stops it here; your account keeps it.</span>
        </span>
      </label>
    </section>
  </div>
</template>
