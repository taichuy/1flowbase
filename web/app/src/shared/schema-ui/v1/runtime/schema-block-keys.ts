import type { SchemaBlock } from '../contracts/canvas-node-schema';

// Labels, options and current values are presentation/draft data, not identity.
// A container title is part of its declared semantic identity.
function semanticDescriptor(block: SchemaBlock): readonly string[] {
  switch (block.kind) {
    case 'field':
      return [block.kind, block.renderer, block.path];
    case 'view':
      return [block.kind, block.renderer, block.key ?? ''];
    case 'dynamic_form':
      return [block.kind, block.form_key];
    default:
      return [block.kind, block.title ?? ''];
  }
}

export function keyedSchemaBlocks(blocks: SchemaBlock[]) {
  const occurrences = new Map<string, number>();
  return blocks.map((block) => {
    const descriptor = JSON.stringify(semanticDescriptor(block));
    const occurrence = occurrences.get(descriptor) ?? 0;
    occurrences.set(descriptor, occurrence + 1);
    // Repeated descriptors, including anonymous containers, pair by occurrence
    // within their sibling equivalence class. The schema supplies no physical ID.
    return { block, key: `${descriptor}:${occurrence}` };
  });
}
