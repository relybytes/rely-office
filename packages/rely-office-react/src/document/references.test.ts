// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

import type { Comment, DocumentTree } from '../model.js';
import { compactComments, compactNotes, contentReferences, threadRoot } from './references.js';

const comment = (author: string, parent?: number): Comment => ({
  author,
  blocks: [{ type: 'paragraph', content: [{ type: 'text', text: author }] }],
  resolved: false,
  ...(parent === undefined ? {} : { parent }),
});

function tree(): DocumentTree {
  return {
    blocks: [
      {
        type: 'paragraph',
        content: [
          { type: 'noteReference', note: 2 },
          { type: 'text', text: 'a', marks: [{ type: 'bold' }, { type: 'comment', comment: 2 }] },
          { type: 'noteReference', note: 0 },
          { type: 'noteReference', note: 2 },
          { type: 'commentAnchor', comment: 4 },
        ],
      },
    ],
    media: [],
    report: {} as DocumentTree['report'],
    notes: [
      { kind: 'footnote', blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'zero' }] }] },
      { kind: 'footnote', blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'orfana' }] }] },
      { kind: 'endnote', blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'due' }] }] },
    ],
    headersFooters: { header: [{ type: 'paragraph', content: [{ type: 'commentAnchor', comment: 0 }] }], footer: [] },
    page: { widthTwips: 1, heightTwips: 1, marginsTwips: [0, 0, 0, 0] },
    comments: [comment('in testata'), comment('senza testo'), comment('radice'), comment('risposta', 2), comment('ancora'), comment('risposta orfana', 1)],
  };
}

test('notes are kept in the order the text refers to them, an orphan dropped', () => {
  const compacted = compactNotes(tree());

  expect(compacted.notes.map((note) => note.blocks[0].type === 'paragraph' && note.blocks[0].content[0])).toEqual([
    { type: 'text', text: 'due' },
    { type: 'text', text: 'zero' },
  ]);
  const first = compacted.blocks[0];
  expect(first.type === 'paragraph' && first.content.filter((inline) => inline.type === 'noteReference')).toEqual([
    { type: 'noteReference', note: 0 },
    { type: 'noteReference', note: 1 },
    { type: 'noteReference', note: 0 },
  ]);
});

test('comments the text marks or anchors are kept with their replies, and numbered again', () => {
  const compacted = compactComments(tree());

  expect(compacted.comments!.map((each) => each.author)).toEqual(['in testata', 'radice', 'risposta', 'ancora']);
  expect(compacted.comments![2].parent).toBe(1);
  const first = compacted.blocks[0];
  expect(first.type === 'paragraph' && first.content[1]).toEqual({
    type: 'text',
    text: 'a',
    marks: [{ type: 'bold' }, { type: 'comment', comment: 1 }],
  });
  expect(first.type === 'paragraph' && first.content[4]).toEqual({ type: 'commentAnchor', comment: 3 });
  expect(compacted.headersFooters.header).toEqual([{ type: 'paragraph', content: [{ type: 'commentAnchor', comment: 0 }] }]);
});

test('a comment whose mark is gone takes its mark-less run back to plain text, and no comments are left', () => {
  const compacted = compactComments({
    ...tree(),
    blocks: [{ type: 'paragraph', content: [{ type: 'text', text: 'x', marks: [{ type: 'comment', comment: 9 }] }] }],
    headersFooters: { header: [], footer: [] },
  });

  expect(compacted.comments).toBeUndefined();
  expect(compacted.blocks).toEqual([{ type: 'paragraph', content: [{ type: 'text', text: 'x' }] }]);
});

test('a tree with no comments comes back as it was', () => {
  const { comments: _comments, ...without } = tree();
  expect(compactComments(without)).toBe(without);
});

test('the content says which comments and notes it still refers to', () => {
  expect(
    contentReferences({
      type: 'doc',
      content: [
        {
          type: 'paragraph',
          content: [
            { type: 'text', text: 'a', marks: [{ type: 'comment', attrs: { comment: 3 } }] },
            { type: 'noteReference', attrs: { note: 1 } },
            { type: 'commentAnchor', attrs: { comment: 5 } },
            { type: 'noteReference', attrs: { note: 0 } },
            { type: 'noteReference', attrs: { note: 1 } },
          ],
        },
      ],
    }),
  ).toEqual({ comments: new Set([3, 5]), notes: [1, 0] });
});

test('a reply leads to the first comment of its thread, and a loop stops', () => {
  const comments = [comment('a'), comment('b', 0), comment('c', 1), { ...comment('d', 4) }, { ...comment('e', 3) }];

  expect(threadRoot(comments, 2)).toBe(0);
  expect(threadRoot(comments, 0)).toBe(0);
  expect([3, 4]).toContain(threadRoot(comments, 3));
});
