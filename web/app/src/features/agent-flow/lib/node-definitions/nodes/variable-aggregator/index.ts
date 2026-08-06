import { basicFields } from '../../base';
import type { NodeDefinition } from '../../types';
import { i18nText } from '../../../../../../shared/i18n/text';

export const variableAggregatorNodeDefinition: NodeDefinition = {
  label: i18nText('agentFlow', 'auto.variable_aggregator'),
  sections: [
    {
      key: 'basics',
      title: i18nText('agentFlow', 'auto.basic_information'),
      fields: basicFields
    },
    {
      key: 'inputs',
      title: i18nText('agentFlow', 'auto.input'),
      fields: [
        {
          key: 'bindings.groups',
          label: i18nText('agentFlow', 'auto.variable_aggregator_candidates'),
          editor: 'variable_groups',
          required: true
        }
      ]
    },
    {
      key: 'outputs',
      title: i18nText('agentFlow', 'auto.outputs'),
      fields: []
    }
  ]
};
