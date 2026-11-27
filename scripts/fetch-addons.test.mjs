// node --test scripts/fetch-addons.test.mjs
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { inPlace, pins, place, readPin, unpack } from "./fetch-addons.mjs";
import { pinProblems, readVersions } from "./versions.mjs";
import { writeZip } from "./zip.mjs";

const sha = (buf) => createHash("sha256").update(buf).digest("hex");
const toc = (version) => Buffer.from(`## Title: Meter\n## Version: ${version}\n## Dependencies: Ryna_Core\nMod.lua\n`);
const meterZip = (version = "0.5.1", more = []) =>
  writeZip([
    { name: "Ryna_Meter/Ryna_Meter.toc", data: toc(version) },
    { name: "Ryna_Meter/Mod.lua", data: Buffer.from("-- mod") },
    { name: "Ryna_Meter/Locales/enUS.lua", data: Buffer.from("-- words") },
    ...more,
  ]);
const pinFor = (zip, over = {}) =>
  readPin(JSON.stringify({ folder: "Ryna_Meter", repo: "Ryna-Mods/meter", tag: "v0.5.1", sha256: sha(zip), ...over }));

test("a pin names a release of a Ryna-Mods repository and nothing else", () => {
  const zip = meterZip();
  const pin = pinFor(zip);
  assert.equal(pin.url, "https://github.com/Ryna-Mods/meter/releases/download/v0.5.1/Ryna_Meter-0.5.1.zip");
  assert.equal(pin.version, "0.5.1");
  assert.throws(() => pinFor(zip, { repo: "Someone/meter" }), /repo isn't Ryna-Mods/);
  assert.throws(() => pinFor(zip, { repo: "Ryna-Mods/meter/../../x" }), /repo isn't Ryna-Mods/);
  assert.throws(() => pinFor(zip, { tag: "main" }), /tag isn't vX\.Y\.Z/);
  assert.throws(() => pinFor(zip, { sha256: "abc" }), /sha256/);
  assert.throws(() => pinFor(zip, { folder: "../Ryna_Meter" }), /folder/);
  assert.throws(() => pinFor(zip, { asset: "../x.zip" }), /asset/);
});

test("a zip is taken only when it is the pinned file, one folder, at the pinned version", () => {
  const zip = meterZip();
  const files = unpack(zip, pinFor(zip));
  assert.deepEqual(files.map((f) => f.name).sort(), ["Locales/enUS.lua", "Mod.lua", "Ryna_Meter.toc"]);
  assert.throws(() => unpack(meterZip("0.5.2"), pinFor(zip)), /isn't the pinned file/);
  const older = meterZip("0.5.0");
  assert.throws(() => unpack(older, pinFor(older)), /TOC says 0\.5\.0, the tag says 0\.5\.1/);
  const stray = meterZip("0.5.1", [{ name: "Ryna_Core/Core.lua", data: Buffer.from("-- not ours") }]);
  assert.throws(() => unpack(stray, pinFor(stray)), /isn't in Ryna_Meter\//);
  const out = meterZip("0.5.1", [{ name: "Ryna_Meter/../outside.lua", data: Buffer.from("-- out") }]);
  assert.throws(() => unpack(out, pinFor(out)), /isn't in Ryna_Meter\//);
});

/** A repository root with one pinned mod, its folder not fetched yet. */
function repo(zip, manifest = {}) {
  const root = mkdtempSync(join(tmpdir(), "ryna-pins-"));
  const mod = join(root, "mods", "wow", "meter");
  mkdirSync(mod, { recursive: true });
  writeFileSync(join(mod, "mod.json"), JSON.stringify({ id: "meter", version: "0.5.1", addons: ["Ryna_Meter"], ...manifest }));
  writeFileSync(join(mod, "pin.json"), JSON.stringify({ folder: "Ryna_Meter", repo: "Ryna-Mods/meter", tag: "v0.5.1", sha256: sha(zip) }));
  return root;
}

test("a fetched folder is in place until the pin moves", () => {
  const zip = meterZip();
  const root = repo(zip);
  const [p] = pins(root);
  assert.equal(inPlace(p), false);
  place(p, unpack(zip, p.pin));
  assert.equal(inPlace(p), true);
  assert.equal(readFileSync(join(p.dir, "Locales", "enUS.lua"), "utf8"), "-- words");
  assert.equal(existsSync(join(root, "mods", "wow", "meter", "addon", ".new-Ryna_Meter")), false);
  // A second fetch replaces the folder whole: nothing of the old one is left behind.
  writeFileSync(join(p.dir, "Old.lua"), "-- from before");
  place(p, unpack(zip, p.pin));
  assert.equal(existsSync(join(p.dir, "Old.lua")), false);
  const next = meterZip("0.5.1", [{ name: "Ryna_Meter/New.lua", data: Buffer.from("-- new") }]);
  writeFileSync(join(root, "mods", "wow", "meter", "pin.json"), JSON.stringify({ folder: "Ryna_Meter", repo: "Ryna-Mods/meter", tag: "v0.5.1", sha256: sha(next) }));
  assert.equal(inPlace(pins(root)[0]), false);
});

test("a pinned mod is held to its pin, not to the release's version", () => {
  const zip = meterZip();
  const root = repo(zip);
  assert.deepEqual(pinProblems(root), []);
  const [p] = pins(root);
  place(p, unpack(zip, p.pin));
  assert.deepEqual(pinProblems(root), []);
  writeFileSync(join(p.dir, "Ryna_Meter.toc"), toc("0.5.0"));
  assert.match(pinProblems(root)[0], /Ryna_Meter\.toc says 0\.5\.0, its pin's tag is v0\.5\.1/);
  assert.match(pinProblems(repo(zip, { version: "0.5.0" }))[0], /mod\.json says 0\.5\.0, its pin's tag is v0\.5\.1/);
  assert.match(pinProblems(repo(zip, { addons: ["Ryna_Other"] }))[0], /addons aren't its pin's folder/);
});

/** A repository root with a pinned core, its folder not fetched yet. */
function coreRepo(zip, manifest = { api: 2, apiMin: 2 }) {
  const root = mkdtempSync(join(tmpdir(), "ryna-pins-"));
  mkdirSync(join(root, "cores", "wow", "addon"), { recursive: true });
  writeFileSync(join(root, "cores", "wow", "core.json"), JSON.stringify({ id: "wow", version: "0.6.0", ...manifest }));
  writeFileSync(join(root, "cores", "wow", "addon", "pin.json"), JSON.stringify({ folder: "Ryna_Core", repo: "Ryna-Mods/core", tag: "v0.5.1", sha256: sha(zip) }));
  return root;
}
const coreZip = (version = "0.5.1", api = 2, apiMin = 2) =>
  writeZip([
    { name: "Ryna_Core/Ryna_Core.toc", data: Buffer.from(`## Title: Ryna Core\n## Version: ${version}\n## X-Ryna-API: ${api}\n## X-Ryna-API-Min: ${apiMin}\nCore.lua\n`) },
    { name: "Ryna_Core/Core.lua", data: Buffer.from("-- core") },
  ]);

test("the core is pinned beside where its folder goes, and only as Ryna_Core", () => {
  const zip = coreZip();
  const root = coreRepo(zip);
  const [p] = pins(root);
  assert.deepEqual([p.id, p.core, p.pin.url], ["core", true, "https://github.com/Ryna-Mods/core/releases/download/v0.5.1/Ryna_Core-0.5.1.zip"]);
  assert.equal(p.dir, join(root, "cores", "wow", "addon", "Ryna_Core"));
  place(p, unpack(zip, p.pin));
  assert.equal(inPlace(p), true);
  assert.equal(existsSync(join(root, "cores", "wow", "addon", ".fetched")), true);
  writeFileSync(join(root, "cores", "wow", "addon", "pin.json"), JSON.stringify({ folder: "Ryna_Meter", repo: "Ryna-Mods/meter", tag: "v0.5.1", sha256: sha(zip) }));
  assert.throws(() => pins(root), /the core's folder is Ryna_Core/);
  const meter = meterZip();
  const modRoot = repo(meter);
  writeFileSync(join(modRoot, "mods", "wow", "meter", "pin.json"), JSON.stringify({ folder: "Ryna_Core", repo: "Ryna-Mods/core", tag: "v0.5.1", sha256: sha(meter) }));
  assert.throws(() => pins(modRoot), /Ryna_Core is the core's folder/);
});

test("the pinned core has its own version, and the API levels core.json names", () => {
  const zip = coreZip();
  const root = coreRepo(zip);
  assert.deepEqual(pinProblems(root), [], "nothing to hold before the folder is fetched");
  const [p] = pins(root);
  place(p, unpack(zip, p.pin));
  assert.deepEqual(pinProblems(root), [], "its version (0.5.1) needn't be the release's (0.6.0)");
  const raised = coreZip("0.5.1", 3, 2);
  const off = coreRepo(raised);
  place(pins(off)[0], unpack(raised, pins(off)[0].pin));
  assert.match(pinProblems(off)[0], /core\.json's api is 2, the pinned core's X-Ryna-API is 3/);
  const lowest = coreRepo(zip, { api: 2, apiMin: 1 });
  place(pins(lowest)[0], unpack(zip, pins(lowest)[0].pin));
  assert.match(pinProblems(lowest)[0], /core\.json's apiMin is 1, the pinned core's X-Ryna-API-Min is 2/);
});

test("this repository's pinned addons are out of the release's versions and agree with their pins", () => {
  const root = join(import.meta.dirname, "..");
  const found = readVersions(root);
  for (const { id, core } of pins(root)) {
    const under = core ? "cores/wow/addon/" : `mods/wow/${id}/`;
    assert.ok(!Object.keys(found).some((k) => k.startsWith(under)), id);
  }
  // Every addon is in a repository of its own; the Settings mod runs in the agent and has no folder.
  assert.deepEqual(pins(root).map((p) => p.id), ["core", "automation", "chronicle", "cleanse", "fights", "guild", "meter", "prices"]);
  assert.deepEqual(pinProblems(root), []);
});
