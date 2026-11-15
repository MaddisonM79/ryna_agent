/** What `decompress` throws. */
export class KrakenDecodeError extends Error {
  /** "truncated", "corrupt", "unsupported" or "size mismatch". */
  kind: string;
  /** The part of the stream at fault, when the decoder names one. */
  detail: string;
}

/**
 * Decode `data` (a whole compressed stream) to exactly `rawSize` bytes.
 * Returns a new Uint8Array that the caller owns. Throws a KrakenDecodeError
 * on a truncated, corrupt or unsupported stream.
 */
export function decompress(data: Uint8Array, rawSize: number): Uint8Array;

/** Instantiate a separate decoder from the bytes of the WebAssembly module. */
export function instantiate(wasmBytes: BufferSource): { decompress: typeof decompress };
