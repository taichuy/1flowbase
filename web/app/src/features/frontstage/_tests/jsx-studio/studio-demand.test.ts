import { beforeEach, expect, test, vi } from 'vitest';

const demand = vi.hoisted(() => ({
  studio: vi.fn(),
  monaco: vi.fn(async () => ({ default: () => null })),
  declarations: vi.fn(async () => [])
}));
vi.mock('../../components/jsx-studio/FrontstageJsxStudioDrawer', () => {
  demand.studio();
  return { FrontstageJsxStudioDrawer: () => null };
});
vi.mock('../../../../shared/code-block/monaco-runtime', () => ({
  loadMonacoEditorModule: demand.monaco
}));
vi.mock('../../lib/native-modules/editor-declarations', () => ({
  frontstageNativeDeclarationLoader: { loadForSource: demand.declarations }
}));

beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
});

test('loading the boundary itself does not fetch Studio and the first action shares the preparation flight', async () => {
  const module = await import('../../lib/jsx-studio/studio-demand');
  expect(demand.studio).not.toHaveBeenCalled();
  const preparation = module.prepareFrontstageStudio();
  const firstClick = module.loadFrontstageStudio();
  expect(module.loadFrontstageStudio()).toBe(firstClick);
  expect(await firstClick).toHaveProperty('FrontstageJsxStudioDrawer');
  await preparation;
  expect(demand.studio).toHaveBeenCalledOnce();
});

test('opening Studio is not gated on unrelated declaration preparation finishing', async () => {
  let finish!: (value: never[]) => void;
  demand.declarations.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      })
  );
  const module = await import('../../lib/jsx-studio/studio-demand');
  const preparation = module.prepareFrontstageStudio();
  await expect(module.loadFrontstageStudio()).resolves.toHaveProperty(
    'FrontstageJsxStudioDrawer'
  );
  await vi.waitFor(() => expect(finish).toBeDefined());
  finish([]);
  await preparation;
});
