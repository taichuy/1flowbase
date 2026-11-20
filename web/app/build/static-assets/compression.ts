import { promisify } from 'node:util';
import { gzip } from 'node:zlib';
import type { Plugin } from 'vite';

const compress = promisify(gzip);

/** Build text representations once; Nginx negotiates gzip without changing URLs. */
export function staticAssetCompressionPlugin(): Plugin {
  return {
    name: '1flowbase-static-asset-compression',
    apply: 'build',
    enforce: 'post',
    generateBundle: {
      order: 'post',
      async handler(_options, bundle) {
        // One resource at a time: no extra worker pool or deployment assumptions.
        // Vite's CSS removal/import rewriting has already completed in normal hooks.
        for (const file of Object.values(bundle)) {
          if (!/\.(?:js|css|json)$/u.test(file.fileName)) continue;
          const source = file.type === 'chunk' ? file.code : file.source;
          this.emitFile({
            type: 'asset',
            fileName: `${file.fileName}.gz`,
            source: await compress(source)
          });
        }
      }
    }
  };
}
