declare module 'virtual:1flowbase-default-password' {
  import type { ComponentType } from 'react';

  export const sourceSha256: string;
  export function sha256Text(value: string): string;
  const PasswordLocalAuth: ComponentType<{
    ctx: import('./context').DefaultPasswordContext;
  }>;
  export default PasswordLocalAuth;
}
