// node --test scripts/whats-new.test.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { SOURCE, TS, parse, refusal, toTs } from "./whats-new.mjs";

const note = (version, items) => ({ version, headline: "A headline.", items });
const text = (notes) => JSON.stringify(notes);

test("the repository's notes are well formed and the app's copy is current", () => {
  const notes = parse(readFileSync(SOURCE, "utf8"));
  assert.equal(readFileSync(TS, "utf8").replace(/\r\n/g, "\n"), toTs(notes));
});

test("a note needs a version, a headline, items, and a known place for each", () => {
  const ok = [{ where: "app", text: "A change." }];
  assert.throws(() => parse(text([note("soon", ok)])), /version/);
  assert.throws(() => parse(text([{ version: "1.0.0", headline: " ", items: ok }])), /headline/);
  assert.throws(() => parse(text([note("1.0.0", [])])), /items/);
  assert.throws(() => parse(text([note("1.0.0", [{ where: "elsewhere", text: "x" }])])), /where/);
  assert.throws(() => parse(text([note("1.0.0", [{ where: "app", text: "" }])])), /text/);
  assert.throws(() => parse(text([note("1.0.0", ok), note("1.0.0", ok)])), /second note/);
  assert.throws(() => parse(text([note("1.0.0", ok), note("next", ok)])), /first/);
  assert.equal(parse(text([note("next", ok), note("1.0.0", ok)])).length, 2);
});

test("the app's copy keeps every note; a note after 0.5.0 may not name an addon", () => {
  const notes = [
    note("0.5.1", [{ where: "app", text: "App only." }, { where: "settings", text: "The agent's own mod." }]),
    note("0.5.0", [{ where: "meter", new: true, text: "From when the addons shared the release." }]),
  ];
  const ts = toTs(parse(text(notes)));
  assert.match(ts, /App only\./);
  assert.match(ts, /"version": "0\.5\.0"/);
  for (const version of ["0.5.1", "0.6.0", "1.0.0", "next"]) {
    assert.throws(() => parse(text([note(version, [{ where: "core", text: "The addon's." }])])), /an addon's changes are in its own repository/);
  }
  assert.equal(parse(text([note("0.4.9", [{ where: "core", text: "Then." }])])).length, 1);
});

test("a release is refused without its note, or with one still marked next", () => {
  const ok = [{ where: "app", text: "A change." }];
  assert.match(refusal([note("next", ok)], "1.0.0"), /"next"/);
  assert.match(refusal([note("0.9.0", ok)], "1.0.0"), /no note for 1\.0\.0/);
  assert.equal(refusal([note("1.0.0", ok), note("0.9.0", ok)], "1.0.0"), null);
});
