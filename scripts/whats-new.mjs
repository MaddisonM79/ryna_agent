#!/usr/bin/env node
// What's new: the note a player sees once after an update, in the game (a popup at login) and in the
// app (a dialog at first launch). The words are written by hand in whats-new.json at the repository
// root, newest release first; this script makes the two copies from it, so they can't disagree
// (no dependencies; Node 18+). From the repository root:
//
//   node scripts/whats-new.mjs                  write the two copies
//   node scripts/whats-new.mjs --check          exit 1 when a copy isn't what the file makes (CI)
//   node scripts/whats-new.mjs --require 0.5.0  exit 1 unless that release has a note and none is
//                                               still marked "next" (the release workflow)
//
// A note: { version, headline, items: [{ where, text, new? }] }. `version` is the release's ("0.5.0"),
// or "next" while the release has no number yet: rename it in the change that sets the version.
// `where` says whose change it is: "core" (the addon itself), a WoW mod's id, "app", or another core's
// id; the game shows the core's and the mods', the app shows them all. `new` marks the item that
// introduces a mod, so the note can offer it to someone who doesn't have it on.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const SOURCE = join(ROOT, "whats-new.json");
export const LUA = join(ROOT, "cores", "wow", "addon", "Ryna_Core", "WhatsNew.lua");
export const TS = join(ROOT, "agent", "app", "src", "whatsNewData.ts");

// Where a change can be, and whether the game shows it.
export const WHERE = {
  core: true, chronicle: true, automation: true, prices: true, fights: true, meter: true,
  settings: false, app: false, palworld: false, satisfactory: false,
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
      if (typeof it.text !== "string" || !it.text.trim()) throw new Error(`${here}: no text`);
      if (it.new !== undefined && typeof it.new !== "boolean") throw new Error(`${here}: "new" must be true or false`);
    });
  });
  return notes;
}

const luaString = (s) => JSON.stringify(s).replace(/\\u([0-9a-f]{4})/gi, (_, h) => String.fromCodePoint(parseInt(h, 16)));

/** The game's copy: the notes with something the game shows, as a Lua table on the core's namespace. */
export function toLua(notes) {
  const out = [
    "-- WhatsNew: the notes News.lua shows after an update. Written by scripts/whats-new.mjs from",
    "-- whats-new.json at the repository root: change that file and run the script, never this one.",
    "-- luacheck: ignore 631",
    "",
    "local _, ns = ...",
    "",
    "ns.NEWS = {",
  ];
  for (const n of notes) {
    const items = n.items.filter((it) => WHERE[it.where]);
    if (items.length === 0) continue;
    out.push("  {", `    version = ${luaString(n.version)},`, `    headline = ${luaString(n.headline)},`, "    items = {");
    for (const it of items) {
      out.push(`      { where = ${luaString(it.where)},${it.new ? " new = true," : ""} text = ${luaString(it.text)} },`);
    }
    out.push("    },", "  },");
  }
  out.push("}", "");
  return out.join("\n");
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
  const made = [[LUA, toLua(notes)], [TS, toTs(notes)]];
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
    console.log(`whats-new: ${notes.length} note${notes.length === 1 ? "" : "s"}, both copies current`);
    return 0;
  }
  for (const [path, text] of made) writeFileSync(path, text);
  console.log(`whats-new: wrote ${made.length} copies of ${notes.length} note${notes.length === 1 ? "" : "s"}`);
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
