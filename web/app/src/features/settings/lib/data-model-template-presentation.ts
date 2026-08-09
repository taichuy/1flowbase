import type { SettingsCompatibleDataModelTemplate } from '../api/data-models';
import { i18nText } from '../../../shared/i18n/text';

export function dataModelTemplateIdentity(
  template: SettingsCompatibleDataModelTemplate
) {
  return `${template.template_provider}/${template.template_code}/${template.template_version}`;
}

export function dataModelTemplatePresentation(
  template: SettingsCompatibleDataModelTemplate
) {
  const identity = dataModelTemplateIdentity(template);
  if (identity === 'core/general/v1') {
    return {
      title: i18nText('settings', 'auto.general_data_model_template'),
      description: i18nText(
        'settings',
        'auto.general_data_model_template_description'
      )
    };
  }
  if (identity === 'core/ordered_tree/v1') {
    return {
      title: i18nText('settings', 'auto.ordered_tree_data_model_template'),
      description: i18nText(
        'settings',
        'auto.ordered_tree_data_model_template_description'
      )
    };
  }
  return { title: template.summary, description: template.description };
}
