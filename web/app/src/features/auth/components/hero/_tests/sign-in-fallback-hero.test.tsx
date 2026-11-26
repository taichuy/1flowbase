import { StrictMode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { act, render, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { HeroArtwork, HeroTagline } from '../HeroArtwork';
import type { DocumentArtwork } from '../document-motion';

const motion = vi.hoisted(() => ({ start: vi.fn(), stop: vi.fn() }));
vi.mock('../leaf-motion', () => ({ startLeafMotion: motion.start }));
let release: () => void;
let requested = vi.fn();

beforeEach(() => {
  vi.resetModules();
  localStorage.clear();
  history.replaceState({}, '', '/sign-in?language=en');
  motion.start.mockReset().mockReturnValue(motion.stop);
  motion.stop.mockReset();
  requested = vi.fn();
  const ready = new Promise<void>((resolve) => {
    release = resolve;
  });
  vi.doMock('../SignInFallbackHero', async (importOriginal) => {
    requested();
    await ready;
    return importOriginal();
  });
});

afterEach(async () => {
  await resolveFallback();
  document.getElementById('sign-in-document-scene')?.remove();
  vi.doUnmock('../SignInFallbackHero');
});

async function resolveFallback() {
  await act(async () => {
    release();
    await vi.dynamicImportSettled();
  });
}

function earlyScene() {
  const scene = document.createElement('div');
  scene.id = 'sign-in-document-scene';
  scene.innerHTML = renderToStaticMarkup(
    <div data-hero-artwork="">
      <HeroArtwork>
        <div data-opening-brand="" hidden>
          <HeroTagline slogan="" />
        </div>
      </HeroArtwork>
    </div>
  );
  document.body.append(scene);
  return scene.firstElementChild as DocumentArtwork;
}

async function mountHero() {
  const { HeroAnimation } = await import('../../HeroAnimation');
  const view = render(
    <StrictMode>
      <HeroAnimation />
    </StrictMode>
  );
  // The lazy module has been requested but its controllable load is unresolved.
  await act(async () => {
    await Promise.resolve();
  });
  expect(requested).toHaveBeenCalledTimes(1);
  return { view, HeroAnimation };
}

test('no opening lazily renders the complete original artwork and motion, including StrictMode cleanup', async () => {
  const { view, HeroAnimation } = await mountHero();
  expect(view.container.querySelector('canvas')).toBeNull();
  expect(motion.start).not.toHaveBeenCalled();
  await resolveFallback();
  await waitFor(() =>
    expect(view.container.querySelector('canvas')).not.toBeNull()
  );
  const canvas = view.container.querySelector('canvas');
  const hero = view.container.querySelector('.auth-sign-in-hero');
  const expected = document.createElement('div');
  expected.innerHTML = renderToStaticMarkup(
    <HeroArtwork>
      <HeroTagline slogan="AI app base begins from chat history." />
    </HeroArtwork>
  );
  // SSR and the DOM serialize identical style declarations differently. Compare
  // the complete element tree and normalized declarations, not those spellings.
  const describeNode = (node: Node): unknown => {
    if (!(node instanceof Element)) return node.textContent;
    const style = (node as HTMLElement).style;
    return {
      tag: node.tagName,
      attributes: [...node.attributes]
        .filter(({ name }) => name !== 'style')
        .map(({ name, value }) => [name, value])
        .sort(),
      style: style
        ? [...Array(style.length)]
            .map((_, i) => style.item(i))
            .sort()
            .map((name) => [
              name,
              style.getPropertyValue(name),
              style.getPropertyPriority(name)
            ])
        : [],
      children: [...node.childNodes].map(describeNode)
    };
  };
  expect(
    [...view.container.firstElementChild!.childNodes].map(describeNode)
  ).toEqual([...expected.childNodes].map(describeNode));
  expect(motion.start).toHaveBeenCalledTimes(2);
  expect(motion.start).toHaveBeenNthCalledWith(1, canvas, hero);
  expect(motion.start).toHaveBeenNthCalledWith(2, canvas, hero);
  expect(motion.stop).toHaveBeenCalledTimes(1);
  view.rerender(
    <StrictMode>
      <HeroAnimation hide_login_tagline />
    </StrictMode>
  );
  expect(view.container.querySelector('canvas')).toBe(canvas);
  expect(view.container.querySelector('.hero-tagline')).toBeNull();
  expect(motion.start).toHaveBeenCalledTimes(2);
  view.unmount();
  expect(motion.stop).toHaveBeenCalledTimes(2);
});

test('missing stopMotion adopts synchronously then recovers exactly once on the same canvas', async () => {
  const art = earlyScene();
  const canvas = art.querySelector('canvas');
  const hero = art.querySelector('.auth-sign-in-hero');
  const { view } = await mountHero();
  expect(document.getElementById('sign-in-document-scene')).toBeNull();
  expect(view.container.querySelector('canvas')).toBe(canvas);
  expect(view.container.querySelector('[data-hero-artwork]')).toBe(art);
  expect(art.querySelector('.hero-slogan-line')).toHaveTextContent(
    'AI app base begins from chat history.'
  );
  expect(motion.start).not.toHaveBeenCalled();
  await resolveFallback();
  expect(view.container.querySelector('canvas')).toBe(canvas);
  expect(motion.start).toHaveBeenCalledExactlyOnceWith(canvas, hero);
  expect(art.stopMotion).toBe(motion.stop);
  expect(motion.stop).not.toHaveBeenCalled();
  view.unmount();
  await act(async () => {
    await Promise.resolve();
  });
  expect(motion.stop).toHaveBeenCalledTimes(1);
  expect(art.stopMotion).toBeUndefined();
});

test('unmount before the no-opening module resolves never starts an abandoned simulation', async () => {
  const { view } = await mountHero();
  view.unmount();
  await resolveFallback();
  expect(motion.start).not.toHaveBeenCalled();
  expect(motion.stop).not.toHaveBeenCalled();
  expect(document.querySelector('canvas')).toBeNull();
});

test('unmount during recovery cancels late simulation startup on the adopted canvas', async () => {
  const art = earlyScene();
  const { view } = await mountHero();
  expect(view.container.querySelector('canvas')).toBe(
    art.querySelector('canvas')
  );
  view.unmount();
  await resolveFallback();
  expect(art.isConnected).toBe(false);
  expect(art.stopMotion).toBeUndefined();
  expect(motion.start).not.toHaveBeenCalled();
  expect(motion.stop).not.toHaveBeenCalled();
});

test('recovery does not restart motion supplied while the fallback module was loading', async () => {
  const art = earlyScene();
  const { view } = await mountHero();
  const stop = vi.fn();
  art.stopMotion = stop;
  await resolveFallback();
  expect(motion.start).not.toHaveBeenCalled();
  expect(art.stopMotion).toBe(stop);
  expect(stop).not.toHaveBeenCalled();
  view.unmount();
  await act(async () => {
    await Promise.resolve();
  });
  expect(stop).toHaveBeenCalledTimes(1);
});
