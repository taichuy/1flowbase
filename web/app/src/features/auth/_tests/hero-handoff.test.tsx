import { StrictMode } from 'react';
import { act, render } from '@testing-library/react';
import { expect, test, vi } from 'vitest';
import { HeroAnimation } from '../components/HeroAnimation';
import { startLeafMotion } from '../components/hero/leaf-motion';
import type { DocumentArtwork } from '../components/hero/document-motion';

const fallbackLoad = vi.hoisted(() => vi.fn());
vi.mock('../components/hero/SignInFallbackHero', async (importOriginal) => {
  fallbackLoad();
  return importOriginal();
});
vi.mock('../components/hero/leaf-motion', () => ({ startLeafMotion: vi.fn() }));
test('StrictMode adopts the existing document canvas and simulation without restarting motion', async () => {
  localStorage.clear();
  history.replaceState({}, '', '/sign-in?language=en');
  const scene = document.createElement('div');
  scene.id = 'sign-in-document-scene';
  const art = document.createElement('div') as DocumentArtwork;
  art.dataset.heroArtwork = '';
  art.innerHTML =
    '<canvas></canvas><div class="auth-sign-in-hero"><div data-opening-brand><span class="hero-slogan-line"></span></div></div>';
  const canvas = art.querySelector('canvas');
  const stop = vi.fn();
  art.stopMotion = stop;
  scene.append(art);
  document.body.append(scene);
  const view = render(
    <StrictMode>
      <HeroAnimation hide_login_tagline={false} />
    </StrictMode>
  );
  await act(async () => {
    await Promise.resolve();
  });
  expect(view.container.querySelector('canvas')).toBe(canvas);
  expect(document.getElementById('sign-in-document-scene')).toBeNull();
  expect(view.container.querySelector('.hero-slogan-line')).toHaveTextContent(
    'AI app base begins from chat history.'
  );
  expect(startLeafMotion).not.toHaveBeenCalled();
  expect(fallbackLoad).not.toHaveBeenCalled();
  expect(stop).not.toHaveBeenCalled();
  view.rerender(
    <StrictMode>
      <HeroAnimation hide_login_tagline />
    </StrictMode>
  );
  expect(art.querySelector('[data-opening-brand]')).toHaveAttribute('hidden');
  expect(view.container.querySelector('canvas')).toBe(canvas);
  view.unmount();
  await act(async () => {
    await Promise.resolve();
  });
  expect(stop).toHaveBeenCalledTimes(1);
});
