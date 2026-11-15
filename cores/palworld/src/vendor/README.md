# Vendored: kraken-decode 0.1.1

`kraken-decode.mjs` and `kraken-decode.d.mts` are release files of
[kraken-decode](https://github.com/MaddisonM79/kraken-decode/releases/tag/v0.1.1) (MIT), copied
unchanged. The `.mjs` is the whole decoder: a small loader with the WebAssembly module embedded.
Don't edit them; `test/sav.test.ts` holds the `.mjs` to the release's checksum.

To update: download both files and `SHA256SUMS` from the new release, check them
(`shasum -a 256 -c SHA256SUMS`), replace the files, and change the version here, the checksum in
the test and the version in `NOTICE.md`.

```
48d07b34d928e2f02a7026d48ecb0bb3aee081d7eedd1a5269a8d36ed08697d7  kraken-decode.mjs
ec6ff955f7aa18946bdecfc689c7ba14d6df518155d89606ff3881fe1df6e81e  kraken-decode.d.mts
```
