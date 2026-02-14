/* tslint:disable */
/* eslint-disable */

/**
 * Full in-memory decryption (for small files or when streaming isn't available)
 */
export function decrypt_blob(encrypted_data: Uint8Array, key_base64: string): Uint8Array;

/**
 * Decrypt a single chunk given its encrypted data, key, base nonce, and chunk index
 * Used by the streaming Web Worker to decrypt chunk-by-chunk
 */
export function decrypt_chunk(encrypted_chunk: Uint8Array, key_base64: string, nonce_bytes: Uint8Array, chunk_index: bigint): Uint8Array;

/**
 * Parse the 40-byte header from the encrypted blob
 * Returns [nonce(24), total_chunks(8), original_size(8)] as a flat Uint8Array
 */
export function parse_header(data: Uint8Array): Uint8Array;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly decrypt_blob: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly decrypt_chunk: (a: number, b: number, c: number, d: number, e: number, f: number, g: bigint) => [number, number, number, number];
    readonly parse_header: (a: number, b: number) => [number, number, number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
