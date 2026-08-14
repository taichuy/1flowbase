import type {
  NativeReactCatalogDependencyLock,
  NativeReactResolvedModuleAsset
} from '@1flowbase/page-runtime';
import { sha256Text } from '@1flowbase/page-runtime';
import {
  compileTailwindBase,
  compileTailwindUtilities,
  extractStaticTailwindCandidates,
  findUnboundedTailwindClassExpressions,
  sourceImportsTailwind
} from '@1flowbase/tailwindcss-catalog/compiler';

import tailwindStyleCompilerWorkerUrl from './native-react-style-compiler.worker?worker&url';

export const NATIVE_REACT_TAILWIND_COMPILER_ABI =
  'tailwindcss-4.3.3-candidates-v1';

export interface NativeReactExecutableStyleCompilation {
  candidates: string[];
  candidate_identity: string;
  base_css: string;
  base_css_sha256: string;
  utility_css: string;
  utility_css_sha256: string;
  assets: NativeReactResolvedModuleAsset[];
}

type WorkerResponse =
  | {
      requestId: string;
      ok: true;
      baseCss: string;
      utilityCss: string;
      acceptedCandidates: string[];
    }
  | { requestId: string; ok: false; message: string };

const memory = new Map<string, NativeReactExecutableStyleCompilation>();
const flights = new Map<
  string,
  Promise<NativeReactExecutableStyleCompilation>
>();
const STYLE_CACHE_BUDGET = 8 * 1024 * 1024;

export async function compileNativeReactExecutableStyle(
  sourceCode: string,
  _dependencyLock: NativeReactCatalogDependencyLock = []
): Promise<NativeReactExecutableStyleCompilation> {
  if (!sourceImportsTailwind(sourceCode)) return emptyStyle();
  if (findUnboundedTailwindClassExpressions(sourceCode).length > 0) {
    throw new Error(
      'Tailwind className must resolve to a finite set of local literals; use a static string or finite conditional/template.'
    );
  }
  const candidates = extractStaticTailwindCandidates(sourceCode);
  const candidateIdentity = sha256Text(
    JSON.stringify({ abi: NATIVE_REACT_TAILWIND_COMPILER_ABI, candidates })
  );
  const cached = memory.get(candidateIdentity);
  if (cached) return cached;
  let flight = flights.get(candidateIdentity);
  if (!flight) {
    flight = compileCandidateSet(candidateIdentity, candidates).finally(() => {
      flights.delete(candidateIdentity);
    });
    flights.set(candidateIdentity, flight);
  }
  return flight;
}

async function compileCandidateSet(
  candidateIdentity: string,
  candidates: string[]
): Promise<NativeReactExecutableStyleCompilation> {
  const persisted = await readPersistedStyle(candidateIdentity, candidates);
  if (persisted) {
    memory.set(candidateIdentity, persisted);
    return persisted;
  }
  const result = await compileInWorker(candidates);
  const baseSha = sha256Text(result.baseCss);
  const utilitySha = sha256Text(result.utilityCss);
  const value: NativeReactExecutableStyleCompilation = {
    candidates,
    candidate_identity: candidateIdentity,
    base_css: result.baseCss,
    base_css_sha256: baseSha,
    utility_css: result.utilityCss,
    utility_css_sha256: utilitySha,
    assets: [
      createStyleAsset('frontstage/tailwind-base', result.baseCss, baseSha),
      createStyleAsset(
        'frontstage/tailwind-utilities',
        result.utilityCss,
        utilitySha
      )
    ]
  };
  memory.set(candidateIdentity, value);
  void persistStyle(value);
  return value;
}

async function compileInWorker(candidates: string[]): Promise<{
  baseCss: string;
  utilityCss: string;
  acceptedCandidates: string[];
}> {
  if (typeof Worker !== 'function') {
    const [baseCss, utilities] = await Promise.all([
      compileTailwindBase(),
      compileTailwindUtilities(candidates)
    ]);
    return {
      baseCss,
      utilityCss: utilities.css,
      acceptedCandidates: utilities.acceptedCandidates
    };
  }
  return new Promise((resolve, reject) => {
    const requestId = crypto.randomUUID();
    const worker = new Worker(tailwindStyleCompilerWorkerUrl, {
      type: 'module',
      name: 'native-react-tailwind-compiler'
    });
    const finish = () => worker.terminate();
    worker.onmessage = (event: MessageEvent<WorkerResponse>) => {
      if (event.data.requestId !== requestId) return;
      finish();
      if (!event.data.ok) {
        reject(new Error(event.data.message));
        return;
      }
      resolve({
        baseCss: event.data.baseCss,
        utilityCss: event.data.utilityCss,
        acceptedCandidates: event.data.acceptedCandidates
      });
    };
    worker.onerror = (event) => {
      finish();
      reject(new Error(event.message || 'Tailwind compiler Worker failed.'));
    };
    worker.postMessage({ requestId, candidates });
  });
}

function emptyStyle(): NativeReactExecutableStyleCompilation {
  const emptySha = sha256Text('');
  return {
    candidates: [],
    candidate_identity: sha256Text(
      NATIVE_REACT_TAILWIND_COMPILER_ABI + ':disabled'
    ),
    base_css: '',
    base_css_sha256: emptySha,
    utility_css: '',
    utility_css_sha256: emptySha,
    assets: []
  };
}

function createStyleAsset(
  moduleSource: string,
  css: string,
  sha256: string
): NativeReactResolvedModuleAsset {
  return {
    module_source: moduleSource,
    role: 'shadow_style',
    media_type: 'text/css',
    sha256,
    url: `frontstage-style:${sha256}`,
    bytes: new TextEncoder().encode(css).buffer
  };
}

export function createNativeReactExecutableStyleAsset(
  generatedCss: string,
  generatedCssSha256 = sha256Text(generatedCss)
): NativeReactResolvedModuleAsset {
  return createStyleAsset(
    'frontstage/executable-style',
    generatedCss,
    generatedCssSha256
  );
}

interface PersistedStyleRecord {
  key: string;
  kind: 'tailwind_style';
  abi: string;
  identity: string;
  candidates: string[];
  baseCss: string;
  utilityCss: string;
  byteSize: number;
  lastAccessedAt: number;
}

async function readPersistedStyle(
  identity: string,
  candidates: string[]
): Promise<NativeReactExecutableStyleCompilation | null> {
  try {
    const record = await withStyleStore('readonly', (store) =>
      requestValue<PersistedStyleRecord | undefined>(
        store.get(styleCacheKey(identity))
      )
    );
    if (
      !record ||
      record.kind !== 'tailwind_style' ||
      record.abi !== NATIVE_REACT_TAILWIND_COMPILER_ABI ||
      record.identity !== identity ||
      JSON.stringify(record.candidates) !== JSON.stringify(candidates)
    )
      return null;
    const value = styleFromCss(
      identity,
      candidates,
      record.baseCss,
      record.utilityCss
    );
    void persistStyle(value);
    return value;
  } catch {
    return null;
  }
}

async function persistStyle(
  value: NativeReactExecutableStyleCompilation
): Promise<void> {
  try {
    const record: PersistedStyleRecord = {
      key: styleCacheKey(value.candidate_identity),
      kind: 'tailwind_style',
      abi: NATIVE_REACT_TAILWIND_COMPILER_ABI,
      identity: value.candidate_identity,
      candidates: value.candidates,
      baseCss: value.base_css,
      utilityCss: value.utility_css,
      byteSize: new Blob([value.base_css, value.utility_css]).size,
      lastAccessedAt: Date.now()
    };
    if (record.byteSize > STYLE_CACHE_BUDGET) return;
    await withStyleStore('readwrite', async (store) => {
      const all = await requestValue<unknown[]>(store.getAll());
      const styles = all
        .filter(isPersistedStyleRecord)
        .sort((left, right) => left.lastAccessedAt - right.lastAccessedAt);
      let bytes = styles
        .filter((entry) => entry.key !== record.key)
        .reduce((sum, entry) => sum + entry.byteSize, 0);
      for (const entry of styles) {
        if (bytes + record.byteSize <= STYLE_CACHE_BUDGET) break;
        if (entry.key === record.key) continue;
        await requestValue(store.delete(entry.key));
        bytes -= entry.byteSize;
      }
      await requestValue(store.put(record));
    });
  } catch {
    // Persistent caching is an optimization; memory and Worker compilation remain valid.
  }
}

function styleFromCss(
  identity: string,
  candidates: string[],
  baseCss: string,
  utilityCss: string
): NativeReactExecutableStyleCompilation {
  const baseSha = sha256Text(baseCss);
  const utilitySha = sha256Text(utilityCss);
  return {
    candidates,
    candidate_identity: identity,
    base_css: baseCss,
    base_css_sha256: baseSha,
    utility_css: utilityCss,
    utility_css_sha256: utilitySha,
    assets: [
      createStyleAsset('frontstage/tailwind-base', baseCss, baseSha),
      createStyleAsset('frontstage/tailwind-utilities', utilityCss, utilitySha)
    ]
  };
}

function styleCacheKey(identity: string): string {
  return `tailwind-style/${identity}`;
}

function isPersistedStyleRecord(value: unknown): value is PersistedStyleRecord {
  return (
    typeof value === 'object' &&
    value !== null &&
    (value as PersistedStyleRecord).kind === 'tailwind_style' &&
    typeof (value as PersistedStyleRecord).byteSize === 'number'
  );
}

async function withStyleStore<T>(
  mode: IDBTransactionMode,
  run: (store: IDBObjectStore) => Promise<T>
): Promise<T> {
  if (typeof indexedDB === 'undefined')
    throw new Error('IndexedDB unavailable');
  const database = await new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open('1flowbase-frontstage-runtime-records', 1);
    request.onupgradeneeded = () => {
      if (!request.result.objectStoreNames.contains('records')) {
        request.result.createObjectStore('records', { keyPath: 'key' });
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
  try {
    const transaction = database.transaction('records', mode);
    const completed = new Promise<void>((resolve, reject) => {
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error);
      transaction.onabort = () => reject(transaction.error);
    });
    const result = await run(transaction.objectStore('records'));
    await completed;
    return result;
  } finally {
    database.close();
  }
}

function requestValue<T = unknown>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}
