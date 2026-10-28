// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

import { useMemo, useState, type ReactElement } from 'react';
import type {
  Border,
  BorderPlacement,
  CellWindow,
  ComparisonOperator,
  ConditionalFormat,
  DataValidation,
  FilterChoice,
} from '../model.js';
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
