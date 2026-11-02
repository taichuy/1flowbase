import '@testing-library/jest-dom/vitest';
import { waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { preferredEditorMode, rememberEditorMode } from '../mode-preference';

const key = '1flowbase.rich-text.editor-mode';
let release = () => {};
beforeEach(() => {
  window.localStorage.clear();
  document.body.innerHTML = '<div id="editor"><div id="mode-menu"><button data-type="edit-mode"></button></div></div>';
});
afterEach(() => { release(); vi.restoreAllMocks(); });

it.each([undefined, false, true])(
  'keeps a changed mode temporary unless the host explicitly permits persistence: %s',
  async (allowed) => {
    window.localStorage.setItem(key, 'ir');
    const write = vi.spyOn(Storage.prototype, 'setItem');
    let mode: 'ir' | 'sv' = 'ir';
    const editor = { getCurrentMode: vi.fn(() => mode) };
    release = rememberEditorMode(document.getElementById('editor')!, editor,
      allowed === undefined ? undefined : () => allowed);
    mode = 'sv';
    document.getElementById('mode-menu')!.classList.add('changed');
    await waitFor(() => expect(editor.getCurrentMode).toHaveBeenCalledTimes(2));
    expect(editor.getCurrentMode()).toBe('sv');
    expect(preferredEditorMode(document, 'wysiwyg')).toBe(allowed ? 'sv' : 'ir');
    if (allowed) expect(write).toHaveBeenCalledWith(key, 'sv');
    else expect(write).not.toHaveBeenCalled();
  }
);

it('checks current host authority on every observed mode change', async () => {
  let allowed = true;
  let mode: 'ir' | 'sv' | 'wysiwyg' = 'ir';
  const editor = { getCurrentMode: vi.fn(() => mode) };
  release = rememberEditorMode(document.getElementById('editor')!, editor, () => allowed);
  mode = 'sv';
  document.getElementById('mode-menu')!.className = 'sv';
  await waitFor(() => expect(window.localStorage.getItem(key)).toBe('sv'));
  const write = vi.spyOn(Storage.prototype, 'setItem');
  allowed = false;
  mode = 'wysiwyg';
  document.getElementById('mode-menu')!.className = 'wysiwyg';
  await waitFor(() => expect(editor.getCurrentMode).toHaveBeenCalledTimes(3));
  expect(write).not.toHaveBeenCalled();
  expect(window.localStorage.getItem(key)).toBe('sv');
});
