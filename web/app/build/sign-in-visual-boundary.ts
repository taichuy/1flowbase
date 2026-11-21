import { brotliCompressSync, constants, gzipSync } from 'node:zlib';
import { signInPreloads, signInPreloadRoots } from './sign-in-resource-hints';

// Check emitted chunks, not source-level lazy() calls. A shared vendor can pull
// a dynamic feature back onto the visual path. File names are content-hashed.
type Chunk = {
  type: 'chunk';
  modules: Record<string, unknown>;
  imports: readonly string[];
  code?: string;
  viteMetadata?: { importedCss: ReadonlySet<string> };
};
type Asset = { type: 'asset'; source?: string | Uint8Array };
type Bundle = Record<string, Chunk | Asset>;

const page = '/features/auth/pages/SignInPage.tsx';
const hero = '/features/auth/components/HeroAnimation.tsx';
const deferred = [
  '/features/auth/components/PublicAuthBlock.tsx',
  '/features/auth/pages/mcp-authorization/McpAuthorizePage.tsx'
];

export function assertSignInVisualBoundary(bundle: Bundle): void {
  const owners = new Map<string, string[]>();
  for (const [file, chunk] of Object.entries(bundle)) {
    if (chunk.type === 'chunk') {
      owners.set(
        file,
        Object.keys(chunk.modules).map((id) => id.replaceAll('\\', '/'))
      );
    }
  }
  const rootsFor = (suffixes: readonly string[]) => {
    const roots: string[] = [];
    for (const [file, ids] of owners) {
      if (ids.some((id) => suffixes.some((s) => id.endsWith(s))))
        roots.push(file);
    }
    return roots;
  };
  const pageRoots = rootsFor([page]);
  if (!pageRoots.length) return; // Worker-only outputs do not contain a page.
  const chunks = new Map(
    Object.entries(bundle).flatMap(([name, value]) =>
      value.type === 'chunk' ? [[name, value] as const] : []
    )
  );
  const styleRoots = rootsFor(['/node_modules/antd-style/es/index.js']);
  for (const file of signInPreloads(chunks, styleRoots)) {
    for (const id of owners.get(file)!) {
      if (/\/antd\/es\/(?:table|tour|list)\//u.test(id))
        throw new Error(
          `Sign-in style provider eagerly includes ${id} in ${file}`
        );
    }
  }
  const pageClosure = signInPreloads(chunks, pageRoots);
  if (
    !pageClosure.some((file) =>
      owners.get(file)!.some((id) => id.endsWith(hero))
    )
  )
    throw new Error(
      'Sign-in visual path is missing the original HeroAnimation'
    );
  // Every graph the sign-in HTML speculatively fetches contributes to the cold
  // visit, even if the page component itself doesn't import it.
  const visual = signInPreloads(chunks, [
    ...pageRoots,
    ...rootsFor(signInPreloadRoots)
  ]);
  for (const file of visual) {
    for (const id of owners.get(file)!) {
      if (
        deferred.some((owner) => id.endsWith(owner)) ||
        /\/node_modules\/(?:antd|@ant-design\/cssinjs|@tanstack\/(?:react-router|router-core|react-query|query-core)|i18next|react-i18next|antd-style)\//u.test(
          id
        )
      )
        throw new Error(
          `Sign-in visual path eagerly includes ${id} in ${file}`
        );
    }
  }
}

// Explicit user-approved transfer budgets, in bytes. They are build acceptance
// constraints, never runtime business/concurrency limits.
export const SIGN_IN_BUDGET = {
  discoveryDepth: 1,
  files: 4,
  raw: 240_000,
  gzip: 85_000,
  htmlRaw: 20_000,
  htmlGzip: 7_000
} as const;

function representations(bytes: Uint8Array) {
  return {
    raw: bytes.byteLength,
    gzip: gzipSync(bytes, { level: 9 }).byteLength,
    br: brotliCompressSync(bytes, {
      params: {
        [constants.BROTLI_PARAM_QUALITY]: 11,
        [constants.BROTLI_PARAM_MODE]: constants.BROTLI_MODE_TEXT
      }
    }).byteLength
  };
}

function bytesOf(value: Chunk | Asset): Uint8Array {
  const source = value.type === 'chunk' ? value.code : value.source;
  if (source === undefined)
    throw new Error('Sign-in budget requires final emitted bytes');
  return typeof source === 'string' ? Buffer.from(source) : source;
}

/** URLs in our generated HTML, including the inline-activated preload template.
 * Module edges always come from the compiler; never parse JS text for imports. */
function documentResources(html: string, bundle: Bundle): Set<string> {
  const result = new Set<string>();
  for (const match of html.matchAll(/<(link|script)\b[^>]*>/giu)) {
    const attributes = new Map<string, string>();
    for (const attr of match[0].matchAll(
      /\b([\w-]+)\s*=\s*(?:"([^"]*)"|'([^']*)')/gu
    ))
      attributes.set(attr[1].toLowerCase(), attr[2] ?? attr[3]);
    const rel = attributes.get('rel');
    const requested =
      match[1].toLowerCase() === 'script'
        ? attributes.get('type') === 'module'
        : rel === 'modulepreload' ||
          rel === 'stylesheet' ||
          (rel === 'preload' && attributes.get('as') === 'style');
    if (!requested) continue;
    const href = attributes.get('href') ?? attributes.get('src');
    if (!href) continue;
    const pathname = new URL(href, 'https://build.invalid').pathname;
    const file = Object.keys(bundle).find((key) =>
      pathname.endsWith('/' + key)
    );
    if (!file)
      throw new Error(`Sign-in HTML requests an unaccounted resource: ${href}`);
    result.add(file);
  }
  return result;
}

export function assertSignInBudget(bundle: Bundle) {
  const document = bundle['sign-in-preload.html'];
  if (!document || document.type !== 'asset') return null; // Worker/fixture output.
  const html = new TextDecoder().decode(bytesOf(document));
  const chunks = new Map(
    Object.entries(bundle).flatMap(([name, value]) =>
      value.type === 'chunk' ? [[name, value] as const] : []
    )
  );
  const roots = [...chunks]
    .filter(([, chunk]) =>
      Object.keys(chunk.modules).some((id) =>
        [page, ...signInPreloadRoots].some((root) =>
          id.replaceAll('\\', '/').endsWith(root)
        )
      )
    )
    .map(([file]) => file);
  if (!roots.length)
    throw new Error('Sign-in budget has no anonymous entry roots');
  const closure = signInPreloads(chunks, roots);
  const discovered = documentResources(html, bundle);
  const required = new Set(closure);
  for (const file of closure) {
    for (const css of chunks.get(file)?.viteMetadata?.importedCss ?? [])
      required.add(css);
  }
  // Anything speculatively requested is paid by the cold visit, too.
  const resources = new Set([...required, ...discovered]);
  const late = [...required].filter((file) => !discovered.has(file));
  const files = [...resources].sort().map((file) => {
    if (!bundle[file]) throw new Error(`Missing sign-in resource: ${file}`);
    return { file, ...representations(bytesOf(bundle[file])) };
  });
  const totals = files.reduce(
    (sum, file) => ({
      raw: sum.raw + file.raw,
      gzip: sum.gzip + file.gzip,
      br: sum.br + file.br
    }),
    { raw: 0, gzip: 0, br: 0 }
  );
  const summary = {
    version: 1,
    semantics:
      'All default-sign-in static JS/CSS and HTML speculative roots; React included. Network discovery wave counts HTML-linked dependencies, not CPU execution depth.',
    discoveryDepth: late.length ? 2 : 1,
    lateDiscovery: late,
    files,
    totals,
    html: representations(bytesOf(document)),
    budgets: SIGN_IN_BUDGET
  };
  const failures = [
    summary.discoveryDepth > SIGN_IN_BUDGET.discoveryDepth && 'discovery depth',
    files.length > SIGN_IN_BUDGET.files && 'JS/CSS count',
    totals.raw > SIGN_IN_BUDGET.raw && 'JS/CSS raw bytes',
    totals.gzip > SIGN_IN_BUDGET.gzip && 'JS/CSS gzip bytes',
    summary.html.raw > SIGN_IN_BUDGET.htmlRaw && 'HTML raw bytes',
    summary.html.gzip > SIGN_IN_BUDGET.htmlGzip && 'HTML gzip bytes'
  ].filter(Boolean);
  if (failures.length)
    throw new Error(
      `Sign-in budget exceeded: ${failures.join(', ')}\n${JSON.stringify(
        {
          ...summary,
          contributors: files
            .filter((file) => bundle[file.file].type === 'chunk')
            .map((file) => ({
              file: file.file,
              modules: Object.entries((bundle[file.file] as Chunk).modules)
                .map(([id, info]) => ({
                  id,
                  renderedLength: (info as { renderedLength?: number })
                    .renderedLength
                }))
                .sort(
                  (a, b) => (b.renderedLength ?? 0) - (a.renderedLength ?? 0)
                )
            }))
        },
        null,
        2
      )}`
    );
  return summary;
}
