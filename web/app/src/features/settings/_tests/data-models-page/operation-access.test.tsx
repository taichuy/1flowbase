import { fireEvent, screen, within } from '@testing-library/react';
import { describe, expect, test } from 'vitest';
import './data-models-page.fixtures';
import { useAuthStore } from '../../../../state/auth-store';
import { dataModelsApi, renderApp, SLOW_SETTINGS_PAGE_TEST_TIMEOUT } from './support';

const reads = ['settings_feature.access.system.data-models', 'data_sources.list', 'data_sources.view', 'data_sources.preview', 'model_definitions.list', 'model_templates.list', 'model_scope_grants.list', 'model_definitions.openapi.view', 'model_definitions.advisor.view'];
function grant(operations: string[] = []) {
  const state = useAuthStore.getState();
  useAuthStore.setState({ actor: { ...state.actor!, effective_display_role: 'readonly.audit.20261102' }, me: { ...state.me!, effective_display_role: 'readonly.audit.20261102', permissions: [...reads, ...operations] } });
}

describe('Data model exact operation controls', () => {
  test('readonly can preview external resource rows while discovery, validation and mapping remain disabled', async () => {
    grant();
    renderApp('/settings/data-models?source=source-1');
    const preview = await screen.findByRole('button', { name: /预览/ }, { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT });
    expect(preview).toBeEnabled();
    expect(screen.getByRole('button', { name: /发现资源/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: /验证数据源/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: /映射/ })).toBeDisabled();
    fireEvent.click(preview);
    expect(await screen.findByText('Contacts 预览')).toBeInTheDocument();
    expect(dataModelsApi.previewSettingsDataSourceResource).toHaveBeenCalled();
    expect(dataModelsApi.discoverSettingsDataSourceResources).not.toHaveBeenCalled();
    expect(dataModelsApi.mapSettingsDataSourceResourceToModel).not.toHaveBeenCalled();
  });

  test('readonly retains model and field detail access without model or field updates', async () => {
    grant();
    renderApp('/settings/data-models?source=source-1');
    await screen.findAllByText('Contacts', {}, { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT });
    const modelRow = screen.getAllByRole('row').find((row) => within(row).queryByText('Contacts') && within(row).queryByRole('button', { name: '查看' }))!;
    fireEvent.click(within(modelRow).getByRole('button', { name: '查看' }));
    const detail = await screen.findByRole('region', { name: 'Data Model 详情' });
    expect(within(detail).getByRole('button', { name: /新增字段/ })).toBeDisabled();
    fireEvent.click(within(detail).getByRole('button', { name: 'Email' }));
    expect(await screen.findByRole('button', { name: '保存字段' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '删除字段' })).toBeDisabled();
    expect(dataModelsApi.updateSettingsDataModelField).not.toHaveBeenCalled();
    expect(dataModelsApi.updateSettingsDataModel).not.toHaveBeenCalled();
  });

  test('one model-create grant does not enable source defaults', async () => {
    grant(['model_definitions.create']);
    renderApp('/settings/data-models?source=main');
    expect(await screen.findByRole('button', { name: '新建数据表' }, { timeout: SLOW_SETTINGS_PAGE_TEST_TIMEOUT })).toBeEnabled();
    expect(screen.getByRole('switch', { name: '新建默认开放 API' })).toBeDisabled();
  });
});
