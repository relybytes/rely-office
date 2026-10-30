// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type ReactElement } from 'react';
import { mediaDataUrl } from '../document/media.js';
import type { Animation, Presentation, Transition } from '../model.js';
import { ENGLISH_SLIDE_EDITOR_LABELS, type SlideEditorLabels } from './labels.js';
import { animationName, initiallyHidden, showSteps, shownSlides, type ShowEffect } from './slide-tools.js';
import { ShapeView, backgroundCss, frameCss, type MediaUrls } from './slide-view.js';

export interface SlideShowProps {
  presentation: Presentation;
  /** The slide to start from, an index of `presentation.slides`. */
  start?: number;
  /** Called when the show ends or the viewer leaves it. */
  onClose: () => void;
  labels?: Partial<SlideEditorLabels>;
  /** False plays in the page instead of asking the browser for the whole screen. */
  fullscreen?: boolean;
}

/** Where the show is: the slide, the steps of its effects run so far, and the end screen. */
interface Place {
  slide: number;
  steps: number;
}

const DIRECTIONS: Record<string, string> = { l: '100%, 0', r: '-100%, 0', u: '0, 100%', d: '0, -100%' };

/** The animation CSS of a slide coming in. */
export function transitionCss(transition: Transition | null | undefined): CSSProperties {
  if (!transition || transition.effect === 'none' || transition.effect === 'cut') return {};
  const duration = transition.durationMs ?? 700;
  const from = DIRECTIONS[transition.direction ?? 'l'] ?? DIRECTIONS.l;
  const name = ['push', 'cover', 'pull'].includes(transition.effect) ? 'rely-transition-slide' : `rely-transition-${transition.effect}`;
  return { animation: `${name} ${duration}ms ease-out both`, '--rely-transition-from': `translate(${from})` } as CSSProperties;
}

/** The animation CSS of one effect on its shape. */
export function effectCss(effect: ShowEffect): CSSProperties {
  const animation: Animation = effect.animation;
  const name = `rely-effect-${animation.class}-${animationName(animation)}`;
  const duration = animation.preset === 1 && animation.class !== 'emphasis' ? 1 : Math.max(1, animation.durationMs || 500);
  return { animation: `${name} ${duration}ms ease-out ${effect.startMs}ms both` };
}

/**
 * The slides played one after the other over the whole screen, as PowerPoint plays them: a
 * click, the space bar or an arrow runs the next effect or brings the next slide in with its
 * transition; Escape leaves.
 */
export function SlideShow({ presentation, start = 0, onClose, labels, fullscreen = true }: SlideShowProps): ReactElement {
  const words = useMemo(() => ({ ...ENGLISH_SLIDE_EDITOR_LABELS, ...labels }), [labels]);
  const order = useMemo(() => shownSlides(presentation), [presentation]);
  const firstPosition = Math.max(0, order.findIndex((index) => index >= start));
  const [place, setPlace] = useState<Place>({ slide: firstPosition, steps: 1 });
  const [size, setSize] = useState({ width: 1280, height: 720 });
  const root = useRef<HTMLDivElement>(null);
  const media: MediaUrls = useMemo(() => new Map(presentation.media.map((item) => [item.index, mediaDataUrl(item)])), [presentation.media]);

  const slideIndex = order[place.slide];
  const slide = slideIndex === undefined ? undefined : presentation.slides[slideIndex];
  const steps = useMemo(() => (slide ? showSteps(slide) : [[]]), [slide]);
  const ended = place.slide >= order.length;

  const next = useCallback(() => {
    setPlace((current) => {
      if (current.slide >= order.length) {
        onClose();
        return current;
      }
      const currentSlide = presentation.slides[order[current.slide]];
      const total = showSteps(currentSlide).length;
      return current.steps < total ? { ...current, steps: current.steps + 1 } : { slide: current.slide + 1, steps: 1 };
    });
  }, [order, presentation.slides, onClose]);

  const previous = useCallback(() => {
    setPlace((current) => {
      if (current.steps > 1 && current.slide < order.length) return { ...current, steps: current.steps - 1 };
      if (current.slide === 0) return current;
      const before = current.slide - 1;
      return { slide: before, steps: showSteps(presentation.slides[order[before]]).length };
    });
  }, [order, presentation.slides]);

  useLayoutEffect(() => {
    const measure = () => setSize({ width: window.innerWidth, height: window.innerHeight });
    measure();
    window.addEventListener('resize', measure);
    return () => window.removeEventListener('resize', measure);
  }, []);

  useEffect(() => {
    const element = root.current;
    element?.focus();
    if (!fullscreen || !element || typeof element.requestFullscreen !== 'function') return;
    element.requestFullscreen().catch(() => undefined);
    const left = () => {
      if (!document.fullscreenElement) onClose();
    };
    document.addEventListener('fullscreenchange', left);
    return () => {
      document.removeEventListener('fullscreenchange', left);
      if (document.fullscreenElement && typeof document.exitFullscreen === 'function') document.exitFullscreen().catch(() => undefined);
    };
  }, [fullscreen, onClose]);

  useEffect(() => {
    const after = slide?.transition?.advanceAfterMs;
    if (after === undefined || after === null || ended) return;
    const timer = setTimeout(() => setPlace((current) => ({ slide: current.slide + 1, steps: 1 })), Math.max(0, after));
    return () => clearTimeout(timer);
  }, [slide, place.slide, ended]);

  const scale = Math.min(size.width / presentation.widthEmu, size.height / presentation.heightEmu);
  const hidden = slide ? initiallyHidden(slide) : new Set<number>();
  const latest = new Map<number, { effect: ShowEffect; step: number }>();
  steps.slice(0, place.steps).forEach((step, stepIndex) => {
    for (const effect of step) latest.set(effect.shape, { effect, step: stepIndex });
  });

  return (
    <div
      ref={root}
      className="rely-slide-show"
      role="dialog"
      aria-label={words.slideShow}
      tabIndex={-1}
      onClick={next}
      onContextMenu={(event) => {
        event.preventDefault();
        previous();
      }}
      onKeyDown={(event) => {
        if (['ArrowRight', 'ArrowDown', 'PageDown', ' ', 'Enter', 'n', 'N'].includes(event.key)) next();
        else if (['ArrowLeft', 'ArrowUp', 'PageUp', 'Backspace', 'p', 'P'].includes(event.key)) previous();
        else if (event.key === 'Home') setPlace({ slide: 0, steps: 1 });
        else if (event.key === 'End') setPlace({ slide: Math.max(0, order.length - 1), steps: 1 });
        else if (event.key === 'Escape') onClose();
        else return;
        event.preventDefault();
      }}
    >
      {ended || !slide ? (
        <p className="rely-slide-show-end">{words.endOfShow}</p>
      ) : (
        <div
          key={place.slide}
          className="rely-slide rely-slide-show-slide"
          aria-label={words.slide(slideIndex + 1)}
          style={{
            width: presentation.widthEmu * scale,
            height: presentation.heightEmu * scale,
            ...backgroundCss(slide.background, media),
            ...transitionCss(slide.transition),
          }}
        >
          {(slide.shapes ?? []).map((shape, index) => {
            const last = latest.get(index);
            const running = last !== undefined && last.step === place.steps - 1;
            const shown = last
              ? last.effect.animation.class !== 'exit' || running
              : !hidden.has(index);
            if (!shown) return null;
            return (
              <div
                key={`${index}-${last?.step ?? 'still'}`}
                className="rely-slide-shape"
                data-shape={index}
                style={{ ...frameCss(shape, scale), ...(running ? effectCss(last.effect) : {}) }}
              >
                <ShapeView shape={shape} scale={scale} media={media} />
              </div>
            );
          })}
        </div>
      )}
      <span className="rely-slide-show-counter" aria-live="polite">
        {ended ? '' : `${place.slide + 1} / ${order.length}`}
      </span>
    </div>
  );
}
