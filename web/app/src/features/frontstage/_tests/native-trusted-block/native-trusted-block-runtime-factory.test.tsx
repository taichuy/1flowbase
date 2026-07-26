import { render, waitFor, within } from '@testing-library/react';
import { readdirSync, readFileSync } from 'node:fs';
import { extname, join } from 'node:path';
import type { ComponentType, ReactNode } from 'react';
import { describe, expect, test, vi } from 'vitest';

import type { NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime';
import {
  NATIVE_TRUSTED_BLOCK_ALLOWED_IMPORTS,
  NATIVE_TRUSTED_BLOCK_PERMISSION,
  NATIVE_TRUSTED_BLOCK_RUNTIME
} from '@1flowbase/page-runtime';

import antdPackageJson from 'antd/package.json';
import appPackageJson from '../../../../../package.json';
import reactPackageJson from 'react/package.json';
import uiPackageJson from '../../../../../../packages/ui/package.json';

import {
  FrontstageNativeTrustedBlockPortalHost,
  createFrontstageUnavailableBlockContext
} from '../../lib/native-trusted-block-react-adapter';
import {
  createFrontstageNativeTrustedBlockModuleMap,
  createFrontstageNativeTrustedBlockRuntimeFactory,
  getFrontstageNativeTrustedBlockRuntimeCompatibility
} from '../../lib/native-trusted-block-runtime-factory';

function createPlan(
  overrides: Partial<NativeTrustedBlockPreparePlan> = {}
): NativeTrustedBlockPreparePlan {
  return {
    runtime: 'native_trusted_block',
    blockId: 'native-block-1',
    entry: 'default',
    source: `
import React from 'react';
import { Button } from 'antd';
import { AppThemeProvider } from '@1flowbase/ui';

export default function Block(props) {
  return React.createElement(
    AppThemeProvider,
    null,
    React.createElement(Button, null, props.props.title)
  );
}
`,
    normalizedSource: '',
    props: { title: 'Native runtime ready' },
    requiredPermissions: ['ui_block.javascript.native'],
    ...overrides
  };
}

function createBlockRoot(): HTMLDivElement {
  const root = document.createElement('div');
  document.body.append(root);
  return root;
}

describe('frontstage native trusted block runtime factory', () => {
  test('exposes a serializable host compatibility manifest for injected modules', () => {
    const manifest = getFrontstageNativeTrustedBlockRuntimeCompatibility();

    expect(JSON.parse(JSON.stringify(manifest))).toEqual(manifest);
    expect(manifest).toEqual({
      runtime: NATIVE_TRUSTED_BLOCK_RUNTIME,
      contractVersion: expect.any(String),
      requiredPermission: NATIVE_TRUSTED_BLOCK_PERMISSION,
      allowedImports: NATIVE_TRUSTED_BLOCK_ALLOWED_IMPORTS,
      host: {
        packageName: appPackageJson.name,
        appVersion: appPackageJson.version
      },
      modules: {
        react: {
          importSource: 'react',
          hostDependencyRange: appPackageJson.dependencies.react,
          packageVersion: reactPackageJson.version
        },
        antd: {
          importSource: 'antd',
          hostDependencyRange: appPackageJson.dependencies.antd,
          packageVersion: antdPackageJson.version
        },
        '@1flowbase/ui': {
          importSource: '@1flowbase/ui',
          hostDependencyRange: appPackageJson.dependencies['@1flowbase/ui'],
          packageVersion: uiPackageJson.version
        }
      }
    });
    expect(manifest.contractVersion).toMatch(/^\d+\.\d+\.\d+$/);
  });

  test('evaluates valid non-JSX source through host modules and renders through the surface portal', async () => {
    const root = createBlockRoot();
    const plan = createPlan();
    const component = createFrontstageNativeTrustedBlockRuntimeFactory()(plan);
    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="runtime:1"
        plan={plan}
        component={component}
        ctx={createFrontstageUnavailableBlockContext(plan)}
      />
    );

    expect(
      await within(root.shadowRoot as unknown as HTMLElement).findByRole(
        'button',
        { name: 'Native runtime ready' }
      )
    ).toBeInTheDocument();
  });

  test('rejects evaluator failures before rendering', () => {
    const resolver = createFrontstageNativeTrustedBlockRuntimeFactory();
    let failure: unknown;
    try {
      resolver(
        createPlan({
          source: `
import React from 'react';

eval('2 + 2');

export default function Block() {
  return React.createElement('div', null, 'Denied');
}
`
        })
      );
    } catch (error) {
      failure = error;
    }

    expect(failure).toMatchObject({
      kind: 'source_policy_failed',
      message: 'Native trusted block source policy failed.'
    });
  });

  test('reports component render capability guard failures with structured runtime paths', async () => {
    const onRuntimeError = vi.fn();
    const consoleErrorSpy = vi
      .spyOn(console, 'error')
      .mockImplementation(() => {});
    const root = createBlockRoot();
    const plan = createPlan({
      source: `
import React from 'react';

export default function Block() {
  f\\u0065tch('/api/native-trusted-block');
  return React.createElement('div', null, 'Denied');
}
`
    });
    const component = createFrontstageNativeTrustedBlockRuntimeFactory()(plan);

    try {
      render(
        <FrontstageNativeTrustedBlockPortalHost
          root={root}
          renderEpoch="capability:1"
          plan={plan}
          component={component}
          ctx={createFrontstageUnavailableBlockContext(plan)}
          onRuntimeError={onRuntimeError}
        />
      );

      await waitFor(() => {
        expect(onRuntimeError).toHaveBeenCalledWith(
          expect.objectContaining({
            code: 'runtime_error',
            path: 'runtime.capability.fetch'
          }),
          expect.objectContaining({ blockId: 'native-block-1' })
        );
      });
    } finally {
      consoleErrorSpy.mockRestore();
    }
  });

  test('scopes module overrides to each created resolver', async () => {
    const OverrideButton: ComponentType<{ children?: ReactNode }> = ({
      children
    }) => (
      <button data-testid="override-button" type="button">
        Override: {children}
      </button>
    );

    const overrideElement = createBlockRoot();
    const overridePlan = createPlan({ props: { title: 'Scoped override' } });
    const overrideComponent = createFrontstageNativeTrustedBlockRuntimeFactory(
      {
        modules: {
          antd: { Button: OverrideButton }
        }
      }
    )(overridePlan);
    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={overrideElement}
        renderEpoch="override:1"
        plan={overridePlan}
        component={overrideComponent}
        ctx={createFrontstageUnavailableBlockContext(overridePlan)}
      />
    );

    expect(
      await within(
        overrideElement.shadowRoot as unknown as HTMLElement
      ).findByTestId('override-button')
    ).toHaveTextContent('Override: Scoped override');

    const defaultElement = createBlockRoot();
    const defaultPlan = createPlan({ props: { title: 'Default modules' } });
    const defaultComponent =
      createFrontstageNativeTrustedBlockRuntimeFactory()(defaultPlan);
    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={defaultElement}
        renderEpoch="default:1"
        plan={defaultPlan}
        component={defaultComponent}
        ctx={createFrontstageUnavailableBlockContext(defaultPlan)}
      />
    );

    expect(
      await within(
        defaultElement.shadowRoot as unknown as HTMLElement
      ).findByRole('button', { name: 'Default modules' })
    ).toBeInTheDocument();
    expect(
      within(
        defaultElement.shadowRoot as unknown as HTMLElement
      ).queryByText('Override: Default modules')
    ).not.toBeInTheDocument();
  });

  test('does not statically expose API or query clients through the runtime module map', () => {
    const runtimeFactorySource = readFileSync(
      join(
        process.cwd(),
        'src/features/frontstage/lib/native-trusted-block-runtime-factory.ts'
      ),
      'utf8'
    );
    const moduleMap = createFrontstageNativeTrustedBlockModuleMap();

    expect(runtimeFactorySource).not.toContain('@1flowbase/api-client');
    expect(runtimeFactorySource).not.toContain('@tanstack/react-query');
    expect(runtimeFactorySource).not.toContain('QueryClient');
    expect(Object.keys(moduleMap).sort()).toEqual([
      '@1flowbase/ui',
      'antd',
      'react',
      'react/jsx-runtime'
    ]);
  });

  test('is consumed only by the shared TrialPanel and not by catalog code', () => {
    const frontstageDir = join(process.cwd(), 'src/features/frontstage');
    const scannedFiles = collectSourceFiles([
      join(frontstageDir, 'pages'),
      join(frontstageDir, 'components')
    ]).concat(
      collectSourceFiles([
        join(frontstageDir, 'api'),
        join(frontstageDir, 'hooks'),
        join(frontstageDir, 'lib')
      ]).filter((filePath) => filePath.includes('block-catalog'))
    );

    const matches = scannedFiles.filter((filePath) =>
      readFileSync(filePath, 'utf8').includes(
        'native-trusted-block-runtime-factory'
      )
    );

    expect(matches).toEqual([
      expect.stringMatching(/components\/JsBlockTrialPanel\.tsx$/u)
    ]);
  });
});

function collectSourceFiles(directories: string[]): string[] {
  const files: string[] = [];

  for (const directory of directories) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const entryPath = join(directory, entry.name);
      if (entry.isDirectory()) {
        files.push(...collectSourceFiles([entryPath]));
        continue;
      }

      if (SOURCE_FILE_EXTENSIONS.has(extname(entry.name))) {
        files.push(entryPath);
      }
    }
  }

  return files;
}

const SOURCE_FILE_EXTENSIONS = new Set(['.ts', '.tsx']);
