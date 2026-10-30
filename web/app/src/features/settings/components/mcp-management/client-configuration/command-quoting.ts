export function quotePosix(value: string) {
  return `'${value.replaceAll("'", `'"'"'`)}'`;
}

export function quotePowerShell(value: string) {
  return `'${value.replaceAll("'", "''")}'`;
}

export function quoteWindowsCommand(value: string) {
  return `"${value.replaceAll('"', '""')}"`;
}
