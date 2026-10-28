// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import { useId, type CSSProperties, type ReactElement, type ReactNode } from 'react';
import type { Gradient, Paint, PathCommand, ShapeGeometry, ShapePath, Slide, SlideParagraph, SlideShape, SlideTable, TextBody } from '../model.js';
import { EMU_PER_POINT } from './slide-model.js';

/** The images of a presentation by index, as the URLs a page shows them with. */
export type MediaUrls = ReadonlyMap<number, string>;

const SHADES: Record<ShapePath['shade'], string | null> = {
  normal: null,
  darken: 'rgb(0 0 0 / 40%)',
  darkenLess: 'rgb(0 0 0 / 20%)',
  lighten: 'rgb(255 255 255 / 40%)',
  lightenLess: 'rgb(255 255 255 / 20%)',
};

function pathData(commands: PathCommand[]): string {
  return commands
    .map((command) => {
      switch (command.type) {
        case 'moveTo':
          return `M${command.to.x} ${command.to.y}`;
        case 'lineTo':
          return `L${command.to.x} ${command.to.y}`;
        case 'cubicTo':
          return `C${command.controls[0].x} ${command.controls[0].y} ${command.controls[1].x} ${command.controls[1].y} ${command.to.x} ${command.to.y}`;
        case 'close':
          return 'Z';
      }
    })
    .join(' ');
}

function GradientDefinition({ id, gradient }: Readonly<{ id: string; gradient: Gradient }>): ReactElement {
  const stops = gradient.stops.map((stop, index) => (
    <stop key={index} offset={stop.offset} stopColor={`#${stop.colour}`} />
  ));
  if (gradient.kind.type === 'radial') {
    const [cx, cy] = gradient.kind.centre;
    return (
      <radialGradient id={id} cx={cx} cy={cy} r={0.75}>
        {stops}
      </radialGradient>
    );
  }
  const radians = (gradient.kind.angle * Math.PI) / 180;
  const dx = Math.cos(radians) / 2;
  const dy = Math.sin(radians) / 2;
  return (
    <linearGradient id={id} x1={0.5 - dx} y1={0.5 - dy} x2={0.5 + dx} y2={0.5 + dy}>
      {stops}
    </linearGradient>
  );
}

/** The SVG paint of a fill, with the definition a gradient or a picture needs. */
function paintOf(paint: Paint | null, id: string, width: number, height: number, media: MediaUrls): { fill: string; definition: ReactNode } {
  if (paint === null) return { fill: 'none', definition: null };
  switch (paint.type) {
    case 'solid':
      return { fill: `#${paint.colour}`, definition: null };
    case 'gradient':
      return { fill: `url(#${id})`, definition: <GradientDefinition id={id} gradient={paint.gradient} /> };
    case 'picture': {
      const href = media.get(paint.media);
      if (href === undefined) return { fill: 'none', definition: null };
      const [tileWidth, tileHeight] = paint.tileEmu ?? [width, height];
      return {
        fill: `url(#${id})`,
        definition: (
          <pattern id={id} patternUnits="userSpaceOnUse" width={tileWidth} height={tileHeight}>
            <image href={href} width={tileWidth} height={tileHeight} preserveAspectRatio="none" />
          </pattern>
        ),
      };
    }
  }
}

function outlines(geometry: ShapeGeometry, width: number, height: number): { d: string; filled: boolean; stroked: boolean; shade: ShapePath['shade'] }[] {
  const whole = (d: string) => [{ d, filled: true, stroked: true, shade: 'normal' as const }];
  switch (geometry.type) {
    case 'ellipse': {
      const rx = width / 2;
      const ry = height / 2;
      return whole(`M0 ${ry} A${rx} ${ry} 0 1 0 ${width} ${ry} A${rx} ${ry} 0 1 0 0 ${ry} Z`);
    }
    case 'roundedRectangle': {
      const r = Math.min(width, height) * 0.16667;
      return whole(
        `M${r} 0 H${width - r} A${r} ${r} 0 0 1 ${width} ${r} V${height - r} A${r} ${r} 0 0 1 ${width - r} ${height} H${r} A${r} ${r} 0 0 1 0 ${height - r} V${r} A${r} ${r} 0 0 1 ${r} 0 Z`,
      );
    }
    case 'line':
      return [{ d: geometry.flipped ? `M0 ${height} L${width} 0` : `M0 0 L${width} ${height}`, filled: false, stroked: true, shade: 'normal' }];
    case 'path':
      return geometry.paths.map((path) => ({ d: pathData(path.commands), filled: path.filled, stroked: path.stroked, shade: path.shade }));
    default:
      return whole(`M0 0 H${width} V${height} H0 Z`);
  }
}

function ShapeOutline({ shape, media }: Readonly<{ shape: SlideShape; media: MediaUrls }>): ReactElement | null {
  const id = useId().replace(/:/g, '');
  const { width, height } = shape.frame;
  if (width <= 0 && height <= 0) return null;
  const { fill, definition } = paintOf(shape.fill, `rely-paint-${id}`, width, height, media);
  const stroke = shape.line ? `#${shape.line.colour}` : 'none';
  const strokeWidth = shape.line?.widthEmu ?? 0;
  if (fill === 'none' && stroke === 'none') return null;
  return (
    <svg className="rely-slide-outline" viewBox={`0 0 ${Math.max(width, 1)} ${Math.max(height, 1)}`} preserveAspectRatio="none" overflow="visible">
      {definition ? <defs>{definition}</defs> : null}
      {outlines(shape.geometry, width, height).map((outline, index) => (
        <g key={index}>
          <path d={outline.d} fill={outline.filled ? fill : 'none'} stroke={outline.stroked ? stroke : 'none'} strokeWidth={strokeWidth} />
          {outline.filled && SHADES[outline.shade] ? <path d={outline.d} fill={SHADES[outline.shade]!} /> : null}
        </g>
      ))}
    </svg>
  );
}

function paragraphCss(paragraph: SlideParagraph, scale: number): CSSProperties {
  return {
    textAlign: paragraph.align,
    paddingLeft: Math.max(0, paragraph.marginLeftEmu) * scale,
    textIndent: paragraph.indentEmu * scale,
    lineHeight: paragraph.lineSpacingPercent / 100 * 1.2,
    marginTop: (paragraph.spaceBefore / 100) * EMU_PER_POINT * scale,
    marginBottom: (paragraph.spaceAfter / 100) * EMU_PER_POINT * scale,
  };
}

/** A text body at a scale of pixels per EMU, each run with its own look. */
export function TextBodyView({
  body,
  scale,
  editable = false,
  onCommit,
}: Readonly<{ body: TextBody; scale: number; editable?: boolean; onCommit?: (element: HTMLDivElement) => void }>): ReactElement {
  const [left, top, right, bottom] = body.insetsEmu;
  const firstSize = body.paragraphs[0]?.runs[0]?.size ?? 1800;
  return (
    <div
      className="rely-slide-text"
      style={{
        padding: `${top * scale}px ${right * scale}px ${bottom * scale}px ${left * scale}px`,
        justifyContent: body.anchor === 'middle' ? 'center' : body.anchor === 'bottom' ? 'flex-end' : 'flex-start',
        whiteSpace: body.wrap ? 'pre-wrap' : 'pre',
        fontSize: (firstSize / 100) * EMU_PER_POINT * scale,
      }}
    >
      <div
        className="rely-slide-paragraphs"
        contentEditable={editable || undefined}
        suppressContentEditableWarning
        onBlur={editable && onCommit ? (event) => onCommit(event.currentTarget) : undefined}
      >
        {body.paragraphs.map((paragraph, index) => (
          <div key={index} data-paragraph={index} style={paragraphCss(paragraph, scale)}>
            {paragraph.bullet ? <span className="rely-slide-bullet" contentEditable={false}>{`${paragraph.bullet.text} `}</span> : null}
            {paragraph.runs.length === 0 ? <br /> : null}
            {paragraph.runs.map((run, runIndex) =>
              run.lineBreak ? (
                <br key={runIndex} />
              ) : (
                <span
                  key={runIndex}
                  data-run={runIndex}
                  style={{
                    fontSize: (run.size / 100) * EMU_PER_POINT * scale,
                    fontWeight: run.bold ? 700 : 400,
                    fontStyle: run.italic ? 'italic' : 'normal',
                    textDecoration: [run.underline ? 'underline' : '', run.strike ? 'line-through' : ''].filter(Boolean).join(' ') || 'none',
                    color: `#${run.colour}`,
                    fontFamily: run.family ? `${run.family}, sans-serif` : undefined,
                  }}
                >
                  {run.text}
                </span>
              ),
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

/** A cell of a table by its row and its column, as the editor names the one being written. */
export interface TableCellPlace {
  row: number;
  column: number;
}

function TableView({
  table,
  scale,
  activeCell,
  editingCell = false,
  onCellDown,
  onCommitCell,
}: Readonly<{
  table: SlideTable;
  scale: number;
  activeCell?: TableCellPlace | null;
  editingCell?: boolean;
  onCellDown?: (cell: TableCellPlace) => void;
  onCommitCell?: (cell: TableCellPlace, element: HTMLDivElement) => void;
}>): ReactElement {
  return (
    <table className="rely-slide-table">
      <colgroup>
        {table.columnWidthsEmu.map((width, index) => (
          <col key={index} style={{ width: width * scale }} />
        ))}
      </colgroup>
      <tbody>
        {table.rows.map((row, rowIndex) => (
          <tr key={rowIndex} style={{ height: row.heightEmu * scale }}>
            {row.cells.map((cell, cellIndex) =>
              cell.covered ? null : (
                <td
                  key={cellIndex}
                  colSpan={cell.colSpan}
                  rowSpan={cell.rowSpan}
                  className={activeCell?.row === rowIndex && activeCell.column === cellIndex ? 'rely-slide-cell-active' : undefined}
                  style={{ background: cell.fill ? `#${cell.fill}` : undefined }}
                  onPointerDown={onCellDown ? () => onCellDown({ row: rowIndex, column: cellIndex }) : undefined}
                >
                  <TextBodyView
                    body={cell.body}
                    scale={scale}
                    editable={editingCell && activeCell?.row === rowIndex && activeCell.column === cellIndex}
                    onCommit={onCommitCell ? (element) => onCommitCell({ row: rowIndex, column: cellIndex }, element) : undefined}
                  />
                </td>
              ),
            )}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** One shape at its frame: its outline, then what it holds. */
export function ShapeView({
  shape,
  scale,
  media,
  editingText = false,
  onCommitText,
  activeCell,
  onCellDown,
  onCommitCell,
}: Readonly<{
  shape: SlideShape;
  scale: number;
  media: MediaUrls;
  editingText?: boolean;
  onCommitText?: (element: HTMLDivElement) => void;
  activeCell?: TableCellPlace | null;
  onCellDown?: (cell: TableCellPlace) => void;
  onCommitCell?: (cell: TableCellPlace, element: HTMLDivElement) => void;
}>): ReactElement {
  const { content } = shape;
  return (
    <>
      <ShapeOutline shape={shape} media={media} />
      {content.type === 'text' ? (
        <TextBodyView body={content} scale={scale} editable={editingText} onCommit={onCommitText} />
      ) : null}
      {content.type === 'picture' && media.has(content.media) ? (
        <img className="rely-slide-picture" src={media.get(content.media)} alt="" draggable={false} />
      ) : null}
      {content.type === 'table' ? (
        <TableView
          table={content}
          scale={scale}
          activeCell={activeCell}
          editingCell={editingText}
          onCellDown={onCellDown}
          onCommitCell={onCommitCell}
        />
      ) : null}
    </>
  );
}

/** The CSS box of a frame at a scale, rotated about its centre. */
export function frameCss(shape: SlideShape, scale: number): CSSProperties {
  return {
    left: shape.frame.x * scale,
    top: shape.frame.y * scale,
    width: shape.frame.width * scale,
    height: shape.frame.height * scale,
    transform: shape.rotation ? `rotate(${shape.rotation}deg)` : undefined,
  };
}

/** The CSS background of a slide. */
export function backgroundCss(background: Paint | null | undefined, media: MediaUrls): CSSProperties {
  if (!background) return { background: '#ffffff' };
  switch (background.type) {
    case 'solid':
      return { background: `#${background.colour}` };
    case 'gradient': {
      const stops = background.gradient.stops.map((stop) => `#${stop.colour} ${stop.offset * 100}%`).join(', ');
      return background.gradient.kind.type === 'linear'
        ? { background: `linear-gradient(${background.gradient.kind.angle + 90}deg, ${stops})` }
        : { background: `radial-gradient(circle at ${background.gradient.kind.centre[0] * 100}% ${background.gradient.kind.centre[1] * 100}%, ${stops})` };
    }
    case 'picture': {
      const url = media.get(background.media);
      return url ? { background: `url("${url}") center / cover no-repeat, #ffffff` } : { background: '#ffffff' };
    }
  }
}

/** A slide drawn at a width in pixels, read only: the thumbnails and the play view. */
export function SlideView({
  slide,
  widthEmu,
  heightEmu,
  width,
  media,
}: Readonly<{ slide: Slide; widthEmu: number; heightEmu: number; width: number; media: MediaUrls }>): ReactElement {
  const scale = width / widthEmu;
  return (
    <div className="rely-slide" style={{ width, height: heightEmu * scale, ...backgroundCss(slide.background, media) }}>
      {(slide.shapes ?? []).map((shape, index) => (
        <div key={index} className="rely-slide-shape" style={frameCss(shape, scale)}>
          <ShapeView shape={shape} scale={scale} media={media} />
        </div>
      ))}
    </div>
  );
}
