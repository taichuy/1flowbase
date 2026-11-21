import type {
  BlockSourceDeclarationLoader,
  BlockSourceExtraLib
} from '../../../../../shared/code-block/extra-lib';
import { declarationImportSources } from './imports';

export interface NativeDeclarationIndex {
  version: 1;
  roots: Record<string, readonly string[]>;
  resources: Record<string, string>;
}

const IMMUTABLE_DECLARATION_RESOURCE_RE =
  /(?:^|\/)assets\/native-declarations\/[a-f0-9]{64}\.json$/u;

function isImmutableDeclarationResourceUrl(url: string): boolean {
  const pathname = url.split('?')[0] ?? '';
  return IMMUTABLE_DECLARATION_RESOURCE_RE.test(pathname);
}

export function createNativeDeclarationLoader(
  index: NativeDeclarationIndex,
  frontendDeclarations: readonly BlockSourceExtraLib[],
  fetchResource: (
    url: string
  ) => Promise<readonly BlockSourceExtraLib[]> = async (url) => {
    const response = isImmutableDeclarationResourceUrl(url)
      ? await fetch(url, { cache: 'force-cache' })
      : await fetch(url);
    if (!response.ok)
      throw new Error(`Declaration resource failed (${response.status})`);
    return response.json();
  }
): BlockSourceDeclarationLoader {
  const flights = new Map<string, Promise<readonly BlockSourceExtraLib[]>>();
  const frontend = new Map(
    frontendDeclarations.map((file) => [file.source, file])
  );
  const fetchOnce = (id: string) => {
    let flight = flights.get(id);
    if (!flight) {
      const url = index.resources[id];
      if (!url) throw new Error(`Missing declaration resource ${id}`);
      flight = fetchResource(url);
      flights.set(id, flight);
      const attempt = flight;
      void attempt.catch(() => {
        if (flights.get(id) === attempt) flights.delete(id);
      });
    }
    return flight;
  };
  async function loadSources(sources: readonly string[]) {
    const pending = [...sources];
    const visited = new Set<string>();
    const local: BlockSourceExtraLib[] = [];
    const resources = new Set<string>();
    while (pending.length) {
      const source = pending.pop()!;
      if (visited.has(source)) continue;
      visited.add(source);
      const declaration = frontend.get(source);
      if (declaration) {
        local.push(declaration);
        pending.push(...declarationImportSources(declaration.content));
      }
      const rootResources = Object.hasOwn(index.roots, source)
        ? index.roots[source]
        : [];
      for (const id of rootResources ?? []) resources.add(id);
    }
    // No global background queue: current-source demand starts immediately and
    // shares only the exact resource flights already needed by another action.
    const loaded = await Promise.all([...resources].map(fetchOnce));
    return [
      ...new Map(
        [...local, ...loaded.flat()].map((file) => [file.filePath, file])
      ).values()
    ];
  }
  return {
    loadForSource: (source) =>
      loadSources([
        'react',
        'react/jsx-runtime',
        ...declarationImportSources(source)
      ]),
    loadAll: () =>
      loadSources([...Object.keys(index.roots), ...frontend.keys()])
  };
}
