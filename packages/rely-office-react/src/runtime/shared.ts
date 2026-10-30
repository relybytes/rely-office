// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

import type { DocumentBytes, ErrorCode, Limits } from '../model.js';

/** What a WebAssembly module of this package is loaded from: a URL, a Response, bytes or a module. */
export type WasmSource = string | URL | Response | BufferSource | WebAssembly.Module;

/** An error of rely-office, with the stable code of what went wrong. */
export class RelyOfficeError extends Error {
  override readonly name = 'RelyOfficeError';

  constructor(
    readonly code: ErrorCode,
    message: string,
  ) {
    super(message);
  }
}

const CODED_MESSAGE = /^([A-Z_]+): ([\s\S]*)$/;

/** Run a wasm call, turning its coded `Error` into a `RelyOfficeError`. */
export function call<T>(operation: () => T): T {
  try {
    return operation();
  } catch (error) {
    const match = error instanceof Error ? CODED_MESSAGE.exec(error.message) : null;
    if (match) {
      throw new RelyOfficeError(match[1] as ErrorCode, match[2]);
    }
    throw error;
  }
}

/** The document as a `Uint8Array`, without copying a view. */
export function toBytes(input: DocumentBytes): Uint8Array {
  if (input instanceof Uint8Array) {
    return input;
  }
  if (input instanceof ArrayBuffer) {
    return new Uint8Array(input);
  }
  if (ArrayBuffer.isView(input)) {
    return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
  }
  throw new TypeError('rely-office: the document must be a Uint8Array or an ArrayBuffer');
}

export function toFileName(fileName: string | null | undefined): string | undefined {
  return fileName === undefined || fileName === null ? undefined : String(fileName);
}

export function toLimitsJson(limits: Limits | null | undefined): string | undefined {
  return limits === undefined || limits === null ? undefined : JSON.stringify(limits);
}
