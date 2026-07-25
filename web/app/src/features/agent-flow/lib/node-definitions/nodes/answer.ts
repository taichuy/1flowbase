import { basicFields } from '../base';
import type { NodeDefinition } from '../types';
import { i18nText } from '../../../../../shared/i18n/text';

export const answerNodeDefinition: NodeDefinition = {
  label: 'Answer',
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
          key: 'bindings.answer_template',
          label: i18nText("agentFlow", "auto.reply_content"),
          editor: 'templated_text',
          required: true
        }
      ]
    },
    {
      key: 'outputs',
      title: 'Outputs',
      fields: [
        {
          key: 'outputs.answer',
          label: i18nText("agentFlow", "auto.dialog_output"),
          editor: 'text',
          required: true
        }
      ]
    }
  ]
};
