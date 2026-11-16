// node --test scripts/whats-new.test.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { LUA, SOURCE, TS, parse, refusal, toLua, toTs } from "./whats-new.mjs";

const note = (version, items) => ({ version, headline: "A headline.", items });
const text = (notes) => JSON.stringify(notes);

test("the repository's notes are well formed and both copies are current", () => {
  const notes = parse(readFileSync(SOURCE, "utf8"));
  assert.equal(readFileSync(LUA, "utf8").replace(/\r\n/g, "\n"), toLua(notes));
  assert.equal(readFileSync(TS, "utf8").replace(/\r\n/g, "\n"), toTs(notes));
});

test("a note needs a version, a headline, items, and a known place for each", () => {
  const ok = [{ where: "core", text: "A change." }];
  assert.throws(() => parse(text([note("soon", ok)])), /version/);
  assert.throws(() => parse(text([{ version: "1.0.0", headline: " ", items: ok }])), /headline/);
  assert.throws(() => parse(text([note("1.0.0", [])])), /items/);
  assert.throws(() => parse(text([note("1.0.0", [{ where: "elsewhere", text: "x" }])])), /where/);
  assert.throws(() => parse(text([note("1.0.0", [{ where: "core", text: "" }])])), /text/);
  assert.throws(() => parse(text([note("1.0.0", ok), note("1.0.0", ok)])), /second note/);
  assert.throws(() => parse(text([note("1.0.0", ok), note("next", ok)])), /first/);
  assert.equal(parse(text([note("next", ok), note("1.0.0", ok)])).length, 2);
});

test("the game's copy leaves out what the game doesn't show; the app's keeps it all", () => {
  const notes = [
    note("1.1.0", [{ where: "meter", new: true, text: 'Say "hi" · twice' }, { where: "app", text: "App only." }]),
    note("1.0.0", [{ where: "palworld", text: "Not the game's." }]),
  ];
  const lua = toLua(notes);
  assert.match(lua, /version = "1\.1\.0"/);
  assert.match(lua, /\{ where = "meter", new = true, text = "Say \\"hi\\" · twice" \}/);
  assert.doesNotMatch(lua, /App only|1\.0\.0/);
  const ts = toTs(notes);
  assert.match(ts, /App only\./);
  assert.match(ts, /"version": "1\.0\.0"/);
});

test("a release is refused without its note, or with one still marked next", () => {
  const ok = [{ where: "core", text: "A change." }];
  assert.match(refusal([note("next", ok)], "1.0.0"), /"next"/);
  assert.match(refusal([note("0.9.0", ok)], "1.0.0"), /no note for 1\.0\.0/);
  assert.equal(refusal([note("1.0.0", ok), note("0.9.0", ok)], "1.0.0"), null);
});
