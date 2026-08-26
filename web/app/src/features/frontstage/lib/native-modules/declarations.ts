import type { BlockSourceExtraLib } from '../../../../shared/code-block/extra-lib';

const REACT_DECLARATIONS = `declare module 'react' {
  export namespace React {
    export type ReactNode = unknown;
    export type CSSProperties = Record<string, string | number | undefined>;
    export type ComponentType<P = {}> = (props: P) => ReactNode;
    export type ElementType = string | ComponentType<any>;
    export type FC<P = {}> = (props: P) => ReactNode;
    export interface HTMLAttributes<T> { className?: string; style?: CSSProperties; children?: ReactNode; [key: string]: unknown; }
    export const Fragment: ComponentType<{ children?: ReactNode }>;
    export function createElement(type: ElementType, props?: Record<string, unknown> | null, ...children: ReactNode[]): ReactNode;
    export function useState<T>(initial: T | (() => T)): [T, (value: T | ((previous: T) => T)) => void];
    export function useEffect(effect: () => void | (() => void), dependencies?: readonly unknown[]): void;
    export function useMemo<T>(factory: () => T, dependencies: readonly unknown[]): T;
    export function useCallback<T extends (...args: any[]) => any>(callback: T, dependencies: readonly unknown[]): T;
    export function useRef<T>(initial: T): { current: T };
    export function useSyncExternalStore<T>(subscribe: (listener: () => void) => () => void, getSnapshot: () => T): T;
  }
  export type ReactNode = React.ReactNode;
  export type CSSProperties = React.CSSProperties;
  export type ComponentType<P = {}> = React.ComponentType<P>;
  export type ElementType = React.ElementType;
  export type FC<P = {}> = React.FC<P>;
  export interface HTMLAttributes<T> extends React.HTMLAttributes<T> {}
  export const Fragment: typeof React.Fragment;
  export const createElement: typeof React.createElement;
  export const useState: typeof React.useState;
  export const useEffect: typeof React.useEffect;
  export const useMemo: typeof React.useMemo;
  export const useCallback: typeof React.useCallback;
  export const useRef: typeof React.useRef;
  export const useSyncExternalStore: typeof React.useSyncExternalStore;
  export function React(): React.ReactNode;
  export default React;
}
`;

const ANTD_DECLARATIONS = `declare module 'antd' {
  export type ColumnRender<RecordType = any> = (value: unknown, record: RecordType, index: number) => unknown;
  export interface ColumnType<RecordType = any> {
    title?: unknown;
    dataIndex?: string | readonly string[];
    key?: string;
    render?: ColumnRender<RecordType>;
    [property: string]: unknown;
  }
  export interface ColumnGroupType<RecordType = any> extends ColumnType<RecordType> {
    children: ColumnsType<RecordType>;
  }
  export type ColumnsType<RecordType = any> = Array<ColumnGroupType<RecordType> | ColumnType<RecordType>>;
  export interface TableProps<RecordType = any> {
    columns?: ColumnsType<RecordType>;
    dataSource?: readonly RecordType[];
    [property: string]: unknown;
  }
}
`;

const ANTD_STYLE_DECLARATIONS = `declare module 'antd-style' {
  export type ResponsiveKey = 'xs' | 'sm' | 'md' | 'lg' | 'xl' | 'xxl';
  export function useResponsive(): Partial<Record<ResponsiveKey, boolean>>;
}
`;

const BLOCK_SDK_DECLARATIONS = `declare module '@1flowbase/block-sdk' {
  export type BlockContextRecord = Record<string, unknown>;
  export interface BlockContextIdentity { readonly id: string; readonly displayName?: string; }
  export interface BlockContextEntity { readonly id: string; readonly name?: string; }
  export interface BlockContextPage { readonly id: string; readonly route: string; readonly title?: string; }
  export interface BlockExternalAssetHandle { dispose(): void; }
  export interface BlockContextAssets {
    importModule<TModule = Record<string, unknown>>(url: string): Promise<TModule>;
    loadStyle(url: string): Promise<BlockExternalAssetHandle>;
    loadScript(url: string): Promise<BlockExternalAssetHandle>;
    loadSvgSprite(url: string): Promise<BlockExternalAssetHandle>;
  }
  export interface BlockContext {
    readonly root: ShadowRoot;
    readonly assets: BlockContextAssets;
    readonly currentUser: BlockContextIdentity | null;
    readonly workspace: BlockContextEntity;
    readonly application: BlockContextEntity | null;
    readonly page: BlockContextPage;
    readonly inputs: Readonly<BlockContextRecord>;
    readonly outputs: { publish(values: BlockContextRecord): { ok: boolean; stale: boolean; error?: string } };
    readonly params: Readonly<BlockContextRecord>;
    readonly props: Readonly<BlockContextRecord>;
    readonly state: Readonly<BlockContextRecord>;
    patch(next: BlockContextRecord): void | Promise<void>;
    readonly api: {
      get<TResponse = unknown>(path: string, request?: BlockContextRecord): Promise<TResponse>;
      post<TResponse = unknown>(path: string, request?: BlockContextRecord): Promise<TResponse>;
      put<TResponse = unknown>(path: string, request?: BlockContextRecord): Promise<TResponse>;
      patch<TResponse = unknown>(path: string, request?: BlockContextRecord): Promise<TResponse>;
      delete<TResponse = unknown>(path: string, request?: BlockContextRecord): Promise<TResponse>;
    };
    readonly events: { emit(name: string, payload?: BlockContextRecord): void };
    readonly theme: { readonly mode: 'light' | 'dark'; readonly tokens: Readonly<BlockContextRecord> };
    readonly ui: { readonly locale?: string; readonly density?: 'compact' | 'comfortable' };
  }
  export interface BlockComponentProps { readonly ctx: BlockContext; }
  export const blockSdkVersion: string;
}
`;

export function createFrontendModuleExtraLib(
  moduleSource: string,
  exports: readonly string[]
): BlockSourceExtraLib {
  return {
    source: moduleSource,
    filePath: `file:///node_modules/${moduleSource}/index.d.ts`,
    content:
      moduleSource === 'react'
        ? REACT_DECLARATIONS
        : moduleSource === '@1flowbase/block-sdk'
          ? BLOCK_SDK_DECLARATIONS
          : moduleSource === 'antd'
            ? `${ANTD_DECLARATIONS}${createGenericModuleDeclarations(moduleSource, exports)}`
            : moduleSource === 'antd-style'
              ? `${ANTD_STYLE_DECLARATIONS}${createGenericModuleDeclarations(
                  moduleSource,
                  exports.filter((exportName) => exportName !== 'useResponsive')
                )}`
              : createGenericModuleDeclarations(moduleSource, exports)
  };
}

function createGenericModuleDeclarations(
  moduleSource: string,
  exports: readonly string[]
): string {
  const namedExports = exports
    .filter((exportName) => exportName !== 'default')
    .map((exportName) => `  export const ${exportName}: any;`)
    .join('\n');
  const defaultExport = exports.includes('default')
    ? '\n  const defaultExport: any;\n  export default defaultExport;'
    : '';
  return `declare module '${moduleSource}' {\n${namedExports}${defaultExport}\n}\n`;
}
