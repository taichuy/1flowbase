import { describe, expect, test } from 'vitest';
import { resolveProductionModulePreloadDependencies } from '../production-module-preload';

describe('production module preload boundaries', () => {
  test.each(['AppShellFrame', 'SettingsPage', 'SettingsExtensionCenterSection',
    '_virtual_1flowbase-native-ant-design-icons-loaders',
    '_virtual_1flowbase-page-tree-icon-previews', 'WorkflowEditorPage'])
  ('keeps compiler-provided required static dependencies of %s discoverable', (name) => {
    const dependencies = [`assets/${name}-hash.js`, 'assets/shared-hash.js', 'assets/style-hash.css'];
    expect(resolveProductionModulePreloadDependencies(`${name}-hash.js`, dependencies,
      { hostId: 'assets/router.js', hostType: 'js' })).toBe(dependencies);
  });
  test('preserves entry HTML dependencies without inventing speculative dynamic imports', () => {
    const dependencies = ['assets/react.js'];
    expect(resolveProductionModulePreloadDependencies('assets/entry.js', dependencies,
      { hostId: 'index.html', hostType: 'html' })).toBe(dependencies);
  });
});
