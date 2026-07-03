import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';

import {
  createConsoleDataModel,
  createConsoleDataModelField,
  createConsoleDataModelScopeGrant,
  createConsoleRuntimeModelRecord,
  batchDeleteConsoleDataModels,
  deleteConsoleDataModel,
  deleteConsoleDataModelField,
  deleteConsoleRuntimeModelRecord,
  fetchConsoleDataModelAdvisorFindings,
  fetchConsoleDataModelOpenApiDocument,
  fetchConsoleDataModelRecordPreview,
  fetchConsoleDataModelScopeGrants,
  fetchConsoleAgentFlowDataModelOptions,
  fetchConsoleDataModels,
  fetchConsoleDataSourceInstances,
  fetchConsoleRuntimeModelRecord,
  fetchConsoleRuntimeModelRecords,
  updateConsoleDataModel,
  updateConsoleDataModelField,
  updateConsoleDataModelScopeGrant,
  updateConsoleRuntimeModelRecord,
  updateConsoleDataSourceDefaults
} from '../console-data-models';

describe('console-data-models client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('updateConsoleDataSourceDefaults patches defaults with CSRF', async () => {
    await expect(
      updateConsoleDataSourceDefaults(
        'source-1',
        {
          default_data_model_status: 'draft'
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/data-sources/instances/source-1/defaults',
      method: 'PATCH',
      csrfToken: 'csrf-123'
    });
  });

  test.each([
    {
      name: 'data source collection',
      request: () => fetchConsoleDataSourceInstances(),
      expected: {
        path: '/api/console/data-sources/instances'
      }
    },
    {
      name: 'filtered model collection',
      request: () =>
        fetchConsoleDataModels({
          data_source_instance_id: 'main_source',
          filter: { code: { $includes: 'customer profile' } }
        }),
      expected: {
        path: '/api/console/models?data_source_instance_id=main_source&filter=%7B%22code%22%3A%7B%22%24includes%22%3A%22customer+profile%22%7D%7D'
      }
    },
    {
      name: 'agent-flow data model options',
      request: () => fetchConsoleAgentFlowDataModelOptions(),
      expected: {
        path: '/api/console/models/agent-flow-options'
      }
    }
  ])('reads the $name route', async ({ request, expected }) => {
    await expect(request()).resolves.toMatchObject(expected);
  });

  test('create and update Data Models use the console model routes', async () => {
    await expect(
      createConsoleDataModel(
        {
          scope_kind: 'workspace',
          data_source_instance_id: 'source-1',
          external_resource_key: 'contacts',
          external_table_id: 'crm.contacts',
          code: 'orders',
          title: 'Orders',
          status: 'draft'
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/models',
      method: 'POST',
      body: {
        scope_kind: 'workspace',
        data_source_instance_id: 'source-1',
        external_resource_key: 'contacts',
        external_table_id: 'crm.contacts',
        code: 'orders',
        title: 'Orders',
        status: 'draft'
      },
      csrfToken: 'csrf-123'
    });

    await expect(
      updateConsoleDataModel(
        'model-1',
        {
          status: 'published'
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/models/model-1',
      method: 'PATCH',
      body: {
        status: 'published'
      },
      csrfToken: 'csrf-123'
    });
  });

  test('deleteConsoleDataModel uses the confirmed model delete route', async () => {
    await expect(
      deleteConsoleDataModel('model-1', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/models/model-1?confirmed=true',
      method: 'DELETE',
      csrfToken: 'csrf-123'
    });
  });

  test('batchDeleteConsoleDataModels posts filterByTk to the model batch delete action', async () => {
    await expect(
      batchDeleteConsoleDataModels(
        {
          filterByTk: ['model-1', 'model-2'],
          confirmed: true
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/models:batchDelete',
      method: 'POST',
      body: {
        filterByTk: ['model-1', 'model-2'],
        confirmed: true
      },
      csrfToken: 'csrf-123'
    });
  });

  test('field mutations use field routes and confirmation query', async () => {
    await expect(
      createConsoleDataModelField(
        'model-1',
        {
          code: 'email',
          title: 'Email',
          description: 'Primary email address',
          field_kind: 'string',
          is_required: true,
          is_unique: false,
          default_value: null,
          display_interface: 'input',
          display_options: {},
          relation_target_model_id: null,
          relation_options: {}
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/models/model-1/fields',
      method: 'POST',
      body: expect.objectContaining({
        description: 'Primary email address'
      }),
      csrfToken: 'csrf-123'
    });

    await expect(
      updateConsoleDataModelField(
        'model-1',
        'field-1',
        {
          title: 'Email',
          description: null,
          is_required: false,
          is_unique: true,
          default_value: null,
          display_interface: 'input',
          display_options: {},
          relation_options: {}
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/models/model-1/fields/field-1',
      method: 'PATCH',
      body: expect.objectContaining({
        description: null
      }),
      csrfToken: 'csrf-123'
    });

    await expect(
      deleteConsoleDataModelField('model-1', 'field-1', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/console/models/model-1/fields/field-1?confirmed=true',
      method: 'DELETE',
      csrfToken: 'csrf-123'
    });
  });

  test('scope grant list and mutations use scope-grant routes', async () => {
    await expect(
      fetchConsoleDataModelScopeGrants('model-1')
    ).resolves.toMatchObject({
      path: '/api/console/models/model-1/scope-grants'
    });

    await expect(
      createConsoleDataModelScopeGrant(
        'model-1',
        {
          scope_kind: 'system',
          scope_id: '00000000-0000-0000-0000-000000000000',
          enabled: true,
          permission_profile: 'system_all',
          confirm_unsafe_external_source_system_all: true
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/models/model-1/scope-grants',
      method: 'POST',
      csrfToken: 'csrf-123'
    });

    await expect(
      updateConsoleDataModelScopeGrant(
        'model-1',
        'grant-1',
        {
          enabled: false,
          permission_profile: 'owner',
          confirm_unsafe_external_source_system_all: false
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/console/models/model-1/scope-grants/grant-1',
      method: 'PATCH',
      csrfToken: 'csrf-123'
    });
  });

  test('advisor and runtime record preview use existing read routes', async () => {
    await expect(
      fetchConsoleDataModelAdvisorFindings('model-1')
    ).resolves.toMatchObject({
      path: '/api/console/models/model-1/advisor-findings'
    });

    await expect(
      fetchConsoleDataModelRecordPreview('orders')
    ).resolves.toMatchObject({
      path: '/api/runtime/models/orders/list?page=1&page_size=20'
    });

    await expect(
      fetchConsoleDataModelOpenApiDocument('model-1')
    ).resolves.toMatchObject({
      path: '/api/console/docs/data-models/model-1/openapi.json',
      unwrapSuccess: false
    });
  });

  test('runtime model records list serializes query options and encoded model code', async () => {
    await expect(
      fetchConsoleRuntimeModelRecords('sales/orders', {
        page: 2,
        page_size: 50,
        filter: {
          status: {
            $eq: 'needs review'
          }
        },
        sort: {
          field: 'created_at',
          direction: 'desc'
        },
        expand: ['customer', 'line items']
      })
    ).resolves.toMatchObject({
      path: '/api/runtime/models/sales%2Forders/list?page=2&page_size=50&filter=%7B%22status%22%3A%7B%22%24eq%22%3A%22needs+review%22%7D%7D&sort=created_at%3Adesc&expand=customer%2Cline+items'
    });
  });

  test('runtime model record get encodes model code and record id', async () => {
    await expect(
      fetchConsoleRuntimeModelRecord('sales/orders', 'record/1')
    ).resolves.toMatchObject({
      path: '/api/runtime/models/sales%2Forders/get/record%2F1'
    });
  });

  test('runtime model record mutations use body and CSRF token', async () => {
    await expect(
      createConsoleRuntimeModelRecord(
        'sales/orders',
        {
          title: 'Needs review',
          total: 42
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/runtime/models/sales%2Forders/create',
      method: 'POST',
      body: {
        title: 'Needs review',
        total: 42
      },
      csrfToken: 'csrf-123'
    });

    await expect(
      updateConsoleRuntimeModelRecord(
        'sales/orders',
        'record/1',
        {
          title: 'Approved'
        },
        'csrf-123'
      )
    ).resolves.toMatchObject({
      path: '/api/runtime/models/sales%2Forders/update/record%2F1',
      method: 'PATCH',
      body: {
        title: 'Approved'
      },
      csrfToken: 'csrf-123'
    });

    await expect(
      deleteConsoleRuntimeModelRecord('sales/orders', 'record/1', 'csrf-123')
    ).resolves.toMatchObject({
      path: '/api/runtime/models/sales%2Forders/delete/record%2F1',
      method: 'DELETE',
      csrfToken: 'csrf-123'
    });
  });
});
