// credits.json is the one list of people thanked; the About tab imports it, and this holds
// CREDITS.md and the README to it, and the pinned core's Credits.lua to nothing beyond it. From the
// repository root: node --test scripts/
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (p) => readFileSync(join(ROOT, p), "utf8");
const credits = JSON.parse(read("credits.json"));
const CORE_CREDITS = "cores/wow/addon/Ryna_Core/Credits.lua";

/** `text` as it's written inside a double-quoted Lua string: backslashes first, then quotes. */
const luaQuoted = (text) => text.replace(/\\/g, "\\\\").replace(/"/g, '\\"');

test("a credits line is quoted for Lua the way Lua reads it back", () => {
  assert.equal(luaQuoted('say "hi"'), 'say \\"hi\\"');
  assert.equal(luaQuoted("a\\b"), "a\\\\b");
  assert.equal(luaQuoted('\\"'), '\\\\\\"');
});

test("credits.json has names, roles and lines", () => {
  assert.ok(credits.length >= 1);
  for (const c of credits) {
    assert.match(c.name, /\S/);
    assert.match(c.role, /\S/);
    assert.match(c.line, /\S/);
  }
});

test("CREDITS.md carries every name and line", () => {
  const md = read("CREDITS.md");
  for (const c of credits) {
    assert.ok(md.includes(c.name), `CREDITS.md lacks ${c.name}`);
    assert.ok(md.includes(c.line), `CREDITS.md lacks ${c.name}'s line`);
  }
});

// The addon's Credits.lua is Ryna Core's (Ryna-Mods/core), held to that repository's copy of this
// list. Here it is the pinned release's, present once fetched: a name added here reaches the game
// with the next core release, so only the lines the pinned core has are held to this list.
test("the pinned core's Credits.lua says nothing this list doesn't", { skip: !existsSync(join(ROOT, CORE_CREDITS)) }, () => {
  const lua = read(CORE_CREDITS);
  const names = [...lua.matchAll(/name = "((?:[^"\\]|\\.)*)"/g)].map((m) => m[1]);
  assert.ok(names.length >= 1, "Credits.lua names someone");
  for (const name of names) assert.ok(credits.some((c) => luaQuoted(c.name) === name), `${name} is in Credits.lua and not in credits.json`);
});

test("the README points at CREDITS.md", () => {
  assert.ok(read("README.md").includes("CREDITS.md"));
});
