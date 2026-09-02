import antDesignColorsPackageJson from '@ant-design/colors/package.json';
import antdPackageJson from 'antd/package.json';
import antdStylePackageJson from 'antd-style/package.json';
import appPackageJson from '../../../../package.json';
import { ANT_DESIGN_ICONS_PACKAGE } from 'virtual:1flowbase-native-ant-design-icons-modules';
import { DAYJS_PACKAGE } from 'virtual:1flowbase-native-dayjs-modules';
import { DND_KIT_PACKAGES } from 'virtual:1flowbase-native-dnd-kit-modules';
import {
  NATIVE_TRUSTED_BLOCK_ALLOWED_IMPORTS,
  NATIVE_TRUSTED_BLOCK_PERMISSION,
  NATIVE_TRUSTED_BLOCK_RUNTIME
} from '@1flowbase/page-runtime';
import reactPackageJson from 'react/package.json';
import uiPackageJson from '../../../../../packages/ui/package.json';

export const FRONTSTAGE_NATIVE_TRUSTED_BLOCK_COMPATIBILITY_CONTRACT_VERSION =
  '1.6.0';

type FrontstageNativeTrustedBlockAllowedImport =
  (typeof NATIVE_TRUSTED_BLOCK_ALLOWED_IMPORTS)[number];

export interface FrontstageNativeTrustedBlockRuntimeCompatibilityModule<
  TImportSource extends string = FrontstageNativeTrustedBlockAllowedImport
> {
  importSource: TImportSource;
  hostDependencyRange: string;
  packageVersion: string;
}

export interface FrontstageNativeTrustedBlockRuntimeCompatibilityDomainPackage {
  packageName: string;
  hostDependencyRange: string | null;
  packageVersion: string;
}

export interface FrontstageNativeTrustedBlockRuntimeCompatibilityManifest {
  runtime: typeof NATIVE_TRUSTED_BLOCK_RUNTIME;
  contractVersion: typeof FRONTSTAGE_NATIVE_TRUSTED_BLOCK_COMPATIBILITY_CONTRACT_VERSION;
  requiredPermission: typeof NATIVE_TRUSTED_BLOCK_PERMISSION;
  allowedImports: FrontstageNativeTrustedBlockAllowedImport[];
  host: {
    packageName: string;
    appVersion: string;
  };
  modules: Record<
    FrontstageNativeTrustedBlockAllowedImport,
    FrontstageNativeTrustedBlockRuntimeCompatibilityModule
  >;
  lazyModules: {
    '@ant-design/colors': FrontstageNativeTrustedBlockRuntimeCompatibilityModule<'@ant-design/colors'>;
  };
  moduleDomains: {
    '@ant-design/icons': {
      packageName: string;
      hostDependencyRange: string;
      packageVersion: string;
      moduleCount: number;
    };
    '@dnd-kit': {
      packages: FrontstageNativeTrustedBlockRuntimeCompatibilityDomainPackage[];
    };
    dayjs: {
      packageName: string;
      hostDependencyRange: string;
      packageVersion: string;
      moduleCount: number;
    };
  };
}

export function getFrontstageNativeTrustedBlockRuntimeCompatibility(): FrontstageNativeTrustedBlockRuntimeCompatibilityManifest {
  const hostDependencies = appPackageJson.dependencies as Record<
    string,
    string | undefined
  >;
  return {
    runtime: NATIVE_TRUSTED_BLOCK_RUNTIME,
    contractVersion:
      FRONTSTAGE_NATIVE_TRUSTED_BLOCK_COMPATIBILITY_CONTRACT_VERSION,
    requiredPermission: NATIVE_TRUSTED_BLOCK_PERMISSION,
    allowedImports: [...NATIVE_TRUSTED_BLOCK_ALLOWED_IMPORTS],
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
        hostDependencyRange: appPackageJson.dependencies['@ant-design/colors'],
        packageVersion: antDesignColorsPackageJson.version
      }
    },
    moduleDomains: {
      '@ant-design/icons': {
        packageName: ANT_DESIGN_ICONS_PACKAGE.package_name,
        hostDependencyRange: appPackageJson.dependencies['@ant-design/icons'],
        packageVersion: ANT_DESIGN_ICONS_PACKAGE.package_version,
        moduleCount: ANT_DESIGN_ICONS_PACKAGE.module_count
      },
      '@dnd-kit': {
        packages: DND_KIT_PACKAGES.map(({ package_name, package_version }) => ({
          packageName: package_name,
          hostDependencyRange: hostDependencies[package_name] ?? null,
          packageVersion: package_version
        }))
      },
      dayjs: {
        packageName: DAYJS_PACKAGE.package_name,
        hostDependencyRange: appPackageJson.dependencies.dayjs,
        packageVersion: DAYJS_PACKAGE.package_version,
        moduleCount: DAYJS_PACKAGE.module_count
      }
    }
  };
}
