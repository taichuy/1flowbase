import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { transformWithOxc, type Plugin } from 'vite';
import { compileNativeReactComponent } from '../../packages/page-runtime/src/native-react-compiler/component-compiler';

import { defaultLoginImports } from '../src/features/auth/lib/default-login-imports';

const virtualId = 'virtual:1flowbase-default-login-artifact';
const passwordId = 'virtual:1flowbase-default-password';

export function publicSignInPlugin(root: string): Plugin {
  const template = path.resolve(
    root,
    '../../api/crates/control-plane/src/auth/public_ui_blocks/password_local.tsx'
  );
  return {
    name: '1flowbase-public-sign-in',
    resolveId(id, importer) {
      if (id === virtualId || id === passwordId) return '\0' + id;
      // This alias belongs ONLY to the build-time official template. Custom
      // providers and the legacy artifact registry still receive real Antd.
      if (
        importer === '\0' + passwordId &&
        (id === 'antd' || id === '@ant-design/icons')
      ) {
        return path.resolve(
          root,
          'src/features/auth/components/default-password/primitives.tsx'
        );
      }
    },
    async load(id) {
      if (id === '\0' + passwordId) {
        this.addWatchFile(template);
        const source = readFileSync(template, 'utf8');
        // Compile the backend's TSX verbatim: no separate form/business logic,
        // browser evaluator, serialized program or sandbox capability runtime.
        const compiled = await transformWithOxc(source, template, {
          lang: 'tsx',
          jsx: { runtime: 'automatic' },
          sourcemap: true
        });
        const sha256 = createHash('sha256').update(source).digest('hex');
        // Reuse the existing pure SHA implementation, NOT the runtime barrel.
        const hashModule = path.resolve(
          root,
          '../packages/page-runtime/src/sha256.ts'
        );
        return {
          ...compiled,
          code: `${compiled.code}\nexport const sourceSha256 = ${JSON.stringify(sha256)};\nexport { sha256Text } from ${JSON.stringify(hashModule)};`
        };
      }
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
