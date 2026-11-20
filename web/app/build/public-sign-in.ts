import { readFileSync } from 'node:fs';
import path from 'node:path';
import type { Plugin } from 'vite';
import { compileNativeReactComponent } from '../../packages/page-runtime/src/native-react-compiler/component-compiler';

import { defaultLoginImports } from '../src/features/auth/lib/default-login-imports';

const virtualId = 'virtual:1flowbase-default-login-artifact';

export function publicSignInPlugin(root: string): Plugin {
  const template = path.resolve(
    root,
    '../../api/crates/control-plane/src/auth/public_ui_blocks/password_local.tsx'
  );
  return {
    name: '1flowbase-public-sign-in',
    resolveId(id) {
      if (id === virtualId) return '\0' + id;
    },
    load(id) {
      if (id !== '\0' + virtualId) return;
      this.addWatchFile(template);
      const result = compileNativeReactComponent(
        readFileSync(template, 'utf8'),
        defaultLoginImports
      );
      if (!result.ok) {
        this.warn(
          'Default login is not precompiled; the browser compiler will handle it.'
        );
        return 'export default null;';
      }
      return `export default ${JSON.stringify(result.artifact)};`;
    }
  };
}
