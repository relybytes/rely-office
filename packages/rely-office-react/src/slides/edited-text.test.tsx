// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

import { render } from '@testing-library/react';
import type { TextBody } from '../model.js';
import { bodyFromEditedElement } from './edited-text.js';
import { DEFAULT_PARAGRAPH, DEFAULT_RUN } from './slide-model.js';
import { TextBodyView } from './slide-view.js';

const BODY: TextBody = {
  anchor: 'top',
  insetsEmu: [0, 0, 0, 0],
  wrap: true,
  paragraphs: [
    {
      ...DEFAULT_PARAGRAPH,
      align: 'center',
      runs: [
        { ...DEFAULT_RUN, text: 'Ricavi ', bold: true },
        { ...DEFAULT_RUN, text: '2026', colour: 'C00000' },
      ],
    },
    {
      ...DEFAULT_PARAGRAPH,
      bullet: { type: 'character', text: '•' },
      runs: [
        { ...DEFAULT_RUN, text: 'prima' },
        { ...DEFAULT_RUN, text: '', lineBreak: true },
        { ...DEFAULT_RUN, text: 'riga' },
      ],
    },
  ],
};

function paragraphsElement(): HTMLElement {
  const { container } = render(<TextBodyView body={BODY} scale={0.0001} editable />);
  return container.querySelector('.rely-slide-paragraphs') as HTMLElement;
}

describe('text edited in a shape', () => {
  test('untouched, the body comes back as it was, bullets and line breaks included', () => {
    expect(bodyFromEditedElement(paragraphsElement(), BODY)).toEqual(BODY);
  });

  test('a run changed in place keeps its own look', () => {
    const element = paragraphsElement();
    element.querySelector('[data-run="1"]')!.textContent = '2027';

    const body = bodyFromEditedElement(element, BODY);

    expect(body.paragraphs[0].runs[1]).toEqual({ ...DEFAULT_RUN, text: '2027', colour: 'C00000' });
    expect(body.paragraphs[0].runs[0].bold).toBe(true);
  });

  test('text typed after a run joins it, and a line the browser added copies the paragraph before', () => {
    const element = paragraphsElement();
    element.querySelector('[data-paragraph="0"]')!.append(' previsti');
    const added = document.createElement('div');
    added.textContent = 'nuova';
    element.append(added);

    const body = bodyFromEditedElement(element, BODY);

    expect(body.paragraphs[0].runs[1].text).toBe('2026 previsti');
    expect(body.paragraphs).toHaveLength(3);
    expect(body.paragraphs[2]).toMatchObject({ bullet: { type: 'character', text: '•' } });
    expect(body.paragraphs[2].runs).toEqual([{ ...DEFAULT_RUN, text: 'nuova' }]);
  });

  test('emptied, the body keeps one empty paragraph with its look', () => {
    const element = paragraphsElement();
    element.replaceChildren();

    expect(bodyFromEditedElement(element, BODY).paragraphs).toEqual([{ ...BODY.paragraphs[0], runs: [] }]);
  });

  test('a bare text node in the box is a paragraph of its own', () => {
    const element = document.createElement('div');
    element.append('solo testo');

    expect(bodyFromEditedElement(element, { ...BODY, paragraphs: [] }).paragraphs[0].runs).toEqual([{ ...DEFAULT_RUN, text: 'solo testo' }]);
  });
});
