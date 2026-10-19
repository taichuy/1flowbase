const OUTPUT_VARIABLE_KEY_PATTERN = /^[A-Za-z0-9_]+$/;

export function isOutputVariableKeyAllowed(key: string) {
  return OUTPUT_VARIABLE_KEY_PATTERN.test(key);
}
