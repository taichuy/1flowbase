const DEMAND_DRIVEN_PRELOAD_BOUNDARIES = [
  /^(?:assets\/)?SettingsPage-[^/]+\.js$/u,
  /^(?:assets\/)?SettingsExtensionCenterSection-[^/]+\.js$/u,
  /^(?:assets\/)?AppShellFrame-[^/]+\.js$/u,
  /^(?:assets\/)?_virtual_1flowbase-native-ant-design-icons-loaders-[^/]+\.js$/u
] as const;

export function resolveProductionModulePreloadDependencies(
  filename: string,
  dependencies: string[],
  context: { hostId: string; hostType: 'html' | 'js' }
): string[] {
  if (
    context.hostType === 'js' &&
    DEMAND_DRIVEN_PRELOAD_BOUNDARIES.some((pattern) => pattern.test(filename))
  ) {
    // These aggregate boundaries contain nested lazy feature islands or
    // demand-loaded module inventories. Their real static imports remain
    // browser-resolved; speculative traversal into nested imports does not.
    return [];
  }
  return dependencies;
}
