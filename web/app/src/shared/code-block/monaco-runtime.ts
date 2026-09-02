type MonacoEditorModule = typeof import('@monaco-editor/react');

let runtimeFlight: Promise<MonacoEditorModule> | undefined;

export function loadMonacoEditorModule(): Promise<MonacoEditorModule> {
  if (runtimeFlight) return runtimeFlight;

  runtimeFlight = Promise.all([
    import('@monaco-editor/react'),
    import('./monaco-configuration')
  ])
    .then(([editorModule, configuration]) => {
      configuration.initializeMonacoEditor();
      return editorModule;
    })
    .catch((error: unknown) => {
      runtimeFlight = undefined;
      throw error;
    });

  return runtimeFlight;
}
