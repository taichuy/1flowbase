import path from 'node:path';
import type { Plugin } from 'vite';
import { motionDocument } from './sign-in-motion-document';

/** Static reachability only: a lazy route, worker or form is not a visual dependency. */
export function signInPreloads(
  chunks: ReadonlyMap<string, { imports: readonly string[] }>,
  roots: readonly string[]
): string[] {
  const seen = new Set<string>();
  const visit = (file: string) => {
    if (seen.has(file) || !chunks.has(file)) return;
    seen.add(file);
    for (const dep of chunks.get(file)!.imports) visit(dep);
  };
  roots.forEach(visit);
  return [...seen];
}

// Preload, but do not apply route styles until Vite imports them. This keeps
// the authenticated path and the existing CSS cascade unchanged.
export function signInStylePreloads(
  chunks: ReadonlyMap<
    string,
    { viteMetadata?: { importedCss: ReadonlySet<string> } }
  >,
  files: readonly string[]
): string[] {
  const styles = new Set<string>();
  for (const file of files) {
    for (const css of chunks.get(file)?.viteMetadata?.importedCss ?? [])
      styles.add(css);
  }
  return [...styles];
}

// After native motion starts, prepare the application graph without waking the
// lazy form, the workspace or unrelated public routes.
export const signInPreloadRoots = [
  '/src/bootstrap.ts',
  '/src/app/AnonymousAppRuntime.tsx',
  '/src/app/ApplicationRuntimeBootstrap.tsx',
  '/src/main.tsx'
] as const;

export function signInResourceHintsPlugin(): Plugin {
  let base: string;
  let root: string;
  let building = false;
  const prewarmId = 'virtual:1flowbase-authenticated-preload';
  return {
    name: '1flowbase-sign-in-resource-hints',
    enforce: 'post',
    configResolved(config) {
      base = config.base;
      root = config.root;
      building = config.command === 'build';
    },
    resolveId(id) {
      if (id === prewarmId) return '\0' + prewarmId;
    },
    load(id) {
      if (id !== '\0' + prewarmId) return;
      if (!building)
        return `export default ${JSON.stringify(`${base}src/app/bootstrap/AuthenticatedStartup.tsx`)};`;
      const reference = this.emitFile({
        type: 'chunk',
        id: path.join(root, 'src/app/bootstrap/AuthenticatedStartup.tsx')
      });
      // A URL, not an import: idle modulepreload fetches/parses without evaluating
      // authenticated providers or initializing i18next in the anonymous state.
      return `export default import.meta.ROLLUP_FILE_URL_${reference};`;
    },
    generateBundle: {
      order: 'post',
      async handler(_, bundle) {
        const doc = bundle['index.html'];
        if (doc?.type !== 'asset' || typeof doc.source !== 'string') return;
        const html = doc.source;
        const chunks = new Map(
          Object.values(bundle).flatMap((c) =>
            c.type === 'chunk' ? [[c.fileName, c] as const] : []
          )
        );
        const entries = signInPreloadRoots.map((root) => {
          for (const [file, chunk] of chunks)
            if (
              Object.keys(chunk.modules).some((m) =>
                m.replaceAll('\\', '/').endsWith(root)
              )
            )
              return file;
          throw new Error(`Missing sign-in preload root: ${root}`);
        });
        const href = (file: string) => `${base}${file}`;
        // The guarded index has no document-owned hero. Discover its fallback
        // graph in HTML, not after React mounts. The motion document deliberately
        // uses the original HTML below: its live artwork needs no second copy.
        const fallbackRoots = [...chunks]
          .filter(([, chunk]) =>
            Object.keys(chunk.modules).some((id) =>
              id.replaceAll('\\', '/').endsWith('/hero/SignInFallbackHero.tsx')
            )
          )
          .map(([file]) => file);
        if (fallbackRoots.length) {
          const fallback = signInPreloads(chunks, fallbackRoots);
          const resources = [
            ...fallback.map((file) => ({
              rel: 'modulepreload',
              href: href(file)
            })),
            ...signInStylePreloads(chunks, fallback).map((file) => ({
              rel: 'preload',
              href: href(file),
              as: 'style'
            }))
          ];
          doc.source = html.replace(
            '</body>',
            `<script>if(/^\\/sign-in\\/?$/.test(location.pathname)){for(const r of ${JSON.stringify(resources)}){if(Array.from(document.querySelectorAll('link')).some(l=>l.href===new URL(r.href,document.baseURI).href))continue;const l=document.createElement('link');Object.assign(l,r);l.crossOrigin='';document.head.append(l)}}</script></body>`
          );
        }

        const visual = signInPreloads(chunks, entries);
        const hints: string[] = [];
        for (const file of signInStylePreloads(chunks, visual)) {
          const url = href(file);
          if (!html.includes(`href="${url}"`)) {
            hints.push(
              `<link rel="preload" as="style" crossorigin href="${url}">`
            );
          }
        }
        for (const file of visual) {
          const url = href(file);
          if (!html.includes(`href="${url}"`)) {
            hints.push(`<link rel="modulepreload" crossorigin href="${url}">`);
          }
        }
        this.emitFile({
          type: 'asset',
          fileName: 'sign-in-preload.html',
          source: await motionDocument(html, root, hints)
        });
      }
    }
  };
}
