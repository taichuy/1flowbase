// FP55 disposable diagnostic only; never part of the product assembly.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { version as viteVersion, type Plugin, type ResolvedConfig } from 'vite';

function sha256(value: string | Uint8Array): string {
  return createHash('sha256').update(value).digest('hex');
}

function optionalLength(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

export function chunkProvenancePlugin(graph: 'main' | 'worker'): Plugin {
  let config: ResolvedConfig;
  return {
    name: `fp55-diagnostic-chunk-provenance-${graph}`,
    apply: 'build',
    enforce: 'post',
    configResolved(resolved) {
      config = resolved;
    },
    generateBundle: {
      order: 'post',
      handler(options, bundle) {
        const chunks = Object.values(bundle)
          .filter((output) => output.type === 'chunk')
          .map((chunk) => ({
            fileName: chunk.fileName,
            name: chunk.name,
            facadeModuleId: chunk.facadeModuleId,
            isEntry: chunk.isEntry,
            isDynamicEntry: chunk.isDynamicEntry,
            bytes: Buffer.byteLength(chunk.code, 'utf8'),
            sha256: sha256(chunk.code),
            imports: [...chunk.imports].sort(),
            dynamicImports: [...chunk.dynamicImports].sort(),
            moduleIds: [...chunk.moduleIds].sort(),
            modules: Object.entries(chunk.modules)
              .map(([id, module]) => ({
                id,
                renderedLength: optionalLength(module.renderedLength),
                originalLength: optionalLength(
                  Reflect.get(module, 'originalLength')
                )
              }))
              .sort((left, right) => left.id.localeCompare(right.id))
          }))
          .sort((left, right) => left.fileName.localeCompare(right.fileName));
        const assets = Object.values(bundle)
          .filter((output) => output.type === 'asset')
          .map((asset) => ({
            fileName: asset.fileName,
            diagnosticReceipt: asset.fileName.startsWith('chunk-provenance/'),
            bytes:
              typeof asset.source === 'string'
                ? Buffer.byteLength(asset.source, 'utf8')
                : asset.source.byteLength,
            sha256: sha256(asset.source)
          }))
          .sort((left, right) => left.fileName.localeCompare(right.fileName));
        const graphKey = sha256(
          JSON.stringify(
            chunks.map((chunk) => ({
              fileName: chunk.fileName,
              facadeModuleId: chunk.facadeModuleId,
              sha256: chunk.sha256
            }))
          )
        );
        const receiptFileName = `${graph}-${graphKey}.json`;
        const receipt = {
          version: 1,
          diagnostic: 'FP55-read-only-output-provenance',
          candidateSha: execFileSync('git', ['rev-parse', 'HEAD'], {
            cwd: config.root,
            encoding: 'utf8'
          }).trim(),
          nodeVersion: process.version,
          viteVersion,
          workflowRunId: process.env.GITHUB_RUN_ID ?? null,
          workflowRunAttempt: process.env.GITHUB_RUN_ATTEMPT ?? null,
          root: config.root,
          mode: config.mode,
          graph,
          graphKey,
          format: options.format,
          minify: config.build.minify,
          chunkSizeWarningLimitKb: config.build.chunkSizeWarningLimit,
          metricNote:
            'renderedLength is a bundler metric, not an additive attribution of minified output bytes; unsupported originalLength is null. Output snapshot precedes emission of this diagnostic receipt.',
          outputBytes:
            chunks.reduce((total, chunk) => total + chunk.bytes, 0) +
            assets.reduce((total, asset) => total + asset.bytes, 0),
          chunkBytes: chunks.reduce((total, chunk) => total + chunk.bytes, 0),
          assetBytes: assets.reduce((total, asset) => total + asset.bytes, 0),
          chunks,
          assets
        };
        const source = `${JSON.stringify(receipt, null, 2)}\n`;
        this.emitFile({
          type: 'asset',
          fileName: `chunk-provenance/${receiptFileName}`,
          source
        });
        // This path is already archived by the normal quality-gate workflow.
        const artifactDirectory = path.resolve(
          config.root,
          '../../tmp/test-governance/health-20261020/chunk-provenance'
        );
        mkdirSync(artifactDirectory, { recursive: true });
        writeFileSync(path.join(artifactDirectory, receiptFileName), source);
        const largestChunks = [...chunks]
          .sort((left, right) => right.bytes - left.bytes)
          .slice(0, 5)
          .map((chunk) => ({
            fileName: chunk.fileName,
            bytes: chunk.bytes,
            facadeModuleId: chunk.facadeModuleId?.slice(0, 240) ?? null,
            moduleCount: chunk.modules.length,
            topModules: [...chunk.modules]
              .sort(
                (left, right) =>
                  (right.renderedLength ?? -1) - (left.renderedLength ?? -1)
              )
              .slice(0, 8)
              .map((module) => ({
                id: module.id.slice(0, 240),
                renderedLength: module.renderedLength
              }))
          }));
        console.log(
          `[FP55_CHUNK_PROVENANCE] ${JSON.stringify({
            graph,
            graphKey,
            receiptFileName,
            candidateSha: receipt.candidateSha,
            chunkCount: chunks.length,
            assetCount: assets.length,
            outputBytes: receipt.outputBytes,
            chunkBytes: receipt.chunkBytes,
            assetBytes: receipt.assetBytes,
            largestChunks
          })}`
        );
      }
    }
  };
}
