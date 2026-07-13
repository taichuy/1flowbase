import { describe, expect, test, vi } from 'vitest';

vi.mock('@1flowbase/api-client', () => ({
  batchDeleteConsoleDataModels: vi.fn().mockResolvedValue({
    deleted: true,
    deleted_count: 2,
    deleted_ids: ['model-1', 'model-2']
  }),
  createConsoleDataModel: vi.fn().mockResolvedValue({ id: 'model-1' }),
  createConsoleDataModelField: vi.fn().mockResolvedValue({ id: 'field-1' }),
  createConsoleDataModelScopeGrant: vi.fn().mockResolvedValue({
    id: 'grant-1'
  }),
  deleteConsoleDataModel: vi.fn().mockResolvedValue({ deleted: true }),
  deleteConsoleDataModelField: vi.fn().mockResolvedValue({ deleted: true }),
  fetchConsoleDataModelAdvisorFindings: vi.fn().mockResolvedValue([]),
  fetchConsoleDataModelOpenApiDocument: vi.fn().mockResolvedValue({
    openapi: '3.1.0'
  }),
  fetchConsoleDataModelRecordPreview: vi.fn().mockResolvedValue({
    records: []
  }),
  fetchConsoleDataModelScopeGrants: vi.fn().mockResolvedValue([]),
  fetchConsoleDataModels: vi.fn().mockResolvedValue([]),
  fetchConsoleMainDataSource: vi.fn().mockResolvedValue({ id: 'main_source' }),
  fetchConsoleDataSourceConnections: vi.fn().mockResolvedValue([]),
  fetchConsoleDataSourceCatalog: vi.fn().mockResolvedValue({ entries: [] }),
  fetchConsoleDataSourceResources: vi.fn().mockResolvedValue({ entries: [] }),
  createConsoleDataSourceConnection: vi.fn(),
  validateConsoleDataSourceConnection: vi.fn(),
  discoverConsoleDataSourceResources: vi.fn(),
  previewConsoleDataSourceResource: vi.fn(),
  mapConsoleDataSourceResourceToModel: vi.fn(),
  updateConsoleDataModel: vi.fn().mockResolvedValue({ id: 'model-1' }),
  updateConsoleDataModelField: vi.fn().mockResolvedValue({ id: 'field-1' }),
  updateConsoleDataModelScopeGrant: vi.fn().mockResolvedValue({
    id: 'grant-1'
  }),
  updateConsoleDataSourceConnectionDefaults: vi.fn().mockResolvedValue({
    id: 'source-1'
  }),
  updateConsoleMainDataSourceDefaults: vi.fn().mockResolvedValue({
    id: 'main_source'
  })
}));

import {
  batchDeleteConsoleDataModels,
  createConsoleDataModel,
  createConsoleDataModelField,
  createConsoleDataModelScopeGrant,
  deleteConsoleDataModel,
  deleteConsoleDataModelField,
  fetchConsoleDataModels,
  fetchConsoleDataSourceConnections,
  fetchConsoleMainDataSource,
  updateConsoleDataModel,
  updateConsoleDataModelField,
  updateConsoleDataSourceConnectionDefaults
} from '@1flowbase/api-client';
import {
  batchDeleteSettingsDataModels,
  createSettingsDataModel,
  createSettingsDataModelField,
  createSettingsDataModelScopeGrant,
  deleteSettingsDataModel,
  deleteSettingsDataModelField,
  fetchSettingsDataModels,
  fetchSettingsDataSourceConnections,
  fetchSettingsMainDataSource,
  settingsDataModelAdvisorFindingsQueryKey,
  settingsDataModelRecordPreviewQueryKey,
  settingsDataModelOpenApiQueryKey,
  settingsDataModelsQueryKey,
  settingsDataModelScopeGrantsQueryKey,
  settingsDataSourceConnectionsQueryKey,
  settingsMainDataSourceQueryKey,
  type SettingsDataSourceConnection,
  updateSettingsDataModel,
  updateSettingsDataModelField,
  updateSettingsDataSourceConnectionDefaults
} from '../data-models';

describe('settings data models API wrappers', () => {
  test('exports stable query keys for data sources, models, grants, preview, and Advisor', () => {
    expect(settingsMainDataSourceQueryKey).toEqual([
      'settings',
      'data-models',
      'main-source'
    ]);
    expect(settingsDataSourceConnectionsQueryKey).toEqual([
      'settings',
      'data-models',
      'connections'
    ]);
    expect(settingsDataModelsQueryKey('main_source')).toEqual([
      'settings',
      'data-models',
      'models',
      'main_source',
      '{}'
    ]);
    expect(settingsDataModelScopeGrantsQueryKey('model-1')).toEqual([
      'settings',
      'data-models',
      'scope-grants',
      'model-1'
    ]);
    expect(settingsDataModelRecordPreviewQueryKey('orders')).toEqual([
      'settings',
      'data-models',
      'record-preview',
      'orders'
    ]);
    expect(settingsDataModelAdvisorFindingsQueryKey('model-1')).toEqual([
      'settings',
      'data-models',
      'advisor',
      'model-1'
    ]);
    expect(settingsDataModelOpenApiQueryKey('model-1')).toEqual([
      'settings',
      'data-models',
      'openapi',
      'model-1'
    ]);
  });

  test('delegates source and model reads to the API client', async () => {
    await fetchSettingsMainDataSource();
    expect(fetchConsoleMainDataSource).toHaveBeenCalled();
    await fetchSettingsDataSourceConnections();
    expect(fetchConsoleDataSourceConnections).toHaveBeenCalled();

    const connection = {
      id: 'source-1',
      source_kind: 'external_source'
    } as SettingsDataSourceConnection;
    await fetchSettingsDataModels(connection);
    expect(fetchConsoleDataModels).toHaveBeenCalledWith({
      data_source_instance_id: 'source-1'
    });

    await fetchSettingsDataModels(connection, {
      code: { $includes: 'orders' }
    });
    expect(fetchConsoleDataModels).toHaveBeenCalledWith({
      data_source_instance_id: 'source-1',
      filter: { code: { $includes: 'orders' } }
    });
  });

  test('delegates mutations with CSRF tokens', async () => {
    await updateSettingsDataSourceConnectionDefaults(
      'source-1',
      {
        default_data_model_status: 'draft'
      },
      'csrf-123'
    );
    expect(updateConsoleDataSourceConnectionDefaults).toHaveBeenCalledWith(
      'source-1',
      {
        default_data_model_status: 'draft'
      },
      'csrf-123'
    );

    await createSettingsDataModel(
      {
        scope_kind: 'workspace',
        code: 'orders',
        title: 'Orders'
      },
      'csrf-123'
    );
    expect(createConsoleDataModel).toHaveBeenCalledWith(
      {
        scope_kind: 'workspace',
        code: 'orders',
        title: 'Orders'
      },
      'csrf-123'
    );

    await updateSettingsDataModel(
      'model-1',
      { status: 'published' },
      'csrf-123'
    );
    expect(updateConsoleDataModel).toHaveBeenCalledWith(
      'model-1',
      { status: 'published' },
      'csrf-123'
    );

    await deleteSettingsDataModel('model-1', 'csrf-123');
    expect(deleteConsoleDataModel).toHaveBeenCalledWith('model-1', 'csrf-123');

    await batchDeleteSettingsDataModels(
      { filterByTk: ['model-1', 'model-2'], confirmed: true },
      'csrf-123'
    );
    expect(batchDeleteConsoleDataModels).toHaveBeenCalledWith(
      { filterByTk: ['model-1', 'model-2'], confirmed: true },
      'csrf-123'
    );

    await createSettingsDataModelField(
      'model-1',
      {
        code: 'email',
        title: 'Email',
        field_kind: 'string',
        is_required: false,
        is_unique: false,
        default_value: null,
        display_interface: 'input',
        display_options: {},
        relation_target_model_id: null,
        relation_options: {}
      },
      'csrf-123'
    );
    expect(createConsoleDataModelField).toHaveBeenCalled();

    await updateSettingsDataModelField(
      'model-1',
      'field-1',
      {
        title: 'Primary Email',
        is_required: true,
        is_unique: true,
        default_value: null,
        display_interface: 'input',
        display_options: {},
        relation_options: {}
      },
      'csrf-123'
    );
    expect(updateConsoleDataModelField).toHaveBeenCalledWith(
      'model-1',
      'field-1',
      {
        title: 'Primary Email',
        is_required: true,
        is_unique: true,
        default_value: null,
        display_interface: 'input',
        display_options: {},
        relation_options: {}
      },
      'csrf-123'
    );

    await deleteSettingsDataModelField('model-1', 'field-1', 'csrf-123');
    expect(deleteConsoleDataModelField).toHaveBeenCalledWith(
      'model-1',
      'field-1',
      'csrf-123'
    );

    await createSettingsDataModelScopeGrant(
      'model-1',
      {
        scope_kind: 'system',
        scope_id: '00000000-0000-0000-0000-000000000000',
        enabled: true,
        permission_profile: 'system_all',
        confirm_unsafe_external_source_system_all: true
      },
      'csrf-123'
    );
    expect(createConsoleDataModelScopeGrant).toHaveBeenCalled();
  });
});
