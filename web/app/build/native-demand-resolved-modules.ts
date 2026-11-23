export interface DemandResolvedModuleSource {
  devLoaderSource?: string;
  loaderSource: string;
  moduleSource: string;
}

export function createDemandResolvedModuleDomain({
  errorLabel,
  modules,
  virtualPrefix
}: {
  errorLabel: string;
  modules: readonly DemandResolvedModuleSource[];
  virtualPrefix: string;
}) {
  const moduleByVirtualId = new Map<
    string,
    { devLoaderSource?: string; loaderSource: string }
  >();
  for (const { devLoaderSource, loaderSource, moduleSource } of modules) {
    moduleByVirtualId.set(`${virtualPrefix}${moduleSource}`, {
      devLoaderSource,
      loaderSource
    });
    if (!moduleByVirtualId.has(`${virtualPrefix}${loaderSource}`)) {
      moduleByVirtualId.set(`${virtualPrefix}${loaderSource}`, {
        devLoaderSource,
        loaderSource
      });
    }
  }

  return {
    devImportUrl(moduleSource: string) {
      return `/@id/__x00__${virtualPrefix}${moduleSource}`;
    },
    importId(moduleSource: string) {
      return `${virtualPrefix}${moduleSource}`;
    },
    load(id: string, command: 'build' | 'serve') {
      if (!id.startsWith(`\0${virtualPrefix}`)) return undefined;
      const module = moduleByVirtualId.get(id.slice(1));
      if (!module) {
        throw new Error(`${errorLabel} is not installed or resolvable: ${id}.`);
      }
      const loaderSource =
        command === 'serve' && module.devLoaderSource
          ? module.devLoaderSource
          : module.loaderSource;
      return `import * as moduleNamespace from ${JSON.stringify(loaderSource)}; export { moduleNamespace };`;
    },
    resolveId(id: string) {
      if (!id.startsWith(virtualPrefix)) return undefined;
      if (!moduleByVirtualId.has(id)) {
        throw new Error(`${errorLabel} is not installed or resolvable: ${id}.`);
      }
      return `\0${id}`;
    }
  };
}

export function generateDemandResolvedLoaderRuntime({
  command,
  errorLabel,
  modules,
  virtualPrefix
}: {
  command: 'build' | 'serve';
  errorLabel: string;
  modules: readonly DemandResolvedModuleSource[];
  virtualPrefix: string;
}) {
  const demandDomain = createDemandResolvedModuleDomain({
    errorLabel,
    modules,
    virtualPrefix
  });
  const moduleSources = modules.map(({ moduleSource }) => moduleSource);
  const canonicalSources = [
    ...new Set(modules.map(({ loaderSource }) => loaderSource))
  ].sort((left, right) => left.localeCompare(right));
  const aliasPairs = modules
    .filter(({ loaderSource, moduleSource }) => moduleSource !== loaderSource)
    .map(({ loaderSource, moduleSource }) => [moduleSource, loaderSource] as const);
  const loaderIndex =
    command === 'build'
      ? `const canonicalLoaders = {${canonicalSources
          .map(
            (loaderSource) =>
              `\n  ${JSON.stringify(loaderSource)}: () => import(${JSON.stringify(demandDomain.importId(loaderSource))}).then(({ moduleNamespace }) => moduleNamespace),`
          )
          .join('')}\n};\nconst aliasToCanonical = ${JSON.stringify(Object.fromEntries(aliasPairs))};`
      : `const moduleSourceSet = new Set(${JSON.stringify(moduleSources)});`;
  const loadSelection =
    command === 'build'
      ? `const canonical = aliasToCanonical[moduleSource] ?? moduleSource;\n  const load = canonicalLoaders[canonical];
  if (!load) throw new Error(${JSON.stringify(`${errorLabel} is not installed or resolvable: `)} + moduleSource + '.');`
      : `if (!moduleSourceSet.has(moduleSource)) throw new Error(${JSON.stringify(`${errorLabel} is not installed or resolvable: `)} + moduleSource + '.');
  const moduleId = ${JSON.stringify(demandDomain.devImportUrl(''))} + moduleSource;
  const load = () => import(/* @vite-ignore */ moduleId).then(({ moduleNamespace }) => moduleNamespace);`;

  return {
    loadBody: `const current = moduleFlights.get(moduleSource);
  if (current) return current;
  ${loadSelection}
  const flight = load().catch((error) => {
    if (moduleFlights.get(moduleSource) === flight) moduleFlights.delete(moduleSource);
    throw error;
  });
  moduleFlights.set(moduleSource, flight);
  return flight;`,
    moduleSources,
    preamble: `${loaderIndex}\n\nconst moduleFlights = new Map();`
  };
}

export function generateDemandResolvedLoadersModule({
  errorLabel,
  modules,
  virtualPrefix
}: {
  errorLabel: string;
  modules: readonly DemandResolvedModuleSource[];
  virtualPrefix: string;
}): string {
  const runtime = generateDemandResolvedLoaderRuntime({
    command: 'build',
    errorLabel,
    modules,
    virtualPrefix
  });
  return `${runtime.preamble}\n\nexport async function loadDemandResolvedModule(moduleSource) {\n  ${runtime.loadBody}\n}\n`;
}
