#!/usr/bin/env node
// Brings in the addon folders of mods kept in a repository of their own (docs/ryna-agent-cores-mods.md
// 6.2). Such a mod has `mods/wow/<id>/pin.json` beside its mod.json:
//
//   { "folder": "Ryna_Meter", "repo": "Ryna-Mods/meter", "tag": "v0.5.2", "sha256": "…" }
//
// and its addon folder (`mods/wow/<id>/addon/<folder>`, where a mod kept here has its source) is not
// committed: this script downloads the release's zip, holds it to the pinned SHA-256, and unpacks it
// there, so the app's build, the release zip and the tests find it where they always did.
//
//   node scripts/fetch-addons.mjs            fetch what is missing or pinned to something else
//   node scripts/fetch-addons.mjs --check    no network: exit 1 unless every pinned folder is in place
//
// A folder already unpacked from the pinned zip is left alone (its `.fetched` stamp says which zip),
// so a second run needs no network. No dependencies; Node 22+.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { readZip } from "./zip.mjs";

/** The most a pinned zip may be. */
export const MAX_ZIP_BYTES = 64 * 1024 * 1024;
const STAMP = ".fetched";

/** A pin, checked: only a release of a `Ryna-Mods` repository, named by a version tag and a SHA-256. */
export function readPin(json, where = "pin.json") {
  const pin = JSON.parse(json);
  const bad = (what) => new Error(`${where}: ${what}`);
  if (!/^Ryna_[A-Za-z0-9]+$/.test(pin.folder ?? "")) throw bad(`folder isn't an addon folder (${pin.folder})`);
  if (!/^Ryna-Mods\/[a-z0-9][a-z0-9-]*$/.test(pin.repo ?? "")) throw bad(`repo isn't Ryna-Mods/<name> (${pin.repo})`);
  if (!/^v\d+\.\d+\.\d+$/.test(pin.tag ?? "")) throw bad(`tag isn't vX.Y.Z (${pin.tag})`);
  if (!/^[0-9a-f]{64}$/.test(pin.sha256 ?? "")) throw bad("sha256 isn't 64 hex digits");
  const version = pin.tag.slice(1);
  const asset = pin.asset ?? `${pin.folder}-${version}.zip`;
  if (!/^[A-Za-z0-9._-]+\.zip$/.test(asset)) throw bad(`asset isn't a zip's name (${asset})`);
  return {
    folder: pin.folder,
    repo: pin.repo,
    tag: pin.tag,
    version,
    asset,
    sha256: pin.sha256,
    url: `https://github.com/${pin.repo}/releases/download/${pin.tag}/${asset}`,
  };
}

/** Every pinned mod under `root`: its id, its pin, and where its addon folder goes. */
export function pins(root) {
  const mods = join(root, "mods", "wow");
  const out = [];
  if (!existsSync(mods)) return out;
  for (const id of readdirSync(mods).sort()) {
    const file = join(mods, id, "pin.json");
    if (!existsSync(file)) continue;
    const pin = readPin(readFileSync(file, "utf8"), `mods/wow/${id}/pin.json`);
    out.push({ id, pin, dir: join(mods, id, "addon", pin.folder), stamp: join(mods, id, "addon", STAMP) });
  }
  return out;
}

/** Whether the folder in place is the pinned zip's, as far as its stamp and TOC say. */
export function inPlace({ pin, dir, stamp }) {
  if (!existsSync(join(dir, `${pin.folder}.toc`)) || !existsSync(stamp)) return false;
  try {
    return JSON.parse(readFileSync(stamp, "utf8")).sha256 === pin.sha256;
  } catch {
    return false;
  }
}

/**
 * The files of `zip`, held to `pin`: the pinned bytes, one top folder and nothing outside it, and a
 * TOC that says the pinned version. Returns `[{ name, data }]` with names relative to the folder.
 */
export function unpack(zip, pin) {
  const got = createHash("sha256").update(zip).digest("hex");
  if (got !== pin.sha256) throw new Error(`${pin.asset} isn't the pinned file: sha256 ${got}, pinned ${pin.sha256}`);
  const files = [];
  for (const e of readZip(zip)) {
    const parts = e.name.split("/");
    if (parts[0] !== pin.folder || e.name.includes("\\") || parts.some((p) => p === ".." || p === ".")) {
      throw new Error(`${pin.asset} holds ${e.name}, which isn't in ${pin.folder}/`);
    }
    if (e.dir) continue;
    if (parts.slice(1).some((p) => p === "")) throw new Error(`${pin.asset} holds an unusable name (${e.name})`);
    files.push({ name: parts.slice(1).join("/"), data: e.data });
  }
  const toc = files.find((f) => f.name === `${pin.folder}.toc`);
  if (!toc) throw new Error(`${pin.asset} has no ${pin.folder}/${pin.folder}.toc`);
  const version = /^##\s*Version:\s*(\S+)\s*$/m.exec(toc.data.toString("utf8"))?.[1];
  if (version !== pin.version) throw new Error(`${pin.asset}'s TOC says ${version}, the tag says ${pin.version}`);
  return files;
}

/** Put `files` at `dir`, whole: written beside it first, then swapped in, the stamp last. */
export function place({ pin, dir, stamp }, files) {
  const parent = dirname(dir);
  const fresh = join(parent, `.new-${pin.folder}`);
  rmSync(fresh, { recursive: true, force: true });
  for (const f of files) {
    const to = join(fresh, ...f.name.split("/"));
    mkdirSync(dirname(to), { recursive: true });
    writeFileSync(to, f.data);
  }
  rmSync(stamp, { force: true });
  rmSync(dir, { recursive: true, force: true });
  renameSync(fresh, dir);
  writeFileSync(stamp, `${JSON.stringify({ repo: pin.repo, tag: pin.tag, sha256: pin.sha256 }, null, 2)}\n`);
}

async function download(url) {
  const res = await fetch(url, { redirect: "follow", headers: { "user-agent": "ryna-fetch-addons" } });
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  const size = Number(res.headers.get("content-length") ?? 0);
  if (size > MAX_ZIP_BYTES) throw new Error(`${url} is ${size} bytes, more than a pinned addon may be`);
  const buf = Buffer.from(await res.arrayBuffer());
  if (buf.length > MAX_ZIP_BYTES) throw new Error(`${url} is ${buf.length} bytes, more than a pinned addon may be`);
  return buf;
}

async function main() {
  const root = join(dirname(fileURLToPath(import.meta.url)), "..");
  const check = process.argv.includes("--check");
  let missing = 0;
  for (const p of pins(root)) {
    const what = `${p.pin.folder} ${p.pin.version} (${p.pin.repo})`;
    if (inPlace(p)) {
      console.log(`${what}: in place`);
    } else if (check) {
      console.error(`${what}: not in place; run node scripts/fetch-addons.mjs`);
      missing++;
    } else {
      const files = unpack(await download(p.pin.url), p.pin);
      place(p, files);
      console.log(`${what}: fetched, ${files.length} files`);
    }
  }
  if (missing) process.exit(1);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main().catch((err) => {
    console.error(`fetch-addons: ${err.message}`);
    process.exit(1);
  });
}
