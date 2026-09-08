import {
  diagnoseLegacyBlockModuleSource,
  NATIVE_REACT_COMPONENT_ARTIFACT_VERSION,
  NATIVE_REACT_COMPILER_ABI,
  NATIVE_REACT_RUNTIME_ABI,
  sha256Text
} from '@1flowbase/page-runtime/browser';
import {
  compileNativeReactComponentInBrowser,
  type NativeReactBrowserCompileResult
} from '../../../shared/code-block/native-react-compiler-browser';

import { createFrontstageNativeReactModuleRegistry } from '../../frontstage/lib/native-modules/registry';

type Compile = typeof compileNativeReactComponentInBrowser;
type Request = Omit<Parameters<Compile>[0], 'signal'>;
type Priority = 'demand' | 'intent';
type Job = {
  key: string;
  request: Request;
  priority: Priority;
  promise: Promise<NativeReactBrowserCompileResult>;
  resolve: (result: NativeReactBrowserCompileResult) => void;
};
const MAX_ARTIFACTS = 4;
const MAX_CACHE_BYTES = 1024 * 1024;
const MAX_JOBS = 4;
const COMPILE_DEADLINE_MS = 9000;

// Only immutable compilation is shared. Evaluation, generated styles and auth
// callbacks continue to belong to each mounted PublicAuthBlock instance.
export function createPublicAuthCompilation(
  compiler: Compile = compileNativeReactComponentInBrowser
) {
  const cache = new Map<
    string,
    { result: NativeReactBrowserCompileResult; bytes: number }
  >();
  const jobs = new Map<string, Job>();
  const queue: Job[] = [];
  let running = false;
  let cacheBytes = 0;

  function drain() {
    if (running) return;
    const index = queue.findIndex((job) => job.priority === 'demand');
    const job = queue.splice(index < 0 ? 0 : index, 1)[0];
    if (!job) return;
    running = true;
    const controller = new AbortController();
    let settled = false;
    const finish = (result: NativeReactBrowserCompileResult) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      jobs.delete(job.key);
      if (result.ok) {
        const bytes = new TextEncoder().encode(
          JSON.stringify(result.artifact)
        ).byteLength;
        if (bytes <= MAX_CACHE_BYTES) {
          cache.set(job.key, { result, bytes });
          cacheBytes += bytes;
          while (cache.size > MAX_ARTIFACTS || cacheBytes > MAX_CACHE_BYTES) {
            const oldest = cache.keys().next().value!;
            cacheBytes -= cache.get(oldest)!.bytes;
            cache.delete(oldest);
          }
        }
      }
      running = false;
      job.resolve(result);
      drain();
    };
    const timeout = setTimeout(() => {
      controller.abort();
      finish(unavailable('Public auth compilation deadline exceeded.'));
    }, COMPILE_DEADLINE_MS);
    try {
      compiler({ ...job.request, signal: controller.signal }).then(
        finish,
        (error: unknown) => {
          finish(
            unavailable(
              error instanceof Error
                ? error.message
                : 'Public auth compilation failed.'
            )
          );
        }
      );
    } catch (error) {
      finish(
        unavailable(
          error instanceof Error
            ? error.message
            : 'Public auth compilation failed.'
        )
      );
    }
  }

  function enqueue(
    request: Request,
    priority: Priority
  ): Promise<NativeReactBrowserCompileResult> {
    // Custom Worker owners retain their own lifecycle and result scope.
    if (request.workerFactory) return compiler(request);
    const key = sha256Text(
      JSON.stringify([
        request.source,
        NATIVE_REACT_COMPONENT_ARTIFACT_VERSION,
        NATIVE_REACT_COMPILER_ABI,
        NATIVE_REACT_RUNTIME_ABI,
        request.moduleDefinitions
      ])
    );
    const cached = cache.get(key);
    if (cached) {
      cache.delete(key);
      cache.set(key, cached);
      return Promise.resolve(cached.result);
    }
    const existing = jobs.get(key);
    if (existing) {
      if (priority === 'demand') existing.priority = 'demand';
      return existing.promise;
    }
    // Keep only the latest queued hover/focus intent; never execute an inventory.
    const staleIntent = queue.findIndex((job) => job.priority === 'intent');
    if (priority === 'intent' && staleIntent >= 0) {
      const [stale] = queue.splice(staleIntent, 1);
      jobs.delete(stale.key);
      stale.resolve(unavailable('Public auth prefetch superseded.'));
    }
    if (jobs.size >= MAX_JOBS)
      return Promise.resolve(
        unavailable('Public auth compilation capacity reached.')
      );
    let resolve!: Job['resolve'];
    const promise = new Promise<NativeReactBrowserCompileResult>((done) => {
      resolve = done;
    });
    const job: Job = { key, request, priority, promise, resolve };
    jobs.set(key, job);
    queue.push(job);
    drain();
    return promise;
  }
  return {
    compile: (request: Request) => enqueue(request, 'demand'),
    prefetch: (request: Request) => enqueue(request, 'intent')
  };
}

function unavailable(message: string): NativeReactBrowserCompileResult {
  return {
    ok: false,
    diagnostics: [
      { phase: 'compile', code: 'transform_failed', path: 'worker', message }
    ]
  };
}

export const publicAuthCompilation = createPublicAuthCompilation();

export function prefetchPublicAuthSource(source: string) {
  if (diagnoseLegacyBlockModuleSource(source)) return;
  return publicAuthCompilation.prefetch({
    source,
    requestId: `public-auth-prefetch:${sha256Text(source)}`,
    moduleDefinitions: createFrontstageNativeReactModuleRegistry().definitions
  });
}
