import { i18nText } from '../../../../../../shared/i18n/text';
import { basicFields } from '../../base';
import type { NodeDefinition } from '../../types';

export const sqlNodeDefinition: NodeDefinition = {
  label: 'SQL',
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
          key: 'config.data_source_instance_id',
          label: i18nText('agentFlow', 'auto.data_source'),
          editor: 'data_source',
          required: true
        }
      ]
    },
    {
      key: 'advanced',
      title: 'SQL',
      fields: [
        {
          key: 'bindings.sql',
          label: i18nText('agentFlow', 'auto.sql_statement'),
          editor: 'sql_source',
          required: true
        }
      ]
    },
    {
      key: 'outputs',
      title: 'Outputs',
      fields: []
    }
  ]
};
