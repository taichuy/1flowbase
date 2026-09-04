import * as ReactModule from 'react';
import * as ReactJsxRuntimeModule from 'react/jsx-runtime';
import {
  ANTD_ROOT_EXPORTS,
  ANTD_ES_MODULE_DEFINITIONS,
  loadAntDesignRootModule,
  loadAntDesignEsModule
} from 'virtual:1flowbase-native-antd-es-modules';
import {
  ANT_DESIGN_ICONS_MODULE_DEFINITIONS,
  loadAntDesignIconsModule
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
} from '@1flowbase/page-runtime';

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
    { NativeBlockMenu },
    { NativeBlockPopover, NativeBlockTooltip }
  ] = await Promise.all([
    loadAntDesignRootModule(),
    import('./native-affix-runtime'),
    import('./native-anchor-runtime'),
    import('./native-dropdown-runtime'),
    import('./native-message-runtime'),
    import('./menu/native-menu-runtime'),
    import('./overlay/native-tooltip-popover-runtime')
  ]);
  return {
    ...antdModule,
    Affix: NativeBlockAffix,
    Anchor: NativeBlockAnchor,
    Dropdown: NativeBlockDropdown,
    Menu: NativeBlockMenu,
    Popover: NativeBlockPopover,
    Tooltip: NativeBlockTooltip,
    message: NativeBlockMessage
  };
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
  registration('antd', ANTD_ROOT_EXPORTS, async () => ({
    module: await loadNativeAntDesignModule()
  })),
  ...ANTD_ES_MODULE_DEFINITIONS.map(({ module_source, exports }) =>
    registration(module_source, exports, async () => ({
      module: await loadAntDesignEsModule(module_source)
    }))
  ),
  ...DND_KIT_MODULE_DEFINITIONS.map(({ module_source, exports }) =>
    registration(module_source, exports, async () => ({
      module: await loadDndKitModule(module_source)
    }))
  ),
  registration('@ant-design/colors', ANT_DESIGN_COLORS_EXPORTS, async () => ({
    module: await loadAntDesignColorsModule()
  })),
  registration('antd-img-crop', ['default'], loadAntdImgCropModule),
  ...DAYJS_MODULE_DEFINITIONS.map(({ module_source, exports }) =>
    registration(module_source, exports, async () => ({
      module: await loadDayjsModule(module_source)
    }))
  ),
  registration('lodash/debounce', ['default'], async () => {
    const debounceModule = await import('lodash/debounce');
    return { module: { default: debounceModule.default } };
  }),
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
  registration('@1flowbase/block-sdk', ['blockSdkVersion'], async () => ({
    module: await import('@1flowbase/block-sdk')
  })),
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
    registration(module_source, exports, async () => ({
      module: await loadAntDesignIconsModule(module_source)
    }))
  ),
  registration('@1flowbase/charts', ['EChart'], async () => ({
    module: await import('@1flowbase/charts')
  })),
  registration('@1flowbase/rich-text', ['VditorEditor'], async () => {
    const [module, vditorStyle, richTextStyle] = await Promise.all([
      import('@1flowbase/rich-text'),
      import('vditor/dist/index.css?inline'),
      import('@1flowbase/rich-text/styles.css?inline')
    ]);
    return {
      module,
      styles: [{ css: vditorStyle.default }, { css: richTextStyle.default }]
    };
  }),
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
  overrides: NativeTrustedBlockInjectedModuleMap = {}
): NativeReactModuleRegistry {
  // Each artifact evaluation owns its module flights and generated style assets.
  return createNativeReactModuleRegistry(
    registrations.map((entry) => {
      const override = overrides[entry.module_source];
      if (!override) return entry;
      return {
        ...entry,
        async load(): Promise<NativeReactFrontendModuleLoadResult> {
          const loaded = await entry.load();
          return { ...loaded, module: { ...loaded.module, ...override } };
        }
      };
    })
  );
}

function registration(
  moduleSource: string,
  exports: readonly string[],
  load: () => Promise<NativeReactFrontendModuleLoadResult>
): NativeReactFrontendModuleRegistration {
  return { module_source: moduleSource, exports: [...exports], load };
}

function reactModule(): ModuleNamespace {
  return {
    ...ReactModule,
    default: 'default' in ReactModule ? ReactModule.default : ReactModule
  };
}
