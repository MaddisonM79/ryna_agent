#!/usr/bin/env node
// What's new: the note a person sees once in the app after an update (a dialog at first launch).
// The words are written by hand in whats-new.json at the repository root, newest release first; this
// script makes the app's copy from it (no dependencies; Node 18+). From the repository root:
//
//   node scripts/whats-new.mjs                  write the app's copy
//   node scripts/whats-new.mjs --check          exit 1 when it isn't what the file makes (CI)
//   node scripts/whats-new.mjs --require 0.5.1  exit 1 unless that release has a note and none is
//                                               still marked "next" (the release workflow)
//
// A note: { version, headline, items: [{ where, text, new? }] }. `version` is the release's ("0.5.1"),
// or "next" while the release has no number yet: rename it in the change that sets the version.
// `where` says whose change it is: "app", "settings" (the agent's own mod), or another core's id.
// The addons' changes are no longer written here (Maddison, 2026-11-18): each addon has a version
// and a repository of its own, and says what's new in the game, from its own notes (Ryna Core's
// whats-new.json makes the popup). The notes up to 0.5.0, from when every addon shared the app's
// release, keep their addon lines; a newer note that names an addon is refused.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const SOURCE = join(ROOT, "whats-new.json");
export const TS = join(ROOT, "agent", "app", "src", "whatsNewData.ts");

// Where a change can be, and whether it is an addon's (true): only the notes up to LAST_SHARED,
// the last release every addon shared with the app, may still name one.
export const WHERE = {
  core: true, chronicle: true, automation: true, prices: true, fights: true, meter: true,
  settings: false, app: false, palworld: false, satisfactory: false,
};
export const LAST_SHARED = "0.5.0";
const newer = (version) => {
  if (version === "next") return true;
  const [a, b] = [version, LAST_SHARED].map((v) => v.split(".").map(Number));
  return a[0] !== b[0] ? a[0] > b[0] : a[1] !== b[1] ? a[1] > b[1] : a[2] > b[2];
};
const VERSION = /^\d+\.\d+\.\d+$/;

/** The notes, checked. Throws an Error naming the first thing wrong. */
export function parse(text) {
  const notes = JSON.parse(text);
  if (!Array.isArray(notes)) throw new Error("whats-new.json must be a list of notes");
  const seen = new Set();
  notes.forEach((n, i) => {
    const at = `note ${i + 1}`;
    if (typeof n.version !== "string" || !(n.version === "next" || VERSION.test(n.version))) {
      throw new Error(`${at}: version must be a release number or "next"`);
    }
    if (seen.has(n.version)) throw new Error(`${at}: a second note for ${n.version}`);
    seen.add(n.version);
    if (n.version === "next" && i !== 0) throw new Error(`${at}: "next" must be the first note`);
    if (typeof n.headline !== "string" || !n.headline.trim()) throw new Error(`${at}: no headline`);
    if (!Array.isArray(n.items) || n.items.length === 0) throw new Error(`${at}: no items`);
    n.items.forEach((it, j) => {
      const here = `${at}, item ${j + 1}`;
      if (!(it.where in WHERE)) throw new Error(`${here}: unknown "where" ${JSON.stringify(it.where)}`);
      if (WHERE[it.where] && newer(n.version)) {
        throw new Error(`${here}: "${it.where}" is an addon, and an addon's changes are in its own repository's notes since ${LAST_SHARED}`);
      }
      if (typeof it.text !== "string" || !it.text.trim()) throw new Error(`${here}: no text`);
      if (it.new !== undefined && typeof it.new !== "boolean") throw new Error(`${here}: "new" must be true or false`);
    });
  });
  return notes;
}

/** The app's copy: every note, typed. */
export function toTs(notes) {
  return [
    "// The notes the app shows after an update. Written by scripts/whats-new.mjs from whats-new.json at",
    "// the repository root: change that file and run the script, never this one.",
    "",
    "export interface NewsItem {",
    "  where: string;",
    "  text: string;",
    "  new?: boolean;",
    "}",
    "",
    "export interface NewsNote {",
    "  version: string;",
    "  headline: string;",
    "  items: NewsItem[];",
    "}",
    "",
    `export const NEWS: NewsNote[] = ${JSON.stringify(notes, null, 2)};`,
    "",
  ].join("\n");
}

/** Why a release can't go out with these notes, or null. */
export function refusal(notes, version) {
  if (notes.some((n) => n.version === "next")) {
    return `whats-new.json still has a note marked "next": name it ${version} (or remove it).`;
  }
  if (!notes.some((n) => n.version === version)) {
    return `whats-new.json has no note for ${version}: write one, then run node scripts/whats-new.mjs.`;
  }
  return null;
}

function main(argv) {
  const notes = parse(readFileSync(SOURCE, "utf8"));
  const made = [[TS, toTs(notes)]];
  const need = argv.indexOf("--require");
  if (need >= 0) {
    const why = refusal(notes, argv[need + 1] ?? "");
    if (why) {
      console.error(why);
      return 1;
    }
  }
  if (argv.includes("--check") || need >= 0) {
    let stale = false;
    for (const [path, text] of made) {
      let have = "";
      try { have = readFileSync(path, "utf8"); } catch { /* missing counts as stale */ }
      if (have.replace(/\r\n/g, "\n") !== text) {
        console.error(`${path} isn't what whats-new.json makes: run node scripts/whats-new.mjs`);
        stale = true;
      }
    }
    if (stale) return 1;
    console.log(`whats-new: ${notes.length} note${notes.length === 1 ? "" : "s"}, the app's copy current`);
    return 0;
  }
  for (const [path, text] of made) writeFileSync(path, text);
  console.log(`whats-new: wrote the app's copy of ${notes.length} note${notes.length === 1 ? "" : "s"}`);
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    process.exit(main(process.argv.slice(2)));
  } catch (e) {
    console.error(`whats-new: ${e.message}`);
    process.exit(1);
  }
}
