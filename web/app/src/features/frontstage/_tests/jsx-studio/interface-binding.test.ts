import { describe, expect, test } from 'vitest';

import type { ConsoleFrontstageCallableInterface } from '@1flowbase/api-client';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import {
  bindFrontstageCallableInterface,
  resolveFrontstageInterfaceBindings
} from '../../lib/jsx-studio/interface-binding';

const operation = {
  operation_id: 'get_frontstage_page_detail',
  schema_digest: 'digest-2',
  scope: 'frontstage_page_tab',
  risk_level: 'low',
  request_media_type: null,
  response_media_type: 'application/json',
  bindable: true,
  disabled_reason: null
} as ConsoleFrontstageCallableInterface;

const block = {
  id: 'block-1',
  interfaces: []
} as unknown as FrontstageBlockInstance;

describe('Frontstage interface binding document', () => {
  test('AC-004 persists alias-to-operation identity and reports digest drift', () => {
    const bound = bindFrontstageCallableInterface(
      block,
      'getCurrentPage',
      operation
    );
    expect(bound.interfaces).toEqual([
      {
        alias: 'getCurrentPage',
        operation_id: 'get_frontstage_page_detail',
        schema_digest: 'digest-2',
        scope: 'frontstage_page_tab',
        risk_level: 'low',
        request_media_type: null,
        response_media_type: 'application/json'
      }
    ]);
    expect(
      resolveFrontstageInterfaceBindings(bound, [operation])[0].status
    ).toBe('current');
    expect(
      resolveFrontstageInterfaceBindings(bound, [
        { ...operation, schema_digest: 'digest-3' }
      ])[0].status
    ).toBe('stale');
  });
});
