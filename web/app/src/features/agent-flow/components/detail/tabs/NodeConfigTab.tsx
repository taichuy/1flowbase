import type { CanvasNodeSchema } from '../../../../../shared/schema-ui/v1/contracts/canvas-node-schema';
import type { SchemaAdapter } from '../../../../../shared/schema-ui/v1/registry/create-renderer-registry';

import { NodeInspector } from '../../inspector/NodeInspector';
import { useNodeSchemaRuntime } from '../../inspector/use-node-schema-runtime';

export function NodeConfigTab({
  schema,
  adapter
}: {
  schema?: CanvasNodeSchema;
  adapter?: SchemaAdapter;
} = {}) {
  const runtime = useNodeSchemaRuntime(!schema || !adapter);
  const activeSchema = schema ?? runtime.schema;
  const activeAdapter = adapter ?? runtime.adapter;

  if (!activeSchema || !activeAdapter) {
    return null;
  }

  return (
    <div className="agent-flow-node-detail__config-tab">
      <NodeInspector schema={activeSchema} adapter={activeAdapter} />
    </div>
  );
}
