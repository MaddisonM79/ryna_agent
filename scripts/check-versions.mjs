#!/usr/bin/env node
// Refuses a release whose versions disagree: each bundled mod's mod.json and TOC (the Settings mod
// has a mod.json and no addon folder), the WoW core's manifest, the Palworld
// core's manifest and package.json, the app's package.json, the cargo workspace (so every crate),
// Cargo.lock, and the tag.
//
//   node scripts/check-versions.mjs [--tag vX.Y.Z]
//
// Prints `version=X.Y.Z` (for $GITHUB_OUTPUT) and exits 0 when they agree; otherwise names each
// file and its version and exits 1.
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { agree, pinProblems, readVersions } from "./versions.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const i = process.argv.indexOf("--tag");
const tag = i > 0 ? process.argv[i + 1] || null : null;

const { version, problems } = agree(readVersions(root), tag);
// An addon in a repository of its own (the core's, a pinned mod's) isn't on the release's version:
// it is held to its pin instead, and the core's API levels to cores/wow/core.json.
problems.push(...pinProblems(root));
if (problems.length) {
  for (const p of problems) console.error(`::error::${p.split("\n")[0]}\n${p}`);
  process.exit(1);
}
console.log(`version=${version}`);
