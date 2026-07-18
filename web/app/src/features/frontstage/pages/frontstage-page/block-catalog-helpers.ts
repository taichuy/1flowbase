import type { NormalizedFrontstageBlockCatalogEntry } from '../../lib/block-catalog';
import type { FrontstageBlockCompositionInput } from '../../lib/block-composition';
import { FRONTSTAGE_BLOCK_RENDERER_VERSION_V1 } from '../../lib/block-renderer-version';
import type { FrontstageBlockInstance } from '../../lib/page-document';

function createCatalogBlockInput(
  entry: NormalizedFrontstageBlockCatalogEntry,
  blockIndex: number
): FrontstageBlockCompositionInput {
  const codeTemplate = entry.codeCapabilities?.template;
  if (!codeTemplate) {
    throw new Error('Catalog entry is missing a code template.');
  }

  const blockId = `frontstage-js-block-${crypto.randomUUID()}`;

  return {
    id: blockId,
    rendererVersion: FRONTSTAGE_BLOCK_RENDERER_VERSION_V1,
    codeRef: `${blockId}-code`,
    catalog: {
      providerCode: entry.providerCode,
      installationId: entry.installationId
    },
    contribution: {
      pluginId: entry.pluginId,
      pluginVersion: entry.pluginVersion,
      code: entry.contributionCode
    },
    props: {},
    layout: {
      order: blockIndex,
      region: 'main'
    },
    runtime: {
      kind: entry.runtimeKind,
      entry: entry.entry,
      hint: entry.runtimeKind,
      code_template_version: codeTemplate.version,
      code_template_language: codeTemplate.language
    }
  };
}

function findMatchingFrontstageBlockCatalogEntry(
  block: FrontstageBlockInstance | null | undefined,
  catalogItems: NormalizedFrontstageBlockCatalogEntry[]
): NormalizedFrontstageBlockCatalogEntry | null {
  if (!block) {
    return null;
  }

  return (
    catalogItems.find(
      (item) =>
        block.catalog.providerCode === item.providerCode &&
        block.catalog.installationId === item.installationId &&
        block.contribution.pluginId === item.pluginId &&
        block.contribution.pluginVersion === item.pluginVersion &&
        block.contribution.code === item.contributionCode
    ) ?? null
  );
}

export { createCatalogBlockInput, findMatchingFrontstageBlockCatalogEntry };
