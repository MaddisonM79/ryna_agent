# Vendored: kraken-decode 0.1.0

`kraken-decode.mjs` and `kraken-decode.d.mts` are release files of
[kraken-decode](https://github.com/MaddisonM79/kraken-decode/releases/tag/v0.1.0) (MIT), copied
unchanged. The `.mjs` is the whole decoder: a small loader with the WebAssembly module embedded.
Don't edit them; `test/sav.test.ts` holds the `.mjs` to the release's checksum.

To update: download both files and `SHA256SUMS` from the new release, check them
(`shasum -a 256 -c SHA256SUMS`), replace the files, and change the version here, the checksum in
the test and the version in `NOTICE.md`.

```
709dc6d1a1283856b74d7864aa762b35eccfb4ea388383e0aa95b1d456a2e179  kraken-decode.mjs
ec6ff955f7aa18946bdecfc689c7ba14d6df518155d89606ff3881fe1df6e81e  kraken-decode.d.mts
```
