import type { NativeReactResolvedModuleAsset } from '@1flowbase/page-runtime/browser';

// These host-owned namespaces are plain React/Ant Design primitives. Their
// loaders do not consume antd-style hooks. This is a proof set, not a module
// allowlist: every unknown/fetched namespace keeps the full provider contract.
const styleIndependentHostModules = new Set([
  'react',
  'react/jsx-runtime',
  'antd',
  '@ant-design/icons'
]);

export function needsAntdStyleSurface(
  moduleSources: readonly string[] | undefined,
  moduleAssets: readonly NativeReactResolvedModuleAsset[]
): boolean {
  // An absent/empty inventory can be an indirect host-supplied component.
  if (!moduleSources?.length) return true;
  return (
    moduleSources.some((source) => !styleIndependentHostModules.has(source)) ||
    moduleAssets.some(
      (asset) => !styleIndependentHostModules.has(asset.module_source)
    )
  );
}
