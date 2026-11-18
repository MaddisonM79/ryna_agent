# Cores

A core is a game behind the agent: it knows how to find the game, read what the game leaves on disk,
push it to ryna.io, and bring data back down, and it runs fine with no mods. Each core has a folder
here with its manifest, `core.json` (id, name, version, `runtime` builtin or sidecar, the token
`resource` and `scopes` it needs, the folders it `detect`s the game in, the game files it `reads`
and `writes`, and where its `mods` live), which the agent reads at start; the agent drives the core
through one interface, `ryna_agent::core::Core` in `agent/host`. There are three:

- `wow/`, builtin: the Rust core (`wow/core`, crate `ryna-wow-core`), its in-game addon
  (`wow/addon/Ryna_Core`, the plumbing its mods build on; its source is in
  [Ryna-Mods/core](https://github.com/Ryna-Mods/core) and it is fetched here from a pinned release), and the test fixtures; its mods (Chronicle
  first) are in `mods/wow`.
- `palworld/`, a sidecar (Node, macOS only for now): the Mac App Store build's save parser, the
  snapshot and the push, which the agent starts as `node agent.mjs serve` and drives over
  stdin/stdout. It isn't in the app: it's a signed core package on each release, fetched only for
  someone provisioned for it (`palworld/README.md`).
- `satisfactory/`, builtin: a relay (`satisfactory/core`, crate `ryna-satisfactory-core`) that
  holds one outbound WebSocket to the API and answers its requests from the Ficsit Remote
  Monitoring mod on `127.0.0.1`, on the PC that hosts the factory. Shown only to the host, and off
  until turned on there.

See `docs/ryna-agent-cores-mods.md` (sections 4 and 5, 13 for sidecars and fetched cores, and 14 for the Satisfactory relay).
