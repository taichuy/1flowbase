import { compileAndTransformJsBlockSource } from '../js-block-source-evaluator';
import type {
  JsBlockCompiledArtifactProgram,
  JsBlockRunRequest,
  JsBlockSourceProgram
} from '../js-block-worker-runtime';
import {
  canonicalizeCompiledBlockArtifact,
  createCompiledBlockArtifact,
  type CompiledBlockArtifact
} from './compiled-artifact';

export type PreparedJsBlockProgram =
  | {
      ok: true;
      request: JsBlockRunRequest & { program: JsBlockCompiledArtifactProgram };
      artifact: CompiledBlockArtifact;
      compiled: boolean;
    }
  | {
      ok: false;
      fallback: JsBlockSourceProgram;
    };

export function prepareJsBlockProgram(
  request: JsBlockRunRequest,
  runtimeFingerprint: string
): PreparedJsBlockProgram {
  if (request.program.kind === 'compiled_artifact') {
    const artifact = canonicalizeCompiledBlockArtifact(request.program.artifact);
    if (
      artifact &&
      artifact.runtimeFingerprint === runtimeFingerprint &&
      artifact.sourceSha256 === request.program.sourceSha256
    ) {
      return {
        ok: true,
        request: {
          ...request,
          program: {
            kind: 'compiled_artifact',
            artifact,
            sourceSha256: artifact.sourceSha256,
            fallback: cloneSourceProgram(request.program.fallback)
          }
        },
        artifact,
        compiled: false
      };
    }
    return compileSourceProgram(request, request.program.fallback, runtimeFingerprint);
  }

  return compileSourceProgram(request, request.program, runtimeFingerprint);
}

export function repairJsBlockProgram(
  request: JsBlockRunRequest,
  runtimeFingerprint: string
): PreparedJsBlockProgram {
  const fallback =
    request.program.kind === 'compiled_artifact'
      ? request.program.fallback
      : request.program;
  return compileSourceProgram(request, fallback, runtimeFingerprint);
}

function compileSourceProgram(
  request: JsBlockRunRequest,
  fallback: JsBlockSourceProgram,
  runtimeFingerprint: string
): PreparedJsBlockProgram {
  const transformed = compileAndTransformJsBlockSource(
    fallback.source,
    fallback.allowedImports
  );
  if (!transformed.ok) return { ok: false, fallback };
  const artifact = createCompiledBlockArtifact({
    source: fallback.source,
    runtimeFingerprint,
    allowedImports: fallback.allowedImports ?? [],
    transformed
  });
  return {
    ok: true,
    request: {
      ...request,
      program: {
        kind: 'compiled_artifact',
        artifact,
        sourceSha256: artifact.sourceSha256,
        fallback: cloneSourceProgram(fallback)
      }
    },
    artifact,
    compiled: true
  };
}

function cloneSourceProgram(program: JsBlockSourceProgram): JsBlockSourceProgram {
  return {
    kind: 'source',
    source: program.source,
    ...(program.allowedImports
      ? { allowedImports: [...program.allowedImports] }
      : {})
  };
}
