export const FRONTSTAGE_NATIVE_REACT_RESOLVED_DECLARATION_SOURCES = [
  'react',
  'react/jsx-runtime',
  'antd'
] as const;

export function isFrontstageNativeReactResolvedDeclarationSource(
  moduleSource: string
): boolean {
  return (
    FRONTSTAGE_NATIVE_REACT_RESOLVED_DECLARATION_SOURCES.includes(
      moduleSource as (typeof FRONTSTAGE_NATIVE_REACT_RESOLVED_DECLARATION_SOURCES)[number]
    ) ||
    moduleSource.startsWith('antd/es/') ||
    isDndKitPackageRoot(moduleSource)
  );
}

function isDndKitPackageRoot(moduleSource: string): boolean {
  return /^@dnd-kit\/[^/]+$/u.test(moduleSource);
}
