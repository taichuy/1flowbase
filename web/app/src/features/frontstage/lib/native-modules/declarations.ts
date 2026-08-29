import type { BlockSourceExtraLib } from '../../../../shared/code-block/extra-lib';

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
    readonly ui: {
      readonly locale?: string;
      readonly density?: 'compact' | 'comfortable';
      readonly sizing?: {
        readonly available: { readonly width: number; readonly height: number };
        reportIntrinsicSize(size: { readonly height: number }): void;
      };
    };
  }
  export interface BlockComponentProps { readonly ctx: BlockContext; }
  export const blockSdkVersion: string;
}
`;

export function createFrontendModuleExtraLib(
  moduleSource: string,
  exports: readonly string[]
): BlockSourceExtraLib {
  const dndKitInternalModule = isDndKitInternalModule(moduleSource);
  const dayjsRuntimeModule = moduleSource.startsWith('dayjs/');
  return {
    source: moduleSource,
    filePath: dndKitInternalModule
      ? `file:///node_modules/.1flowbase-native/dnd-kit/${encodeURIComponent(moduleSource)}.d.ts`
      : dayjsRuntimeModule
        ? `file:///node_modules/.1flowbase-native/dayjs/${encodeURIComponent(moduleSource)}.d.ts`
        : `file:///node_modules/${moduleSource}/index.d.ts`,
    content: dndKitInternalModule
      ? createDndKitInternalModuleDeclaration(moduleSource)
      : dayjsRuntimeModule
        ? createDayjsRuntimeModuleDeclaration(moduleSource)
        : moduleSource === '@1flowbase/block-sdk'
          ? BLOCK_SDK_DECLARATIONS
          : moduleSource === 'antd-style'
            ? `${ANTD_STYLE_DECLARATIONS}${createGenericModuleDeclarations(
                moduleSource,
                exports.filter((exportName) => exportName !== 'useResponsive')
              )}`
            : createGenericModuleDeclarations(moduleSource, exports)
  };
}

function createDayjsRuntimeModuleDeclaration(moduleSource: string): string {
  return `declare module '${moduleSource}' {\n  const defaultExport: any;\n  export default defaultExport;\n}\n`;
}

function isDndKitInternalModule(moduleSource: string): boolean {
  return /^@dnd-kit\/[^/]+\/.+/u.test(moduleSource);
}

function createDndKitInternalModuleDeclaration(moduleSource: string): string {
  const packageRoot = moduleSource.split('/').slice(0, 2).join('/');
  return `declare module '${moduleSource}' {\n  export * from '${packageRoot}';\n}\n`;
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
