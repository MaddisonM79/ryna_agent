#!/usr/bin/env bash
# Runs each pinned mod's own tests against the core this repository pins: the core the app carries.
# A mod's tests live in its repository (Ryna-Mods/<id>), where they run against the oldest core the
# mod supports; this is the other end. From the repository root, after scripts/fetch-addons.mjs:
#
#   scripts/pinned-tests.sh          needs git, node and lua5.1 (or LUA=<a lua>)
#
# Each mod's repository is cloned at its pin's tag into .pinned/<id>. The Fights mod's fixture is
# the app's own output (cores/wow/core, fights_data's test keeps mods/wow/fights/test/fixtures
# current), so its tests read this repository's copy: a writer that changed is caught here.
set -euo pipefail
cd "$(dirname "$0")/.."
LUA="${LUA:-$(command -v lua5.1 || command -v lua)}"
core="$PWD/cores/wow/addon/Ryna_Core"
[ -f "$core/Ryna_Core.toc" ] || { echo "the pinned core isn't fetched: node scripts/fetch-addons.mjs"; exit 1; }
failed=0
for pin in mods/wow/*/pin.json; do
  id="$(basename "$(dirname "$pin")")"
  repo="$(node -p 'JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).repo' "$pin")"
  tag="$(node -p 'JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).tag' "$pin")"
  case "$repo" in Ryna-Mods/*) ;; *) echo "$pin: $repo isn't a Ryna-Mods repository"; exit 1 ;; esac
  dest=".pinned/$id"
  if [ ! -d "$dest/.git" ] || [ "$(git -C "$dest" describe --tags --exact-match 2>/dev/null || true)" != "$tag" ]; then
    rm -rf "./.pinned/$id"
    git clone -q --depth 1 --branch "$tag" -c advice.detachedHead=false "https://github.com/$repo.git" "$dest"
  fi
  if [ -d "mods/wow/$id/test/fixtures" ]; then cp -R "mods/wow/$id/test/fixtures/." "$dest/test/fixtures/"; fi
  echo "== $id $tag"
  for t in "$dest"/test/*_test.lua; do
    (cd "$dest" && RYNA_CORE="$core" "$LUA" "test/$(basename "$t")") || failed=$((failed + 1))
  done
done
if [ "$failed" -ne 0 ]; then echo "$failed test file(s) failed"; exit 1; fi
echo "every pinned mod's tests pass against the pinned core"
