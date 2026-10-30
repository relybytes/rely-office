// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

import type { Block, Inline, MediaItem } from '../model.js';

const BASE64_CHUNK = 0x8000;

/** The bytes of an image as a `data:` URL, the source an image shows with by default. */
export function mediaDataUrl(item: MediaItem): string {
  let binary = '';
  for (let offset = 0; offset < item.bytes.length; offset += BASE64_CHUNK) {
    binary += String.fromCharCode(...item.bytes.subarray(offset, offset + BASE64_CHUNK));
  }
  return `data:${item.contentType};base64,${btoa(binary)}`;
}

/** The content type an image's bytes prove, for an image the editor inserts. */
export function sniffImageType(bytes: Uint8Array): string {
  const starts = (...signature: number[]): boolean =>
    signature.every((byte, index) => bytes[index] === byte);
  if (starts(0x89, 0x50, 0x4e, 0x47)) return 'image/png';
  if (starts(0xff, 0xd8, 0xff)) return 'image/jpeg';
  if (starts(0x47, 0x49, 0x46, 0x38)) return 'image/gif';
  if (starts(0x52, 0x49, 0x46, 0x46) && bytes[8] === 0x57 && bytes[9] === 0x45) return 'image/webp';
  if (starts(0x42, 0x4d)) return 'image/bmp';
  return 'application/octet-stream';
}

/** Apply `visit` to every inline of the blocks, nested ones included; null drops the inline. */
export function mapBlockInlines(blocks: Block[], visit: (inline: Inline) => Inline | null): Block[] {
  const inlines = (content: Inline[]): Inline[] =>
    content.flatMap((inline) => {
      const mapped = visit(inline);
      return mapped === null ? [] : [mapped];
    });
  return blocks.map((block): Block => {
    switch (block.type) {
      case 'heading':
      case 'paragraph':
      case 'listItem':
        return { ...block, content: inlines(block.content) };
      case 'blockquote':
        return { ...block, blocks: mapBlockInlines(block.blocks, visit) };
      case 'table':
        return {
          ...block,
          rows: block.rows.map((row) => ({
            ...row,
            cells: row.cells.map((cell) => ({ ...cell, blocks: mapBlockInlines(cell.blocks, visit) })),
          })),
        };
      default:
        return block;
    }
  });
}

/** Apply `visit` to every image inline of the blocks, nested ones included. */
export function mapBlockImages(
  blocks: Block[],
  visit: (image: Extract<Inline, { type: 'image' }>) => Inline | null,
): Block[] {
  return mapBlockInlines(blocks, (inline) => (inline.type === 'image' ? visit(inline) : inline));
}
