import * as ReactModule from 'react';
import * as ReactJsxRuntimeModule from 'react/jsx-runtime';
import {
  ANTD_PRODUCT_EXPORT_PATHS,
  ANTD_ROOT_EXPORTS,
  ANTD_ES_MODULE_DEFINITIONS,
  loadAntDesignRootModule,
  loadAntDesignEsModule,
  prewarmAntDesignProductDomain
} from 'virtual:1flowbase-native-antd-es-modules';
import {
  ANT_DESIGN_ICONS_MODULE_DEFINITIONS,
  loadAntDesignIconsModule,
  prewarmAntDesignIconsDomain
} from 'virtual:1flowbase-native-ant-design-icons-modules';
import {
  DND_KIT_MODULE_DEFINITIONS,
  loadDndKitModule
} from 'virtual:1flowbase-native-dnd-kit-modules';
import {
  DAYJS_MODULE_DEFINITIONS,
  loadDayjsModule
} from 'virtual:1flowbase-native-dayjs-modules';

import {
  createNativeReactModuleRegistry,
  type NativeReactFrontendModuleLoadResult,
  type NativeReactFrontendModuleRegistration,
  type NativeReactModuleDefinition,
  type NativeReactModuleRegistry,
  type NativeTrustedBlockInjectedModuleMap
} from '@1flowbase/page-runtime/browser';

import {
  ANT_DESIGN_COLORS_EXPORTS,
  loadAntDesignColorsModule
} from './ant-design-colors-runtime';
import {
  ANTD_STYLE_EXPORTS,
  loadAntdStyleModuleForArtifact
} from './antd-style-runtime';
import { loadAntdImgCropModule } from './image-crop/antd-img-crop-runtime';

type ModuleNamespace = Record<string, unknown>;

const UI_MODULE_EXPORTS = ['AppShell', 'AppThemeProvider'] as const;

async function loadNativeAntDesignModule(): Promise<ModuleNamespace> {
  const [
    antdModule,
    { NativeBlockAffix },
    { NativeBlockAnchor },
    { NativeBlockDropdown },
    { NativeBlockMessage },
    { NativeBlockNotification },
    { NativeBlockMenu },
    {
      NativeBlockCascader,
      NativeBlockDatePicker,
      NativeBlockSelect,
      NativeBlockTreeSelect
    },
    { NativeBlockPopover, NativeBlockTooltip },
    { NativeBlockDrawer },
    { NativeBlockPopconfirm }
  ] = await Promise.all([
    loadAntDesignRootModule(),
    import('./affix-anchor/native-affix-runtime'),
    import('./affix-anchor/native-anchor-runtime'),
    import('./native-dropdown-runtime'),
    import('./native-message-runtime'),
    import('./notification/facade'),
    import('./menu/native-menu-runtime'),
    import('./overlay/native-generic-overlay-runtime'),
    import('./overlay/native-tooltip-popover-runtime'),
    import('./overlay/native-drawer-runtime'),
    import('./overlay/native-popconfirm-runtime')
  ]);
  return {
    ...antdModule,
    Affix: NativeBlockAffix,
    Anchor: NativeBlockAnchor,
    Cascader: NativeBlockCascader,
    DatePicker: NativeBlockDatePicker,
    Drawer: NativeBlockDrawer,
    Dropdown: NativeBlockDropdown,
    Menu: NativeBlockMenu,
    Popconfirm: NativeBlockPopconfirm,
    Popover: NativeBlockPopover,
    Select: NativeBlockSelect,
    Tooltip: NativeBlockTooltip,
    TreeSelect: NativeBlockTreeSelect,
    message: NativeBlockMessage,
    notification: NativeBlockNotification
  };
}

const ANTD_HOST_EXPORT_LOADERS: Readonly<
  Record<string, () => Promise<unknown>>
> = {
  Affix: () =>
    import('./affix-anchor/native-affix-runtime').then(
      ({ NativeBlockAffix }) => NativeBlockAffix
    ),
  Anchor: () =>
    import('./affix-anchor/native-anchor-runtime').then(
      ({ NativeBlockAnchor }) => NativeBlockAnchor
    ),
  Cascader: () =>
    import('./overlay/native-cascader-runtime').then(
      ({ NativeBlockCascader }) => NativeBlockCascader
    ),
  DatePicker: () =>
    import('./overlay/native-date-picker-runtime').then(
      ({ NativeBlockDatePicker }) => NativeBlockDatePicker
    ),
  Drawer: () =>
    import('./overlay/native-drawer-runtime').then(
      ({ NativeBlockDrawer }) => NativeBlockDrawer
    ),
  Dropdown: () =>
    import('./native-dropdown-runtime').then(
      ({ NativeBlockDropdown }) => NativeBlockDropdown
    ),
  Menu: () =>
    import('./menu/native-menu-runtime').then(
      ({ NativeBlockMenu }) => NativeBlockMenu
    ),
  Popconfirm: () =>
    import('./overlay/native-popconfirm-runtime').then(
      ({ NativeBlockPopconfirm }) => NativeBlockPopconfirm
    ),
  Popover: () =>
    import('./overlay/native-popover-runtime').then(
      ({ NativeBlockPopover }) => NativeBlockPopover
    ),
  Select: () =>
    import('./overlay/native-select-runtime').then(
      ({ NativeBlockSelect }) => NativeBlockSelect
    ),
  Tooltip: () =>
    import('./overlay/native-tooltip-runtime').then(
      ({ NativeBlockTooltip }) => NativeBlockTooltip
    ),
  TreeSelect: () =>
    import('./overlay/native-tree-select-runtime').then(
      ({ NativeBlockTreeSelect }) => NativeBlockTreeSelect
    ),
  message: () =>
    import('./native-message-runtime').then(
      ({ NativeBlockMessage }) => NativeBlockMessage
    ),
  notification: () =>
    import('./notification/facade').then(
      ({ NativeBlockNotification }) => NativeBlockNotification
    )
};

async function loadSelectiveNativeAntDesignModule(
  requestedExports: readonly string[]
): Promise<NativeReactFrontendModuleLoadResult> {
  if (
    requestedExports.some(
      (name) =>
        !Object.hasOwn(ANTD_HOST_EXPORT_LOADERS, name) &&
        !Object.hasOwn(ANTD_PRODUCT_EXPORT_PATHS, name)
    )
  ) {
    return { module: await loadNativeAntDesignModule() };
  }
  const entries = await Promise.all(
    requestedExports.map(async (name) => {
      const hostLoader = ANTD_HOST_EXPORT_LOADERS[name];
      if (hostLoader) {
        return [name, await hostLoader()] as const;
      }
      const subpath = ANTD_PRODUCT_EXPORT_PATHS[name]!;
      const loaded = await loadAntDesignEsModule(subpath);
      return [name, loaded.default] as const;
    })
  );
  return { module: Object.fromEntries(entries) };
}

const ICON_ROOT_HELPER_EXPORTS = new Set([
  'default',
  'IconProvider',
  'createFromIconfontCN',
  'getTwoToneColor',
  'setTwoToneColor'
]);

async function loadSelectiveAntDesignIconsRootModule(
  requestedExports: readonly string[]
): Promise<NativeReactFrontendModuleLoadResult> {
  if (requestedExports.some((name) => ICON_ROOT_HELPER_EXPORTS.has(name))) {
    return { module: await loadAntDesignIconsModule('@ant-design/icons') };
  }
  const entries = await Promise.all(
    requestedExports.map(async (name) => {
      const leaf = await loadAntDesignIconsModule(`@ant-design/icons/${name}`);
      return [name, leaf.default] as const;
    })
  );
  return { module: Object.fromEntries(entries) };
}

const ANT_DESIGN_X_EXPORTS = [
  'Actions',
  'Attachments',
  'Bubble',
  'CodeHighlighter',
  'Conversations',
  'FileCard',
  'Folder',
  'Mermaid',
  'Prompts',
  'Sender',
  'SenderSwitch',
  'Sources',
  'Suggestion',
  'Think',
  'ThoughtChain',
  'Welcome',
  'XProvider',
  'notification',
  'version'
] as const;

const registrations: readonly NativeReactFrontendModuleRegistration[] = [
  registration('react', Object.keys(reactModule()), async () => ({
    module: reactModule()
  })),
  registration(
    'react/jsx-runtime',
    Object.keys(ReactJsxRuntimeModule),
    async () => ({ module: ReactJsxRuntimeModule })
  ),
  registration(
    'antd',
    ANTD_ROOT_EXPORTS,
    async () => ({
      module: await loadNativeAntDesignModule()
    }),
    loadSelectiveNativeAntDesignModule
  ),
  ...ANTD_ES_MODULE_DEFINITIONS.map(({ module_source, exports }) =>
    registration(module_source, exports, async () => {
      const module = await loadAntDesignEsModule(module_source);
      if (/^antd\/es\/drawer(?:\/index)?(?:\.js)?$/u.test(module_source)) {
        const { NativeBlockDrawer } =
          await import('./overlay/native-drawer-runtime');
        return { module: { ...module, default: NativeBlockDrawer } };
      }
      if (/^antd\/es\/popconfirm(?:\/index)?(?:\.js)?$/u.test(module_source)) {
        const { NativeBlockPopconfirm } =
          await import('./overlay/native-popconfirm-runtime');
        return { module: { ...module, default: NativeBlockPopconfirm } };
      }
      return { module };
    })
  ),
  ...DND_KIT_MODULE_DEFINITIONS.map(({ module_source, exports }) =>
    registration(module_source, exports, async () => ({
      module: await loadDndKitModule(module_source)
    }))
  ),
  registration('@ant-design/colors', ANT_DESIGN_COLORS_EXPORTS, async () => ({
    module: await loadAntDesignColorsModule()
  })),
  registration('@ant-design/cssinjs', ['StyleProvider'], async () => ({
    module: await import('./effects/cssinjs-runtime')
  })),
  registration('@ant-design/happy-work-theme', ['HappyProvider'], async () => ({
    module: await import('./effects/happy-work-runtime')
  })),
  registration('antd-img-crop', ['default'], loadAntdImgCropModule),
  ...DAYJS_MODULE_DEFINITIONS.map(({ module_source, exports }) =>
    registration(module_source, exports, async () => ({
      module: await loadDayjsModule(module_source)
    }))
  ),
  registration('lodash/difference', ['default'], async () => {
    const module = await import('lodash/difference');
    return { module: { default: module.default } };
  }),
  registration('react-draggable', ['default', 'DraggableCore'], async () => {
    const module = await import('react-draggable');
    return {
      module: { default: module.default, DraggableCore: module.DraggableCore }
    };
  }),
  registration('lodash/debounce', ['default'], async () => {
    const debounceModule = await import('lodash/debounce');
    return { module: { default: debounceModule.default } };
  }),
  registration('react-countup', ['default', 'useCountUp'], async () => ({
    module: await import('./countup/runtime')
  })),
  registration('@rc-component/virtual-list', ['default'], async () => ({
    module: await import('@rc-component/virtual-list')
  })),
  registration('react-infinite-scroll-component', ['default'], async () => ({
    module: await import('./infinite-scroll/runtime')
  })),
  registration('clsx', ['default', 'clsx'], async () => ({
    module: await import('clsx')
  })),
  registration(
    'antd-style',
    ANTD_STYLE_EXPORTS,
    loadAntdStyleModuleForArtifact
  ),
  registration('@1flowbase/ui', UI_MODULE_EXPORTS, async () => ({
    module: await import('@1flowbase/ui')
  })),
  registration('@1flowbase/resizable-drawer', ['ResizableDrawer'], async () => {
    const [{ NativeBlockResizableDrawer }, drawerStyle] = await Promise.all([
      import('./overlay/native-resizable-drawer-runtime'),
      import('../../../../shared/ui/resizable-drawer/resizable-drawer.css?inline')
    ]);
    return {
      module: {
        ResizableDrawer: NativeBlockResizableDrawer
      },
      styles: [{ css: drawerStyle.default }]
    };
  }),
  registration(
    '@1flowbase/plugin-settings',
    [
      'request',
      'openStream',
      'RecordPermissionEditor',
      'getRecordCapabilities'
    ],
    async () => ({
      module: await import('./plugin-settings/runtime')
    })
  ),
  registration(
    '@1flowbase/data-table',
    [
      'DataTable',
      'DataTableColumnSettings',
      'usePersistedDataTableConfiguration',
      'DataTableLayout'
    ],
    async () => {
      const [
        table,
        state,
        layout,
        tableStyle,
        layoutStyle,
        mobileStyle,
        columnStyle
      ] = await Promise.all([
        import('../../../../shared/ui/data-table/DataTable'),
        import('../../../../shared/ui/data-table/data-table-state'),
        import('../../../../shared/ui/data-table/DataTableLayout'),
        import('../../../../shared/ui/data-table/data-table.css?inline'),
        import('../../../../shared/ui/data-table/data-table-layout.css?inline'),
        import('../../../../shared/ui/data-table/data-table-mobile-list.css?inline'),
        import('../../../../shared/ui/data-table/data-table-column-settings.css?inline')
      ]);
      return {
        module: {
          DataTable: table.DataTable,
          DataTableColumnSettings: table.DataTableColumnSettings,
          usePersistedDataTableConfiguration:
            state.usePersistedDataTableConfiguration,
          DataTableLayout: layout.DataTableLayout
        },
        styles: [tableStyle, layoutStyle, mobileStyle, columnStyle].map(
          (style) => ({
            css: style.default
          })
        )
      };
    }
  ),
  registration(
    '@1flowbase/block-design',
    [
      'useRegisterBlockSettings',
      'BlockDesignButton',
      'BlockDesignToolbar',
      'BlockDesignMenu',
      'BLOCK_DESIGN_THEME'
    ],
    async () => ({
      module: await import('./block-design/module'),
      styles: [
        {
          css:
            (await import('./block-design/block-design.css?inline')).default +
            (
              await import('../../components/frontstage-node-action-button.css?inline')
            ).default
        }
      ]
    })
  ),
  registration(
    '@1flowbase/data-blocks',
    ['TableBlock', 'FormBlock'],
    async () => {
      const [table, form, tableStyle, formStyle, designStyle, nodeActionStyle] =
        await Promise.all([
          import('../../components/data-blocks/table/TableBlock'),
          import('../../components/data-blocks/form/FormBlock'),
          import('../../components/data-blocks/table/table-block.css?inline'),
          import('../../components/data-blocks/form/form-block.css?inline'),
          import('./block-design/block-design.css?inline'),
          import('../../components/frontstage-node-action-button.css?inline')
        ]);
      return {
        module: { TableBlock: table.TableBlock, FormBlock: form.FormBlock },
        styles: [tableStyle, formStyle, designStyle, nodeActionStyle].map(
          (style) => ({
            css: style.default
          })
        )
      };
    },
    async (requestedExports) => {
      const needsTable = requestedExports.includes('TableBlock');
      const needsForm = requestedExports.includes('FormBlock');
      const [table, form, tableStyle, formStyle, designStyle, nodeActionStyle] =
        await Promise.all([
          needsTable
            ? import('../../components/data-blocks/table/TableBlock')
            : null,
          needsForm
            ? import('../../components/data-blocks/form/FormBlock')
            : null,
          needsTable
            ? import('../../components/data-blocks/table/table-block.css?inline')
            : null,
          needsForm
            ? import('../../components/data-blocks/form/form-block.css?inline')
            : null,
          import('./block-design/block-design.css?inline'),
          import('../../components/frontstage-node-action-button.css?inline')
        ]);
      return {
        module: {
          ...(table ? { TableBlock: table.TableBlock } : {}),
          ...(form ? { FormBlock: form.FormBlock } : {})
        },
        styles: [tableStyle, formStyle, designStyle, nodeActionStyle]
          .filter((style): style is { default: string } => style !== null)
          .map((style) => ({
            css: style.default
          }))
      };
    }
  ),
  registration('@1flowbase/block-sdk', ['blockSdkVersion'], async () => ({
    module: await import('@1flowbase/block-sdk')
  })),
  registration('@1flowbase/files', ['FileTableView'], async () => {
    const [
      module,
      tableStyle,
      layoutStyle,
      mobileStyle,
      columnStyle,
      fileStyle
    ] = await Promise.all([
      import('../../components/files/FileTableView'),
      import('../../../../shared/ui/data-table/data-table.css?inline'),
      import('../../../../shared/ui/data-table/data-table-layout.css?inline'),
      import('../../../../shared/ui/data-table/data-table-mobile-list.css?inline'),
      import('../../../../shared/ui/data-table/data-table-column-settings.css?inline'),
      import('../../components/files/file-table-view.css?inline')
    ]);
    return {
      module,
      styles: [
        tableStyle,
        layoutStyle,
        mobileStyle,
        columnStyle,
        fileStyle
      ].map((style) => ({
        css: style.default
      }))
    };
  }),
  registration(
    '@1flowbase/application-run-logs',
    ['ApplicationRunLogViewer', 'ApplicationLogs', 'AllAgentFlowLogs'],
    async () => {
      const [viewer, workspace] = await Promise.all([
        import('../../../applications/components/logs/ApplicationRunLogViewer'),
        import('../../../applications/components/logs/workspace/ApplicationLogs')
      ]);
      return {
        module: {
          ApplicationRunLogViewer: viewer.ApplicationRunLogViewer,
          ...workspace
        }
      };
    },
    async (requestedExports) => {
      const needsViewer = requestedExports.includes('ApplicationRunLogViewer');
      const needsWorkspace = requestedExports.some(
        (name) => name === 'ApplicationLogs' || name === 'AllAgentFlowLogs'
      );
      const [viewer, workspace] = await Promise.all([
        needsViewer
          ? import('../../../applications/components/logs/ApplicationRunLogViewer')
          : null,
        needsWorkspace
          ? import('../../../applications/components/logs/workspace/ApplicationLogs')
          : null
      ]);
      return {
        module: {
          ...(viewer
            ? { ApplicationRunLogViewer: viewer.ApplicationRunLogViewer }
            : {}),
          ...(workspace ?? {})
        }
      };
    }
  ),
  registration(
    '@1flowbase/native-components',
    ['ScrollableSurface', 'Surface'],
    async () => {
      const [module, style] = await Promise.all([
        import('@1flowbase/native-components'),
        import('@1flowbase/native-components/styles.css?inline')
      ]);
      return { module, styles: [{ css: style.default }] };
    }
  ),
  ...ANT_DESIGN_ICONS_MODULE_DEFINITIONS.map(({ module_source, exports }) =>
    registration(
      module_source,
      exports,
      async () => ({
        module: await loadAntDesignIconsModule(module_source)
      }),
      module_source === '@ant-design/icons'
        ? loadSelectiveAntDesignIconsRootModule
        : undefined
    )
  ),
  registration('@1flowbase/token-trend', ['TokenTrendChart'], async () => {
    const { NativeBlockTokenTrendChart } = await import('./charts/runtime');
    return { module: { TokenTrendChart: NativeBlockTokenTrendChart } };
  }),
  registration('@1flowbase/charts', ['EChart'], async () => {
    const { NativeBlockEChart } = await import('./charts/runtime');
    return { module: { EChart: NativeBlockEChart } };
  }),
  registration(
    '@1flowbase/rich-text',
    ['VditorEditor', 'VditorPreview'],
    async () => {
      const [module, host, vditorStyle, richTextStyle] = await Promise.all([
        import('@1flowbase/rich-text'),
        import('../../../../shared/ui/markdown-ir-editor/UserPreferenceVditorEditor'),
        import('vditor/dist/index.css?inline'),
        import('@1flowbase/rich-text/styles.css?inline')
      ]);
      return {
        module: { ...module, VditorEditor: host.UserPreferenceVditorEditor },
        styles: [{ css: vditorStyle.default }, { css: richTextStyle.default }]
      };
    },
    async (requestedExports) => {
      const needsEditor = requestedExports.includes('VditorEditor');
      const [module, host, vditorStyle, richTextStyle] = await Promise.all([
        import('@1flowbase/rich-text'),
        needsEditor
          ? import('../../../../shared/ui/markdown-ir-editor/UserPreferenceVditorEditor')
          : null,
        import('vditor/dist/index.css?inline'),
        import('@1flowbase/rich-text/styles.css?inline')
      ]);
      return {
        module: {
          ...module,
          ...(host ? { VditorEditor: host.UserPreferenceVditorEditor } : {})
        },
        styles: [{ css: vditorStyle.default }, { css: richTextStyle.default }]
      };
    }
  ),
  registration('@ant-design/x', [...ANT_DESIGN_X_EXPORTS], async () => ({
    module: await import('@ant-design/x')
  })),
  registration(
    '@ant-design/x-markdown',
    ['default', 'XMarkdown', 'AnimationText', 'useStreaming', 'version'],
    async () => ({ module: await import('@ant-design/x-markdown') })
  )
];

export const FRONTSTAGE_NATIVE_REACT_MODULE_DEFINITIONS: readonly NativeReactModuleDefinition[] =
  registrations.map(({ module_source, exports }) => ({
    module_source,
    exports: [...exports]
  }));

let sharedRegistry: NativeReactModuleRegistry | undefined;

export function getFrontstageNativeReactModuleRegistry(): NativeReactModuleRegistry {
  sharedRegistry ??= createNativeReactModuleRegistry(registrations);
  return sharedRegistry;
}

export function createFrontstageNativeReactModuleRegistry(
  overrides: NativeTrustedBlockInjectedModuleMap = {},
  additionalRegistrations: readonly NativeReactFrontendModuleRegistration[] = []
): NativeReactModuleRegistry {
  // Each artifact evaluation owns its module flights and generated style assets.
  const registry = createNativeReactModuleRegistry([
    ...registrations.map((entry) => {
      const override = overrides[entry.module_source];
      if (!override) return entry;
      return {
        ...entry,
        async load(): Promise<NativeReactFrontendModuleLoadResult> {
          const loaded = await entry.load();
          return { ...loaded, module: { ...loaded.module, ...override } };
        },
        ...(entry.loadExports
          ? {
              async loadExports(
                requestedExports: readonly string[]
              ): Promise<NativeReactFrontendModuleLoadResult> {
                const loaded = await entry.loadExports!(requestedExports);
                return { ...loaded, module: { ...loaded.module, ...override } };
              }
            }
          : {})
      };
    }),
    ...additionalRegistrations
  ]);
  return Object.assign(registry, {
    prewarmSource: prewarmNativeModuleSources
  });
}

export interface ExtractedNativeSourceImport {
  source: string;
  named: string[];
  hasDefaultOrNamespace: boolean;
}

/**
 * Linear O(|S|) comment-stripping import clause scanner used to start fetching
 * heavy runtime dependencies (charts, styles, selective antd/icon leaves) in
 * parallel with Web Worker TSX compilation.
 */
export function extractNativeSourceImports(
  source: string
): ExtractedNativeSourceImport[] {
  const uncommented = source
    .replace(/\/\*[\s\S]*?\*\//gu, ' ')
    .replace(/(^|[^:\\])\/\/[^\r\n]*/gu, '$1 ');
  const bySource = new Map<
    string,
    { named: Set<string>; hasDefaultOrNamespace: boolean }
  >();
  const importPattern =
    /\bimport\s+(?!type\s)([\s\S]*?)\s+from\s*['"]([^'"]+)['"]/gu;
  for (const match of uncommented.matchAll(importPattern)) {
    const clause = match[1]?.trim() ?? '';
    const moduleSource = match[2]?.trim() ?? '';
    if (!clause || !moduleSource) continue;
    let bucket = bySource.get(moduleSource);
    if (!bucket) {
      bucket = { named: new Set<string>(), hasDefaultOrNamespace: false };
      bySource.set(moduleSource, bucket);
    }
    const braceStart = clause.indexOf('{');
    const braceEnd = clause.lastIndexOf('}');
    const outsideBraces =
      braceStart >= 0 && braceEnd > braceStart
        ? `${clause.slice(0, braceStart)}${clause.slice(braceEnd + 1)}`
        : clause;
    if (/[A-Za-z0-9_$*]/u.test(outsideBraces.replace(/,/gu, '').trim())) {
      bucket.hasDefaultOrNamespace = true;
    }
    if (braceStart >= 0 && braceEnd > braceStart) {
      const inner = clause.slice(braceStart + 1, braceEnd);
      for (const rawPart of inner.split(',')) {
        const part = rawPart.trim();
        if (!part || /^type\s+/u.test(part)) continue;
        const imported = part.split(/\s+as\s+/u)[0]?.trim();
        if (imported) bucket.named.add(imported);
      }
    }
  }
  return [...bySource.entries()].map(([moduleSource, info]) => ({
    source: moduleSource,
    named: [...info.named],
    hasDefaultOrNamespace: info.hasDefaultOrNamespace
  }));
}

const registrationBySource = new Map(
  registrations.map((entry) => [entry.module_source, entry])
);

export function prewarmNativeModuleSources(source: string): void {
  for (const item of extractNativeSourceImports(source)) {
    if (item.source === 'react' || item.source === 'react/jsx-runtime')
      continue;
    if (item.source === 'antd' || item.source.startsWith('antd/es/')) {
      void prewarmAntDesignProductDomain()?.catch(() => undefined);
    }
    if (
      item.source === '@ant-design/icons' ||
      item.source.startsWith('@ant-design/icons/')
    ) {
      void prewarmAntDesignIconsDomain()?.catch(() => undefined);
    }
    const entry = registrationBySource.get(item.source);
    if (!entry) continue;
    if (
      entry.loadExports &&
      !item.hasDefaultOrNamespace &&
      item.named.length > 0
    ) {
      void entry.loadExports(item.named).catch(() => undefined);
    } else {
      void entry.load().catch(() => undefined);
    }
  }
}

function registration(
  moduleSource: string,
  exports: readonly string[],
  load: () => Promise<NativeReactFrontendModuleLoadResult>,
  loadExports?: (
    requestedExports: readonly string[]
  ) => Promise<NativeReactFrontendModuleLoadResult>
): NativeReactFrontendModuleRegistration {
  return {
    module_source: moduleSource,
    exports: [...exports],
    load,
    ...(loadExports ? { loadExports } : {})
  };
}

function reactModule(): ModuleNamespace {
  return {
    ...ReactModule,
    default: 'default' in ReactModule ? ReactModule.default : ReactModule
  };
}
