import { StrictMode } from 'react';
import { render } from '@testing-library/react';
import { renderToStaticMarkup } from 'react-dom/server';
import { expect, test, vi } from 'vitest';
import { HeroAnimation } from '../../HeroAnimation';
import { HeroArtwork, HeroTagline } from '../HeroArtwork';
import {
  documentSceneId,
  discardDocumentScene,
  findDocumentArtwork,
  setDocumentBrand
} from '../document-motion';

function earlyScene() {
  const el = document.createElement('div');
  el.id = documentSceneId;
  el.innerHTML = renderToStaticMarkup(
    <div data-hero-artwork="">
      <HeroArtwork>
        <div data-opening-brand="" hidden>
          <HeroTagline slogan="" />
        </div>
      </HeroArtwork>
    </div>
  );
  document.body.append(el);
  const art = findDocumentArtwork()!;
  const stop = vi.fn();
  art.stopMotion = stop;
  return {
    art,
    stop,
    canvas: art.querySelector('canvas'),
    brand: art.querySelector<HTMLElement>('[data-opening-brand]')!
  };
}

test('route adoption preserves the existing simulation and title; cleanup stops it once, including StrictMode', async () => {
  const { art, stop, canvas, brand } = earlyScene();
  expect(brand.hidden).toBe(true);
  setDocumentBrand(art, 'already visible');
  const view = render(
    <StrictMode>
      <HeroAnimation hide_login_tagline={false} />
    </StrictMode>
  );
  expect(document.getElementById(documentSceneId)).toBeNull();
  expect(view.container.querySelector('canvas')).toBe(canvas);
  expect(view.container.querySelector('[data-opening-brand]')).toBe(brand);
  expect(brand.hidden).toBe(false);
  await Promise.resolve();
  expect(stop).not.toHaveBeenCalled();
  view.rerender(
    <StrictMode>
      <HeroAnimation hide_login_tagline />
    </StrictMode>
  );
  expect(brand.hidden).toBe(true);
  view.unmount();
  await Promise.resolve();
  expect(stop).toHaveBeenCalledTimes(1);
});

test('discarding the pending document for the authenticated app stops it and leaves no login UI', () => {
  const { stop } = earlyScene();
  discardDocumentScene();
  discardDocumentScene();
  expect(stop).toHaveBeenCalledTimes(1);
  expect(findDocumentArtwork()).toBeNull();
});
