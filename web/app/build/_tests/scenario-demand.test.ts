import { describe, expect, test } from 'vitest';
import { collectScenarioDemandManifest, staticChunkClosure } from '../scenario-demand/manifest';

function chunk(fileName: string, owner: string, imports: string[] = [], dynamicImports: string[] = []) {
  return { type: 'chunk' as const, fileName, name: fileName, modules: { [owner]: {} }, imports, dynamicImports };
}
const workspaceShell = '/src/features/frontstage/pages/FrontstageWorkspacePage.tsx';
const page = '/src/features/frontstage/pages/FrontStagePage.tsx';
const studio = '/src/features/frontstage/components/jsx-studio/FrontstageJsxStudioDrawer.tsx';

describe('compiler scenario demand graph', () => {
  test('records every chunk, CSS and asset without crossing lazy editing or runtime Worker edges', () => {
    const manifest = collectScenarioDemandManifest({
      'page.js': { ...chunk('page.js', page, ['runtime.js'], ['studio.js']),
        viteMetadata: { importedCss: new Set(['page.css']), importedAssets: new Set(['runtime-worker.js']) } },
      'runtime.js': chunk('runtime.js', '/runtime/compiler.ts'),
      'studio.js': chunk('studio.js', studio),
      'runtime-worker.js': { type: 'asset', fileName: 'runtime-worker.js' },
      'declarations-hash.json': { type: 'asset', fileName: 'declarations-hash.json' }
    });
    expect(manifest.scenes.browse.staticClosure).toEqual(['page.js', 'runtime.js']);
    expect(manifest.chunks).toHaveLength(3);
    expect(manifest.chunks.find((c) => c.file === 'page.js')?.css).toEqual(['page.css']);
    expect(manifest.assets).toContain('declarations-hash.json');
  });
  test('rejects reintroducing Studio through a shared vendor on the browse path', () => {
    expect(() => collectScenarioDemandManifest({
      'page.js': chunk('page.js', page, ['vendor.js']),
      'vendor.js': chunk('vendor.js', studio)
    })).toThrow(/browse eagerly includes authoring/);
  });
  test('rejects declarations merged into an otherwise ordinary shared chunk', () => {
    expect(() => collectScenarioDemandManifest({
      'page.js': chunk('page.js', page, ['shared.js']),
      'shared.js': chunk('shared.js', '\0virtual:1flowbase-native-module-declarations')
    })).toThrow(/authoring module/);
  });
  test('records authenticatedStartup scene and rejects main-thread Acorn/Sucrase compiler on shell or browse', () => {
    const manifest = collectScenarioDemandManifest({
      'startup.js': chunk('startup.js', '/src/app/bootstrap/AuthenticatedStartup.tsx', ['runtime-auth.js']),
      'runtime-auth.js': chunk('runtime-auth.js', '/src/app/AuthenticatedAppRuntime.tsx')
    });
    expect(manifest.scenes.authenticatedStartup.staticClosure).toEqual(['runtime-auth.js', 'startup.js']);
    expect(() => collectScenarioDemandManifest({
      'page.js': chunk('page.js', page, ['compiler.js']),
      'compiler.js': chunk('compiler.js', '/repo/node_modules/acorn/dist/acorn.mjs')
    })).toThrow(/main-thread compiler/);
  });
  test('static closure terminates on cycles and ignores external modules', () => {
    expect(staticChunkClosure(new Map([
      ['a', { imports: ['b'] }], ['b', { imports: ['a', 'external'] }]
    ]), ['a'])).toEqual(['a', 'b']);
  });

  test('enforces progressive concentric boundary: workspaceShell (left sidebar) cannot statically import FrontStagePage, PageCanvas, react-grid-layout, or page-runtime', () => {
    const cleanManifest = collectScenarioDemandManifest({
      'ws-shell.js': chunk('ws-shell.js', workspaceShell, [], ['canvas.js']),
      'canvas.js': chunk('canvas.js', page)
    });
    expect(cleanManifest.scenes.workspaceShell.staticClosure).toEqual(['ws-shell.js']);

    expect(() => collectScenarioDemandManifest({
      'ws-shell.js': chunk('ws-shell.js', workspaceShell, ['canvas.js']),
      'canvas.js': chunk('canvas.js', page)
    })).toThrow(/workspaceShell eagerly includes canvas\/runtime/);

    expect(() => collectScenarioDemandManifest({
      'ws-shell.js': chunk('ws-shell.js', workspaceShell, ['rgl.js']),
      'rgl.js': chunk('rgl.js', '/repo/node_modules/react-grid-layout/dist/index.mjs')
    })).toThrow(/workspaceShell eagerly includes canvas\/runtime/);
  });
});

const settingsPage = '/src/features/settings/pages/SettingsPage.tsx';
const memberTabs = '/src/features/settings/pages/settings-page/MemberSettingsTabs.tsx';
const loginStudio = '/src/features/settings/components/auth-center/LoginEntryUiBlockStudio.tsx';

test('Settings closures allow lazy studios and billing but reject compiler-owned static leaks', () => {
  const clean = {
    'settings.js': chunk('settings.js', settingsPage, [], ['members.js']),
    'members.js': chunk('members.js', memberTabs, [], ['billing.js']),
    'billing.js': chunk('billing.js', '/repo/web/packages/api-client/src/console/administration/billing.ts'),
    'login-studio.js': chunk('login-studio.js', loginStudio)
  };
  const { scenes } = collectScenarioDemandManifest(clean);
  expect(scenes.settingsShell.staticClosure).toEqual(['settings.js']);
  expect(scenes.settingsMembers.staticClosure).toEqual(['members.js']);
  expect(scenes.loginStudio.roots).toEqual(['login-studio.js']);
  expect(() => collectScenarioDemandManifest({
    ...clean,
    'members.js': chunk('members.js', memberTabs, ['billing.js'])
  })).toThrow(/settingsMembers eagerly includes.*billing/);
});

test('protected startup scenes reject Settings grouping and Splitter even via shared chunks', () => {
  const startup = '/src/app/bootstrap/AuthenticatedStartup.tsx';
  for (const dependency of [
    { ...chunk('shared.js', '/shared/settings-primitive.ts'), name: 'settings-shared~2' },
    chunk('shared.js', '/repo/node_modules/antd/es/splitter/hooks/useResize.js')
  ]) {
    expect(() => collectScenarioDemandManifest({
      'startup.js': chunk('startup.js', startup, ['shared.js']),
      'shared.js': dependency
    })).toThrow(/authenticatedStartup eagerly includes/);
  }
});


test.each([
  ['settingsShell', settingsPage, '/src/FrontstageNativeTrustedBlockPortalHost.tsx'],
  ['settingsMembers', memberTabs, '/src/DepartmentManagementPanel.tsx'],
  ['settingsAuthCenter', '/features/settings/pages/settings-page/SettingsAuthCenterSection.tsx', '/repo/node_modules/monaco-editor/esm/vs/editor.js'],
  ['settingsUiManagement', '/features/settings/components/ui-management/UiManagementPanel.tsx', '/repo/node_modules/vditor/dist/index.js'],
  ['settingsMcp', '/features/settings/pages/settings-page/SettingsMcpManagementSection.tsx', '/src/shared/code-block/monaco-runtime.ts']
])('%s rejects a transitive static leak while allowing a dynamic edge', (scene, owner, dependency) => {
  const lazy = {
    'root.js': chunk('root.js', owner, ['shared.js']),
    'shared.js': chunk('shared.js', '/src/shared.ts', [], ['deferred.js']),
    'deferred.js': chunk('deferred.js', dependency)
  };
  expect(collectScenarioDemandManifest(lazy).scenes[scene].staticClosure).toEqual(['root.js', 'shared.js']);
  expect(() => collectScenarioDemandManifest({
    ...lazy,
    'shared.js': chunk('shared.js', '/src/shared.ts', ['deferred.js'])
  })).toThrow(`${scene} eagerly includes`);
});
