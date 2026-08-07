import type { FlowSelectorOption } from './selector-options';
import { formatNodeVariableLabel } from './variables/variable-labels';

export const TEMPLATE_SELECTOR_REGEX =
  /{{\s*([A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)+)\s*}}/g;

export type SelectorReferenceTransform = (selector: string[]) => string[];

function isSameSelector(left: string[], right: string[]) {
  return (
    left.length === right.length &&
    left.every((segment, index) => segment === right[index])
  );
}

export function createTemplateSelectorToken(selector: string[]) {
  if (selector.length < 2) {
    return '';
  }

  return `{{${selector.join('.')}}}`;
}

export function parseTemplateSelectorTokens(value: string): string[][] {
  const selectors: string[][] = [];

  TEMPLATE_SELECTOR_REGEX.lastIndex = 0;
  for (const match of value.matchAll(TEMPLATE_SELECTOR_REGEX)) {
    const selector = selectorFromTemplateMatch(match);

    if (selector.length >= 2) {
      selectors.push(selector);
    }
  }
  TEMPLATE_SELECTOR_REGEX.lastIndex = 0;

  return selectors;
}

export function dedupeSelectors(selectors: string[][]): string[][] {
  const seen = new Set<string>();

  return selectors.filter((selector) => {
    const key = selector.join('\u0000');

    if (seen.has(key)) {
      return false;
    }

    seen.add(key);
    return true;
  });
}

export function getTemplateSelectorLabel(
  selector: string[],
  options: FlowSelectorOption[]
) {
  const matchedOption = options.find((option) =>
    isSameSelector(option.value, selector)
  );

  return matchedOption
    ? matchedOption.displayLabel
    : formatNodeVariableLabel(selector[0], selector.slice(1).join('.'));
}

export function remapTemplateSelectorTokens(
  value: string,
  remap: ReadonlyMap<string, string> | SelectorReferenceTransform
) {
  return value.replace(
    TEMPLATE_SELECTOR_REGEX,
    (_match, selectorPath: string) => {
      const selector = selectorPath.split('.');
      const remappedSelector =
        typeof remap === 'function'
          ? remap(selector)
          : [remap.get(selector[0] ?? '') ?? selector[0], ...selector.slice(1)];

      return createTemplateSelectorToken(remappedSelector);
    }
  );
}

export function getTemplateSelectorTokenMatch(value: string) {
  TEMPLATE_SELECTOR_REGEX.lastIndex = 0;

  const match = TEMPLATE_SELECTOR_REGEX.exec(value);

  TEMPLATE_SELECTOR_REGEX.lastIndex = 0;
  return match;
}

export function isTemplateSelectorToken(value: string) {
  const match = getTemplateSelectorTokenMatch(value);

  return match !== null && match[0] === value;
}

export function selectorFromTemplateMatch(
  match: RegExpMatchArray | RegExpExecArray
): string[] {
  return typeof match[1] === 'string' ? match[1].split('.') : [];
}
