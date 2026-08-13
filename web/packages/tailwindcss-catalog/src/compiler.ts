import { compile } from 'tailwindcss';
import themeCss from 'tailwindcss/theme.css?raw';
import utilitiesCss from 'tailwindcss/utilities.css?raw';

export interface TailwindCompilation {
  css: string;
  acceptedCandidates: string[];
}

/**
 * Collects complete static strings from TSX source. Tailwind remains the
 * authority that decides which tokens are valid candidates; 1flowbase does
 * not maintain a parallel utility inventory.
 */
export function extractStaticTailwindCandidates(source: string): string[] {
  const candidates = new Set<string>();
  for (const value of staticStringValues(source)) {
    for (const candidate of value.split(/\s+/u)) {
      const normalized = candidate.trim();
      if (normalized) candidates.add(normalized);
    }
  }
  return [...candidates].sort();
}

export async function compileTailwindUtilities(
  candidates: readonly string[],
  stylesheets: {
    themeCss: string;
    utilitiesCss: string;
  } = { themeCss, utilitiesCss }
): Promise<TailwindCompilation> {
  const compiler = await compile(
    `${stylesheets.themeCss}\n${stylesheets.utilitiesCss}`
  );
  let previousCss = compiler.build([]);
  const acceptedCandidates: string[] = [];

  for (const candidate of [...new Set(candidates)].sort()) {
    const nextCss = compiler.build([candidate]);
    if (nextCss !== previousCss) acceptedCandidates.push(candidate);
    previousCss = nextCss;
  }

  return { css: previousCss, acceptedCandidates };
}

function staticStringValues(source: string): string[] {
  const values: string[] = [];
  let index = 0;

  while (index < source.length) {
    const quote = source[index];
    if (quote !== "'" && quote !== '"' && quote !== '`') {
      index += 1;
      continue;
    }

    const start = index + 1;
    let dynamicTemplate = false;
    index += 1;
    while (index < source.length) {
      if (source[index] === '\\') {
        index += 2;
        continue;
      }
      if (quote === '`' && source[index] === '$' && source[index + 1] === '{') {
        dynamicTemplate = true;
      }
      if (source[index] === quote) {
        if (!dynamicTemplate) values.push(source.slice(start, index));
        index += 1;
        break;
      }
      index += 1;
    }
  }

  return values;
}
