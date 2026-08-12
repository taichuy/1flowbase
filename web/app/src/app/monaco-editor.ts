import loader from '@monaco-editor/loader';
import * as monaco from 'monaco-editor';
import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
import JsonWorker from 'monaco-editor/esm/vs/language/json/json.worker?worker';
import TypeScriptWorker from 'monaco-editor/esm/vs/language/typescript/ts.worker?worker';

export function initializeMonacoEditor() {
  self.MonacoEnvironment = {
    getWorker(_moduleId, label) {
      if (label === 'json') return new JsonWorker();
      if (label === 'typescript' || label === 'javascript') {
        return new TypeScriptWorker();
      }
      return new EditorWorker();
    }
  };
  loader.config({ monaco });
}
