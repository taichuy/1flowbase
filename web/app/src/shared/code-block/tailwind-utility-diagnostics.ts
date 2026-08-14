import type { BlockProtocolError } from '@1flowbase/page-protocol';
import { findUnboundedTailwindClassExpressions } from '@1flowbase/tailwindcss-catalog/compiler';

/**
 * Tailwind validity belongs to the source-driven compiler. Unknown utilities
 * remain valid, while expressions without a finite candidate set must be made
 * finite so the compiler cannot silently emit incomplete CSS.
 */
export function diagnoseUnsupportedTailwindUtilities(
  source: string
): BlockProtocolError[] {
  return findUnboundedTailwindClassExpressions(source).map((expression) => {
    const prefix = source.slice(0, expression.index);
    const lines = prefix.split('\n');
    const line = lines.length;
    const column = (lines.at(-1)?.length ?? 0) + 1;
    return {
      code: 'transform_failed',
      path: 'tailwind.className',
      message:
        'Tailwind className must resolve to a finite set of local literals; use a static string or finite conditional/template.',
      sourceLocation: {
        line,
        column,
        endLine: line,
        endColumn: column + expression.length
      }
    };
  });
}
