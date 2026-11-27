import { fireEvent, render, screen } from '@testing-library/react';
import { expect, test, vi } from 'vitest';
import { i18nText } from '../../../../shared/i18n/text';
import { DataModelPermissionsTab } from '../../components/data-models/DataModelPermissionsTab';
import './data-models-page.fixtures';

test('system_all confirmation is local to the current grant snapshot', () => {
  const grant = {
    id: 'id',
    data_model_id: 'model',
    scope_kind: 'system',
    scope_id: 'system',
    permission_profile: 'system_all',
    enabled: true
  } as const;
  const save = vi.fn();
  const editor = () => (
    <DataModelPermissionsTab
      grants={[grant]}
      loading={false}
      saving={false}
      onSave={save}
    />
  );
  const view = render(editor());
  const confirm = () =>
    screen.getByRole('checkbox', {
      name: i18nText('settings', 'auto.confirm_system_all')
    });
  expect(confirm()).not.toBeChecked();
  fireEvent.click(confirm());
  expect(confirm()).toBeChecked();
  expect(save).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole('button', {
      name: i18nText('settings', 'auto.save_permissions')
    })
  );
  expect(save).toHaveBeenCalledExactlyOnceWith(grant, {
    enabled: true,
    permission_profile: 'system_all',
    confirm_unsafe_external_source_system_all: true
  });
  view.rerender(editor());
  expect(confirm()).not.toBeChecked();
});
