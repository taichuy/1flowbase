// A small lexical pass, not a compiler or an authorization check. It tolerates
// incomplete editor input and skips comments/strings rather than matching their
// contents as imports. Runtime compilation and its allowlist remain unchanged.
export function declarationImportSources(source: string): string[] {
  const tokens =
    source.match(
      /\/\*[\s\S]*?(?:\*\/|$)|\/\/[^\n]*|"(?:\\[\s\S]|[^"\\])*"|'(?:\\[\s\S]|[^'\\])*'|`(?:\\[\s\S]|[^`\\])*`|[\w$]+|[^\s]/g
    ) ?? [];
  const result = new Set<string>();
  let previous = '';
  let beforePrevious = '';
  for (const token of tokens) {
    if (token.startsWith('//') || token.startsWith('/*')) continue;
    if (
      /^["']/.test(token) &&
      (previous === 'from' ||
        previous === 'import' ||
        (previous === '(' &&
          (beforePrevious === 'import' || beforePrevious === 'require')))
    ) {
      result.add(
        token
          .slice(1, -1)
          .replace(
            /\\(?:u\{([\da-f]+)\}|u([\da-f]{4})|x([\da-f]{2})|([^]))/gi,
            (escape, codePoint, unicode, hex, character) => {
              if (!codePoint && !unicode && !hex) return character;
              const value = Number.parseInt(codePoint ?? unicode ?? hex, 16);
              return value <= 0x10ffff ? String.fromCodePoint(value) : escape;
            }
          )
      );
    }
    beforePrevious = previous;
    previous = token;
  }
  for (const match of source.matchAll(
    /^\s*\/\/\/\s*<reference\s+types=["']([^"']+)["']/gm
  )) {
    result.add(match[1]!);
  }
  return [...result];
}
