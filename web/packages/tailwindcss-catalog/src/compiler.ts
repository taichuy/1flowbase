import * as tailwindcss from 'tailwindcss';

import {
  TAILWIND_PREFLIGHT_CSS,
  TAILWIND_THEME_CSS,
  TAILWIND_UTILITIES_CSS
} from './stylesheet-contract.ts';

export interface TailwindCompilation {
  css: string;
  acceptedCandidates: string[];
}

export interface UnboundedTailwindClassExpression {
  expression: string;
  index: number;
  length: number;
}

export async function compileTailwindBase(): Promise<string> {
  const compiler = await tailwindcss.compile(
    `${TAILWIND_THEME_CSS}\n${TAILWIND_PREFLIGHT_CSS}`
  );
  return compiler.build([]);
}

/**
 * Extracts the finite literal candidate set owned by one block. Dynamic
 * values are intentionally not expanded without an explicit finite literal.
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

/**
 * Finds JSX class expressions whose possible strings cannot be bounded from
 * local literals. Callers surface these as a safelist/finite-expression
 * diagnostic instead of silently emitting incomplete CSS.
 */
export function findUnboundedTailwindClassExpressions(
  source: string
): UnboundedTailwindClassExpression[] {
  if (!sourceImportsTailwind(source)) return [];
  const finiteVariables = collectFiniteVariables(source);
  const assignments = [...source.matchAll(/\bclass(?:Name)?\s*=/gu)];
  const unbounded: UnboundedTailwindClassExpression[] = [];
  for (const assignment of assignments) {
    const assignmentIndex = assignment.index ?? 0;
    let cursor = assignmentIndex + assignment[0].length;
    while (/\s/u.test(source[cursor] ?? '')) cursor += 1;
    if (source[cursor] !== '{') continue;
    const end = findMatchingBrace(source, cursor);
    if (end === -1) continue;
    const expression = source.slice(cursor + 1, end).trim();
    if (resolveFiniteExpression(expression, finiteVariables) !== null) continue;
    unbounded.push({
      expression,
      index: assignmentIndex,
      length: assignment[0].length
    });
  }
  return unbounded;
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
  const compiler = await tailwindcss.compile(
    `${stylesheets.themeCss}\n${stylesheets.utilitiesCss}`
  );
  const emptyCss = compiler.build([]);
  const acceptedCandidates: string[] = [];

  for (const candidate of [...new Set(candidates)].sort()) {
    if (compiler.build([candidate]) !== emptyCss)
      acceptedCandidates.push(candidate);
  }

  return { css: compiler.build(acceptedCandidates), acceptedCandidates };
}

export function sourceImportsTailwind(source: string): boolean {
  return /(?:import|export)\s+(?:[^'";]+?\s+from\s+)?['"]tailwindcss['"]/u.test(
    source
  );
}

function staticStringValues(source: string): string[] {
  const values: string[] = [];
  const finiteVariables = collectFiniteVariables(source);
  for (const match of source.matchAll(
    /\bclass(?:Name)?\s*=\s*(?:"([^"]*)"|'([^']*)'|`([^`]*)`|\{([^}]*)\})/gu
  )) {
    const direct = match[1] ?? match[2] ?? match[3];
    if (direct !== undefined) {
      values.push(...expandFiniteTemplate(direct, finiteVariables));
      continue;
    }
    const expression = match[4] ?? '';
    const finiteExpression = resolveFiniteExpression(
      expression.trim(),
      finiteVariables
    );
    if (finiteExpression) values.push(...finiteExpression);
    for (const literal of expression.matchAll(/(['"])(.*?)\1/gu)) {
      values.push(literal[2]);
    }
    for (const template of expression.matchAll(/`([^`]*)`/gu)) {
      values.push(...expandFiniteTemplate(template[1], finiteVariables));
    }
  }
  for (const match of source.matchAll(
    /\bclass(?:Name)?\s*=\s*\{\s*`([^`]*)`\s*\}/gu
  )) {
    values.push(...expandFiniteTemplate(match[1], finiteVariables));
  }
  return values;
}

function collectFiniteVariables(source: string): Map<string, string[]> {
  const finiteVariables = new Map<string, string[]>();
  for (const match of source.matchAll(
    /\bconst\s+([A-Za-z_$][\w$]*)\s*=\s*[^;?]+\?\s*(['"])(.*?)\2\s*:\s*(['"])(.*?)\4/gu
  )) {
    finiteVariables.set(match[1], [match[3], match[5]]);
  }
  return finiteVariables;
}

function resolveFiniteExpression(
  expression: string,
  finiteVariables: ReadonlyMap<string, string[]>
): string[] | null {
  const quoted = expression.match(/^(['"])(.*?)\1$/u);
  if (quoted) return [quoted[2]];
  const template = expression.match(/^`([\s\S]*)`$/u);
  if (template) {
    const expanded = expandFiniteTemplate(template[1], finiteVariables);
    return expanded.length > 0 ? expanded : null;
  }
  const variable = finiteVariables.get(expression);
  if (variable) return variable;
  return ternaryChoices(expression);
}

function findMatchingBrace(source: string, start: number): number {
  let depth = 0;
  let quote: string | null = null;
  let escaped = false;
  for (let index = start; index < source.length; index += 1) {
    const character = source[index];
    if (quote) {
      if (escaped) escaped = false;
      else if (character === '\\') escaped = true;
      else if (character === quote) quote = null;
      continue;
    }
    if (character === "'" || character === '"' || character === '`') {
      quote = character;
      continue;
    }
    if (character === '{') depth += 1;
    if (character === '}' && --depth === 0) return index;
  }
  return -1;
}

function expandFiniteTemplate(
  template: string,
  finiteVariables: ReadonlyMap<string, string[]>
): string[] {
  let results = [''];
  let cursor = 0;
  for (const match of template.matchAll(/\$\{([^}]+)\}/gu)) {
    const start = match.index ?? 0;
    const prefix = template.slice(cursor, start);
    const expression = match[1].trim();
    const choices =
      finiteVariables.get(expression) ?? ternaryChoices(expression);
    if (!choices || results.length * choices.length > 128) return [];
    results = results.flatMap((result) =>
      choices.map((choice) => result + prefix + choice)
    );
    cursor = start + match[0].length;
  }
  const suffix = template.slice(cursor);
  return results.map((result) => result + suffix);
}

function ternaryChoices(expression: string): string[] | null {
  const match = expression.match(
    /^[^?]+\?\s*(['"])(.*?)\1\s*:\s*(['"])(.*?)\3$/u
  );
  return match ? [match[2], match[4]] : null;
}
