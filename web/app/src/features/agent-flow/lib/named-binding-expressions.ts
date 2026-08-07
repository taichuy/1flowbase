import type {
  NamedBindingEntry,
  NamedBindingExpression
} from '@1flowbase/flow-schema';

import {
  parseTemplateSelectorTokens,
  remapTemplateSelectorTokens,
  type SelectorReferenceTransform
} from './template-binding';

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function normalizeSelectorPath(value: unknown) {
  if (
    !Array.isArray(value) ||
    value.length < 2 ||
    !value.every((segment) => typeof segment === 'string')
  ) {
    return null;
  }

  return value;
}

export function isNamedBindingNameAllowed(name: string) {
  return /^[A-Za-z0-9_]+$/.test(name);
}

function normalizeNamedBindingExpression(
  value: unknown
): NamedBindingExpression | null {
  if (!isRecord(value) || typeof value.kind !== 'string') {
    return null;
  }

  if (value.kind === 'selector') {
    const selector = normalizeSelectorPath(value.selector);

    return selector ? { kind: 'selector', selector } : null;
  }

  if (value.kind === 'constant') {
    return { kind: 'constant', value: value.value };
  }

  if (value.kind === 'templated_text' && typeof value.value === 'string') {
    return { kind: 'templated_text', value: value.value };
  }

  return null;
}

export function getNamedBindingExpression(
  entry: NamedBindingEntry
): NamedBindingExpression | null {
  const expression = normalizeNamedBindingExpression(entry.value);

  if (expression) {
    return expression;
  }

  if (entry.content?.kind === 'templated_text') {
    return {
      kind: 'templated_text',
      value: entry.content.value
    };
  }

  const selector = normalizeSelectorPath(entry.selector);

  return selector ? { kind: 'selector', selector } : null;
}

export function extractNamedBindingEntrySelectors(entry: NamedBindingEntry) {
  const expression = getNamedBindingExpression(entry);

  if (!expression) {
    return [];
  }

  if (expression.kind === 'selector') {
    return [expression.selector];
  }

  if (expression.kind === 'templated_text') {
    return parseTemplateSelectorTokens(expression.value);
  }

  return [];
}

export function extractNamedBindingSelectors(entries: NamedBindingEntry[]) {
  return entries.flatMap(extractNamedBindingEntrySelectors);
}

function remapExpression(
  expression: NamedBindingExpression | undefined,
  transformSelector: SelectorReferenceTransform
) {
  if (!expression) {
    return expression;
  }

  if (expression.kind === 'selector') {
    return {
      ...expression,
      selector: transformSelector(expression.selector)
    };
  }

  if (expression.kind === 'templated_text') {
    return {
      ...expression,
      value: remapTemplateSelectorTokens(expression.value, transformSelector)
    };
  }

  return expression;
}

export function remapNamedBindingEntry(
  entry: NamedBindingEntry,
  transformSelector: SelectorReferenceTransform
): NamedBindingEntry {
  return {
    ...entry,
    value: remapExpression(entry.value, transformSelector),
    selector: entry.selector ? transformSelector(entry.selector) : undefined,
    content:
      entry.content?.kind === 'templated_text'
        ? {
            ...entry.content,
            value: remapTemplateSelectorTokens(
              entry.content.value,
              transformSelector
            )
          }
        : entry.content
  };
}
