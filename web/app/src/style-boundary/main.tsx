import React from 'react';
import ReactDOM from 'react-dom/client';

import { AppProviders } from '../app/AppProviders';
import '../styles/tokens.css';
import '../styles/globals.css';
import { StyleBoundaryHarness } from './StyleBoundaryHarness';
import { getRuntimeScene } from './registry';

export function bootstrapStyleBoundary(rootElement: HTMLElement) {
  const params = new URLSearchParams(window.location.search);
  const sceneId = params.get('scene') ?? 'component.account-popup';
  const scene = getRuntimeScene(sceneId);

  ReactDOM.createRoot(rootElement).render(
    <React.StrictMode>
      <AppProviders>
        <StyleBoundaryHarness scene={scene} />
      </AppProviders>
    </React.StrictMode>
  );
}

const rootElement = document.getElementById('root');

if (rootElement) {
  bootstrapStyleBoundary(rootElement);
}
