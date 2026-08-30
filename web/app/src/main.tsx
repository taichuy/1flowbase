import React from 'react';
import ReactDOM from 'react-dom/client';

import { App } from './app/App';
import './styles/tokens.css';
import './styles/globals.css';

if (import.meta.env.DEV) {
  void import('virtual:1flowbase-dev-hmr-probe');
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
