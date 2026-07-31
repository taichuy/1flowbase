import type { NodeDefinitionMetaMap } from './types';
import { dataModelNodeMeta } from './nodes/data-model';
import { i18nText } from '../../../../shared/i18n/text';

export const nodeDefinitionMeta: NodeDefinitionMetaMap = {
  start: {
    summary: i18nText(
      'agentFlow',
      'auto.define_workflow_entries_generate_initial_user_input'
    ),
    helpHref: '/docs/agentflow/nodes/start',
    flowRole: 'entry'
  },
  answer: {
    summary: i18nText(
      'agentFlow',
      'auto.output_response_results_round_workflow_end_user'
    ),
    helpHref: '/docs/agentflow/nodes/answer',
    flowRole: 'terminal'
  },
  llm: {
    summary: i18nText(
      'agentFlow',
      'auto.call_large_language_model_generate_text_results'
    ),
    helpHref: '/docs/agentflow/nodes/llm'
  },
  knowledge_retrieval: {
    summary: i18nText(
      'agentFlow',
      'auto.retrieve_knowledge_base_based_input_question_return_document_results'
    ),
    helpHref: '/docs/agentflow/nodes/knowledge-retrieval'
  },
  question_classifier: {
    summary: i18nText('agentFlow', 'auto.classify_question_output_hit_labels'),
    helpHref: '/docs/agentflow/nodes/question-classifier'
  },
  if_else: {
    summary: i18nText(
      'agentFlow',
      'auto.determine_output_result_node_based_conditional_judgment'
    ),
    helpHref: '/docs/agentflow/nodes/if-else'
  },
  code: {
    summary: i18nText(
      'agentFlow',
      'auto.execute_custom_code_return_structured_results'
    ),
    helpHref: '/docs/agentflow/nodes/code'
  },
  template_transform: {
    summary: i18nText(
      'agentFlow',
      'auto.generate_transformation_results_based_templates_input_variables'
    ),
    helpHref: '/docs/agentflow/nodes/template-transform'
  },
  http_request: {
    summary: i18nText(
      'agentFlow',
      'auto.request_external_http_service_read_response_data'
    ),
    helpHref: '/docs/agentflow/nodes/http-request'
  },
  tool: {
    summary: i18nText(
      'agentFlow',
      'auto.call_external_tool_capabilities_return_tool_execution_results'
    ),
    helpHref: '/docs/agentflow/nodes/tool'
  },
  tool_result: {
    summary: i18nText('agentFlow', 'auto.define_tool_call_return_content'),
    helpHref: '/docs/agentflow/nodes/tool-result'
  },
  ...dataModelNodeMeta,
  sql: {
    summary: i18nText('agentFlow', 'auto.execute_native_sql'),
    helpHref: '/docs/agentflow/nodes/sql'
  },
  variable_assigner: {
    summary: i18nText(
      'agentFlow',
      'auto.assign_conversation_variable_current_run'
    ),
    helpHref: '/docs/agentflow/nodes/variable-assigner'
  },
  parameter_extractor: {
    summary: i18nText(
      'agentFlow',
      'auto.extract_structured_parameter_results_text'
    ),
    helpHref: '/docs/agentflow/nodes/parameter-extractor'
  },
  iteration: {
    summary: i18nText(
      'agentFlow',
      'auto.performs_item_iteration_over_input_collection_summarizes_results'
    ),
    helpHref: '/docs/agentflow/nodes/iteration',
    canEnterContainer: true
  },
  loop: {
    summary: i18nText(
      'agentFlow',
      'auto.execute_sub_processes_within_container_conditional_loop'
    ),
    helpHref: '/docs/agentflow/nodes/loop',
    canEnterContainer: true
  },
  human_input: {
    summary: i18nText('agentFlow', 'auto.wait_manual_input_continue_process'),
    helpHref: '/docs/agentflow/nodes/human-input'
  },
  plugin_node: {
    summary: i18nText('agentFlow', 'auto.plugin_node_definition_summary'),
    helpHref: null
  },
  unresolved_node: {
    summary: i18nText('agentFlow', 'auto.unresolved_node_definition_summary'),
    helpHref: null
  }
};
