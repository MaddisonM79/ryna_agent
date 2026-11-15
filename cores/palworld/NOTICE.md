# Third-party code in the Palworld core

The core's own code is MIT, like the rest of this repository (`LICENSE` at the root). It builds on:

## palworld-save-tools (MIT)

The save parser (`src/gvas`, `src/rawdata`, the unwrap in `src/gvas/unwrap.ts`, and the generated
`src/rawdata/concrete-model-names.ts`) is a TypeScript port of
[palworld-save-tools](https://github.com/cheahjs/palworld-save-tools), from
[oMaN-Rod's fork](https://github.com/oMaN-Rod/palworld-save-tools) at `790e0bc`. Every snapshot names
it (`agent.saveTools`).

```
MIT License

Copyright (c) 2024 Jun Siang Cheah

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## kraken-decode (MIT)

The Mac build's saves are Oodle-compressed. They are decompressed by
[kraken-decode](https://github.com/MaddisonM79/kraken-decode), our own decoder, written without
reading any other decoder's source (its `docs/SOURCES.md` says how). The build bundles the released
`kraken-decode.mjs` (`src/vendor`, version 0.1.0) into `agent.mjs`.

```
MIT License

Copyright (c) 2026 Maddison Markham

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

Through 0.4.1 the core used ooz-wasm (GPL-3.0-or-later) for this, which made those built packages
combined works under the GPL. Nothing of it is in this repository or in later packages.

## Node.js

The core package carries the official Node.js binary for its target, unmodified and still signed
by the Node.js project, with Node's `LICENSE` as `LICENSE-node`.
