import type { ConsoleFrontstageBlockRuntimeLayer } from '@1flowbase/api-client';

import type { FrontstageBlockInstance } from '../page-document';

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function optionalString(value: unknown): string | null {
  return typeof value === 'string' && value.trim() ? value : null;
}

function record(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : {};
}

export function createFrontstageRuntimeAssemblyBlocks(
  layers: readonly ConsoleFrontstageBlockRuntimeLayer[]
): FrontstageBlockInstance[] {
  return layers.map((layer, order) => {
    const descriptor = record(layer.runtime_descriptor);
    const catalog = record(descriptor.catalog);
    const contribution = record(descriptor.contribution);
    const runtime = record(descriptor.runtime);
    const layout = record(descriptor['x-layout'] ?? descriptor.layout);
    const presentation = record(descriptor['x-presentation']);
    const fixedHeight = presentation.heightMode === 'fixed';
    const height = presentation.height;

    return {
      id: layer.block_id,
      rendererVersion:
        optionalString(descriptor.renderer_version) ??
        optionalString(descriptor.rendererVersion),
      sourceId: layer.block_id,
      codeRef: layer.block_id,
      sourceCodeRef: layer.block_id,
      catalog: {
        providerCode:
          optionalString(catalog.providerCode) ??
          optionalString(catalog.provider_code),
        installationId:
          optionalString(catalog.installationId) ??
          optionalString(catalog.installation_id)
      },
      contribution: {
        pluginId:
          optionalString(contribution.pluginId) ??
          optionalString(contribution.plugin_id),
        pluginVersion:
          optionalString(contribution.pluginVersion) ??
          optionalString(contribution.plugin_version),
        code:
          optionalString(contribution.code) ??
          optionalString(descriptor.contribution_code) ??
          'runtime-assembly'
      },
      props: record(descriptor.props),
      presentation: {
        heightMode: fixedHeight ? 'fixed' : 'auto',
        height:
          fixedHeight && typeof height === 'number' && height >= 120
            ? height
            : null
      },
      layout: { ...layout, order },
      order,
      runtime: {
        kind: optionalString(runtime.kind) ?? 'native_react',
        entry: optionalString(runtime.entry) ?? 'index.js',
        hint: optionalString(runtime.hint) ?? 'native_react'
      }
    };
  });
}
