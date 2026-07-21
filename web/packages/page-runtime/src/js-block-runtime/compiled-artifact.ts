import type {
  JsBlockImportBinding,
  JsBlockInjectedModule,
  JsBlockSourceTransformSuccess
} from '../js-block-source-transform';

export const COMPILED_BLOCK_ARTIFACT_FORMAT =
  '1flowbase/js-block-compiled-artifact' as const;
export const COMPILED_BLOCK_ARTIFACT_VERSION = 1 as const;

export interface CompiledBlockTransformProgram {
  injectedModules: JsBlockInjectedModule[];
  importBindings: JsBlockImportBinding[];
  executableBody: string;
  executablePreambleLines: number;
  moduleMapIdentifier: string;
  defaultExportIdentifier: string;
}

export interface CompiledBlockArtifact {
  format: typeof COMPILED_BLOCK_ARTIFACT_FORMAT;
  version: typeof COMPILED_BLOCK_ARTIFACT_VERSION;
  runtimeFingerprint: string;
  sourceSha256: string;
  program: CompiledBlockTransformProgram;
  manifest: {
    allowedImports: string[];
  };
  sourceMap?: JsonValue;
}

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

export function createCompiledBlockRuntimeFingerprint(
  workerAssetIdentity: string | URL
): string {
  return `${COMPILED_BLOCK_ARTIFACT_FORMAT}@${COMPILED_BLOCK_ARTIFACT_VERSION}:worker:${sha256Text(String(workerAssetIdentity))}`;
}

export function createCompiledBlockArtifact({
  source,
  sourceSha256,
  runtimeFingerprint,
  allowedImports,
  transformed
}: {
  source: string;
  sourceSha256?: string;
  runtimeFingerprint: string;
  allowedImports: readonly string[];
  transformed: JsBlockSourceTransformSuccess;
}): CompiledBlockArtifact {
  const sourceMap = canonicalJsonValue(transformed.sourceMap);
  return {
    format: COMPILED_BLOCK_ARTIFACT_FORMAT,
    version: COMPILED_BLOCK_ARTIFACT_VERSION,
    runtimeFingerprint,
    sourceSha256: sourceSha256 ?? sha256Text(source),
    program: {
      injectedModules: transformed.injectedModules.map(canonicalInjectedModule),
      importBindings: transformed.importBindings.map(canonicalImportBinding),
      executableBody: transformed.executableBody,
      executablePreambleLines: transformed.executablePreambleLines,
      moduleMapIdentifier: transformed.moduleMapIdentifier,
      defaultExportIdentifier: transformed.defaultExportIdentifier
    },
    manifest: { allowedImports: [...allowedImports] },
    ...(sourceMap === undefined ? {} : { sourceMap })
  };
}

export function canonicalizeCompiledBlockArtifact(
  value: unknown
): CompiledBlockArtifact | null {
  if (!isRecord(value)) return null;
  if (
    value.format !== COMPILED_BLOCK_ARTIFACT_FORMAT ||
    value.version !== COMPILED_BLOCK_ARTIFACT_VERSION ||
    !isNonEmptyString(value.runtimeFingerprint) ||
    !isSha256(value.sourceSha256) ||
    !isRecord(value.program) ||
    !isRecord(value.manifest) ||
    !Array.isArray(value.manifest.allowedImports) ||
    value.manifest.allowedImports.some((item) => !isNonEmptyString(item))
  ) {
    return null;
  }

  const program = canonicalProgram(value.program);
  if (!program) return null;
  const sourceMap = canonicalJsonValue(value.sourceMap);
  if (value.sourceMap !== undefined && sourceMap === undefined) return null;

  return {
    format: COMPILED_BLOCK_ARTIFACT_FORMAT,
    version: COMPILED_BLOCK_ARTIFACT_VERSION,
    runtimeFingerprint: value.runtimeFingerprint,
    sourceSha256: value.sourceSha256,
    program,
    manifest: { allowedImports: [...value.manifest.allowedImports] },
    ...(sourceMap === undefined ? {} : { sourceMap })
  };
}

export function compiledBlockArtifactMatchesIdentity(
  artifact: CompiledBlockArtifact,
  sourceSha256: string,
  runtimeFingerprint: string
): boolean {
  return (
    artifact.format === COMPILED_BLOCK_ARTIFACT_FORMAT &&
    artifact.version === COMPILED_BLOCK_ARTIFACT_VERSION &&
    artifact.runtimeFingerprint === runtimeFingerprint &&
    artifact.sourceSha256 === sourceSha256
  );
}

export function compiledBlockArtifactToTransform(
  artifact: CompiledBlockArtifact
): JsBlockSourceTransformSuccess {
  return {
    ok: true,
    source: '',
    normalizedSource: '',
    injectedModules: artifact.program.injectedModules,
    importBindings: artifact.program.importBindings,
    executableBody: artifact.program.executableBody,
    executablePreambleLines: artifact.program.executablePreambleLines,
    moduleMapIdentifier: artifact.program.moduleMapIdentifier,
    defaultExportIdentifier: artifact.program.defaultExportIdentifier,
    ...(artifact.sourceMap === undefined ? {} : { sourceMap: artifact.sourceMap }),
    errors: []
  };
}

export function sha256Text(value: string): string {
  const bytes = new TextEncoder().encode(value);
  const bitLength = bytes.length * 8;
  const paddedLength = Math.ceil((bytes.length + 9) / 64) * 64;
  const padded = new Uint8Array(paddedLength);
  padded.set(bytes);
  padded[bytes.length] = 0x80;
  const view = new DataView(padded.buffer);
  view.setUint32(paddedLength - 8, Math.floor(bitLength / 0x100000000));
  view.setUint32(paddedLength - 4, bitLength >>> 0);
  const hash = new Uint32Array(SHA256_INITIAL_HASH);
  const words = new Uint32Array(64);

  for (let offset = 0; offset < paddedLength; offset += 64) {
    for (let index = 0; index < 16; index += 1) {
      words[index] = view.getUint32(offset + index * 4);
    }
    for (let index = 16; index < 64; index += 1) {
      const low = words[index - 15];
      const high = words[index - 2];
      words[index] =
        (words[index - 16] +
          (rotateRight(low, 7) ^ rotateRight(low, 18) ^ (low >>> 3)) +
          words[index - 7] +
          (rotateRight(high, 17) ^ rotateRight(high, 19) ^ (high >>> 10))) >>>
        0;
    }
    compressSha256(hash, words);
  }

  return Array.from(hash, (word) => word.toString(16).padStart(8, '0')).join('');
}

function canonicalProgram(value: Record<string, unknown>): CompiledBlockTransformProgram | null {
  if (
    !Array.isArray(value.injectedModules) ||
    !Array.isArray(value.importBindings) ||
    typeof value.executableBody !== 'string' ||
    !Number.isSafeInteger(value.executablePreambleLines) ||
    (value.executablePreambleLines as number) < 0 ||
    !isNonEmptyString(value.moduleMapIdentifier) ||
    !isNonEmptyString(value.defaultExportIdentifier)
  ) return null;
  const injectedModules = value.injectedModules.map(readInjectedModule);
  const importBindings = value.importBindings.map(readImportBinding);
  if (injectedModules.some((item) => item === null) || importBindings.some((item) => item === null)) return null;
  return {
    injectedModules: injectedModules as JsBlockInjectedModule[],
    importBindings: importBindings as JsBlockImportBinding[],
    executableBody: value.executableBody,
    executablePreambleLines: value.executablePreambleLines as number,
    moduleMapIdentifier: value.moduleMapIdentifier,
    defaultExportIdentifier: value.defaultExportIdentifier
  };
}

function readInjectedModule(value: unknown): JsBlockInjectedModule | null {
  if (!isRecord(value) || !isNonEmptyString(value.source) || !Array.isArray(value.bindings)) return null;
  const bindings = value.bindings.map(readImportBinding);
  return bindings.some((item) => item === null)
    ? null
    : { source: value.source as JsBlockInjectedModule['source'], bindings: bindings as JsBlockImportBinding[] };
}

function readImportBinding(value: unknown): JsBlockImportBinding | null {
  if (!isRecord(value) || !isNonEmptyString(value.source) || !isNonEmptyString(value.local)) return null;
  if (value.kind === 'named' && isNonEmptyString(value.imported)) {
    return { kind: 'named', source: value.source as JsBlockImportBinding['source'], imported: value.imported, local: value.local };
  }
  if (value.kind === 'default' || value.kind === 'namespace') {
    return { kind: value.kind, source: value.source as JsBlockImportBinding['source'], local: value.local };
  }
  return null;
}

function canonicalInjectedModule(value: JsBlockInjectedModule): JsBlockInjectedModule {
  return { source: value.source, bindings: value.bindings.map(canonicalImportBinding) };
}

function canonicalImportBinding(value: JsBlockImportBinding): JsBlockImportBinding {
  return value.kind === 'named'
    ? { kind: value.kind, source: value.source, imported: value.imported, local: value.local }
    : { kind: value.kind, source: value.source, local: value.local };
}

function canonicalJsonValue(value: unknown, seen = new WeakSet<object>()): JsonValue | undefined {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return value;
  if (typeof value === 'number') return Number.isFinite(value) ? value : undefined;
  if (typeof value !== 'object' || seen.has(value)) return undefined;
  seen.add(value);
  if (Array.isArray(value)) {
    const items = value.map((item) => canonicalJsonValue(item, seen));
    return items.some((item) => item === undefined) ? undefined : (items as JsonValue[]);
  }
  const output: Record<string, JsonValue> = {};
  for (const [key, item] of Object.entries(value)) {
    const canonical = canonicalJsonValue(item, seen);
    if (canonical === undefined) return undefined;
    output[key] = canonical;
  }
  return output;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}
function isSha256(value: unknown): value is string {
  return typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
}
function rotateRight(value: number, bits: number): number {
  return (value >>> bits) | (value << (32 - bits));
}

function compressSha256(hash: Uint32Array, words: Uint32Array): void {
  let [a, b, c, d, e, f, g, h] = hash;
  for (let index = 0; index < 64; index += 1) {
    const sigma1 = rotateRight(e, 6) ^ rotateRight(e, 11) ^ rotateRight(e, 25);
    const temp1 = (h + sigma1 + ((e & f) ^ (~e & g)) + SHA256_ROUND_CONSTANTS[index] + words[index]) >>> 0;
    const sigma0 = rotateRight(a, 2) ^ rotateRight(a, 13) ^ rotateRight(a, 22);
    const temp2 = (sigma0 + ((a & b) ^ (a & c) ^ (b & c))) >>> 0;
    h = g; g = f; f = e; e = (d + temp1) >>> 0; d = c; c = b; b = a; a = (temp1 + temp2) >>> 0;
  }
  hash[0] = (hash[0] + a) >>> 0; hash[1] = (hash[1] + b) >>> 0;
  hash[2] = (hash[2] + c) >>> 0; hash[3] = (hash[3] + d) >>> 0;
  hash[4] = (hash[4] + e) >>> 0; hash[5] = (hash[5] + f) >>> 0;
  hash[6] = (hash[6] + g) >>> 0; hash[7] = (hash[7] + h) >>> 0;
}

const SHA256_INITIAL_HASH = new Uint32Array([
  0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
  0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19
]);
const SHA256_ROUND_CONSTANTS = new Uint32Array([
  0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
  0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
  0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
  0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
  0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
  0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
  0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
  0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
]);
