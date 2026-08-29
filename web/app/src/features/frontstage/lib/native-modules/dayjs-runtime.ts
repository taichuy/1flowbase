export const DAYJS_EXPORTS = ['default'] as const;

type DayjsModuleNamespace = Record<string, unknown> & {
  default: typeof import('dayjs');
};

let moduleFlight: Promise<DayjsModuleNamespace> | undefined;

export function loadDayjsModule(): Promise<DayjsModuleNamespace> {
  moduleFlight ??= import('dayjs')
    .then((module) => ({ default: module.default }))
    .catch((error: unknown) => {
      moduleFlight = undefined;
      throw error;
    });
  return moduleFlight;
}
