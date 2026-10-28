import type Vditor from 'vditor';

type EditorMode = ReturnType<Vditor['getCurrentMode']>;
const STORAGE_KEY = '1flowbase.rich-text.editor-mode';

export function preferredEditorMode(
  document: Document,
  fallback: EditorMode
): EditorMode {
  try {
    const value = document.defaultView?.localStorage.getItem(STORAGE_KEY);
    return value === 'ir' || value === 'sv' || value === 'wysiwyg'
      ? value
      : fallback;
  } catch {
    return fallback;
  }
}

export function rememberEditorMode(
  mount: HTMLElement,
  editor: Pick<Vditor, 'getCurrentMode'>
) {
  const view = mount.ownerDocument.defaultView;
  const menu = mount.querySelector('[data-type="edit-mode"]')?.parentElement;
  if (!view || !menu) return () => {};
  let previous = editor.getCurrentMode();
  // Native mode changes update the menu for both mouse and keyboard activation.
  const observer = new (view as Window & typeof globalThis).MutationObserver(
    () => {
      const mode = editor.getCurrentMode();
      if (mode === previous) return;
      previous = mode;
      try {
        view.localStorage.setItem(STORAGE_KEY, mode);
      } catch {
        // Storage may be disabled; changing the current editor must still work.
      }
    }
  );
  observer.observe(menu, {
    attributes: true,
    attributeFilter: ['class'],
    subtree: true
  });
  return () => observer.disconnect();
}
