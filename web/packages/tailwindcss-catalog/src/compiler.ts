import { __unstable__loadDesignSystem, compile } from 'tailwindcss';

import {
  TAILWIND_PREFLIGHT_CSS,
  TAILWIND_THEME_CSS,
  TAILWIND_UTILITIES_CSS
} from './stylesheet-contract.ts';

export const TAILWIND_BLOCK_PRESET_VARIANTS = Object.freeze([
  'hover',
  'focus',
  'focus-visible',
  'active',
  'disabled',
  'sm',
  'md',
  'lg',
  'xl',
  '2xl'
] as const);

let blockPresetFlight:
  | Promise<{ css: string; baseCandidates: number; candidates: number }>
  | undefined;

/**
 * Builds the source-independent stylesheet attached by `import 'tailwindcss'`.
 * The exact Tailwind version owns the finite default class inventory; the
 * product contract adds one standard state or responsive variant at a time.
 */
export async function compileTailwindBlockPreset(): Promise<{
  css: string;
  baseCandidates: number;
  candidates: number;
}> {
  blockPresetFlight ??= buildTailwindBlockPreset();
  return blockPresetFlight;
}

async function buildTailwindBlockPreset(): Promise<{
  css: string;
  baseCandidates: number;
  candidates: number;
}> {
  const stylesheet = [
    TAILWIND_THEME_CSS,
    TAILWIND_PREFLIGHT_CSS,
    TAILWIND_UTILITIES_CSS
  ].join('\n');
  const designSystem = await __unstable__loadDesignSystem(stylesheet);
  const baseCandidates = designSystem
    .getClassList()
    .map(([candidate]) => candidate)
    .sort();
  const candidates = [
    ...baseCandidates,
    ...TAILWIND_BLOCK_PRESET_VARIANTS.flatMap((variant) =>
      baseCandidates.map((candidate) => `${variant}:${candidate}`)
    )
  ];
  const compiler = await compile(stylesheet);
  return {
    css: compiler.build(candidates),
    baseCandidates: baseCandidates.length,
    candidates: candidates.length
  };
}

export interface TailwindCompilation {
  css: string;
  acceptedCandidates: string[];
}

/**
 * Fixture-only compatibility helper. Ready block execution uses the
 * source-independent Block Preset and never calls this candidate compiler.
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

/** @see extractStaticTailwindCandidates */
export async function compileTailwindUtilities(
  candidates: readonly string[],
  stylesheets: {
    themeCss: string;
    utilitiesCss: string;
  } = {
    themeCss: TAILWIND_THEME_CSS,
    utilitiesCss: TAILWIND_UTILITIES_CSS
  }
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

export function sourceImportsTailwind(source: string): boolean {
  return /(?:import|export)\s+(?:[^'";]+?\s+from\s+)?['"]tailwindcss['"]/u.test(
    source
  );
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
