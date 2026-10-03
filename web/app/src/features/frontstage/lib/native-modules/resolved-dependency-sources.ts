export const FRONTSTAGE_NATIVE_REACT_RESOLVED_DECLARATION_SOURCES = [
  'react',
  'react/jsx-runtime',
  'antd',
  'antd-img-crop',
  '@ant-design/colors',
  '@ant-design/cssinjs',
  '@ant-design/happy-work-theme',
  'dayjs',
  'lodash/debounce',
  'react-infinite-scroll-component',
  '@rc-component/virtual-list'
] as const;

export function isFrontstageNativeReactResolvedDeclarationSource(
  moduleSource: string
): boolean {
  return (
    FRONTSTAGE_NATIVE_REACT_RESOLVED_DECLARATION_SOURCES.includes(
      moduleSource as (typeof FRONTSTAGE_NATIVE_REACT_RESOLVED_DECLARATION_SOURCES)[number]
    ) ||
    moduleSource.startsWith('antd/es/') ||
    moduleSource.startsWith('antd/locale/') ||
    isDndKitPackageRoot(moduleSource)
  );
}

function isDndKitPackageRoot(moduleSource: string): boolean {
  return /^@dnd-kit\/[^/]+$/u.test(moduleSource);
}
