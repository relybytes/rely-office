// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

import type { NodeViewRenderer } from '@tiptap/core';

import type { ImageAnchor } from '../model.js';

/** The smallest an image is dragged to, in CSS pixels. */
export const MINIMUM_IMAGE_SIZE = 16;

/** How far from the left a floating image placed by an offset sits before it goes right. */
const RIGHT_HALF_EMU = 2_743_200;

/**
 * Where a floating image goes in the editor's flowing text: beside the text on its left or its
 * right, or on a line of its own; `inline` for an image in the line.
 */
export function floatingSide(anchor: ImageAnchor | null | undefined): 'inline' | 'left' | 'right' | 'center' {
  if (!anchor) return 'inline';
  const { align, offsetEmu } = anchor.horizontal;
  if (align === 'end') return 'right';
  if (align === 'center') return 'center';
  if (align === 'start') return 'left';
  return offsetEmu > RIGHT_HALF_EMU ? 'right' : 'left';
}

/** The size an image takes when dragged by its corner, its proportions kept. */
export function resizedImage(
  start: { width: number; height: number },
  deltaX: number,
  maximumWidth: number,
): { width: number; height: number } {
  const ratio = start.width > 0 ? start.height / start.width : 1;
  const width = Math.round(Math.min(Math.max(MINIMUM_IMAGE_SIZE, start.width + deltaX), Math.max(MINIMUM_IMAGE_SIZE, maximumWidth)));
  return { width, height: Math.max(1, Math.round(width * ratio)) };
}

/** An image with a handle at its corner that resizes it, proportions kept, as Word does. */
export const imageNodeView: NodeViewRenderer = ({ node: initial, getPos, editor }) => {
  let node = initial;
  const wrapper = document.createElement('span');
  wrapper.className = 'rely-image';
  const image = document.createElement('img');
  const handle = document.createElement('span');
  handle.className = 'rely-image-handle';
  handle.setAttribute('aria-hidden', 'true');
  wrapper.append(image, handle);

  const draw = () => {
    const { src, alt, title, width, height } = node.attrs as Record<string, string | number | null>;
    image.src = String(src ?? '');
    if (alt) image.alt = String(alt);
    else image.removeAttribute('alt');
    if (title) image.title = String(title);
    else image.removeAttribute('title');
    image.style.width = width ? `${width}px` : '';
    image.style.height = height ? `${height}px` : '';
    const anchor = node.attrs.anchor as ImageAnchor | null;
    wrapper.dataset.float = floatingSide(anchor);
    wrapper.dataset.wrap = anchor?.wrap.type ?? 'inline';
    if (node.attrs.media !== null && node.attrs.media !== undefined) image.dataset.media = String(node.attrs.media);
  };
  draw();

  handle.addEventListener('mousedown', (event) => {
    if (!editor.isEditable) return;
    event.preventDefault();
    event.stopPropagation();
    const box = image.getBoundingClientRect();
    const start = {
      width: box.width || Number(node.attrs.width) || image.naturalWidth || 100,
      height: box.height || Number(node.attrs.height) || image.naturalHeight || 100,
    };
    const maximum = (editor.view.dom as HTMLElement).clientWidth || Number.POSITIVE_INFINITY;
    const origin = event.clientX;
    let size = start;
    const move = (moved: MouseEvent) => {
      size = resizedImage(start, moved.clientX - origin, maximum);
      image.style.width = `${size.width}px`;
      image.style.height = `${size.height}px`;
    };
    const finish = () => {
      window.removeEventListener('mousemove', move);
      window.removeEventListener('mouseup', finish);
      const position = typeof getPos === 'function' ? getPos() : undefined;
      if (position === undefined) return;
      const anchor = node.attrs.anchor as ImageAnchor | null;
      const anchored = anchor ? { anchor: { ...anchor, widthEmu: size.width * 9_525, heightEmu: size.height * 9_525 } } : {};
      editor.view.dispatch(editor.view.state.tr.setNodeMarkup(position, undefined, { ...node.attrs, ...size, ...anchored }));
    };
    window.addEventListener('mousemove', move);
    window.addEventListener('mouseup', finish);
  });

  return {
    dom: wrapper,
    update: (updated) => {
      if (updated.type !== node.type) return false;
      node = updated;
      draw();
      return true;
    },
    selectNode: () => wrapper.classList.add('ProseMirror-selectednode'),
    deselectNode: () => wrapper.classList.remove('ProseMirror-selectednode'),
    stopEvent: (event) => event.target === handle,
    ignoreMutation: () => true,
  };
};
