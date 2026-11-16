// What's new: the note shown once after the app updated (docs/ryna-agent-cores-mods.md 9.2). The
// notes are whatsNewData.ts's, made from whats-new.json by scripts/whats-new.mjs, the same file the
// game's popup is made from. The first launch on a version that has a note opens the dialog; "Got
// it" marks that version seen. A new install sees none, since nothing is new to it. About reopens it.
import { ref } from "vue";
import { NEWS, type NewsNote } from "./whatsNewData";
import { PAGE_KEY, type TabStorage } from "./tabs";

/** Where the last version whose note was seen is kept. */
export const SEEN_KEY = "ryna.newsSeen";

/** How a change's place is named on its tag. */
const LABELS: Record<string, string> = {
  core: "Ryna addon",
  app: "App",
  chronicle: "Chronicle",
  automation: "Automation",
  prices: "Prices",
  fights: "Fights",
  meter: "Meter",
  settings: "Settings",
  palworld: "Palworld",
  satisfactory: "Satisfactory",
};
/** The order the groups are shown in: the mods, the addon, other cores, then the app itself. */
const ORDER = ["meter", "fights", "chronicle", "prices", "automation", "settings", "core", "palworld", "satisfactory", "app"];
/** Places that belong to a core other than World of Warcraft: shown only to someone who has that core. */
const OTHER_CORES = new Set(["palworld", "satisfactory"]);

export interface NewsGroup {
  where: string;
  /** "Meter", and "Meter · new mod" when the note introduces it. */
  label: string;
  lines: string[];
}

function read(storage: TabStorage | null | undefined, key: string): string | null {
  try {
    return storage?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

function write(storage: TabStorage | null | undefined, key: string, value: string): void {
  try {
    storage?.setItem(key, value);
  } catch {
    // A blocked store: the note shows again next launch, which is better than never.
  }
}

/** The note for a version, or null. */
export function noteFor(version: string, notes: readonly NewsNote[] = NEWS): NewsNote | null {
  return notes.find((n) => n.version === version) ?? null;
}

/** The note About shows: this version's, else the newest there is. */
export function latestNote(version: string, notes: readonly NewsNote[] = NEWS): NewsNote | null {
  return noteFor(version, notes) ?? notes[0] ?? null;
}

/** A note as groups, in order, leaving out other cores' changes for someone without that core. */
export function groups(note: NewsNote, coreIds: readonly string[]): NewsGroup[] {
  const by = new Map<string, { lines: string[]; isNew: boolean }>();
  for (const item of note.items) {
    if (OTHER_CORES.has(item.where) && !coreIds.includes(item.where)) continue;
    const g = by.get(item.where) ?? { lines: [], isNew: false };
    g.lines.push(item.text);
    g.isNew = g.isNew || item.new === true;
    by.set(item.where, g);
  }
  const places = [...ORDER, ...[...by.keys()].filter((w) => !ORDER.includes(w))];
  const out: NewsGroup[] = [];
  for (const where of places) {
    const g = by.get(where);
    if (!g) continue;
    const name = LABELS[where] ?? where;
    out.push({ where, label: g.isNew ? `${name} · new mod` : name, lines: g.lines });
  }
  return out;
}

/**
 * The note to open at launch, or null: this version has one and it hasn't been seen. The first launch
 * ever (nothing of the app's in the store) marks the version seen and shows nothing.
 */
export function dueNote(
  version: string,
  storage: TabStorage | null | undefined,
  notes: readonly NewsNote[] = NEWS,
): NewsNote | null {
  const seen = read(storage, SEEN_KEY);
  if (seen === version) return null;
  if (seen === null && read(storage, PAGE_KEY) === null) {
    write(storage, SEEN_KEY, version);
    return null;
  }
  return noteFor(version, notes);
}

export function markSeen(version: string, storage: TabStorage | null | undefined): void {
  write(storage, SEEN_KEY, version);
}

/** The note the dialog is showing, and the version it is shown under. */
export const openNote = ref<{ note: NewsNote; version: string } | null>(null);

let checked = false;
/** Once per launch, when the app's version is known: open the note if it is due. */
export function checkNews(version: string, storage: TabStorage | null | undefined): void {
  if (checked || !version) return;
  checked = true;
  const note = dueNote(version, storage);
  if (note) openNote.value = { note, version };
}

/** About's link: this version's note, or the newest. False when there is none. */
export function showNews(version: string): boolean {
  const note = latestNote(version);
  if (!note) return false;
  openNote.value = { note, version };
  return true;
}

export function closeNews(storage: TabStorage | null | undefined): void {
  if (openNote.value) markSeen(openNote.value.version, storage);
  openNote.value = null;
}
