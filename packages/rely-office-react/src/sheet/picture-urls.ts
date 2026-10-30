// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

// The addresses an `<img>` shows the pictures and charts of a sheet from.

import { useEffect, useRef, useState } from 'react';

/** A picture's bytes as a `data:` address, where the page cannot make object addresses. */
export function dataUrl(bytes: Uint8Array, contentType: string): string {
  let binary = '';
  for (let start = 0; start < bytes.length; start += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(start, start + 0x8000));
  }
  return `data:${contentType};base64,${btoa(binary)}`;
}

/**
 * An address for each picture, made when the pictures change and released when they are
 * replaced or the grid goes. They are made in an effect, not while rendering, so React's strict
 * mode, which runs an effect, its cleanup and the effect again, never leaves an `<img>` on an
 * address already released, and a chart is never shown as an empty frame.
 */
export function usePictureUrls(pictures: readonly { contentType: string }[], bytes: (position: number) => Uint8Array): string[] {
  const [urls, setUrls] = useState<string[]>([]);
  const read = useRef(bytes);
  read.current = bytes;

  useEffect(() => {
    const objects = typeof URL !== 'undefined' && typeof URL.createObjectURL === 'function';
    const made = pictures.map((picture, position) => {
      const content = new Uint8Array(read.current(position));
      return objects ? URL.createObjectURL(new Blob([content], { type: picture.contentType })) : dataUrl(content, picture.contentType);
    });
    setUrls(made);
    return () => {
      if (objects) for (const url of made) URL.revokeObjectURL(url);
    };
  }, [pictures]);

  return urls;
}
