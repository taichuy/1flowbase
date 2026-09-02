import { describe, expect, test } from 'vitest';

import { resolveProductionModulePreloadDependencies } from '../production-module-preload';

describe('production module preload boundaries', () => {
  test('PB-F02 keeps nested Settings feature islands demand driven', () => {
    expect(
      resolveProductionModulePreloadDependencies(
        'assets/SettingsPage-BbvhEMgQ.js',
        [
          'assets/SettingsPage-BbvhEMgQ.js',
          'assets/JsxStudioResourcePanel-BiYyGCR8.js'
        ],
        { hostId: 'assets/AuthenticatedAppRuntime.js', hostType: 'js' }
      )
    ).toEqual([]);
    expect(
      resolveProductionModulePreloadDependencies(
        'assets/SettingsExtensionCenterSection-B3Y5Q4bH.js',
        ['assets/McpBundleImportFlow-DOfxkFVH.js'],
        { hostId: 'assets/SettingsPage.js', hostType: 'js' }
      )
    ).toEqual([]);
    expect(
      resolveProductionModulePreloadDependencies(
        'SettingsPage-BbvhEMgQ.js',
        ['assets/JsxStudioResourcePanel-BiYyGCR8.js'],
        { hostId: 'assets/AuthenticatedAppRuntime.js', hostType: 'js' }
      )
    ).toEqual([]);
    expect(
      resolveProductionModulePreloadDependencies(
        'AppShellFrame-bKkdNVP4.js',
        [
          'assets/AppShellFrame-bKkdNVP4.js',
          'assets/monaco-runtime-BtwIsWfa.js'
        ],
        { hostId: 'assets/AuthenticatedAppRuntime.js', hostType: 'js' }
      )
    ).toEqual([]);
    expect(
      resolveProductionModulePreloadDependencies(
        '_virtual_1flowbase-native-ant-design-icons-loaders-CFKtburL.js',
        ['assets/AccountBookFilled.js', 'assets/AlertOutlined.js'],
        { hostId: 'assets/native-icons-registry.js', hostType: 'js' }
      )
    ).toEqual([]);
    expect(
      resolveProductionModulePreloadDependencies(
        '_virtual_1flowbase-page-tree-icon-previews-CFKtburL.js',
        ['assets/page-tree-icons-pack-a.svg'],
        { hostId: 'assets/PageTreeIconPicker.js', hostType: 'js' }
      )
    ).toEqual([]);
    expect(
      resolveProductionModulePreloadDependencies(
        '_virtual_1flowbase-page-tree-icon-runtime-CFKtburL.js',
        ['assets/page-tree-icon-component-pack-a.js'],
        { hostId: 'assets/PageTreeIcon.js', hostType: 'js' }
      )
    ).toEqual([]);
  });

  test('PB-F02 preserves entry HTML and focused feature preload decisions', () => {
    const dependencies = ['assets/react.js', 'assets/workflow.js'];
    expect(
      resolveProductionModulePreloadDependencies(
        'assets/WorkflowEditorPage.js',
        dependencies,
        { hostId: 'assets/router.js', hostType: 'js' }
      )
    ).toBe(dependencies);
    expect(
      resolveProductionModulePreloadDependencies(
        'assets/SettingsPage.js',
        dependencies,
        { hostId: 'index.html', hostType: 'html' }
      )
    ).toBe(dependencies);
  });
});
