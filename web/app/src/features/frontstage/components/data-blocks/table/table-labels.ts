import { i18nText } from '../../../../../shared/i18n/text';
import type { DataBlockAction } from '../../../lib/data-blocks/types';

export function actionTitle(action: DataBlockAction): string {
  if (action.title) return action.title;
  switch (action.kind) {
    case 'filter':
      return i18nText('frontstage', 'data_blocks.table.filter');
    case 'create':
      return i18nText('frontstage', 'data_blocks.table.create');
    case 'delete':
      return i18nText('frontstage', 'data_blocks.table.delete');
    case 'refresh':
      return i18nText('frontstage', 'data_blocks.table.refresh');
    case 'view':
      return i18nText('frontstage', 'data_blocks.table.view');
    case 'edit':
      return i18nText('frontstage', 'data_blocks.table.edit');
    case 'open':
      return i18nText('frontstage', 'data_blocks.table.open');
  }
}

export function presentationOptions() {
  return [
    {
      value: 'drawer',
      label: i18nText('frontstage', 'data_blocks.table.drawer')
    },
    {
      value: 'modal',
      label: i18nText('frontstage', 'data_blocks.table.modal')
    },
    { value: 'page', label: i18nText('frontstage', 'data_blocks.table.page') }
  ];
}
export function sizeOptions() {
  return [
    {
      value: 'small',
      label: i18nText('frontstage', 'data_blocks.table.small')
    },
    {
      value: 'middle',
      label: i18nText('frontstage', 'data_blocks.table.middle')
    },
    { value: 'large', label: i18nText('frontstage', 'data_blocks.table.large') }
  ];
}
