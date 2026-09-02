import { render, waitFor, within } from '@testing-library/react';
import { readdirSync, readFileSync } from 'node:fs';
import { extname, join } from 'node:path';
import type { ComponentType, ReactNode } from 'react';
import { describe, expect, test, vi } from 'vitest';

import type { NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime';
import {
  compileNativeReactComponent,
  NATIVE_TRUSTED_BLOCK_ALLOWED_IMPORTS,
  NATIVE_TRUSTED_BLOCK_PERMISSION,
  NATIVE_TRUSTED_BLOCK_RUNTIME
} from '@1flowbase/page-runtime';

import antDesignColorsPackageJson from '@ant-design/colors/package.json';
import antDesignIconsPackageJson from '@ant-design/icons/package.json';
import antdPackageJson from 'antd/package.json';
import antdStylePackageJson from 'antd-style/package.json';
import dndKitCorePackageJson from '@dnd-kit/core/package.json';
import dndKitModifiersPackageJson from '@dnd-kit/modifiers/package.json';
import dndKitSortablePackageJson from '@dnd-kit/sortable/package.json';
import dndKitUtilitiesPackageJson from '@dnd-kit/utilities/package.json';
import appPackageJson from '../../../../../package.json';
import dayjsPackageJson from 'dayjs/package.json';
import reactPackageJson from 'react/package.json';
import uiPackageJson from '../../../../../../packages/ui/package.json';

import {
  FrontstageNativeTrustedBlockPortalHost,
  createFrontstageUnavailableBlockContext
} from '../../lib/native-trusted-block-react-adapter';
import {
  createFrontstageNativeReactModuleRegistry,
  createFrontstageNativeTrustedBlockModuleMap,
  createFrontstageNativeTrustedBlockRuntimeFactory,
  getFrontstageNativeTrustedBlockRuntimeCompatibility
} from '../../lib/native-trusted-block-runtime-factory';
import { ANTD_STYLE_EXPORTS } from '../../lib/native-modules/antd-style-runtime';
import { ANT_DESIGN_COLORS_EXPORTS } from '../../lib/native-modules/ant-design-colors-runtime';

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
import { useResponsive } from 'antd-style';
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
  test('AC-002 exposes allowed exports from the frontend registry', () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    expect(
      registry.definitions.find(({ module_source }) => module_source === 'antd')
        ?.exports
    ).toContain('Button');
    expect(
      registry.definitions.find(
        ({ module_source }) => module_source === '@ant-design/icons'
      )?.exports
    ).toContain('AntDesignOutlined');
  });

  test('AC-002 does not admit exports absent from the frontend registry', () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    expect(
      registry.definitions.find(({ module_source }) => module_source === 'antd')
        ?.exports
    ).not.toContain('RemovedComponent');
  });

  test('I1945-AC-001/003/004 I1949-AC-002/004 compiles and lazily resolves public @ant-design/icons leaves', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const source = `import ClockCircleOutlined from '@ant-design/icons/ClockCircleOutlined';

export default function Block() {
  return <ClockCircleOutlined />;
}`;

    expect(compileNativeReactComponent(source, registry.definitions).ok).toBe(
      true
    );
    expect(
      registry.definitions.find(
        ({ module_source }) =>
          module_source === '@ant-design/icons/ClockCircleOutlined'
      )
    ).toEqual({
      module_source: '@ant-design/icons/ClockCircleOutlined',
      exports: ['default']
    });

    const [first, second] = await Promise.all([
      registry.load('@ant-design/icons/ClockCircleOutlined'),
      registry.load('@ant-design/icons/ClockCircleOutlined')
    ]);
    expect(first).toBe(second);
    expect(first.default).toBeTypeOf('object');

    const rootModule = await registry.load('@ant-design/icons');
    expect(rootModule.ClockCircleOutlined).toBeTypeOf('object');
    expect(rootModule.createFromIconfontCN).toBeTypeOf('function');
    expect(rootModule.IconProvider).toBeTypeOf('object');

    const registrySource = readFileSync(
      join(
        process.cwd(),
        'src/features/frontstage/lib/native-modules/registry.ts'
      ),
      'utf8'
    );
    expect(registrySource).not.toMatch(
      /import\s+\*\s+as\s+antDesignIconsModule\s+from\s+['"]@ant-design\/icons['"]/u
    );
    expect(registrySource).toContain('loadAntDesignIconsModule');

    expect(
      compileNativeReactComponent(
        `import missing from '@ant-design/icons/NotInstalledOutlined'; export default missing;`,
        registry.definitions
      ).ok
    ).toBe(false);
    expect(
      compileNativeReactComponent(
        `import internal from '@ant-design/icons/es/icons/ClockCircleOutlined'; export default internal;`,
        registry.definitions
      ).ok
    ).toBe(false);
  });

  test('I1907-AC-003/006 exposes a stable lazy antd-style module contract', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    expect(
      registry.definitions.find(
        ({ module_source }) => module_source === 'antd-style'
      )
    ).toEqual({
      module_source: 'antd-style',
      exports: ANTD_STYLE_EXPORTS
    });

    const [first, second] = await Promise.all([
      registry.load('antd-style'),
      registry.load('antd-style')
    ]);
    expect(first).toBe(second);
    for (const exportName of ANTD_STYLE_EXPORTS) {
      expect(first).toHaveProperty(exportName);
    }

    const registrySource = readFileSync(
      join(
        process.cwd(),
        'src/features/frontstage/lib/native-modules/registry.ts'
      ),
      'utf8'
    );
    expect(registrySource).not.toMatch(
      /import\s+\*\s+as\s+antdStyleModule\s+from\s+['"]antd-style['"]/u
    );
    expect(registrySource).toContain('loadAntdStyleModule');
  });

  test('I1932-AC-001/002/006 compiles and lazily resolves the @ant-design/colors package root only', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const source = `import { cyan, generate, presetPalettes } from '@ant-design/colors';

export default function Block() {
  void cyan;
  void generate;
  void presetPalettes;
  return <div />;
}`;

    expect(compileNativeReactComponent(source, registry.definitions).ok).toBe(
      true
    );
    expect(
      registry.definitions.find(
        ({ module_source }) => module_source === '@ant-design/colors'
      )
    ).toEqual({
      module_source: '@ant-design/colors',
      exports: ANT_DESIGN_COLORS_EXPORTS
    });
    const [first, second] = await Promise.all([
      registry.load('@ant-design/colors'),
      registry.load('@ant-design/colors')
    ]);
    expect(first).toBe(second);
    expect(first).toEqual(
      expect.objectContaining({
        cyan: expect.any(Array),
        generate: expect.any(Function),
        presetPalettes: expect.any(Object)
      })
    );
    await expect(
      registry.load('@ant-design/colors/es/generate')
    ).rejects.toMatchObject({ code: 'module_not_registered' });

    const registrySource = readFileSync(
      join(
        process.cwd(),
        'src/features/frontstage/lib/native-modules/registry.ts'
      ),
      'utf8'
    );
    expect(registrySource).not.toMatch(
      /import\s+\*\s+as\s+antDesignColorsModule\s+from\s+['"]@ant-design\/colors['"]/u
    );
    expect(registrySource).toContain('loadAntDesignColorsModule');
  });

  test('I1968-AC-001/003 compiles and lazily resolves the antd-img-crop package root only', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const source = `import ImgCrop from 'antd-img-crop';
import { Upload } from 'antd';

export default function Block() {
  return <ImgCrop rotationSlider><Upload /></ImgCrop>;
}`;

    expect(compileNativeReactComponent(source, registry.definitions).ok).toBe(
      true
    );
    expect(
      registry.definitions.find(
        ({ module_source }) => module_source === 'antd-img-crop'
      )
    ).toEqual({
      module_source: 'antd-img-crop',
      exports: ['default']
    });

    const [first, second] = await Promise.all([
      registry.load('antd-img-crop'),
      registry.load('antd-img-crop')
    ]);
    expect(first).toBe(second);
    expect(first.default).toBeTypeOf('object');

    const [asset] = await registry.resolveModuleAssets(['antd-img-crop']);
    const css = new TextDecoder().decode(asset?.bytes);
    expect(asset).toMatchObject({
      module_source: 'antd-img-crop',
      role: 'shadow_style',
      media_type: 'text/css; charset=utf-8'
    });
    expect(css).toContain('.\\[height\\:40vh\\]');
    expect(
      [...document.head.querySelectorAll('style')].some((style) =>
        style.textContent?.includes('.\\[height\\:40vh\\]')
      )
    ).toBe(false);

    for (const deniedSource of [
      'antd-img-crop/dist/antd-img-crop.esm.js',
      'some-unregistered-react-package'
    ]) {
      expect(
        compileNativeReactComponent(
          `import dependency from '${deniedSource}'; export default dependency;`,
          registry.definitions
        ).ok
      ).toBe(false);
      await expect(registry.load(deniedSource)).rejects.toMatchObject({
        code: 'module_not_registered'
      });
    }
  });

  test('I1933-AC-001/003/004b/004c compiles and lazily resolves the installed dayjs module domain', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const source = `import dayjs from 'dayjs';
import type { Dayjs } from 'dayjs';

export default function Block() {
  const start: Dayjs = dayjs('2026-01-01');
  return <div>{start.add(1, 'day').format('YYYY-MM-DD')}</div>;
}`;

    expect(compileNativeReactComponent(source, registry.definitions).ok).toBe(
      true
    );
    expect(
      registry.definitions.find(
        ({ module_source }) => module_source === 'dayjs'
      )
    ).toEqual({
      module_source: 'dayjs',
      exports: ['default']
    });

    const [first, second] = await Promise.all([
      registry.load('dayjs'),
      registry.load('dayjs')
    ]);
    expect(first).toBe(second);
    expect(first).toHaveProperty('default', expect.any(Function));

    for (const allowedSource of [
      'dayjs/plugin/utc',
      'dayjs/plugin/utc.js',
      'dayjs/locale/zh-cn',
      'dayjs/locale/zh-cn.js'
    ]) {
      expect(
        compileNativeReactComponent(
          `import dependency from '${allowedSource}'; export default () => <div>{String(dependency)}</div>;`,
          registry.definitions
        ).ok
      ).toBe(true);
      await expect(registry.load(allowedSource)).resolves.toHaveProperty(
        'default'
      );
    }

    expect(
      compileNativeReactComponent(
        `import missing from 'dayjs/plugin/not-installed'; export default () => <div>{String(missing)}</div>;`,
        registry.definitions
      ).ok
    ).toBe(false);
    await expect(
      registry.load('dayjs/plugin/not-installed')
    ).rejects.toMatchObject({ code: 'module_not_registered' });

    const registrySource = readFileSync(
      join(
        process.cwd(),
        'src/features/frontstage/lib/native-modules/registry.ts'
      ),
      'utf8'
    );
    expect(registrySource).not.toMatch(
      /import\s+\*\s+as\s+dayjsModule\s+from\s+['"]dayjs['"]/u
    );
    expect(registrySource).toContain('loadDayjsModule');
  });

  test('I1951-AC-001/002/003/004 compiles and lazily resolves lodash/debounce only', async () => {
    vi.useFakeTimers();
    try {
      const registry = createFrontstageNativeReactModuleRegistry();
      const source = `import debounce from 'lodash/debounce';

export default function Block() {
  void debounce;
  return <div />;
}`;

      expect(compileNativeReactComponent(source, registry.definitions).ok).toBe(
        true
      );
      expect(
        registry.definitions.find(
          ({ module_source }) => module_source === 'lodash/debounce'
        )
      ).toEqual({
        module_source: 'lodash/debounce',
        exports: ['default']
      });

      const [first, second] = await Promise.all([
        registry.load('lodash/debounce'),
        registry.load('lodash/debounce')
      ]);
      expect(first).toBe(second);

      const debounce = first.default as (
        callback: () => string,
        wait: number
      ) => {
        (): string | undefined;
        cancel(): void;
        flush(): string | undefined;
      };
      const callback = vi.fn(() => 'completed');
      const debounced = debounce(callback, 100);

      debounced();
      debounced();
      await vi.advanceTimersByTimeAsync(99);
      expect(callback).not.toHaveBeenCalled();
      await vi.advanceTimersByTimeAsync(1);
      expect(callback).toHaveBeenCalledTimes(1);

      debounced();
      debounced.cancel();
      await vi.advanceTimersByTimeAsync(100);
      expect(callback).toHaveBeenCalledTimes(1);

      debounced();
      expect(debounced.flush()).toBe('completed');
      expect(callback).toHaveBeenCalledTimes(2);

      for (const deniedSource of ['lodash', 'lodash/throttle']) {
        expect(
          compileNativeReactComponent(
            `import dependency from '${deniedSource}'; export default dependency;`,
            registry.definitions
          ).ok
        ).toBe(false);
        await expect(registry.load(deniedSource)).rejects.toMatchObject({
          code: 'module_not_registered'
        });
      }

      const registrySource = readFileSync(
        join(
          process.cwd(),
          'src/features/frontstage/lib/native-modules/registry.ts'
        ),
        'utf8'
      );
      expect(registrySource).toContain("import('lodash/debounce')");
      expect(registrySource).not.toMatch(
        /import\s+.*\s+from\s+['"]lodash(?:\/debounce)?['"]/u
      );
    } finally {
      vi.useRealTimers();
    }
  });

  test('I1952-AC-001/002/003/004 compiles and lazily resolves the clsx package root only', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const source = `import clsxDefault, { clsx as clsxNamed } from 'clsx';

export default function Block() {
  void clsxDefault;
  void clsxNamed;
  return <div />;
}`;

    expect(compileNativeReactComponent(source, registry.definitions).ok).toBe(
      true
    );
    expect(
      registry.definitions.find(
        ({ module_source }) => module_source === 'clsx'
      )
    ).toEqual({
      module_source: 'clsx',
      exports: ['default', 'clsx']
    });

    const [first, second] = await Promise.all([
      registry.load('clsx'),
      registry.load('clsx')
    ]);
    expect(first).toBe(second);
    expect(first.default).toBe(first.clsx);

    const clsx = first.clsx as (...inputs: unknown[]) => string;
    expect(
      clsx(
        'base',
        false,
        'active',
        null,
        undefined,
        ['nested', { chosen: true, hidden: false }]
      )
    ).toBe('base active nested chosen');

    expect(
      compileNativeReactComponent(
        `import clsx from 'clsx/lite'; export default clsx;`,
        registry.definitions
      ).ok
    ).toBe(false);
    await expect(registry.load('clsx/lite')).rejects.toMatchObject({
      code: 'module_not_registered'
    });

    const registrySource = readFileSync(
      join(
        process.cwd(),
        'src/features/frontstage/lib/native-modules/registry.ts'
      ),
      'utf8'
    );
    expect(registrySource).toContain("import('clsx')");
    expect(registrySource).not.toMatch(
      /import\s+.*\s+from\s+['"]clsx['"]/u
    );
  });

  test('AC-002 exposes every installed Ant Design ES module source', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();

    expect(
      registry.definitions.find(
        ({ module_source }) => module_source === 'antd/es/masonry/MasonryItem'
      )
    ).toEqual({
      module_source: 'antd/es/masonry/MasonryItem',
      exports: ['*']
    });
    await expect(
      registry.load('antd/es/masonry/MasonryItem')
    ).resolves.toHaveProperty('default');
  });

  test('AC-002 compiles type-only imports from installed Ant Design ES modules', () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const result = compileNativeReactComponent(
      `import type { MasonryItemType } from 'antd/es/masonry/MasonryItem';

const items: MasonryItemType<number>[] = [{ key: 'item-1', data: 120 }];

export default function Block() {
  return <div>{items[0].data}</div>;
}`,
      registry.definitions
    );

    expect(result.ok).toBe(true);
  });

  test('AC-002 compiles runtime imports from installed Ant Design ES modules', () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const result = compileNativeReactComponent(
      `import MasonryItem from 'antd/es/masonry/MasonryItem';

export default function Block() {
  return <MasonryItem item={{ key: 'item-1', data: 120 }} />;
}`,
      registry.definitions
    );

    expect(result.ok).toBe(true);
  });

  test('I1929-AC-001/002/005 compiles and lazily resolves the installed @dnd-kit module domain', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const source = `import type { DragEndEvent } from '@dnd-kit/core';
import { closestCenter, DndContext, PointerSensor, useSensor } from '@dnd-kit/core';
import { arrayMove, horizontalListSortingStrategy, SortableContext, useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';

export default function Block() {
  void (undefined as DragEndEvent | undefined);
  void closestCenter;
  void DndContext;
  void PointerSensor;
  void useSensor;
  void arrayMove;
  void horizontalListSortingStrategy;
  void SortableContext;
  void useSortable;
  void CSS;
  return <div />;
}`;

    expect(compileNativeReactComponent(source, registry.definitions).ok).toBe(
      true
    );
    expect(
      registry.definitions.find(
        ({ module_source }) => module_source === '@dnd-kit/core/dist/index.js'
      )
    ).toEqual({
      module_source: '@dnd-kit/core/dist/index.js',
      exports: ['*']
    });

    const [first, second, internal] = await Promise.all([
      registry.load('@dnd-kit/core'),
      registry.load('@dnd-kit/core'),
      registry.load('@dnd-kit/core/dist/index.js')
    ]);
    expect(first).toBe(second);
    expect(first).toHaveProperty('DndContext');
    expect(internal).toHaveProperty('DndContext');
    await expect(registry.load('@dnd-kit/not-installed')).rejects.toMatchObject(
      {
        code: 'module_not_registered',
        message: 'Frontend module is not registered: @dnd-kit/not-installed.'
      }
    );
  });

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
        'antd-style': {
          importSource: 'antd-style',
          hostDependencyRange: appPackageJson.dependencies['antd-style'],
          packageVersion: antdStylePackageJson.version
        },
        '@1flowbase/ui': {
          importSource: '@1flowbase/ui',
          hostDependencyRange: appPackageJson.dependencies['@1flowbase/ui'],
          packageVersion: uiPackageJson.version
        }
      },
      lazyModules: {
        '@ant-design/colors': {
          importSource: '@ant-design/colors',
          hostDependencyRange:
            appPackageJson.dependencies['@ant-design/colors'],
          packageVersion: antDesignColorsPackageJson.version
        }
      },
      moduleDomains: {
        '@ant-design/icons': {
          packageName: '@ant-design/icons',
          hostDependencyRange:
            appPackageJson.dependencies['@ant-design/icons'],
          packageVersion: antDesignIconsPackageJson.version,
          moduleCount: expect.any(Number)
        },
        '@dnd-kit': {
          packages: [
            {
              packageName: '@dnd-kit/core',
              hostDependencyRange: appPackageJson.dependencies['@dnd-kit/core'],
              packageVersion: dndKitCorePackageJson.version
            },
            {
              packageName: '@dnd-kit/modifiers',
              hostDependencyRange:
                appPackageJson.dependencies['@dnd-kit/modifiers'],
              packageVersion: dndKitModifiersPackageJson.version
            },
            {
              packageName: '@dnd-kit/sortable',
              hostDependencyRange:
                appPackageJson.dependencies['@dnd-kit/sortable'],
              packageVersion: dndKitSortablePackageJson.version
            },
            {
              packageName: '@dnd-kit/utilities',
              hostDependencyRange:
                appPackageJson.dependencies['@dnd-kit/utilities'],
              packageVersion: dndKitUtilitiesPackageJson.version
            }
          ]
        },
        dayjs: {
          packageName: 'dayjs',
          hostDependencyRange: (
            appPackageJson.dependencies as Record<string, string>
          ).dayjs,
          packageVersion: dayjsPackageJson.version,
          moduleCount: expect.any(Number)
        }
      }
    });
    expect(
      manifest.moduleDomains['@ant-design/icons'].moduleCount
    ).toBeGreaterThan(800);
    expect(manifest.moduleDomains.dayjs.moduleCount).toBeGreaterThan(100);
    expect(manifest.contractVersion).toBe('1.6.0');
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
    const overrideComponent = createFrontstageNativeTrustedBlockRuntimeFactory({
      modules: {
        antd: { Button: OverrideButton }
      }
    })(overridePlan);
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
      within(defaultElement.shadowRoot as unknown as HTMLElement).queryByText(
        'Override: Default modules'
      )
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
      'antd-style',
      'react',
      'react/jsx-runtime'
    ]);
  });

  test('keeps the legacy synchronous factory out of production components and catalog code', () => {
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

    expect(matches).toEqual([]);
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
