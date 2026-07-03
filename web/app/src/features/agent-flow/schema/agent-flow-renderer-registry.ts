import {
  createRendererRegistry,
  type SchemaFieldRenderer,
  type SchemaViewRenderer
} from '../../../shared/schema-ui/registry/create-renderer-registry';

import { LlmParameterForm } from '../components/detail/fields/LlmParameterForm';
import { agentFlowFieldRenderers } from './agent-flow-field-renderers';
import { agentFlowViewRenderers } from './agent-flow-view-renderers';

export const agentFlowRendererRegistry = createRendererRegistry({
  fields: agentFlowFieldRenderers,
  views: agentFlowViewRenderers,
  dynamicForms: {
    llm_parameters: LlmParameterForm
  },
  shells: {}
});

export function registerAgentFlowRenderers({
  fields,
  views
}: {
  fields?: Record<string, SchemaFieldRenderer>;
  views?: Record<string, SchemaViewRenderer>;
}) {
  Object.assign(agentFlowRendererRegistry.fields, fields ?? {});
  Object.assign(agentFlowRendererRegistry.views, views ?? {});
}
