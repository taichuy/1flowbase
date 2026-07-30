import { basicFields } from '../base';
import type { NodeDefinition } from '../types';
import { i18nText } from '../../../../../shared/i18n/text';

export const startNodeDefinition: NodeDefinition = {
  label: 'Start',
  sections: [
    {
      key: 'basics',
      title: 'Basics',
      fields: basicFields
    },
    {
      key: 'inputs',
      title: i18nText('agentFlow', 'auto.input_field'),
      fields: [
        {
          key: 'config.input_fields',
          label: i18nText('agentFlow', 'auto.input_field'),
          editor: 'start_input_fields'
        }
      ]
    },
    {
      key: 'advanced',
      title: i18nText('agentFlow', 'auto.model_list'),
      fields: [
        {
          key: 'config.model_list',
          label: i18nText('agentFlow', 'auto.model_list'),
          editor: 'start_model_list'
        }
      ]
    }
  ]
};
