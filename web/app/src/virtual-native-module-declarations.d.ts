declare module 'virtual:1flowbase-native-module-declarations' {
  import type { BlockSourceExtraLib } from './shared/code-block/extra-lib';

  const declarations: readonly BlockSourceExtraLib[];
  export default declarations;
}

declare module 'virtual:1flowbase-native-antd-es-modules' {
  export const ANTD_ES_MODULE_DEFINITIONS: readonly {
    module_source: string;
    exports: string[];
  }[];
  export function loadAntDesignEsModule(
    moduleSource: string
  ): Promise<Record<string, unknown>>;
}

declare module 'virtual:1flowbase-native-ant-design-icons-modules' {
  export const ANT_DESIGN_ICONS_MODULE_DEFINITIONS: readonly {
    module_source: string;
    exports: string[];
  }[];
  export const ANT_DESIGN_ICONS_PACKAGE: Readonly<{
    package_name: string;
    package_version: string;
    module_count: number;
  }>;
  export function loadAntDesignIconsModule(
    moduleSource: string
  ): Promise<Record<string, unknown>>;
}

declare module 'virtual:1flowbase-native-dnd-kit-modules' {
  export const DND_KIT_MODULE_DEFINITIONS: readonly {
    module_source: string;
    exports: string[];
  }[];
  export const DND_KIT_PACKAGES: readonly {
    package_name: string;
    package_version: string;
  }[];
  export function loadDndKitModule(
    moduleSource: string
  ): Promise<Record<string, unknown>>;
}

declare module 'virtual:1flowbase-native-dayjs-modules' {
  export const DAYJS_MODULE_DEFINITIONS: readonly {
    module_source: string;
    exports: string[];
  }[];
  export const DAYJS_DECLARATION_SOURCES: readonly string[];
  export const DAYJS_PACKAGE: Readonly<{
    package_name: string;
    package_version: string;
    module_count: number;
  }>;
  export function loadDayjsModule(
    moduleSource: string
  ): Promise<Record<string, unknown>>;
}
