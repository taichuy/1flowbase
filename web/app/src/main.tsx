import React from 'react';
import ReactDOM from 'react-dom/client';

import { App } from './app/App';
import { initializeMonacoEditor } from './app/monaco-editor';
import './styles/tokens.css';
import './styles/globals.css';

initializeMonacoEditor();

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
