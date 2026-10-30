import { basicFields } from '../base';
import type { NodeDefinition } from '../types';
import { i18nText } from '../../../../../shared/i18n/text';

export const loopNodeDefinition: NodeDefinition = {
  label: 'Loop',
  sections: [
    {
      key: 'basics',
      title: 'Basics',
      fields: basicFields
    },
    {
      key: 'inputs',
      title: 'Inputs',
      fields: [
        {
          key: 'bindings.break_condition',
          label: i18nText('agentFlow', 'containers.break_condition'),
          editor: 'condition_group',
          required: false
        }
      ]
    },
    {
      key: 'policy',
      title: 'Policy',
      fields: [
        {
          key: 'config.round_timeout_ms',
          label: i18nText('agentFlow', 'containers.round_timeout_ms'),
          editor: 'number',
          required: true
        },
        {
          key: 'config.max_rounds',
          label: i18nText('agentFlow', 'auto.maximum_number_rounds'),
          editor: 'number'
        }
      ]
    },
    {
      key: 'outputs',
      title: 'Outputs',
      fields: [
        {
          key: 'outputs.result',
          label: i18nText('agentFlow', 'auto.aggregate_output'),
          editor: 'text',
          required: true
        }
      ]
    }
  ]
};
