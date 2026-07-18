import type { ApplicationOperationBindingProjection } from '../../api/public-api';

export function editableOperationBindingsFixture(): ApplicationOperationBindingProjection {
  return {
    editable: true,
    draft: {
      operation_bindings: {
        generate: { target_node_id: 'node-draft-generate' },
        count_tokens: null,
        compact: {
          responses_compact: null,
          responses_compaction_v2: null
        }
      },
      options: [
        {
          operation: 'generate',
          targets: [
            {
              target_node_id: 'node-draft-generate',
              node_alias: 'Draft generate A'
            },
            {
              target_node_id: 'node-draft-generate-b',
              node_alias: 'Draft generate B'
            }
          ]
        },
        {
          operation: 'count_tokens',
          targets: [
            {
              target_node_id: 'node-draft-count-tokens',
              node_alias: 'Draft count tokens'
            }
          ]
        },
        {
          operation: 'compact.responses_compact',
          targets: []
        },
        {
          operation: 'compact.responses_compaction_v2',
          targets: [
            {
              target_node_id: 'node-draft-compact-v2',
              node_alias: 'Draft compact v2'
            }
          ]
        }
      ]
    },
    published: {
      publication_id: 'publication-frozen-1',
      compiled_plan_id: 'compiled-plan-frozen-1',
      bindings: [
        {
          operation: 'generate',
          target_node_id: 'node-frozen-generate',
          status: 'supported',
          target: {
            target_node_id: 'node-frozen-generate',
            node_alias: 'Frozen generate'
          },
          unsupported_reason: null
        },
        {
          operation: 'count_tokens',
          target_node_id: null,
          status: 'unbound',
          target: null,
          unsupported_reason: null
        },
        {
          operation: 'compact.responses_compact',
          target_node_id: 'node-frozen-compact',
          status: 'unsupported',
          target: {
            target_node_id: 'node-frozen-compact',
            node_alias: 'Frozen compact'
          },
          unsupported_reason: 'provider_capability_unsupported'
        },
        {
          operation: 'compact.responses_compaction_v2',
          target_node_id: null,
          status: 'unbound',
          target: null,
          unsupported_reason: null
        }
      ]
    }
  };
}

export function readOnlyOperationBindingsFixture(): ApplicationOperationBindingProjection {
  return {
    ...editableOperationBindingsFixture(),
    editable: false
  };
}

export function emptyOperationBindingOptionsFixture(): ApplicationOperationBindingProjection {
  const projection = editableOperationBindingsFixture();

  return {
    ...projection,
    draft: {
      ...projection.draft,
      options: []
    }
  };
}
