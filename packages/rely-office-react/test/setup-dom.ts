// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

// jsdom lays nothing out: ProseMirror asks for rectangles to scroll a selection into view.

if (typeof document !== 'undefined') {
  const emptyRect = (): DOMRect => ({
    x: 0, y: 0, top: 0, left: 0, bottom: 0, right: 0, width: 0, height: 0, toJSON: () => ({}),
  });
  const emptyList = (): DOMRectList =>
    Object.assign([], { item: () => null }) as unknown as DOMRectList;
  for (const prototype of [Element.prototype, Range.prototype]) {
    if (!('getClientRects' in prototype) || typeof prototype.getClientRects !== 'function') {
      Object.defineProperty(prototype, 'getClientRects', { value: emptyList, configurable: true });
    }
    if (typeof prototype.getBoundingClientRect !== 'function') {
      Object.defineProperty(prototype, 'getBoundingClientRect', { value: emptyRect, configurable: true });
    }
  }
  if (typeof document.elementFromPoint !== 'function') {
    document.elementFromPoint = () => null;
  }
}
