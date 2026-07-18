import type { SchemaRule } from '../contracts/canvas-node-schema';

export interface SchemaRuleEvaluationContext {
  capabilities: readonly string[];
  values: Record<string, unknown>;
}

function getValueAtPath(values: Record<string, unknown>, path?: string) {
  if (!path) {
    return undefined;
  }

  return path.split('.').reduce<unknown>((current, segment) => {
    if (current == null || typeof current !== 'object') {
      return undefined;
    }

    return (current as Record<string, unknown>)[segment];
  }, values);
}

export function evaluateSchemaRule(rule: SchemaRule | undefined, context: SchemaRuleEvaluationContext) {
  if (!rule) {
    return true;
  }

  const actualValue = getValueAtPath(context.values, rule.path);

  switch (rule.operator) {
    case 'eq':
      return actualValue === rule.value;
    case 'neq':
      return actualValue !== rule.value;
    case 'in': {
      const candidates = rule.values ?? (Array.isArray(rule.value) ? rule.value : [rule.value]);
      return candidates.includes(actualValue);
    }
    case 'truthy':
      return Boolean(actualValue);
    case 'falsy':
      return !actualValue;
    case 'hasCapability':
      return rule.capability ? context.capabilities.includes(rule.capability) : false;
    default:
      return false;
  }
}
