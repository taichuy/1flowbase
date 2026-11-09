import type { FrontstageBlockPorts } from '../page-signals/types';
import type { DataBlockConfig } from './types';

export function dataBlockPorts(config: DataBlockConfig): FrontstageBlockPorts {
  const inputs: FrontstageBlockPorts['inputs'] = [
    { name: 'record_id', schema: { type: 'string' } },
    { name: 'source_block_id', schema: { type: 'string' } },
    { name: 'source_model_code', schema: { type: 'string' } },
    { name: 'parent_record_id', schema: { type: 'string' } }
  ];
  if (
    config.relation &&
    !inputs.some((port) => port.name === config.relation!.parent_record_input)
  ) {
    inputs.push({
      name: config.relation.parent_record_input,
      schema: { type: 'string' }
    });
  }
  if (config.kind === 'table' && config.linkage) {
    const existing = inputs.findIndex((port) => port.name === 'linked_value');
    if (existing >= 0) inputs.splice(existing, 1);
    inputs.push({
      name: 'linked_value',
      schema:
        config.linkage.source_output === 'selected_record_ids'
          ? { type: 'array', items: { type: 'string' } }
          : { type: 'string' },
      source: {
        block_id: config.linkage.source_block_id,
        output: config.linkage.source_output,
        scope: 'page'
      }
    });
  }
  return {
    inputs,
    outputs:
      config.kind === 'table'
        ? [
            { name: 'record_id', schema: { type: 'string' } },
            {
              name: 'selected_record_ids',
              schema: { type: 'array', items: { type: 'string' } }
            }
          ]
        : [
            { name: 'saved_record_id', schema: { type: 'string' } },
            { name: 'save_revision', schema: { type: 'integer' } },
            { name: 'saved_record', schema: { type: 'object' } }
          ]
  };
}

export function dataBlockSurfaceWidth(
  config: DataBlockConfig | null
): number | string {
  const size = config?.kind === 'form' ? config.surface_size : undefined;
  return size === 'small' ? 480 : size === 'large' ? 'min(1100px, 94vw)' : 720;
}

/** Preserve author-defined ports and input mappings when changing local block settings. */
export function mergeDataBlockPorts(
  current: unknown,
  previous: DataBlockConfig | null,
  next: DataBlockConfig
): FrontstageBlockPorts {
  const stored =
    current && typeof current === 'object'
      ? (current as Partial<FrontstageBlockPorts>)
      : {};
  const before = previous
    ? dataBlockPorts(previous)
    : { inputs: [], outputs: [] };
  const generated = dataBlockPorts(next);
  const inputs = Array.isArray(stored.inputs) ? stored.inputs : [];
  const outputs = Array.isArray(stored.outputs) ? stored.outputs : [];
  return {
    inputs: [
      ...inputs.filter(
        (port) =>
          !before.inputs.some((owned) => owned.name === port.name) &&
          !generated.inputs.some((owned) => owned.name === port.name)
      ),
      ...generated.inputs.map((port) =>
        port.name === 'linked_value'
          ? port
          : {
              ...port,
              ...(inputs.find((stored) => stored.name === port.name)?.source
                ? {
                    source: inputs.find((stored) => stored.name === port.name)!
                      .source
                  }
                : {})
            }
      )
    ],
    outputs: [
      ...outputs.filter(
        (port) =>
          !before.outputs.some((owned) => owned.name === port.name) &&
          !generated.outputs.some((owned) => owned.name === port.name)
      ),
      ...generated.outputs
    ]
  };
}
