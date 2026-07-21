import type { ConsoleFrontstageInterfaceCapability } from '@1flowbase/api-client';
import type { FrontendBlockMonacoExtraLib } from '@1flowbase/page-protocol';

import type { NormalizedFrontstageBlockCatalogEntry } from '../block-catalog';
import type { FrontstageBlockInstance } from '../page-document';
import {
  resolveFrontstageInterfaceBindings,
  type FrontstageResolvedInterfaceBinding
} from './interface-binding';
import { generateFrontstageInterfaceSource } from './openapi-codegen';

export interface FrontstageJsxEditorProjection {
  bindings: FrontstageResolvedInterfaceBinding[];
  components: string[];
  contextComment: string;
  monacoExtraLibs: FrontendBlockMonacoExtraLib[];
}

export function createFrontstageJsxEditorProjection({
  block,
  catalogEntry,
  interfaceCapabilities
}: {
  block: FrontstageBlockInstance;
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null;
  interfaceCapabilities: readonly ConsoleFrontstageInterfaceCapability[];
}): FrontstageJsxEditorProjection {
  const monacoExtraLibs = catalogEntry?.codeCapabilities?.monacoExtraLibs ?? [];
  const bindings = resolveFrontstageInterfaceBindings(
    block,
    interfaceCapabilities
  );
  return {
    bindings,
    components: collectCatalogComponents(monacoExtraLibs),
    contextComment: createFrontstageContextComment(bindings),
    monacoExtraLibs
  };
}

export function createFrontstageJsxBindingSnippet(
  binding: FrontstageResolvedInterfaceBinding
): string {
  if (!binding.operation) {
    throw new Error(
      `Interface capability is missing: ${binding.binding.operation_id}.`
    );
  }
  return generateFrontstageInterfaceSource(
    binding.operation,
    binding.binding.alias
  ).source;
}

export function createFrontstageContextComment(
  bindings: readonly FrontstageResolvedInterfaceBinding[]
): string {
  const interfaceSummary =
    bindings.length === 0
      ? '无'
      : bindings.map(({ binding }) => binding.alias).join(', ');
  return [
    '/**',
    ' * @1flowbase-context',
    ' * inputs: 无',
    ` * interfaces: ${interfaceSummary}`,
    ' * outputs: 无',
    ' */'
  ].join('\n');
}

function collectCatalogComponents(
  extraLibs: readonly FrontendBlockMonacoExtraLib[]
): string[] {
  const names = new Set<string>();
  const pattern = /export\s+(?:declare\s+)?const\s+([A-Z][A-Za-z0-9_$]*)\b/g;
  for (const extraLib of extraLibs) {
    for (const match of extraLib.content.matchAll(pattern)) {
      if (match[1]) names.add(match[1]);
    }
  }
  return [...names].sort((left, right) => left.localeCompare(right));
}
