import { describe, expect, test, vi } from 'vitest';

vi.mock('@1flowbase/api-client', () => ({
  batchDeleteConsoleDataModels: vi.fn(),
  createConsoleDataModel: vi.fn(),
  createConsoleDataModelField: vi.fn(),
  createConsoleDataModelScopeGrant: vi.fn(),
  deleteConsoleDataModel: vi.fn(),
  deleteConsoleDataModelField: vi.fn(),
  fetchConsoleDataModelAdvisorFindings: vi.fn(),
  fetchConsoleDataModelOpenApiDocument: vi.fn(),
  fetchConsoleDataModelRecordPreview: vi.fn(),
  fetchConsoleDataModelScopeGrants: vi.fn(),
  fetchConsoleCompatibleDataModelTemplates: vi.fn().mockResolvedValue([]),
  fetchConsoleDataModels: vi.fn().mockResolvedValue([]),
  fetchConsoleDataSources: vi.fn().mockResolvedValue([]),
  fetchConsoleDataSourceCatalog: vi.fn().mockResolvedValue({ entries: [] }),
  fetchConsoleDataSourceResources: vi.fn(),
  createConsoleDataSource: vi.fn(),
  validateConsoleDataSource: vi.fn(),
  discoverConsoleDataSourceResources: vi.fn(),
  previewConsoleDataSourceResource: vi.fn(),
  mapConsoleDataSourceResourceToModel: vi.fn(),
  updateConsoleDataModel: vi.fn(),
  updateConsoleDataModelField: vi.fn(),
  updateConsoleDataModelScopeGrant: vi.fn(),
  updateConsoleDataSourceDefaults: vi.fn()
}));

import {
  fetchConsoleDataModels,
  fetchConsoleCompatibleDataModelTemplates,
  fetchConsoleDataSources
} from '@1flowbase/api-client';
import {
  fetchSettingsDataModels,
  fetchSettingsCompatibleDataModelTemplates,
  settingsCompatibleDataModelTemplatesQueryKey,
  fetchSettingsDataSources,
  settingsDataModelAdvisorFindingsQueryKey,
  settingsDataModelRecordPreviewQueryKey,
  settingsDataModelOpenApiQueryKey,
  settingsDataModelsQueryKey,
  settingsDataModelScopeGrantsQueryKey,
  settingsDataSourcesQueryKey
} from '../data-models';

describe('settings data models API wrappers', () => {
  test('exports stable query keys for data sources, models, grants, preview, and Advisor', () => {
    expect(settingsDataSourcesQueryKey).toEqual([
      'settings',
      'data-models',
      'data-sources'
    ]);
    expect(settingsDataModelsQueryKey('main')).toEqual([
      'settings',
      'data-models',
      'models',
      'main',
      '{}'
    ]);
    expect(
      settingsCompatibleDataModelTemplatesQueryKey('source-1', 'contacts')
    ).toEqual([
      'settings',
      'data-models',
      'compatible-templates',
      'source-1',
      'contacts'
    ]);
    expect(
      settingsCompatibleDataModelTemplatesQueryKey('source-1', 'companies')
    ).not.toEqual(
      settingsCompatibleDataModelTemplatesQueryKey('source-1', 'contacts')
    );
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
    await fetchSettingsDataSources();
    expect(fetchConsoleDataSources).toHaveBeenCalled();

    await fetchSettingsDataModels('source-1');
    expect(fetchConsoleDataModels).toHaveBeenCalledWith({
      data_source_id: 'source-1'
    });

    await fetchSettingsDataModels('source-1', {
      code: { $includes: 'orders' }
    });
    await fetchSettingsCompatibleDataModelTemplates('source-1', 'contacts');
    expect(fetchConsoleCompatibleDataModelTemplates).toHaveBeenCalledWith({
      data_source_id: 'source-1',
      resource_key: 'contacts'
    });
    expect(fetchConsoleDataModels).toHaveBeenCalledWith({
      data_source_id: 'source-1',
      filter: { code: { $includes: 'orders' } }
    });
  });
});
