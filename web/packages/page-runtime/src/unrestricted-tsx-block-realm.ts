import { transformNativeReactTsx } from './native-react-compiler/tsx-transform';

export interface UnrestrictedTsxBlockSourceSuccess {
  ok: true;
  moduleSource: string;
  errors: [];
}

export interface UnrestrictedTsxBlockSourceFailure {
  ok: false;
  errors: string[];
}

export type UnrestrictedTsxBlockSourceResult =
  | UnrestrictedTsxBlockSourceSuccess
  | UnrestrictedTsxBlockSourceFailure;

const BROWSER_MODULE_URLS: Readonly<Record<string, string>> = {
  react: 'https://esm.sh/react@19.0.0',
  'react/jsx-runtime': 'https://esm.sh/react@19.0.0/jsx-runtime',
  'react-dom': 'https://esm.sh/react-dom@19.0.0?external=react',
  'react-dom/client': 'https://esm.sh/react-dom@19.0.0/client?external=react',
  antd: 'https://esm.sh/antd@6.1.1?external=react,react-dom',
  '@ant-design/icons': 'https://esm.sh/@ant-design/icons@6.1.0?external=react',
  'antd-style': 'https://esm.sh/antd-style@4.1.0?external=react,react-dom'
};

/**
 * Compiles TSX without the native host's import or browser-global policy.
 * Imports are resolved in the Block-owned iframe, never in the console page.
 */
export function transformUnrestrictedTsxBlockSource(
  source: string
): UnrestrictedTsxBlockSourceResult {
  const transformed = transformNativeReactTsx(source);
  if (!transformed.ok) {
    return { ok: false, errors: transformed.errors.map(({ message }) => message) };
  }

  return {
    ok: true,
    moduleSource: rewriteStaticImports(transformed.code),
    errors: []
  };
}

export function createUnrestrictedTsxBlockSrcdoc({
  moduleSource,
  baseUrl
}: {
  moduleSource: string;
  baseUrl: string;
}): string {
  const policy = [
    "default-src 'none'",
    "script-src 'unsafe-inline' https: blob: data:",
    "style-src 'unsafe-inline' https: data:",
    "img-src https: data: blob:",
    "font-src https: data:",
    "connect-src https:",
    "media-src https: data: blob:",
    "object-src 'none'",
    "frame-src https:"
  ].join('; ');
  const serializedSource = JSON.stringify(moduleSource).replace(
    /<\/script/giu,
    '<\\/script'
  );
  const serializedBaseUrl = escapeHtmlAttribute(baseUrl);

  return [
    '<!doctype html><html><head>',
    `<meta http-equiv="Content-Security-Policy" content="${policy}">`,
    `<base href="${serializedBaseUrl}">`,
    '<style>html,body,#root{margin:0;min-height:100%;box-sizing:border-box}#root{width:100%}</style>',
    '</head><body><div id="root"></div>',
    '<script type="module">',
    "import React from 'https://esm.sh/react@19.0.0';",
    "import { createRoot } from 'https://esm.sh/react-dom@19.0.0/client?external=react';",
    `const moduleSource = ${serializedSource};`,
    'const moduleUrl = URL.createObjectURL(new Blob([moduleSource], { type: "text/javascript" }));',
    'const root = createRoot(document.getElementById("root"));',
    'const reportHeight = () => parent.postMessage({ type: "1flowbase_unrestricted_tsx_height", height: Math.ceil(document.documentElement.scrollHeight) }, "*");',
    'new ResizeObserver(reportHeight).observe(document.documentElement);',
    'try {',
    '  const module = await import(moduleUrl);',
    '  if (typeof module.default !== "function") throw new Error("TSX Block must export a React component as default.");',
    '  root.render(React.createElement(module.default));',
    '  requestAnimationFrame(reportHeight);',
    '} catch (error) {',
    '  root.render(React.createElement("pre", { style: { color: "#cf1322", margin: 12, whiteSpace: "pre-wrap" } }, error instanceof Error ? error.message : String(error)));',
    '  reportHeight();',
    '} finally {',
    '  URL.revokeObjectURL(moduleUrl);',
    '}',
    '</script></body></html>'
  ].join('');
}

function rewriteStaticImports(source: string): string {
  return source.replace(
    /(\bfrom\s*|\bimport\s*)(['"])([^'"\n]+)\2/gu,
    (_whole, prefix: string, quote: string, specifier: string) =>
      `${prefix}${quote}${resolveBrowserModuleSpecifier(specifier)}${quote}`
  );
}

function resolveBrowserModuleSpecifier(specifier: string): string {
  if (specifier.startsWith('//')) return `https:${specifier}`;
  if (/^(?:https?:|data:|blob:|\.\/|\.\.\/|\/)/u.test(specifier)) {
    return specifier;
  }
  return BROWSER_MODULE_URLS[specifier] ?? `https://esm.sh/${specifier}`;
}

function escapeHtmlAttribute(value: string): string {
  return value.replace(/&/gu, '&amp;').replace(/"/gu, '&quot;');
}
