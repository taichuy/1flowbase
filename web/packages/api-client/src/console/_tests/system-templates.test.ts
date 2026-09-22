import { describe, expect, test, vi } from 'vitest';
import * as transport from '../../transport';
import {
  exportSystemTemplate,
  getSystemTemplateCatalog,
  installSystemTemplate,
  previewSystemTemplate
} from '../system-templates';

describe('portable template transport', () => {
  test('uses separate endpoints and forwards complete packages with CSRF', async () => {
    const fetch = vi
      .spyOn(transport, 'apiFetch')
      .mockResolvedValue({} as never);
    const selection = {
      page_ids: ['page'],
      application_ids: [],
      data_model_ids: []
    };
    const body = {
      schema_version: '1flowbase.portable-template/v1',
      pages: [{ id: 'page', tabs: [{ source_code: 'full source' }] }],
      applications: [],
      data_models: [],
      plugins: []
    };
    await getSystemTemplateCatalog();
    expect(fetch).toHaveBeenLastCalledWith({
      path: '/api/console/settings/system-templates/catalog',
      baseUrl: undefined
    });
    await exportSystemTemplate(selection, 'csrf');
    expect(fetch).toHaveBeenLastCalledWith({
      path: '/api/console/settings/system-templates/export',
      method: 'POST',
      body: selection,
      csrfToken: 'csrf',
      baseUrl: undefined
    });
    await previewSystemTemplate(body, 'csrf');
    expect(fetch).toHaveBeenLastCalledWith({
      path: '/api/console/settings/system-templates/preview',
      method: 'POST',
      body,
      csrfToken: 'csrf',
      baseUrl: undefined
    });
    await installSystemTemplate(body, 'csrf');
    expect(fetch).toHaveBeenLastCalledWith({
      path: '/api/console/settings/system-templates/install',
      method: 'POST',
      body,
      csrfToken: 'csrf',
      baseUrl: undefined
    });
    fetch.mockRestore();
  });
});
