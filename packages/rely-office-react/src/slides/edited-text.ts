// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import type { SlideParagraph, SlideRun, TextBody } from '../model.js';
import { DEFAULT_PARAGRAPH, DEFAULT_RUN } from './slide-model.js';

/**
 * The body a shape holds after its text was edited in place. `TextBodyView` draws a paragraph as
 * a `div` and a run as a `span` that name their index, so each piece of text the person kept goes
 * back into its own run with its own look; a line the browser added takes the look of the
 * paragraph before it, and text typed outside a run joins the run before it.
 */
export function bodyFromEditedElement(element: HTMLElement, body: TextBody): TextBody {
  const paragraphs: SlideParagraph[] = [];
  let template: SlideParagraph = body.paragraphs[0] ?? { ...DEFAULT_PARAGRAPH, runs: [] };

  const lines = [...element.childNodes].filter(
    (node) => node.nodeType === Node.ELEMENT_NODE || (node.textContent ?? '').length > 0,
  );
  for (const line of lines) {
    const index = line instanceof HTMLElement ? Number(line.dataset.paragraph) : Number.NaN;
    if (Number.isInteger(index) && body.paragraphs[index]) template = body.paragraphs[index];
    const fallbackRun: SlideRun = template.runs.find((run) => !run.lineBreak) ?? DEFAULT_RUN;
    const runs: SlideRun[] = [];
    const visit = (node: ChildNode): void => {
      if (node instanceof HTMLElement && node.classList.contains('rely-slide-bullet')) return;
      if (node instanceof HTMLElement && node.dataset.run !== undefined) {
        const own = template.runs[Number(node.dataset.run)] ?? fallbackRun;
        runs.push({ ...own, text: node.textContent ?? '', lineBreak: false });
        return;
      }
      if (node instanceof HTMLBRElement) {
        // A lone or trailing <br> only holds the line open; one between runs is a line break.
        if (line.childNodes.length > 1 && node !== line.lastChild) {
          runs.push({ ...fallbackRun, text: '', lineBreak: true });
        }
        return;
      }
      if (node.nodeType === Node.TEXT_NODE) {
        const text = node.textContent ?? '';
        if (text.length === 0) return;
        const last = runs[runs.length - 1];
        if (last && !last.lineBreak) runs[runs.length - 1] = { ...last, text: last.text + text };
        else runs.push({ ...fallbackRun, text, lineBreak: false });
        return;
      }
      node.childNodes.forEach(visit);
    };
    if (line.nodeType === Node.TEXT_NODE) visit(line);
    else line.childNodes.forEach(visit);
    paragraphs.push({ ...template, runs: runs.filter((run) => run.lineBreak || run.text.length > 0) });
  }

  return { ...body, paragraphs: paragraphs.length > 0 ? paragraphs : [{ ...template, runs: [] }] };
}
