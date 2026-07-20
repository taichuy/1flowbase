export function injectFrontstageContextComment(
  source: string,
  comment: string
): string {
  const withoutExisting = source.replace(
    /\/\*\*[\s\S]*?@1flowbase-context[\s\S]*?\*\/\s*/g,
    ''
  );
  const exportIndex = withoutExisting.search(/\bexport\s+default\b/);
  if (exportIndex < 0) {
    const separator = withoutExisting.endsWith('\n') ? '' : '\n';
    return `${withoutExisting}${separator}\n${comment}\n`;
  }
  return `${withoutExisting.slice(0, exportIndex)}${comment}\n${withoutExisting.slice(exportIndex)}`;
}
