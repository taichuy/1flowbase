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
  return {
    name: '1flowbase-sign-in-resource-hints',
    apply: 'build',
    enforce: 'post',
    configResolved(config) {
      base = config.base;
      root = config.root;
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
