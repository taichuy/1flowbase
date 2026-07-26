export const NATIVE_TRUSTED_BLOCK_RUNTIME = 'native_trusted_block';
export const NATIVE_TRUSTED_BLOCK_PERMISSION = 'ui_block.javascript.native';

export const NATIVE_TRUSTED_BLOCK_ALLOWED_IMPORTS = [
  'react',
  'antd',
  '@1flowbase/ui'
] as const;

type NativeTrustedBlockAllowedImport =
  (typeof NATIVE_TRUSTED_BLOCK_ALLOWED_IMPORTS)[number];

export const allowedImports = new Set<string>(
  NATIVE_TRUSTED_BLOCK_ALLOWED_IMPORTS satisfies readonly NativeTrustedBlockAllowedImport[]
);

export const deniedGlobalIdentifiers = new Set([
  'window',
  'document',
  'globalThis',
  'self',
  'localStorage',
  'sessionStorage',
  'cookie'
]);

export const deniedPortalIdentifiers = new Set([
  'ReactDOM',
  'createPortal',
  'createRoot',
  'hydrateRoot'
]);

export const deniedAntdGlobalIdentifiers = new Set(['message', 'notification']);

export const deniedAntdStaticModalMethods = new Set([
  'confirm',
  'destroyAll',
  'error',
  'info',
  'success',
  'useModal',
  'warning',
  'warn'
]);

export const deniedCallIdentifiers = new Set([
  'require',
  'eval',
  'fetch',
  'sendBeacon'
]);

export const deniedConstructorIdentifiers = new Set([
  'Function',
  'XMLHttpRequest',
  'WebSocket'
]);

export const deniedEscapeIdentifiers = new Set([
  'constructor',
  'prototype',
  '__proto__'
]);

export const deniedCallForwarders = new Set(['call', 'apply', 'bind']);
