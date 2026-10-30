// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

import { useState, type ReactElement } from 'react';
import type { Animation, Slide, Transition } from '../model.js';
import type { SlideEditorLabels } from './labels.js';
import { ANIMATION_PRESETS, TRANSITION_EFFECTS, animationName, withAnimationOrder } from './slide-tools.js';

const DIRECTED = new Set(['push', 'wipe', 'cover', 'pull']);
const TRIGGERS: [Animation['trigger'], keyof SlideEditorLabels][] = [
  ['onClick', 'triggerOnClick'],
  ['withPrevious', 'triggerWithPrevious'],
  ['afterPrevious', 'triggerAfterPrevious'],
];

function seconds(milliseconds: number | undefined): string {
  return milliseconds === undefined ? '' : String(Math.round(milliseconds / 100) / 10);
}

function milliseconds(text: string): number | undefined {
  const value = Number(text.replace(',', '.'));
  return text.trim() === '' || !Number.isFinite(value) || value < 0 ? undefined : Math.round(value * 1000);
}

/** The transition of the slide and the animations of its shapes, with the ones to add. */
export function SlideEffectsPanel({
  slide,
  selected,
  words,
  onChange,
  onClose,
}: Readonly<{
  slide: Slide;
  /** The shapes an animation added goes on. */
  selected: number[];
  words: SlideEditorLabels;
  onChange: (slide: Slide) => void;
  onClose: () => void;
}>): ReactElement {
  const [preset, setPreset] = useState(1);
  const transition = slide.transition ?? null;
  const animations = slide.animations ?? [];

  const setTransition = (patch: Partial<Transition> | null) => {
    if (patch === null) {
      onChange({ ...slide, transition: null });
      return;
    }
    const base: Transition = transition ?? { effect: 'fade', advanceOnClick: true, durationMs: 700 };
    onChange({ ...slide, transition: { ...base, ...patch } });
  };
  const setAnimation = (index: number, patch: Partial<Animation>) =>
    onChange({ ...slide, animations: animations.map((each, at) => (at === index ? { ...each, ...patch } : each)) });

  return (
    <aside className="rely-slide-panel" aria-label={words.effects}>
      <header>
        <h2>{words.effects}</h2>
        <button type="button" className="rely-tool" aria-label={words.close} title={words.close} onClick={onClose}>
          {'×'}
        </button>
      </header>
      <fieldset>
        <legend>{words.transition}</legend>
        <label>
          <span>{words.transition}</span>
          <select
            value={transition?.effect ?? 'none'}
            onChange={(event) => setTransition(event.target.value === 'none' ? null : { effect: event.target.value })}
          >
            {TRANSITION_EFFECTS.map((effect) => (
              <option key={effect} value={effect}>
                {words.effectName(effect)}
              </option>
            ))}
          </select>
        </label>
        {transition && DIRECTED.has(transition.effect) ? (
          <label>
            <span>{words.direction}</span>
            <select value={transition.direction ?? 'l'} onChange={(event) => setTransition({ direction: event.target.value })}>
              <option value="l">{words.directionLeft}</option>
              <option value="r">{words.directionRight}</option>
              <option value="u">{words.directionUp}</option>
              <option value="d">{words.directionDown}</option>
            </select>
          </label>
        ) : null}
        {transition ? (
          <>
            <label>
              <span>{words.duration}</span>
              <input
                type="number"
                min={0}
                step={0.1}
                value={seconds(transition.durationMs)}
                onChange={(event) => setTransition({ durationMs: milliseconds(event.target.value) })}
              />
            </label>
            <label className="rely-slide-panel-check">
              <input
                type="checkbox"
                checked={transition.advanceOnClick}
                onChange={(event) => setTransition({ advanceOnClick: event.target.checked })}
              />
              <span>{words.advanceOnClick}</span>
            </label>
            <label>
              <span>{words.advanceAfter}</span>
              <input
                type="number"
                min={0}
                step={0.5}
                value={seconds(transition.advanceAfterMs)}
                onChange={(event) => setTransition({ advanceAfterMs: milliseconds(event.target.value) })}
              />
            </label>
          </>
        ) : null}
      </fieldset>
      <fieldset>
        <legend>{words.animations}</legend>
        {animations.length === 0 ? <p className="rely-slide-panel-empty">{words.noAnimations}</p> : null}
        <ol className="rely-slide-animations">
          {animations.map((animation, index) => (
            <li key={index} className={selected.includes(animation.shape) ? 'rely-slide-animation-here' : undefined}>
              <span className="rely-slide-animation-name">
                {`${index + 1}. ${words.animationOf(animation.shape + 1)} · ${words[animation.class === 'path' ? 'emphasis' : animation.class]} · ${words.effectName(animationName(animation))}`}
              </span>
              <select
                aria-label={words.trigger}
                value={animation.trigger}
                onChange={(event) => setAnimation(index, { trigger: event.target.value as Animation['trigger'] })}
              >
                {TRIGGERS.map(([trigger, key]) => (
                  <option key={trigger} value={trigger}>
                    {words[key] as string}
                  </option>
                ))}
              </select>
              <label>
                <span>{words.duration}</span>
                <input
                  type="number"
                  min={0}
                  step={0.1}
                  value={seconds(animation.durationMs)}
                  onChange={(event) => setAnimation(index, { durationMs: milliseconds(event.target.value) ?? 0 })}
                />
              </label>
              <label>
                <span>{words.delay}</span>
                <input
                  type="number"
                  min={0}
                  step={0.1}
                  value={seconds(animation.delayMs)}
                  onChange={(event) => setAnimation(index, { delayMs: milliseconds(event.target.value) ?? 0 })}
                />
              </label>
              <span className="rely-slide-animation-actions">
                <button type="button" aria-label={words.moveEarlier} title={words.moveEarlier} disabled={index === 0} onClick={() => onChange(withAnimationOrder(slide, index, -1))}>
                  {'↑'}
                </button>
                <button
                  type="button"
                  aria-label={words.moveLater}
                  title={words.moveLater}
                  disabled={index === animations.length - 1}
                  onClick={() => onChange(withAnimationOrder(slide, index, 1))}
                >
                  {'↓'}
                </button>
                <button
                  type="button"
                  aria-label={words.removeAnimation}
                  title={words.removeAnimation}
                  onClick={() => onChange({ ...slide, animations: animations.filter((_, at) => at !== index) })}
                >
                  {'×'}
                </button>
              </span>
            </li>
          ))}
        </ol>
        <div className="rely-slide-panel-add">
          <select aria-label={words.animations} value={preset} onChange={(event) => setPreset(Number(event.target.value))}>
            {ANIMATION_PRESETS.map((each, index) => (
              <option key={index} value={index}>
                {`${words[each.class === 'path' ? 'emphasis' : each.class]}: ${words.effectName(each.name)}`}
              </option>
            ))}
          </select>
          <button
            type="button"
            disabled={selected.length === 0}
            onClick={() => {
              const chosen = ANIMATION_PRESETS[preset];
              const added: Animation[] = selected.map((shape, position) => ({
                shape,
                class: chosen.class,
                preset: chosen.preset,
                presetSubtype: chosen.preset === 2 ? 4 : 0,
                trigger: position === 0 ? 'onClick' : 'withPrevious',
                delayMs: 0,
                durationMs: chosen.preset === 1 && chosen.class !== 'emphasis' ? 0 : 500,
              }));
              onChange({ ...slide, animations: [...animations, ...added] });
            }}
          >
            {words.addAnimation}
          </button>
        </div>
      </fieldset>
    </aside>
  );
}
