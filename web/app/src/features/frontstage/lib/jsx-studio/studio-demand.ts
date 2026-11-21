let studioFlight:
  | Promise<
      typeof import('../../components/jsx-studio/FrontstageJsxStudioDrawer')
    >
  | undefined;
let runPanelFlight:
  | Promise<typeof import('../../components/jsx-studio/JsxStudioRunPanel')>
  | undefined;

export function loadFrontstageStudio() {
  if (!studioFlight) {
    const attempt =
      import('../../components/jsx-studio/FrontstageJsxStudioDrawer');
    studioFlight = attempt;
    void attempt.catch(() => {
      if (studioFlight === attempt) studioFlight = undefined;
    });
  }
  return studioFlight;
}

export function loadFrontstageStudioRunPanel() {
  if (!runPanelFlight) {
    const attempt = import('../../components/jsx-studio/JsxStudioRunPanel');
    runPanelFlight = attempt;
    void attempt.catch(() => {
      if (runPanelFlight === attempt) runPanelFlight = undefined;
    });
  }
  return runPanelFlight;
}

export function prepareFrontstageStudio() {
  // Each demand has its own flight; opening the editor never awaits the topbar
  // designer, a run panel or an unrelated background task.
  return Promise.all([
    loadFrontstageStudio(),
    import('../../../../shared/code-block/monaco-runtime').then((module) =>
      module.loadMonacoEditorModule()
    ),
    import('../native-modules/editor-declarations').then((module) =>
      module.frontstageNativeDeclarationLoader.loadForSource('')
    )
  ]).then(() => undefined);
}
