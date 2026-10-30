// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

import type { ReactElement } from 'react';
import type { Block, Comment, HeadersFooters, MediaItem } from '../model.js';
import { BlocksEditor } from './blocks-editor.js';
import type { DocumentAside } from './editor-document.js';
import type { DocumentEditorLabels } from './labels.js';
import { threadRoot, type ContentReferences } from './references.js';

const HEADER_KEYS: [keyof HeadersFooters, keyof DocumentEditorLabels][] = [
  ['header', 'header'],
  ['footer', 'footer'],
  ['firstHeader', 'firstHeader'],
  ['firstFooter', 'firstFooter'],
  ['evenHeader', 'evenHeader'],
  ['evenFooter', 'evenFooter'],
];

function HeadersEditors({
  headers,
  onChange,
  media,
  imageSource,
  editable,
  labels,
  prefix,
}: Readonly<{
  headers: HeadersFooters;
  onChange: (headers: HeadersFooters) => void;
  media: MediaItem[];
  imageSource?: (item: MediaItem) => string;
  editable: boolean;
  labels: DocumentEditorLabels;
  prefix: string;
}>): ReactElement {
  return (
    <>
      {HEADER_KEYS.filter(([key]) => key === 'header' || key === 'footer' || headers[key] !== undefined).map(
        ([key, label]) => (
          <div key={key} className="rely-side-field">
            <span>{prefix ? `${prefix}: ${labels[label] as string}` : (labels[label] as string)}</span>
            <BlocksEditor
              blocks={headers[key] ?? []}
              media={media}
              imageSource={imageSource}
              editable={editable}
              label={prefix ? `${prefix}: ${labels[label] as string}` : (labels[label] as string)}
              onChange={(blocks) => onChange({ ...headers, [key]: blocks })}
            />
          </div>
        ),
      )}
    </>
  );
}

/**
 * What a document holds beside its text, edited beside it: its comments with their replies, its
 * notes in the order the text refers to them, its headers and footers, a section's own included.
 */
export function DocumentSidePanel({
  aside,
  references,
  media,
  imageSource,
  editable,
  labels,
  author,
  focusComment,
  onAsideChange,
  onDeleteComments,
  onDeleteNote,
}: Readonly<{
  aside: DocumentAside;
  references: ContentReferences;
  media: MediaItem[];
  imageSource?: (item: MediaItem) => string;
  editable: boolean;
  labels: DocumentEditorLabels;
  author: { name: string; initials?: string };
  focusComment: number | null;
  onAsideChange: (aside: DocumentAside) => void;
  onDeleteComments: (comments: number[]) => void;
  onDeleteNote: (note: number) => void;
}>): ReactElement {
  const comments = aside.comments ?? [];
  const shown = comments
    .map((comment, index) => ({ comment, index, root: threadRoot(comments, index) }))
    .filter(({ root, index }) => references.comments.has(root) || references.comments.has(index) || index === focusComment);
  const roots = shown.filter(({ comment }) => comment.parent === undefined);

  const setComment = (index: number, change: Partial<Comment>) =>
    onAsideChange({ ...aside, comments: comments.map((comment, at) => (at === index ? { ...comment, ...change } : comment)) });
  const reply = (parent: number) =>
    onAsideChange({
      ...aside,
      comments: [
        ...comments,
        {
          author: author.name,
          ...(author.initials ? { initials: author.initials } : {}),
          date: new Date().toISOString(),
          blocks: [{ type: 'paragraph', content: [] }],
          resolved: false,
          parent,
        },
      ],
    });
  const setNote = (index: number, blocks: Block[]) =>
    onAsideChange({ ...aside, notes: aside.notes.map((note, at) => (at === index ? { ...note, blocks } : note)) });

  const thread = (root: number) => shown.filter(({ root: of }) => of === root);
  const sections = aside.sections ?? [];

  return (
    <aside className="rely-document-side">
      <details open={roots.length > 0}>
        <summary>
          {labels.comments} ({roots.length})
        </summary>
        <ol className="rely-side-comments" aria-label={labels.comments}>
          {roots.map(({ index: root }) => (
            <li key={root} data-comment={root} className={comments[root].resolved ? 'rely-resolved' : undefined}>
              {thread(root).map(({ comment, index }) => (
                <div key={index} className={index === root ? 'rely-side-comment' : 'rely-side-comment rely-side-reply'}>
                  <div className="rely-side-meta">
                    <strong>{comment.author}</strong>
                    {comment.date ? <time dateTime={comment.date}>{comment.date.slice(0, 10)}</time> : null}
                  </div>
                  <BlocksEditor
                    blocks={comment.blocks}
                    media={media}
                    imageSource={imageSource}
                    editable={editable}
                    label={`${labels.commentText} ${comment.author}`}
                    autoFocus={index === focusComment}
                    onChange={(blocks) => setComment(index, { blocks })}
                  />
                </div>
              ))}
              {editable ? (
                <div className="rely-side-actions">
                  <label>
                    <input
                      type="checkbox"
                      checked={comments[root].resolved}
                      onChange={(event) => setComment(root, { resolved: event.target.checked })}
                    />
                    {labels.resolved}
                  </label>
                  <button type="button" onClick={() => reply(root)}>
                    {labels.reply}
                  </button>
                  <button type="button" onClick={() => onDeleteComments(thread(root).map(({ index }) => index))}>
                    {labels.deleteComment}
                  </button>
                </div>
              ) : null}
            </li>
          ))}
        </ol>
      </details>

      <details open={references.notes.length > 0}>
        <summary>
          {labels.notes} ({references.notes.length})
        </summary>
        <ol className="rely-side-notes" aria-label={labels.notes}>
          {references.notes
            .filter((note) => aside.notes[note] !== undefined)
            .map((note, position) => (
              <li key={note}>
                <BlocksEditor
                  blocks={aside.notes[note].blocks}
                  media={media}
                  imageSource={imageSource}
                  editable={editable}
                  label={labels.noteText(position + 1)}
                  onChange={(blocks) => setNote(note, blocks)}
                />
                {editable ? (
                  <button type="button" onClick={() => onDeleteNote(note)}>
                    {labels.deleteNote}
                  </button>
                ) : null}
              </li>
            ))}
        </ol>
      </details>

      <details>
        <summary>{labels.headersFooters}</summary>
        {sections.slice(0, -1).map((section, index) => (
          <HeadersEditors
            key={index}
            headers={section.headersFooters}
            media={media}
            imageSource={imageSource}
            editable={editable}
            labels={labels}
            prefix={labels.section(index + 1)}
            onChange={(headersFooters) =>
              onAsideChange({
                ...aside,
                sections: sections.map((each, at) => (at === index ? { ...each, headersFooters } : each)),
              })
            }
          />
        ))}
        <HeadersEditors
          headers={aside.headersFooters}
          media={media}
          imageSource={imageSource}
          editable={editable}
          labels={labels}
          prefix={sections.length > 1 ? labels.section(sections.length) : ''}
          onChange={(headersFooters) => onAsideChange({ ...aside, headersFooters })}
        />
      </details>
    </aside>
  );
}
