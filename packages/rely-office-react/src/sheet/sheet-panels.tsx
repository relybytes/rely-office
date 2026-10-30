// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

import { useMemo, useState, type ReactElement } from 'react';
import type {
  Border,
  BorderPlacement,
  CellComment,
  CellWindow,
  ChartDefinition,
  ChartType,
  ComparisonOperator,
  ConditionalFormat,
  DataValidation,
  DefinedName,
  FilterChoice,
  SheetChart,
} from '../model.js';
import { cellName } from './geometry.js';
import type { SheetGridLabels } from './labels.js';
import {
  EMPTY_RULE,
  EMPTY_VALIDATION,
  addConditionalFormat,
  describeCondition,
  rangeName,
  replaceValidation,
  validationOf,
  validationsIn,
  windowRange,
  type RuleDraft,
  type RuleKind,
  type ValidationDraft,
} from './sheet-rules.js';

const RULE_KINDS: [RuleKind, keyof SheetGridLabels][] = [
  ['greater', 'ruleGreater'],
  ['less', 'ruleLess'],
  ['between', 'ruleBetween'],
  ['equal', 'ruleEqual'],
  ['textContains', 'ruleTextContains'],
  ['duplicates', 'ruleDuplicates'],
  ['top', 'ruleTop'],
  ['blanks', 'ruleBlanks'],
  ['colorScale', 'ruleColorScale'],
  ['dataBar', 'ruleDataBar'],
  ['expression', 'ruleExpression'],
];

const OPERATORS: [ComparisonOperator, keyof SheetGridLabels][] = [
  ['between', 'operatorBetween'],
  ['notBetween', 'operatorNotBetween'],
  ['equal', 'operatorEqual'],
  ['notEqual', 'operatorNotEqual'],
  ['greater', 'operatorGreater'],
  ['less', 'operatorLess'],
  ['greaterOrEqual', 'operatorGreaterOrEqual'],
  ['lessOrEqual', 'operatorLessOrEqual'],
];

const VALIDATION_KINDS: [DataValidation['kind'], keyof SheetGridLabels][] = [
  ['any', 'validationAny'],
  ['list', 'validationList'],
  ['whole', 'validationWhole'],
  ['decimal', 'validationDecimal'],
  ['date', 'validationDate'],
  ['textLength', 'validationTextLength'],
  ['custom', 'validationCustom'],
];

interface PanelProps {
  words: SheetGridLabels;
  selection: CellWindow;
  onClose: () => void;
}

/** The conditional formats of a sheet: the rules there are, and a form for a new one. */
export function ConditionalFormatPanel({
  words,
  selection,
  formats,
  onChange,
  onClose,
}: PanelProps & { formats: ConditionalFormat[]; onChange: (formats: ConditionalFormat[]) => void }): ReactElement {
  const [draft, setDraft] = useState<RuleDraft>(EMPTY_RULE);
  const [incomplete, setIncomplete] = useState(false);
  const set = (patch: Partial<RuleDraft>) => {
    setIncomplete(false);
    setDraft((current) => ({ ...current, ...patch }));
  };
  const valued = ['greater', 'less', 'between', 'equal', 'textContains', 'top', 'expression'].includes(draft.kind);
  const scaled = draft.kind === 'colorScale' || draft.kind === 'dataBar';
  return (
    <aside className="rely-sheet-panel" aria-label={words.conditionalFormats}>
      <header>
        <h2>{words.conditionalFormats}</h2>
        <button type="button" className="rely-tool" aria-label={words.close} title={words.close} onClick={onClose}>
          {'×'}
        </button>
      </header>
      {formats.length === 0 ? <p className="rely-sheet-panel-empty">{words.noRules}</p> : null}
      <ol className="rely-sheet-rules">
        {formats.map((format, index) => (
          <li key={index}>
            <span className="rely-sheet-rule-range">{format.ranges.map(rangeName).join(', ')}</span>
            <span className="rely-sheet-rule-text">
              {format.rules.map((rule) => (
                <span key={`${rule.priority}-${describeCondition(rule.condition)}`} className="rely-sheet-rule-chip">
                  {rule.look?.fill ? <i style={{ background: `#${rule.look.fill}` }} /> : null}
                  {describeCondition(rule.condition)}
                </span>
              ))}
            </span>
            <button type="button" onClick={() => onChange(formats.filter((_, position) => position !== index))}>
              {words.remove}
            </button>
          </li>
        ))}
      </ol>
      <form
        className="rely-sheet-panel-form"
        onSubmit={(event) => {
          event.preventDefault();
          const next = addConditionalFormat(formats, selection, draft);
          if (next === null) {
            setIncomplete(true);
            return;
          }
          onChange(next);
          setDraft((current) => ({ ...current, first: '', second: '' }));
        }}
      >
        <p className="rely-sheet-panel-target">
          {words.appliesTo} <strong>{rangeName(windowRange(selection))}</strong>
        </p>
        <label>
          <span>{words.ruleKind}</span>
          <select value={draft.kind} onChange={(event) => set({ kind: event.target.value as RuleKind })}>
            {RULE_KINDS.map(([kind, key]) => (
              <option key={kind} value={kind}>
                {words[key] as string}
              </option>
            ))}
          </select>
        </label>
        {valued ? (
          <label>
            <span>{words.value}</span>
            <input value={draft.first} onChange={(event) => set({ first: event.target.value })} />
          </label>
        ) : null}
        {draft.kind === 'between' ? (
          <label>
            <span>{words.secondValue}</span>
            <input value={draft.second} onChange={(event) => set({ second: event.target.value })} />
          </label>
        ) : null}
        {scaled ? (
          <div className="rely-sheet-panel-row">
            {draft.kind === 'colorScale' ? (
              <label>
                <span>{words.lowColour}</span>
                <input type="color" value={draft.low} onChange={(event) => set({ low: event.target.value })} />
              </label>
            ) : null}
            <label>
              <span>{words.highColour}</span>
              <input type="color" value={draft.high} onChange={(event) => set({ high: event.target.value })} />
            </label>
          </div>
        ) : (
          <div className="rely-sheet-panel-row">
            <label>
              <span>{words.fill}</span>
              <input type="color" value={draft.fill} onChange={(event) => set({ fill: event.target.value })} />
            </label>
            <label>
              <span>{words.fontColour}</span>
              <input type="color" value={draft.fontColour} onChange={(event) => set({ fontColour: event.target.value })} />
            </label>
            <label className="rely-sheet-panel-check">
              <input type="checkbox" checked={draft.bold} onChange={(event) => set({ bold: event.target.checked })} />
              <span>{words.bold}</span>
            </label>
          </div>
        )}
        {incomplete ? (
          <p className="rely-sheet-panel-error" role="alert">
            {words.ruleIncomplete}
          </p>
        ) : null}
        <button type="submit" className="rely-sheet-panel-submit">
          {words.addRule}
        </button>
      </form>
    </aside>
  );
}

/** The validation of the selected cells, with the others of the sheet listed. */
export function ValidationPanel({
  words,
  selection,
  validations,
  onChange,
  onClose,
}: PanelProps & { validations: DataValidation[]; onChange: (validations: DataValidation[]) => void }): ReactElement {
  const [draft, setDraft] = useState<ValidationDraft>(EMPTY_VALIDATION);
  const [incomplete, setIncomplete] = useState(false);
  const set = (patch: Partial<ValidationDraft>) => {
    setIncomplete(false);
    setDraft((current) => ({ ...current, ...patch }));
  };
  const here = validationsIn(validations, selection);
  const compared = !['any', 'list', 'custom'].includes(draft.kind);
  const two = compared && (draft.operator === 'between' || draft.operator === 'notBetween');
  return (
    <aside className="rely-sheet-panel" aria-label={words.validation}>
      <header>
        <h2>{words.validation}</h2>
        <button type="button" className="rely-tool" aria-label={words.close} title={words.close} onClick={onClose}>
          {'×'}
        </button>
      </header>
      {validations.length === 0 ? <p className="rely-sheet-panel-empty">{words.noRules}</p> : null}
      <ol className="rely-sheet-rules">
        {validations.map((validation, index) => (
          <li key={index} className={here.includes(validation) ? 'rely-sheet-rule-here' : undefined}>
            <span className="rely-sheet-rule-range">{validation.ranges.map(rangeName).join(', ')}</span>
            <span className="rely-sheet-rule-text">
              {words[VALIDATION_KINDS.find(([kind]) => kind === validation.kind)?.[1] ?? 'validationAny'] as string}
              {validation.formulas && validation.formulas.length > 0 ? ` ${validation.formulas.join(' – ')}` : ''}
            </span>
            <button type="button" onClick={() => onChange(validations.filter((_, position) => position !== index))}>
              {words.remove}
            </button>
          </li>
        ))}
      </ol>
      <form
        className="rely-sheet-panel-form"
        onSubmit={(event) => {
          event.preventDefault();
          const validation = validationOf(draft, selection);
          if (validation === null && draft.kind !== 'any') {
            setIncomplete(true);
            return;
          }
          onChange(replaceValidation(validations, selection, validation));
        }}
      >
        <p className="rely-sheet-panel-target">
          {words.appliesTo} <strong>{rangeName(windowRange(selection))}</strong>
        </p>
        <label>
          <span>{words.validationKind}</span>
          <select value={draft.kind} onChange={(event) => set({ kind: event.target.value as DataValidation['kind'] })}>
            {VALIDATION_KINDS.map(([kind, key]) => (
              <option key={kind} value={kind}>
                {words[key] as string}
              </option>
            ))}
          </select>
        </label>
        {draft.kind === 'list' ? (
          <label>
            <span>{words.validationItems}</span>
            <input value={draft.first} onChange={(event) => set({ first: event.target.value })} />
          </label>
        ) : null}
        {draft.kind === 'custom' ? (
          <label>
            <span>{words.formula}</span>
            <input value={draft.first} placeholder="=A1>0" onChange={(event) => set({ first: event.target.value })} />
          </label>
        ) : null}
        {compared ? (
          <>
            <label>
              <span>{words.operator}</span>
              <select value={draft.operator} onChange={(event) => set({ operator: event.target.value as ComparisonOperator })}>
                {OPERATORS.map(([operator, key]) => (
                  <option key={operator} value={operator}>
                    {words[key] as string}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <span>{words.value}</span>
              <input value={draft.first} onChange={(event) => set({ first: event.target.value })} />
            </label>
            {two ? (
              <label>
                <span>{words.secondValue}</span>
                <input value={draft.second} onChange={(event) => set({ second: event.target.value })} />
              </label>
            ) : null}
          </>
        ) : null}
        {draft.kind !== 'any' ? (
          <>
            <label className="rely-sheet-panel-check">
              <input type="checkbox" checked={draft.allowBlank} onChange={(event) => set({ allowBlank: event.target.checked })} />
              <span>{words.allowBlank}</span>
            </label>
            <label>
              <span>{words.errorStyle}</span>
              <select
                value={draft.errorStyle}
                onChange={(event) => set({ errorStyle: event.target.value as ValidationDraft['errorStyle'] })}
              >
                <option value="stop">{words.errorStop}</option>
                <option value="warning">{words.errorWarning}</option>
                <option value="information">{words.errorInformation}</option>
              </select>
            </label>
            <label>
              <span>{words.errorMessage}</span>
              <input value={draft.error} onChange={(event) => set({ error: event.target.value })} />
            </label>
            <label>
              <span>{words.promptMessage}</span>
              <input value={draft.prompt} onChange={(event) => set({ prompt: event.target.value })} />
            </label>
          </>
        ) : null}
        {incomplete ? (
          <p className="rely-sheet-panel-error" role="alert">
            {words.ruleIncomplete}
          </p>
        ) : null}
        <button type="submit" className="rely-sheet-panel-submit">
          {words.saveValidation}
        </button>
      </form>
    </aside>
  );
}

/**
 * The comments of a sheet, and the comment of the selected cell to write or remove. A new comment
 * opens a thread, as Excel 365 does; a thread is answered, resolved and reopened here.
 */
export function CommentPanel({
  words,
  selection,
  comments,
  onChange,
  onStart,
  onReply,
  onResolve,
  onRemoveReply,
  onSelect,
  onClose,
}: PanelProps & {
  comments: CellComment[];
  onChange: (row: number, column: number, text: string) => void;
  onStart: (row: number, column: number, text: string) => void;
  onReply: (row: number, column: number, text: string) => void;
  onResolve: (row: number, column: number, resolved: boolean) => void;
  onRemoveReply: (row: number, column: number, reply: number) => void;
  onSelect: (row: number, column: number) => void;
}): ReactElement {
  const target = { row: selection.top, column: selection.left };
  const current = comments.find((comment) => comment.row === target.row && comment.column === target.column);
  const [draft, setDraft] = useState(current?.text ?? '');
  const [reply, setReply] = useState('');
  const [shown, setShown] = useState(`${target.row}:${target.column}:${current?.text ?? ''}`);
  // A new cell selected, or its comment changed elsewhere, starts the draft again.
  const key = `${target.row}:${target.column}:${current?.text ?? ''}`;
  if (key !== shown) {
    setShown(key);
    setDraft(current?.text ?? '');
    setReply('');
  }
  return (
    <aside className="rely-sheet-panel" aria-label={words.comments}>
      <header>
        <h2>{words.comments}</h2>
        <button type="button" className="rely-tool" aria-label={words.close} title={words.close} onClick={onClose}>
          {'×'}
        </button>
      </header>
      {comments.length === 0 ? <p className="rely-sheet-panel-empty">{words.noComments}</p> : null}
      <ol className="rely-sheet-rules">
        {comments.map((comment) => (
          <li
            key={`${comment.row}:${comment.column}`}
            className={comment === current ? 'rely-sheet-rule-here' : undefined}
          >
            <button type="button" className="rely-sheet-rule-range" onClick={() => onSelect(comment.row, comment.column)}>
              {cellName(comment)}
            </button>
            <span className="rely-sheet-rule-text">
              {comment.author ? <strong>{comment.author}: </strong> : null}
              {comment.text}
              {comment.resolved ? <em className="rely-sheet-comment-resolved"> {words.resolved}</em> : null}
            </span>
            <button type="button" onClick={() => onChange(comment.row, comment.column, '')}>
              {words.remove}
            </button>
            {comment.replies.length > 0 ? (
              <ol className="rely-sheet-comment-replies">
                {comment.replies.map((answer, index) => (
                  <li key={index}>
                    <span className="rely-sheet-rule-text">
                      {answer.author ? <strong>{answer.author}: </strong> : null}
                      {answer.text}
                    </span>
                    <button
                      type="button"
                      aria-label={words.removeReply}
                      title={words.removeReply}
                      onClick={() => onRemoveReply(comment.row, comment.column, index)}
                    >
                      {'×'}
                    </button>
                  </li>
                ))}
              </ol>
            ) : null}
          </li>
        ))}
      </ol>
      {current ? (
        <form
          className="rely-sheet-panel-form"
          onSubmit={(event) => {
            event.preventDefault();
            if (reply.trim() === '') return;
            onReply(target.row, target.column, reply);
            setReply('');
          }}
        >
          <label>
            <span>{words.reply}</span>
            <textarea
              value={reply}
              rows={2}
              placeholder={words.replyPlaceholder}
              onChange={(event) => setReply(event.target.value)}
            />
          </label>
          <div className="rely-sheet-panel-actions">
            <button type="submit" className="rely-sheet-panel-submit">
              {words.reply}
            </button>
            <button type="button" onClick={() => onResolve(target.row, target.column, !current.resolved)}>
              {current.resolved ? words.reopen : words.resolve}
            </button>
          </div>
        </form>
      ) : null}
      <form
        className="rely-sheet-panel-form"
        onSubmit={(event) => {
          event.preventDefault();
          if (current) onChange(target.row, target.column, draft);
          else if (draft.trim() !== '') onStart(target.row, target.column, draft);
        }}
      >
        <p className="rely-sheet-panel-target">
          {words.commentOn} <strong>{cellName(target)}</strong>
        </p>
        <label>
          <span>{words.comments}</span>
          <textarea
            value={draft}
            rows={4}
            placeholder={words.commentPlaceholder}
            onChange={(event) => setDraft(event.target.value)}
          />
        </label>
        <button type="submit" className="rely-sheet-panel-submit">
          {words.saveComment}
        </button>
      </form>
    </aside>
  );
}

export const CHART_TYPES: [ChartType, keyof SheetGridLabels][] = [
  ['column', 'chartColumn'],
  ['bar', 'chartBar'],
  ['line', 'chartLine'],
  ['area', 'chartArea'],
  ['pie', 'chartPie'],
  ['doughnut', 'chartDoughnut'],
  ['scatter', 'chartScatter'],
];

/** The charts of a sheet: one to insert from the selection, and the chosen one to change. */
export function ChartPanel({
  words,
  charts,
  chosen,
  error,
  onChoose,
  onInsert,
  onChange,
  onUseSelection,
  onRemove,
  onClose,
}: {
  words: SheetGridLabels;
  charts: SheetChart[];
  chosen: number | null;
  /** Why the last change was refused, if it was. */
  error: string | null;
  onChoose: (index: number | null) => void;
  onInsert: (type: ChartType) => void;
  onChange: (index: number, chart: ChartDefinition) => void;
  onUseSelection: (index: number) => void;
  onRemove: (index: number) => void;
  onClose: () => void;
}): ReactElement {
  const [type, setType] = useState<ChartType>('column');
  const current = chosen === null ? undefined : charts[chosen];
  const spec = current?.spec ?? null;
  const name = (chart: SheetChart) =>
    chart.spec?.title || `${words.chartLabel} ${chart.index + 1}`;
  const stackable = spec !== null && ['column', 'bar', 'line', 'area'].includes(spec.chartType);
  return (
    <aside className="rely-sheet-panel" aria-label={words.charts}>
      <header>
        <h2>{words.charts}</h2>
        <button type="button" className="rely-tool" aria-label={words.close} title={words.close} onClick={onClose}>
          {'×'}
        </button>
      </header>
      {charts.length === 0 ? <p className="rely-sheet-panel-empty">{words.noCharts}</p> : null}
      <ol className="rely-sheet-rules">
        {charts.map((chart) => (
          <li key={chart.index} className={chart.index === chosen ? 'rely-sheet-rule-here' : undefined}>
            <button type="button" className="rely-sheet-rule-range" onClick={() => onChoose(chart.index)}>
              {name(chart)}
            </button>
            <span className="rely-sheet-rule-text">
              {chart.spec ? (words[CHART_TYPES.find(([kind]) => kind === chart.spec?.chartType)?.[1] ?? 'chartColumn'] as string) : ''}
            </span>
            <button type="button" onClick={() => onRemove(chart.index)}>
              {words.remove}
            </button>
          </li>
        ))}
      </ol>
      <form
        className="rely-sheet-panel-form"
        onSubmit={(event) => {
          event.preventDefault();
          onInsert(type);
        }}
      >
        <label>
          <span>{words.chartType}</span>
          <select value={type} onChange={(event) => setType(event.target.value as ChartType)}>
            {CHART_TYPES.map(([kind, key]) => (
              <option key={kind} value={kind}>
                {words[key] as string}
              </option>
            ))}
          </select>
        </label>
        <button type="submit" className="rely-sheet-panel-submit">
          {words.insertChart}
        </button>
      </form>
      {current ? (
        <form className="rely-sheet-panel-form" onSubmit={(event) => event.preventDefault()}>
          <p className="rely-sheet-panel-target">
            <strong>{name(current)}</strong>
          </p>
          {spec ? (
            <>
              <label>
                <span>{words.chartType}</span>
                <select
                  value={spec.chartType}
                  onChange={(event) => onChange(current.index, { ...spec, chartType: event.target.value as ChartType })}
                >
                  {CHART_TYPES.map(([kind, key]) => (
                    <option key={kind} value={kind}>
                      {words[key] as string}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                <span>{words.chartTitle}</span>
                <input
                  key={`${current.index}:${spec.title ?? ''}`}
                  defaultValue={spec.title ?? ''}
                  onBlur={(event) => {
                    const title = event.target.value.trim();
                    if (title !== (spec.title ?? '')) onChange(current.index, { ...spec, title: title || null });
                  }}
                />
              </label>
              <label className="rely-sheet-panel-check">
                <input
                  type="checkbox"
                  checked={spec.legend}
                  onChange={(event) => onChange(current.index, { ...spec, legend: event.target.checked })}
                />
                <span>{words.chartLegend}</span>
              </label>
              {stackable ? (
                <label className="rely-sheet-panel-check">
                  <input
                    type="checkbox"
                    checked={spec.stacked}
                    onChange={(event) => onChange(current.index, { ...spec, stacked: event.target.checked })}
                  />
                  <span>{words.chartStacked}</span>
                </label>
              ) : null}
              <button type="button" onClick={() => onUseSelection(current.index)}>
                {words.chartUseSelection}
              </button>
            </>
          ) : (
            <p className="rely-sheet-panel-empty">{words.chartFromFile}</p>
          )}
          <button type="button" onClick={() => onRemove(current.index)}>
            {words.removeChart}
          </button>
        </form>
      ) : null}
      {error ? (
        <p className="rely-sheet-panel-error" role="alert">
          {error}
        </p>
      ) : null}
    </aside>
  );
}

/** The names a workbook defines, and a form to define one or change what it refers to. */
export function NamesPanel({
  words,
  sheets,
  names,
  error,
  onDefine,
  onRemove,
  onClose,
}: {
  words: SheetGridLabels;
  /** The names of the sheets, for a name's scope. */
  sheets: string[];
  names: DefinedName[];
  /** Why the last definition was refused, if it was. */
  error: string | null;
  onDefine: (name: string, formula: string, sheet: number | undefined) => void;
  onRemove: (name: string, sheet: number | undefined) => void;
  onClose: () => void;
}): ReactElement {
  const [draft, setDraft] = useState({ name: '', formula: '', scope: '' });
  return (
    <aside className="rely-sheet-panel" aria-label={words.definedNames}>
      <header>
        <h2>{words.definedNames}</h2>
        <button type="button" className="rely-tool" aria-label={words.close} title={words.close} onClick={onClose}>
          {'×'}
        </button>
      </header>
      {names.length === 0 ? <p className="rely-sheet-panel-empty">{words.noNames}</p> : null}
      <ol className="rely-sheet-rules">
        {names
          .filter((name) => !name.hidden)
          .map((name) => (
            <li key={`${name.sheet ?? ''}:${name.name}`}>
              <button
                type="button"
                className="rely-sheet-rule-range"
                onClick={() =>
                  setDraft({ name: name.name, formula: name.formula, scope: name.sheet === undefined ? '' : String(name.sheet) })
                }
              >
                {name.name}
              </button>
              <span className="rely-sheet-rule-text">
                {name.formula}
                {name.sheet === undefined ? '' : ` (${sheets[name.sheet] ?? ''})`}
              </span>
              <button type="button" onClick={() => onRemove(name.name, name.sheet)}>
                {words.remove}
              </button>
            </li>
          ))}
      </ol>
      <form
        className="rely-sheet-panel-form"
        onSubmit={(event) => {
          event.preventDefault();
          onDefine(draft.name.trim(), draft.formula, draft.scope === '' ? undefined : Number(draft.scope));
        }}
      >
        <label>
          <span>{words.name}</span>
          <input value={draft.name} onChange={(event) => setDraft({ ...draft, name: event.target.value })} />
        </label>
        <label>
          <span>{words.nameRefersTo}</span>
          <input
            value={draft.formula}
            placeholder="=Foglio1!$A$1:$A$10"
            onChange={(event) => setDraft({ ...draft, formula: event.target.value })}
          />
        </label>
        <label>
          <span>{words.nameScope}</span>
          <select value={draft.scope} onChange={(event) => setDraft({ ...draft, scope: event.target.value })}>
            <option value="">{words.wholeWorkbook}</option>
            {sheets.map((sheet, index) => (
              <option key={sheet} value={String(index)}>
                {sheet}
              </option>
            ))}
          </select>
        </label>
        {error ? (
          <p className="rely-sheet-panel-error" role="alert">
            {error}
          </p>
        ) : null}
        <button type="submit" className="rely-sheet-panel-submit">
          {words.addName}
        </button>
      </form>
    </aside>
  );
}

/** The menu of a filter button: sort, and which values the column shows. */
export function FilterMenu({
  words,
  choices,
  shown,
  position,
  onApply,
  onSort,
  onClose,
}: {
  words: SheetGridLabels;
  choices: FilterChoice[];
  /** The keys shown now; null when the column is not filtered. */
  shown: string[] | null;
  position: { top: number; left: number };
  onApply: (shown: string[] | null) => void;
  onSort: (descending: boolean) => void;
  onClose: () => void;
}): ReactElement {
  const [checked, setChecked] = useState<Set<string>>(() => new Set(shown ?? choices.map((choice) => choice.key)));
  const [search, setSearch] = useState('');
  const visible = useMemo(
    () => choices.filter((choice) => choice.label.toLowerCase().includes(search.trim().toLowerCase())),
    [choices, search],
  );
  const all = visible.every((choice) => checked.has(choice.key));
  return (
    <div
      className="rely-sheet-filter-menu"
      role="dialog"
      aria-label={words.filterColumn}
      style={position}
      onKeyDown={(event) => {
        if (event.key === 'Escape') onClose();
      }}
    >
      <button type="button" onClick={() => onSort(false)}>
        {words.sortAscending}
      </button>
      <button type="button" onClick={() => onSort(true)}>
        {words.sortDescending}
      </button>
      <input
        className="rely-sheet-filter-search"
        aria-label={words.filterSearch}
        placeholder={words.filterSearch}
        value={search}
        autoFocus
        onChange={(event) => setSearch(event.target.value)}
      />
      <div className="rely-sheet-filter-list">
        <label>
          <input
            type="checkbox"
            checked={all}
            onChange={() =>
              setChecked((current) => {
                const next = new Set(current);
                for (const choice of visible) {
                  if (all) next.delete(choice.key);
                  else next.add(choice.key);
                }
                return next;
              })
            }
          />
          <span>{words.filterSelectAll}</span>
        </label>
        {visible.map((choice) => (
          <label key={choice.key}>
            <input
              type="checkbox"
              checked={checked.has(choice.key)}
              onChange={() =>
                setChecked((current) => {
                  const next = new Set(current);
                  if (next.has(choice.key)) next.delete(choice.key);
                  else next.add(choice.key);
                  return next;
                })
              }
            />
            <span>{choice.key === '' ? words.filterBlank : choice.label}</span>
          </label>
        ))}
      </div>
      <div className="rely-sheet-filter-actions">
        <button
          type="button"
          onClick={() => onApply(choices.every((choice) => checked.has(choice.key)) ? null : [...checked])}
        >
          {words.apply}
        </button>
        <button type="button" onClick={onClose}>
          {words.cancel}
        </button>
      </div>
    </div>
  );
}

const LINES: [keyof SheetGridLabels, Border][] = [
  ['lineThin', { style: 'single', widthEighths: 4 }],
  ['lineMedium', { style: 'single', widthEighths: 12 }],
  ['lineThick', { style: 'single', widthEighths: 18 }],
  ['lineDashed', { style: 'dashed', widthEighths: 4 }],
  ['lineDotted', { style: 'dotted', widthEighths: 4 }],
  ['lineDouble', { style: 'double', widthEighths: 4 }],
];

const PLACEMENTS: [BorderPlacement, keyof SheetGridLabels, string][] = [
  ['all', 'borderAll', '▦'],
  ['outline', 'borderOutline', '□'],
  ['inside', 'borderInside', '┼'],
  ['top', 'borderTop', '▔'],
  ['bottom', 'borderBottom', '▁'],
  ['left', 'borderLeft', '▏'],
  ['right', 'borderRight', '▕'],
];

/** The borders menu: where the border goes, its line and its colour. */
export function BorderMenu({
  words,
  onApply,
  onClose,
}: {
  words: SheetGridLabels;
  onApply: (placement: BorderPlacement, border: Border | null) => void;
  onClose: () => void;
}): ReactElement {
  const [line, setLine] = useState(0);
  const [colour, setColour] = useState('#000000');
  const border = (): Border => ({ ...LINES[line][1], colour: colour.replace('#', '').toUpperCase() });
  return (
    <div
      className="rely-sheet-border-menu"
      role="dialog"
      aria-label={words.borders}
      onKeyDown={(event) => {
        if (event.key === 'Escape') onClose();
      }}
    >
      <div className="rely-sheet-border-grid">
        {PLACEMENTS.map(([placement, key, glyph]) => (
          <button
            key={placement}
            type="button"
            className="rely-tool"
            aria-label={words[key] as string}
            title={words[key] as string}
            onClick={() => onApply(placement, border())}
          >
            {glyph}
          </button>
        ))}
        <button
          type="button"
          className="rely-tool"
          aria-label={words.borderNone}
          title={words.borderNone}
          onClick={() => onApply('all', null)}
        >
          {'⊘'}
        </button>
      </div>
      <label>
        <span>{words.borderLine}</span>
        <select value={line} onChange={(event) => setLine(Number(event.target.value))}>
          {LINES.map(([key], index) => (
            <option key={key} value={index}>
              {words[key] as string}
            </option>
          ))}
        </select>
      </label>
      <label>
        <span>{words.borderColour}</span>
        <input type="color" value={colour} onChange={(event) => setColour(event.target.value)} />
      </label>
    </div>
  );
}
