// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

import type { JSONContent } from '@tiptap/core';
import type { DocumentFont, FontGeneric } from '../model.js';

/** The families the package ships in `fonts/`, all under the SIL Open Font License 1.1. */
export const BUNDLED_FONT_FAMILIES = ['Carlito', 'Caladea', 'Liberation Sans', 'Liberation Serif', 'Liberation Mono'];

/**
 * Known families, lower case, with the bundled family of the same metrics or the nearest in kind,
 * and their kind. The same table as `SUBSTITUTES` of rely-office-core; a test keeps them equal.
 */
const SUBSTITUTES: [string, string, FontGeneric][] = [
  ['calibri', 'Carlito', 'swiss'],
  ['calibri light', 'Carlito', 'swiss'],
  ['aptos', 'Carlito', 'swiss'],
  ['aptos display', 'Carlito', 'swiss'],
  ['aptos narrow', 'Carlito', 'swiss'],
  ['candara', 'Carlito', 'swiss'],
  ['corbel', 'Carlito', 'swiss'],
  ['century gothic', 'Carlito', 'swiss'],
  ['gill sans mt', 'Carlito', 'swiss'],
  ['carlito', 'Carlito', 'swiss'],
  ['cambria', 'Caladea', 'roman'],
  ['cambria math', 'Caladea', 'roman'],
  ['constantia', 'Caladea', 'roman'],
  ['caladea', 'Caladea', 'roman'],
  ['arial', 'Liberation Sans', 'swiss'],
  ['arial narrow', 'Liberation Sans', 'swiss'],
  ['helvetica', 'Liberation Sans', 'swiss'],
  ['helvetica neue', 'Liberation Sans', 'swiss'],
  ['verdana', 'Liberation Sans', 'swiss'],
  ['tahoma', 'Liberation Sans', 'swiss'],
  ['segoe ui', 'Liberation Sans', 'swiss'],
  ['trebuchet ms', 'Liberation Sans', 'swiss'],
  ['lucida sans unicode', 'Liberation Sans', 'swiss'],
  ['franklin gothic book', 'Liberation Sans', 'swiss'],
  ['liberation sans', 'Liberation Sans', 'swiss'],
  ['arimo', 'Liberation Sans', 'swiss'],
  ['dejavu sans', 'Liberation Sans', 'swiss'],
  ['times new roman', 'Liberation Serif', 'roman'],
  ['times', 'Liberation Serif', 'roman'],
  ['georgia', 'Liberation Serif', 'roman'],
  ['garamond', 'Liberation Serif', 'roman'],
  ['book antiqua', 'Liberation Serif', 'roman'],
  ['palatino', 'Liberation Serif', 'roman'],
  ['palatino linotype', 'Liberation Serif', 'roman'],
  ['bookman old style', 'Liberation Serif', 'roman'],
  ['century schoolbook', 'Liberation Serif', 'roman'],
  ['liberation serif', 'Liberation Serif', 'roman'],
  ['tinos', 'Liberation Serif', 'roman'],
  ['dejavu serif', 'Liberation Serif', 'roman'],
  ['courier new', 'Liberation Mono', 'modern'],
  ['courier', 'Liberation Mono', 'modern'],
  ['consolas', 'Liberation Mono', 'modern'],
  ['lucida console', 'Liberation Mono', 'modern'],
  ['cascadia code', 'Liberation Mono', 'modern'],
  ['cascadia mono', 'Liberation Mono', 'modern'],
  ['liberation mono', 'Liberation Mono', 'modern'],
  ['cousine', 'Liberation Mono', 'modern'],
  ['dejavu sans mono', 'Liberation Mono', 'modern'],
];

const BY_NAME = new Map(SUBSTITUTES.map(([name, substitute, generic]) => [name, { substitute, generic }]));

/** The kind of a family, from the table of known ones or from its name. */
export function fontGeneric(family: string): FontGeneric {
  const lower = family.trim().toLowerCase();
  const known = BY_NAME.get(lower);
  if (known) return known.generic;
  if (lower.includes('mono') || lower.includes('code') || lower.includes('courier')) return 'modern';
  if (lower.includes('serif') && !lower.includes('sans')) return 'roman';
  return 'auto';
}

/** The bundled family that sets `family` where the reader does not hold it. */
export function fontSubstitute(family: string, generic: FontGeneric = 'auto'): string {
  const lower = family.trim().toLowerCase();
  const known = BY_NAME.get(lower);
  if (known) return known.substitute;
  switch (generic === 'auto' ? fontGeneric(family) : generic) {
    case 'roman':
      return 'Liberation Serif';
    case 'modern':
      return 'Liberation Mono';
    case 'swiss':
      return 'Liberation Sans';
    default:
      return 'Carlito';
  }
}

const CSS_GENERIC: Record<FontGeneric, string> = {
  auto: 'sans-serif',
  swiss: 'sans-serif',
  roman: 'serif',
  modern: 'monospace',
  script: 'cursive',
  decorative: 'fantasy',
};

/**
 * A CSS font stack for a family: the family, its bundled metric-compatible substitute, then the
 * generic family, so Calibri shows as Carlito where Calibri is missing and lines break alike.
 */
export function fontStack(family: string, generic: FontGeneric = 'auto'): string {
  const name = family.trim().replace(/['";{}<>]/g, '');
  const kind = generic === 'auto' ? fontGeneric(name) : generic;
  const substitute = fontSubstitute(name, kind);
  if (!name) return `'${substitute}', ${CSS_GENERIC[kind]}`;
  if (name.toLowerCase() === substitute.toLowerCase()) return `'${name}', ${CSS_GENERIC[kind]}`;
  return `'${name}', '${substitute}', ${CSS_GENERIC[kind]}`;
}

/** The family a font stack starts with, quotes removed. */
export function stackFamily(stack: string | null | undefined): string | null {
  const first = stack?.split(',')[0]?.trim().replace(/^['"]|['"]$/g, '').trim();
  return first ? first : null;
}

const availability = new Map<string, boolean>();

/**
 * Whether the browser draws a family with its own face rather than through another one: a text
 * measured in the family against each generic family it could fall back to. Where nothing can
 * be measured, as outside a browser, the family is taken as available.
 */
export function isFontAvailable(family: string): boolean {
  const name = family.trim().replace(/'/g, '');
  const known = availability.get(name.toLowerCase());
  if (known !== undefined) return known;
  let context: CanvasRenderingContext2D | null = null;
  try {
    context = (globalThis as { document?: Document }).document?.createElement('canvas').getContext('2d') ?? null;
  } catch {
    context = null;
  }
  if (!context) return true;
  const sample = 'mmmmmmmmmmlliWW@#0';
  const width = (font: string) => {
    context.font = `72px ${font}`;
    return context.measureText(sample).width;
  };
  const available = ['monospace', 'serif', 'sans-serif'].some((generic) => width(`'${name}', ${generic}`) !== width(generic));
  availability.set(name.toLowerCase(), available);
  return available;
}

/** The families the runs of editor content set, each once, in the order they first appear. */
export function usedFamilies(content: JSONContent): string[] {
  const families: string[] = [];
  const visit = (node: JSONContent) => {
    for (const mark of node.marks ?? []) {
      const family = mark.type === 'textStyle' ? stackFamily(mark.attrs?.fontFamily as string | undefined) : null;
      if (family && !families.some((known) => known.toLowerCase() === family.toLowerCase())) families.push(family);
    }
    node.content?.forEach(visit);
  };
  visit(content);
  return families;
}

/** The families a picker offers: the document's first, then the standard ones, each once. */
export function pickerFamilies(documentFamilies: readonly string[], standard: readonly string[]): string[] {
  const seen = new Set<string>();
  const families: string[] = [];
  for (const family of [...documentFamilies, ...standard]) {
    const name = family.trim();
    const key = name.toLowerCase();
    if (!name || seen.has(key)) continue;
    seen.add(key);
    families.push(name);
  }
  return families;
}

/** The faces of a document loaded into the page; `release` removes them. */
export interface LoadedFaces {
  families: string[];
  release: () => void;
}

/**
 * Load the faces a document embeds into the page with the FontFace API, so its text shows in its
 * own typeface; a face the browser refuses is skipped.
 */
export async function loadDocumentFonts(
  fonts: readonly DocumentFont[],
  files: readonly Uint8Array[],
): Promise<LoadedFaces> {
  const set = (globalThis as { document?: { fonts?: FontFaceSet } }).document?.fonts;
  const Face = (globalThis as { FontFace?: typeof FontFace }).FontFace;
  if (!set || !Face) return { families: [], release: () => undefined };
  const loaded: FontFace[] = [];
  const families = new Set<string>();
  for (const font of fonts) {
    for (const face of font.faces ?? []) {
      const bytes = files[face.file];
      if (!bytes) continue;
      const bold = face.style === 'bold' || face.style === 'boldItalic';
      const italic = face.style === 'italic' || face.style === 'boldItalic';
      try {
        const fontFace = new Face(font.name, bytes.slice().buffer, {
          weight: bold ? '700' : '400',
          style: italic ? 'italic' : 'normal',
        });
        await fontFace.load();
        set.add(fontFace);
        loaded.push(fontFace);
        families.add(font.name);
      } catch {
        // A face the browser cannot read leaves the family to its substitute.
      }
    }
  }
  return {
    families: [...families],
    release: () => {
      for (const face of loaded) set.delete(face);
    },
  };
}
