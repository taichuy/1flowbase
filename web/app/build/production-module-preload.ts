export function resolveProductionModulePreloadDependencies(
  _filename: string,
  dependencies: string[],
  _context: { hostId: string; hostType: 'html' | 'js' }
): string[] {
  // Vite supplies the requested chunk's static JS/CSS closure, not descendants
  // of its dynamic imports. Filtering this list merely serializes discovery of
  // code we still need. Optional features belong behind actual import boundaries;
  // scenario-demand validates those boundaries against the emitted graph.
  return dependencies;
}
