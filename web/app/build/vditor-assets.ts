import { readFileSync, readdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import type { Plugin } from 'vite';

import {
  VDITOR_ASSET_BASE,
  VDITOR_VERSION
} from '../../packages/rich-text/src/asset-contract';

import { VDITOR_FRAME_DOCUMENT } from '../../packages/rich-text/src/frame-document';

const PREFIX = `${VDITOR_ASSET_BASE}/`;
const SCRIPT_DIRECTORIES = new Set([
  'lute',
  'icons',
  'i18n',
  'highlight.js',
  'katex',
  'mermaid'
]);
const MIME: Record<string, string> = {
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.gif': 'image/gif',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.ttf': 'font/ttf'
};

export function vditorAssetsPlugin(): Plugin {
  const require = createRequire(import.meta.url);
  const manifestPath = require.resolve('vditor/package.json');
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8')) as {
    version: string;
  };
  if (manifest.version !== VDITOR_VERSION)
    throw new Error(
      'Update the Vditor frame asset contract with the installed version.'
    );
  const root = path.dirname(manifestPath);
  const files = new Map<string, string>();
  const visit = (directory: string) => {
    for (const entry of readdirSync(path.join(root, directory), {
      withFileTypes: true
    })) {
      const relative = `${directory}/${entry.name}`;
      if (entry.isDirectory()) {
        if (directory === 'dist/js' && !SCRIPT_DIRECTORIES.has(entry.name))
          continue;
        visit(relative);
      } else if (MIME[path.extname(relative)])
        files.set(relative, path.join(root, relative));
    }
  };
  visit('dist');
  return {
    name: 'oneflow-vditor-assets',
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        const pathname = new URL(request.url ?? '/', 'http://localhost')
          .pathname;
        if (!pathname.startsWith(PREFIX)) return next();
        if (pathname === `${PREFIX}frame.html`) {
          response.setHeader('Content-Type', 'text/html; charset=utf-8');
          response.end(VDITOR_FRAME_DOCUMENT);
          return;
        }
        const file = files.get(pathname.slice(PREFIX.length));
        if (!file) {
          response.statusCode = 404;
          response.end();
          return;
        }
        response.setHeader('Content-Type', MIME[path.extname(file)]);
        response.setHeader('X-Content-Type-Options', 'nosniff');
        response.setHeader(
          'Cache-Control',
          'public, max-age=31536000, immutable'
        );
        response.end(readFileSync(file));
      });
    },
    generateBundle() {
      this.emitFile({
        type: 'asset',
        fileName: `${PREFIX.slice(1)}frame.html`,
        source: VDITOR_FRAME_DOCUMENT
      });
      for (const [relative, file] of files)
        this.emitFile({
          type: 'asset',
          fileName: `${PREFIX.slice(1)}${relative}`,
          source: readFileSync(file)
        });
    }
  };
}
