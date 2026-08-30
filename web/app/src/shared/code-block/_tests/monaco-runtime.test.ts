import { beforeEach, expect, test, vi } from 'vitest';

const runtimeHarness = vi.hoisted(() => ({
  initialize: vi.fn()
}));

vi.mock('@monaco-editor/react', () => ({
  default: function MockMonacoEditor() {
    return null;
  }
}));

vi.mock('../monaco-configuration', () => ({
  initializeMonacoEditor: runtimeHarness.initialize
}));

beforeEach(() => {
  vi.resetModules();
  runtimeHarness.initialize.mockReset();
});

test('AC-002 AC-003 loads and initializes one shared Monaco runtime flight', async () => {
  const { loadMonacoEditorModule } = await import('../monaco-runtime');

  const [left, right] = await Promise.all([
    loadMonacoEditorModule(),
    loadMonacoEditorModule()
  ]);

  expect(left).toBe(right);
  expect(runtimeHarness.initialize).toHaveBeenCalledOnce();
});

test('AC-003 clears a failed initialization flight so it can retry', async () => {
  runtimeHarness.initialize
    .mockImplementationOnce(() => {
      throw new Error('worker configuration failed');
    })
    .mockImplementationOnce(() => undefined);
  const { loadMonacoEditorModule } = await import('../monaco-runtime');

  await expect(loadMonacoEditorModule()).rejects.toThrow(
    'worker configuration failed'
  );
  await expect(loadMonacoEditorModule()).resolves.toHaveProperty('default');
  expect(runtimeHarness.initialize).toHaveBeenCalledTimes(2);
});
