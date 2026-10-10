import { describe, expect, test, vi } from 'vitest';
import * as transport from '../../transport';
import {
  exportSystemTemplate,
  exportSystemTemplateArchive,
  getApplicationTemplateCatalog,
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
      data_model_ids: [],
      mcp_instance_ids: [],
      i18n_keys: ['gateway.title']
    };
    const body = {
      schema_version: '1flowbase.portable-template/v2',
      pages: [{ id: 'page', tabs: [{ source_code: 'full source' }] }],
      applications: [],
      data_models: [],
      plugins: [],
      i18n_entries: [
        { key: 'gateway.title', locale: 'en_US', translation: 'Gateway' }
      ]
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
  test('requests paged metadata and sends pinned references without a package', async () => {
    const fetch = vi
      .spyOn(transport, 'apiFetch')
      .mockResolvedValue({} as never);
    await getApplicationTemplateCatalog({
      cursor: 'page/2',
      q: 'gateway demo'
    });
    expect(fetch).toHaveBeenLastCalledWith({
      path: '/api/console/settings/system-templates/catalog?category=applications-demo&cursor=page%2F2&q=gateway+demo',
      baseUrl: undefined
    });
    const reference = { catalog_id: 'official/demo', release_version: 7 };
    for (const [call, endpoint] of [
      [previewSystemTemplate, 'preview'],
      [installSystemTemplate, 'install']
    ] as const) {
      await call(reference, 'csrf');
      expect(fetch).toHaveBeenLastCalledWith({
        path: `/api/console/settings/system-templates/${endpoint}`,
        method: 'POST',
        body: reference,
        csrfToken: 'csrf',
        baseUrl: undefined
      });
    }
    fetch.mockRestore();
  });
  test('exports archives with the original selection and imports archive bytes', async () => {
    const fetch = vi
      .spyOn(transport, 'apiFetch')
      .mockResolvedValue({} as never);
    const selection = {
      page_ids: ['page'],
      application_ids: [],
      data_model_ids: [],
      mcp_instance_ids: [],
      i18n_keys: ['gateway.title']
    };
    await exportSystemTemplateArchive(selection, 'csrf');
    expect(fetch).toHaveBeenLastCalledWith({
      path: '/api/console/settings/system-templates/export?format=archive',
      method: 'POST',
      body: selection,
      csrfToken: 'csrf',
      baseUrl: undefined
    });
    const archive = { archive_base64: 'UEsDBAD/' };
    for (const [call, endpoint] of [
      [previewSystemTemplate, 'preview'],
      [installSystemTemplate, 'install']
    ] as const) {
      await call(archive, 'csrf');
      expect(fetch).toHaveBeenLastCalledWith({
        path: `/api/console/settings/system-templates/${endpoint}`,
        method: 'POST',
        body: archive,
        csrfToken: 'csrf',
        baseUrl: undefined
      });
    }
    fetch.mockRestore();
  });
});
