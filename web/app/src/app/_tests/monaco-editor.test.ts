import { beforeEach, expect, test, vi } from 'vitest';

const monacoHarness = vi.hoisted(() => ({
  config: vi.fn()
}));

vi.mock('@monaco-editor/loader', () => ({
  default: { config: monacoHarness.config }
}));

vi.mock('monaco-editor', () => ({
  editor: { create: vi.fn() }
}));

vi.mock('monaco-editor/esm/vs/editor/editor.worker?worker', () => ({
  default: class EditorWorker {}
}));

vi.mock('monaco-editor/esm/vs/language/json/json.worker?worker', () => ({
  default: class JsonWorker {}
}));

vi.mock('monaco-editor/esm/vs/language/typescript/ts.worker?worker', () => ({
  default: class TypeScriptWorker {}
}));

import { initializeMonacoEditor } from '../monaco-editor';

beforeEach(() => {
  monacoHarness.config.mockClear();
});

test('AC-001 initializes Monaco from bundled assets instead of the public CDN', () => {
  initializeMonacoEditor();

  expect(monacoHarness.config).toHaveBeenCalledOnce();
  expect(monacoHarness.config).toHaveBeenCalledWith({
    monaco: expect.objectContaining({ editor: expect.any(Object) })
  });

  const environment = self.MonacoEnvironment;
  expect(environment).toBeDefined();
  const getWorker = environment!.getWorker!;
  expect(getWorker('', 'json').constructor.name).toBe('JsonWorker');
  expect(getWorker('', 'typescript').constructor.name).toBe(
    'TypeScriptWorker'
  );
  expect(getWorker('', 'javascript').constructor.name).toBe(
    'TypeScriptWorker'
  );
  expect(getWorker('', 'plaintext').constructor.name).toBe('EditorWorker');
});
