import type {
  FlowBinding,
  FlowNodeOutputDocument,
  FlowNodeType,
  NodeRuntimeSideEffectPolicy,
  NodeRuntimePortDocument,
  NodeRuntimePanelFieldDocument,
  NodeRuntimePanelSectionDocument,
  NodeRuntimeUiContract
} from '@1flowbase/flow-schema';
import {
  DEFAULT_LLM_PROTOCOL_CONTEXT_REFERENCE,
  NODE_CONTRIBUTION_SCHEMA_VERSION,
  getLlmNodeOutputs
} from '@1flowbase/flow-schema';

import {
  getDataModelNodeDefaultConfig,
  getDataModelNodeOutputs,
  getDataModelActionForNodeType
} from './nodes/data-model';
import { normalizeCodeOutput } from '../output-contract/code-output';
import { createDefaultIfElseBranches } from '../if-else-branches';
import {
  HTTP_REQUEST_BODY_TYPE_OPTIONS,
  HTTP_REQUEST_DEFAULT_BINDINGS,
  HTTP_REQUEST_DEFAULT_CONFIG,
  HTTP_REQUEST_MAX_RESPONSE_BYTES,
  HTTP_REQUEST_OUTPUTS,
  HTTP_REQUEST_RESPONSE_BYTES_STEP
} from '../http-request/contract';
import { i18nText } from '../../../../shared/i18n/text';
import {
  getNodeRuntimeContract,
  registerNodeRuntimeContracts
} from '../../../flow-editor/authoring/runtime-contract-registry';

type BuiltinNodeRuntimeContractType = Exclude<FlowNodeType, 'unresolved_node'>;
type ContractCategory = 'io' | 'generation' | 'control' | 'data' | 'external';

const DEFAULT_RUNTIME_POLICY = {
  sideEffect: 'none' as const
};

const COMMON_RUNTIME_INPUTS = [
  { key: 'input_payload', title: 'Input Payload', valueType: 'json' }
];

const DEFAULT_CODE_SOURCE = `function main({arg1, arg2}) {
   const param=arg1 + arg2
    console.log(param)

    return {
        result: param
    }
}`;

function cloneJsonValue<T>(value: T): T {
  return structuredClone(value);
}

function duplicateOutput(
  output: FlowNodeOutputDocument
): FlowNodeOutputDocument {
  return { ...output };
}

function duplicateOutputs(outputs: FlowNodeOutputDocument[]) {
  return outputs.map(duplicateOutput);
}

function duplicateContractDefaults(
  contract: NodeRuntimeUiContract['defaults']
) {
  return {
    ...contract,
    config: cloneJsonValue(contract.config),
    bindings: cloneJsonValue(contract.bindings),
    outputs: duplicateOutputs(contract.outputs)
  };
}

function duplicateContract(
  contract: NodeRuntimeUiContract
): NodeRuntimeUiContract {
  return {
    ...contract,
    meta: { ...contract.meta },
    defaults: duplicateContractDefaults(contract.defaults),
    ports: {
      inputs: contract.ports.inputs.map((port) => ({ ...port })),
      outputs: contract.ports.outputs.map((port) => ({ ...port }))
    },
    card: { ...contract.card },
    panel: {
      sections: cloneJsonValue(contract.panel.sections)
    },
    runtime: {
      inputs: contract.runtime.inputs?.map((item) => ({ ...item })),
      processData: contract.runtime.processData?.map((item) => ({ ...item })),
      outputs: duplicateOutputs(contract.runtime.outputs)
    },
    policies: cloneJsonValue(contract.policies)
  };
}

function createContractDefaults({
  alias,
  description,
  config,
  bindings,
  outputs
}: {
  alias: string;
  description: string;
  config: Record<string, unknown>;
  bindings: Record<string, FlowBinding>;
  outputs: FlowNodeOutputDocument[];
}) {
  return {
    alias,
    description,
    configVersion: 1,
    config,
    bindings,
    outputs
  };
}

function createContractPorts(outputs: FlowNodeOutputDocument[]) {
  return {
    inputs: [],
    outputs: outputs.map((output) => ({
      key: output.key,
      title: output.title,
      valueType: output.valueType
    }))
  };
}

function branchPorts(
  branches: ReturnType<typeof createDefaultIfElseBranches>
): NodeRuntimePortDocument[] {
  return branches.map((branch) => ({
    key: branch.sourceHandle,
    title: branch.title
  }));
}

export function panelField({
  key,
  title,
  renderer,
  valueType = 'string',
  required,
  description,
  options,
  min,
  max,
  step,
  numberFormat
}: Omit<NodeRuntimePanelFieldDocument, 'valueType'> & {
  valueType?: string;
}) {
  return {
    key,
    title,
    renderer,
    valueType,
    required,
    description,
    options,
    min,
    max,
    step,
    numberFormat
  } satisfies NodeRuntimePanelFieldDocument;
}

export function panelSection(
  key: string,
  title: string,
  fields: NodeRuntimePanelFieldDocument[]
): NodeRuntimePanelSectionDocument {
  return {
    key,
    title,
    fields
  };
}

export const basicsPanelSection = panelSection('basics', 'Basics', [
  panelField({
    key: 'alias',
    title: i18nText('agentFlow', 'auto.node_alias'),
    renderer: 'text',
    required: true
  }),
  panelField({
    key: 'description',
    title: i18nText('agentFlow', 'auto.node_introduction'),
    renderer: 'text'
  })
]);

export function outputsPanelSection(outputs: FlowNodeOutputDocument[]) {
  return panelSection(
    'outputs',
    'Outputs',
    outputs.map((output) =>
      panelField({
        key: `outputs.${output.key}`,
        title: output.title,
        renderer: 'text',
        valueType: output.valueType,
        required: true
      })
    )
  );
}

export function createNodeRuntimeContract({
  type,
  title,
  alias,
  description,
  category,
  config,
  bindings = {},
  outputs,
  outputPorts,
  panelSections,
  runtimeOutputs,
  sideEffect = DEFAULT_RUNTIME_POLICY.sideEffect
}: {
  type: BuiltinNodeRuntimeContractType;
  title: string;
  alias?: string;
  description: string;
  category: ContractCategory;
  config: Record<string, unknown>;
  bindings?: Record<string, FlowBinding>;
  outputs: FlowNodeOutputDocument[];
  outputPorts?: NodeRuntimePortDocument[];
  panelSections: NodeRuntimePanelSectionDocument[];
  runtimeOutputs?: FlowNodeOutputDocument[];
  sideEffect?: NodeRuntimeSideEffectPolicy;
}): NodeRuntimeUiContract {
  return {
    meta: {
      type,
      title,
      schemaVersion: NODE_CONTRIBUTION_SCHEMA_VERSION
    },
    defaults: createContractDefaults({
      alias: alias ?? title,
      description,
      config,
      bindings,
      outputs: duplicateOutputs(outputs)
    }),
    ports: outputPorts
      ? { inputs: [], outputs: outputPorts }
      : createContractPorts(outputs),
    card: {
      title,
      description,
      category
    },
    panel: {
      sections: cloneJsonValue(panelSections)
    },
    runtime: {
      inputs: COMMON_RUNTIME_INPUTS.map((item) => ({ ...item })),
      outputs: duplicateOutputs(runtimeOutputs ?? outputs)
    },
    policies: { sideEffect }
  };
}

function createStartContract(): NodeRuntimeUiContract {
  return createNodeRuntimeContract({
    type: 'start',
    title: 'Start',
    description: i18nText('agentFlow', 'auto.workflow_entry'),
    category: 'io',
    config: {
      input_fields: [],
      model_list: []
    },
    outputs: [],
    panelSections: [
      basicsPanelSection,
      panelSection('inputs', i18nText('agentFlow', 'auto.input_field'), [
        panelField({
          key: 'config.input_fields',
          title: i18nText('agentFlow', 'auto.input_field'),
          renderer: 'start_input_fields',
          valueType: 'array'
        })
      ]),
      panelSection('advanced', i18nText('agentFlow', 'auto.model_list'), [
        panelField({
          key: 'config.model_list',
          title: i18nText('agentFlow', 'auto.model_list'),
          renderer: 'start_model_list',
          valueType: 'array'
        })
      ])
    ]
  });
}

function createLlmContract(): NodeRuntimeUiContract {
  const outputs = getLlmNodeOutputs({ response_format: { mode: 'text' } });

  return createNodeRuntimeContract({
    type: 'llm',
    title: 'LLM',
    description: i18nText(
      'agentFlow',
      'auto.call_language_model_generate_text'
    ),
    category: 'generation',
    config: {
      model_provider: {
        provider_code: '',
        model_id: ''
      },
      llm_parameters: {
        schema_version: '1.0.0',
        items: {}
      },
      context_policy: {
        integration_context: 'enabled',
        context_selector: ['node-start', 'history']
      },
      external_reasoning_policy: {
        follow_external_reasoning: false
      },
      external_model_parameter_policy: {
        follow_external_max_output_tokens: false
      },
      protocol_context: cloneJsonValue(DEFAULT_LLM_PROTOCOL_CONTEXT_REFERENCE),
      visible_internal_llm_tools_enabled: false,
      visible_internal_llm_tools: [],
      response_format: {
        mode: 'text'
      }
    },
    bindings: {
      prompt_messages: {
        kind: 'prompt_messages',
        value: [
          {
            id: 'system-1',
            role: 'system',
            content: { kind: 'templated_text', value: '' }
          }
        ]
      }
    },
    outputs,
    runtimeOutputs: outputs,
    panelSections: [
      basicsPanelSection,
      panelSection('inputs', 'Inputs', [
        panelField({
          key: 'config.model_provider',
          title: i18nText('agentFlow', 'auto.model'),
          renderer: 'llm_model',
          valueType: 'json',
          required: true
        }),
        panelField({
          key: 'config.visible_internal_llm_tools_enabled',
          title: i18nText('agentFlow', 'auto.mount_tools'),
          renderer: 'llm_tool_registrations',
          valueType: 'boolean'
        }),
        panelField({
          key: 'config.context_policy',
          title: '上下文',
          renderer: 'llm_context_policy',
          valueType: 'json'
        }),
        panelField({
          key: 'config.protocol_context',
          title: '协议上下文',
          renderer: 'selector',
          valueType: 'json'
        }),
        panelField({
          key: 'bindings.prompt_messages',
          title: i18nText('agentFlow', 'auto.context_alt'),
          renderer: 'llm_prompt_messages',
          valueType: 'array'
        })
      ]),
      outputsPanelSection(outputs),
      panelSection('advanced', 'Advanced', [
        panelField({
          key: 'config.response_format',
          title: i18nText('agentFlow', 'auto.return_format'),
          renderer: 'llm_response_format',
          valueType: 'json'
        })
      ])
    ]
  });
}

function createAnswerContract(): NodeRuntimeUiContract {
  const outputs = [
    {
      key: 'answer',
      title: i18nText('agentFlow', 'auto.dialog_output'),
      valueType: 'string'
    }
  ];

  return createNodeRuntimeContract({
    type: 'answer',
    title: 'Answer',
    description: i18nText('agentFlow', 'auto.returns_final_text_result_user'),
    category: 'io',
    config: {},
    outputs,
    panelSections: [
      basicsPanelSection,
      panelSection('inputs', 'Inputs', [
        panelField({
          key: 'bindings.answer_template',
          title: i18nText('agentFlow', 'auto.reply_content'),
          renderer: 'templated_text',
          required: true
        })
      ]),
      outputsPanelSection(outputs)
    ]
  });
}

function createKnowledgeRetrievalContract(): NodeRuntimeUiContract {
  const outputs = [
    {
      key: 'documents',
      title: i18nText('agentFlow', 'auto.knowledge_results'),
      valueType: 'array'
    }
  ];

  return createNodeRuntimeContract({
    type: 'knowledge_retrieval',
    title: 'Knowledge Retrieval',
    description: i18nText(
      'agentFlow',
      'auto.retrieve_knowledge_base_based_input_question_return_document_results'
    ),
    category: 'generation',
    config: { top_k: 4 },
    outputs,
    panelSections: [
      basicsPanelSection,
      panelSection('inputs', 'Inputs', [
        panelField({
          key: 'bindings.query',
          title: i18nText('agentFlow', 'auto.search_questions'),
          renderer: 'selector',
          required: true
        })
      ]),
      outputsPanelSection(outputs),
      panelSection('policy', 'Policy', [
        panelField({
          key: 'config.top_k',
          title: 'Top K',
          renderer: 'number',
          valueType: 'number'
        })
      ])
    ]
  });
}

function createQuestionClassifierContract(): NodeRuntimeUiContract {
  const outputs = [
    {
      key: 'label',
      title: i18nText('agentFlow', 'auto.classification_tags'),
      valueType: 'string'
    }
  ];

  return createNodeRuntimeContract({
    type: 'question_classifier',
    title: 'Question Classifier',
    description: i18nText(
      'agentFlow',
      'auto.classify_question_output_hit_labels'
    ),
    category: 'control',
    config: { classes: [] },
    outputs,
    panelSections: [
      basicsPanelSection,
      panelSection('inputs', 'Inputs', [
        panelField({
          key: 'bindings.question',
          title: i18nText('agentFlow', 'auto.questions_classified'),
          renderer: 'selector',
          required: true
        })
      ]),
      outputsPanelSection(outputs)
    ]
  });
}

function createIfElseContract(): NodeRuntimeUiContract {
  const branches = createDefaultIfElseBranches();

  return createNodeRuntimeContract({
    type: 'if_else',
    title: 'If / Else',
    description: i18nText(
      'agentFlow',
      'auto.select_paths_based_conditional_judgment'
    ),
    category: 'control',
    config: {},
    bindings: {
      branches: {
        kind: 'if_else_branches',
        value: { branches }
      }
    },
    outputs: [],
    outputPorts: branchPorts(branches),
    panelSections: [
      basicsPanelSection,
      panelSection('inputs', 'Inputs', [
        panelField({
          key: 'bindings.branches',
          title: i18nText('agentFlow', 'auto.branches'),
          renderer: 'if_else_branches',
          valueType: 'json',
          required: true
        })
      ]),
      outputsPanelSection([])
    ]
  });
}

function createCodeContract(): NodeRuntimeUiContract {
  const outputs = [
    normalizeCodeOutput({ key: 'result', title: 'result', valueType: 'string' })
  ];

  return createNodeRuntimeContract({
    type: 'code',
    title: 'Code',
    description: i18nText(
      'agentFlow',
      'auto.execute_custom_code_return_structured_results'
    ),
    category: 'data',
    config: { language: 'javascript', source: DEFAULT_CODE_SOURCE },
    bindings: {
      named_bindings: {
        kind: 'named_bindings',
        value: [
          {
            name: 'arg1',
            valueType: 'string',
            value: { kind: 'constant', value: '' }
          },
          {
            name: 'arg2',
            valueType: 'string',
            value: { kind: 'constant', value: '' }
          }
        ]
      }
    },
    outputs,
    panelSections: [
      basicsPanelSection,
      panelSection('inputs', 'Inputs', [
        panelField({
          key: 'bindings.named_bindings',
          title: i18nText('agentFlow', 'auto.input_variables'),
          renderer: 'templated_named_bindings',
          valueType: 'json'
        })
      ]),
      panelSection('code', 'JavaScript', [
        panelField({
          key: 'config.source',
          title: i18nText('agentFlow', 'auto.javascript_code'),
          renderer: 'code_source',
          valueType: 'string',
          required: true
        })
      ]),
      panelSection('outputs', i18nText('agentFlow', 'auto.output_variable'), [
        panelField({
          key: 'config.output_contract',
          title: i18nText('agentFlow', 'auto.output_variable'),
          renderer: 'output_contract_definition',
          valueType: 'array'
        })
      ])
    ]
  });
}

function createSqlContract(): NodeRuntimeUiContract {
  const outputs = [
    {
      key: 'results',
      title: i18nText('agentFlow', 'auto.sql_results'),
      valueType: 'array'
    }
  ];

  return createNodeRuntimeContract({
    type: 'sql',
    title: 'SQL',
    description: i18nText('agentFlow', 'auto.execute_native_sql'),
    category: 'data',
    config: { data_source_instance_id: 'main' },
    bindings: {
      sql: { kind: 'templated_text', value: '' }
    },
    outputs,
    sideEffect: 'external_write',
    panelSections: [
      basicsPanelSection,
      panelSection('inputs', 'Inputs', [
        panelField({
          key: 'config.data_source_instance_id',
          title: i18nText('agentFlow', 'auto.data_source'),
          renderer: 'data_source',
          required: true
        })
      ]),
      panelSection('advanced', 'SQL', [
        panelField({
          key: 'bindings.sql',
          title: i18nText('agentFlow', 'auto.sql_statement'),
          renderer: 'sql_source',
          valueType: 'string',
          required: true
        })
      ]),
      outputsPanelSection(outputs)
    ]
  });
}

function createTemplateTransformContract(): NodeRuntimeUiContract {
  const outputs = [
    {
      key: 'text',
      title: i18nText('agentFlow', 'auto.conversion_result'),
      valueType: 'string'
    }
  ];

  return createNodeRuntimeContract({
    type: 'template_transform',
    title: 'Template Transform',
    description: i18nText('agentFlow', 'auto.template_output_conversion'),
    category: 'generation',
    config: { template: '' },
    outputs,
    panelSections: [
      basicsPanelSection,
      panelSection('inputs', 'Inputs', [
        panelField({
          key: 'bindings.template',
          title: i18nText('agentFlow', 'auto.template'),
          renderer: 'templated_text',
          required: true
        })
      ]),
      outputsPanelSection(outputs)
    ]
  });
}

function createHttpRequestContract(): NodeRuntimeUiContract {
  const outputs = HTTP_REQUEST_OUTPUTS;

  return createNodeRuntimeContract({
    type: 'http_request',
    title: 'HTTP Request',
    alias: 'HTTP',
    description: i18nText('agentFlow', 'auto.request_external_http_service'),
    category: 'external',
    config: HTTP_REQUEST_DEFAULT_CONFIG,
    bindings: HTTP_REQUEST_DEFAULT_BINDINGS,
    outputs,
    panelSections: [
      basicsPanelSection,
      panelSection('inputs', 'Inputs', [
        panelField({
          key: 'config.url',
          title: 'URL',
          renderer: 'http_request_endpoint',
          required: true
        }),
        panelField({
          key: 'bindings.params',
          title: i18nText('agentFlow', 'auto.request_params'),
          renderer: 'http_request_key_values',
          valueType: 'array'
        }),
        panelField({
          key: 'bindings.headers',
          title: i18nText('agentFlow', 'auto.request_headers'),
          renderer: 'http_request_key_values',
          valueType: 'array'
        }),
        panelField({
          key: 'config.body_type',
          title: i18nText('agentFlow', 'auto.request_body_section'),
          renderer: 'http_request_body',
          valueType: 'string',
          options: [...HTTP_REQUEST_BODY_TYPE_OPTIONS]
        }),
        panelField({
          key: 'config.verify_ssl',
          title: i18nText('agentFlow', 'auto.verify_ssl_certificate'),
          renderer: 'switch',
          valueType: 'boolean'
        }),
        panelField({
          key: 'config.store_response_as_file',
          title: i18nText('agentFlow', 'auto.store_response_as_file'),
          renderer: 'switch',
          valueType: 'boolean'
        }),
        panelField({
          key: 'config.timeout_ms',
          title: i18nText('agentFlow', 'auto.timeout_settings'),
          renderer: 'number',
          valueType: 'number'
        }),
        panelField({
          key: 'config.max_response_bytes',
          title: i18nText('agentFlow', 'auto.max_response_size'),
          renderer: 'number',
          valueType: 'number',
          min: HTTP_REQUEST_RESPONSE_BYTES_STEP,
          max: HTTP_REQUEST_MAX_RESPONSE_BYTES,
          step: HTTP_REQUEST_RESPONSE_BYTES_STEP,
          numberFormat: 'bytes_as_mib'
        }),
        panelField({
          key: 'config.curl_import',
          title: i18nText('agentFlow', 'auto.import_curl'),
          renderer: 'http_request_curl_import',
          valueType: 'string'
        })
      ]),
      outputsPanelSection(outputs)
    ]
  });
}

function createToolContract(): NodeRuntimeUiContract {
  const outputs = [
    {
      key: 'result',
      title: i18nText('agentFlow', 'auto.tool_output'),
      valueType: 'unknown'
    }
  ];

  return createNodeRuntimeContract({
    type: 'tool',
    title: 'Tool',
    description: i18nText('agentFlow', 'auto.call_connected_tool_capabilities'),
    category: 'external',
    config: { tool_name: '' },
    outputs,
    panelSections: [
      basicsPanelSection,
      panelSection('inputs', 'Inputs', [
        panelField({
          key: 'config.tool_name',
          title: i18nText('agentFlow', 'auto.tool_name'),
          renderer: 'text',
          required: true
        }),
        panelField({
          key: 'bindings.parameters',
          title: i18nText('agentFlow', 'auto.tool_input_parameters'),
          renderer: 'named_bindings',
          valueType: 'json'
        })
      ]),
      outputsPanelSection(outputs)
    ]
  });
}

function createToolResultContract(): NodeRuntimeUiContract {
  const outputs = [
    {
      key: 'result',
      title: i18nText('agentFlow', 'auto.tool_result_output'),
      valueType: 'string'
    }
  ];

  return createNodeRuntimeContract({
    type: 'tool_result',
    title: 'Tool Result',
    description: i18nText('agentFlow', 'auto.define_tool_call_return_content'),
    category: 'io',
    config: {},
    outputs,
    panelSections: [
      basicsPanelSection,
      panelSection('inputs', 'Inputs', [
        panelField({
          key: 'bindings.result_template',
          title: i18nText('agentFlow', 'auto.tool_result_content'),
          renderer: 'templated_text',
          required: true
        })
      ]),
      outputsPanelSection(outputs)
    ]
  });
}

function createPluginNodeContract(): NodeRuntimeUiContract {
  const outputs = [
    {
      key: 'result',
      title: i18nText('agentFlow', 'auto.node_output'),
      valueType: 'json'
    }
  ];

  return createNodeRuntimeContract({
    type: 'plugin_node',
    title: 'Plugin Node',
    description: i18nText(
      'agentFlow',
      'auto.declarative_node_placeholders_plugins'
    ),
    category: 'external',
    config: {},
    outputs,
    panelSections: [basicsPanelSection, outputsPanelSection(outputs)]
  });
}

function createVariableAssignerContract(): NodeRuntimeUiContract {
  const outputs: FlowNodeOutputDocument[] = [];

  return createNodeRuntimeContract({
    type: 'variable_assigner',
    title: '变量赋值',
    description: i18nText(
      'agentFlow',
      'auto.assign_conversation_variable_current_run'
    ),
    category: 'data',
    config: {},
    bindings: {
      operations: {
        kind: 'state_write',
        value: []
      }
    },
    outputs,
    panelSections: [
      basicsPanelSection,
      panelSection('inputs', 'Inputs', [
        panelField({
          key: 'bindings.operations',
          title: i18nText('agentFlow', 'auto.variable_alt'),
          renderer: 'variable_assignment',
          valueType: 'array',
          required: true
        })
      ]),
      outputsPanelSection(outputs)
    ]
  });
}

function createParameterExtractorContract(): NodeRuntimeUiContract {
  const outputs = [
    {
      key: 'parameters',
      title: i18nText('agentFlow', 'auto.extract_parameters'),
      valueType: 'json'
    }
  ];

  return createNodeRuntimeContract({
    type: 'parameter_extractor',
    title: 'Parameter Extractor',
    description: i18nText(
      'agentFlow',
      'auto.extract_structured_parameter_results_text'
    ),
    category: 'data',
    config: { schema: [] },
    outputs,
    panelSections: [
      basicsPanelSection,
      panelSection('inputs', 'Inputs', [
        panelField({
          key: 'bindings.source_text',
          title: i18nText('agentFlow', 'auto.source_text'),
          renderer: 'selector',
          required: true
        })
      ]),
      outputsPanelSection(outputs)
    ]
  });
}

function createIterationContract(): NodeRuntimeUiContract {
  const outputs = [
    {
      key: 'result',
      title: i18nText('agentFlow', 'auto.aggregate_output'),
      valueType: 'array'
    }
  ];

  return createNodeRuntimeContract({
    type: 'iteration',
    title: 'Iteration',
    description: i18nText(
      'agentFlow',
      'auto.iterate_through_list_process_each_item'
    ),
    category: 'control',
    config: { max_steps: 10 },
    outputs,
    panelSections: [
      basicsPanelSection,
      panelSection('inputs', 'Inputs', [
        panelField({
          key: 'bindings.items',
          title: i18nText('agentFlow', 'auto.circular_list'),
          renderer: 'selector',
          required: true
        })
      ]),
      outputsPanelSection(outputs)
    ]
  });
}

function createLoopContract(): NodeRuntimeUiContract {
  const outputs = [
    {
      key: 'result',
      title: i18nText('agentFlow', 'auto.aggregate_output'),
      valueType: 'array'
    }
  ];

  return createNodeRuntimeContract({
    type: 'loop',
    title: 'Loop',
    description: i18nText(
      'agentFlow',
      'auto.conditionally_execute_node_repeatedly'
    ),
    category: 'control',
    config: { max_rounds: 10 },
    outputs,
    panelSections: [
      basicsPanelSection,
      panelSection('inputs', 'Inputs', [
        panelField({
          key: 'bindings.entry_condition',
          title: i18nText('agentFlow', 'auto.entry_conditions'),
          renderer: 'condition_group',
          valueType: 'json',
          required: true
        })
      ]),
      panelSection('policy', 'Policy', [
        panelField({
          key: 'config.max_rounds',
          title: i18nText('agentFlow', 'auto.maximum_number_rounds'),
          renderer: 'number',
          valueType: 'number'
        })
      ]),
      outputsPanelSection(outputs)
    ]
  });
}

function createHumanInputContract(): NodeRuntimeUiContract {
  const outputs = [
    {
      key: 'input',
      title: i18nText('agentFlow', 'auto.manual_input'),
      valueType: 'string'
    }
  ];

  return createNodeRuntimeContract({
    type: 'human_input',
    title: 'Human Input',
    description: i18nText('agentFlow', 'auto.waiting_manual_input'),
    category: 'io',
    config: {},
    outputs,
    panelSections: [
      basicsPanelSection,
      panelSection('inputs', 'Inputs', [
        panelField({
          key: 'config.prompt',
          title: i18nText('agentFlow', 'auto.waiting_for_questions'),
          renderer: 'templated_text',
          required: true
        })
      ]),
      outputsPanelSection(outputs)
    ]
  });
}

type BuiltinDataModelRuntimeContractType =
  | 'data_model_list'
  | 'data_model_get'
  | 'data_model_create'
  | 'data_model_update'
  | 'data_model_delete';

const DATA_MODEL_NODE_TITLES = {
  data_model_list: 'Data Model List',
  data_model_get: 'Data Model Get',
  data_model_create: 'Data Model Create',
  data_model_update: 'Data Model Update',
  data_model_delete: 'Data Model Delete'
} satisfies Record<BuiltinDataModelRuntimeContractType, string>;

function createDataModelContract(
  nodeType: BuiltinDataModelRuntimeContractType
): NodeRuntimeUiContract {
  const action = getDataModelActionForNodeType(nodeType);

  if (!action) {
    throw new Error(`Unsupported Data Model node type: ${nodeType}`);
  }

  const outputs = getDataModelNodeOutputs(action);
  const inputFields: NodeRuntimePanelFieldDocument[] = [
    panelField({
      key: 'config.data_model_code',
      title: 'Data Model',
      renderer: 'data_model',
      required: true
    })
  ];

  if (action === 'list') {
    inputFields.push(
      panelField({
        key: 'bindings.query',
        title: 'Query',
        renderer: 'data_model_query',
        valueType: 'json'
      })
    );
  }

  if (action === 'get' || action === 'update' || action === 'delete') {
    inputFields.push(
      panelField({
        key: 'bindings.record_id',
        title: 'Record ID',
        renderer: 'selector',
        required: true
      })
    );
  }

  if (action === 'create' || action === 'update') {
    inputFields.push(
      panelField({
        key: 'bindings.payload',
        title: 'Payload',
        renderer: 'named_bindings',
        valueType: 'json',
        required: true
      })
    );
  }

  if (action === 'create' || action === 'update' || action === 'delete') {
    inputFields.push(
      panelField({
        key: 'config.side_effect_policy',
        title: 'Side Effect Policy',
        renderer: 'static_select',
        required: true,
        options: [
          { label: 'Disabled', value: 'disabled' },
          { label: 'Confirm Each Run', value: 'confirm_each_run' },
          { label: 'Allow With Idempotency', value: 'allow_with_idempotency' }
        ]
      })
    );
  }

  return createNodeRuntimeContract({
    type: nodeType,
    title: DATA_MODEL_NODE_TITLES[nodeType],
    description: i18nText('agentFlow', 'auto.data_model_operation_node'),
    category: 'data',
    config: getDataModelNodeDefaultConfig(nodeType),
    outputs,
    panelSections: [
      basicsPanelSection,
      panelSection('inputs', 'Inputs', inputFields),
      outputsPanelSection(outputs)
    ]
  });
}

export const builtinNodeRuntimeContractTypes = [
  'start',
  'answer',
  'workflow_start',
  'workflow_end',
  'llm',
  'knowledge_retrieval',
  'question_classifier',
  'if_else',
  'code',
  'template_transform',
  'http_request',
  'tool',
  'tool_result',
  'data_model_list',
  'data_model_get',
  'data_model_create',
  'data_model_update',
  'data_model_delete',
  'sql',
  'variable_assigner',
  'parameter_extractor',
  'iteration',
  'loop',
  'human_input',
  'plugin_node'
] as const;

export type StaticBuiltinNodeRuntimeContractType = Exclude<
  BuiltinNodeRuntimeContractType,
  'workflow_start' | 'workflow_end'
>;

export const BUILTIN_NODE_RUNTIME_CONTRACTS: Record<
  StaticBuiltinNodeRuntimeContractType,
  NodeRuntimeUiContract
> = {
  start: createStartContract(),
  answer: createAnswerContract(),
  llm: createLlmContract(),
  knowledge_retrieval: createKnowledgeRetrievalContract(),
  question_classifier: createQuestionClassifierContract(),
  if_else: createIfElseContract(),
  code: createCodeContract(),
  template_transform: createTemplateTransformContract(),
  http_request: createHttpRequestContract(),
  tool: createToolContract(),
  tool_result: createToolResultContract(),
  data_model_list: createDataModelContract('data_model_list'),
  data_model_get: createDataModelContract('data_model_get'),
  data_model_create: createDataModelContract('data_model_create'),
  data_model_update: createDataModelContract('data_model_update'),
  data_model_delete: createDataModelContract('data_model_delete'),
  sql: createSqlContract(),
  variable_assigner: createVariableAssignerContract(),
  parameter_extractor: createParameterExtractorContract(),
  iteration: createIterationContract(),
  loop: createLoopContract(),
  human_input: createHumanInputContract(),
  plugin_node: createPluginNodeContract()
};

registerNodeRuntimeContracts(Object.values(BUILTIN_NODE_RUNTIME_CONTRACTS));

export function getBuiltinNodeRuntimeContract(
  nodeType: FlowNodeType
): NodeRuntimeUiContract | null {
  const contract = getNodeRuntimeContract(nodeType);

  return contract ? duplicateContract(contract) : null;
}

export type { BuiltinNodeRuntimeContractType };
