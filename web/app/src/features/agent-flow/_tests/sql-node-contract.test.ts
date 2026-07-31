import { describe, expect, test } from 'vitest';

import { getBuiltinNodeRuntimeContract } from '../lib/node-definitions/contracts';
import { getNodeDefinition } from '../lib/node-definitions';

describe('SQL node contract', () => {
  test('AC-001 provides one generic SQL node with data source and Monaco fields', () => {
    const contract = getBuiltinNodeRuntimeContract('sql');
    const definition = getNodeDefinition('sql');

    expect(contract?.defaults.config).toEqual({
      data_source_instance_id: 'main'
    });
    expect(contract?.defaults.bindings).toEqual({
      sql: { kind: 'templated_text', value: '' }
    });
    expect(contract?.defaults.outputs).toEqual([
      expect.objectContaining({ key: 'results', valueType: 'array' })
    ]);
    expect(contract?.policies.sideEffect).toBe('external_write');
    expect(definition?.sections).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          fields: expect.arrayContaining([
            expect.objectContaining({
              key: 'config.data_source_instance_id',
              editor: 'data_source'
            })
          ])
        }),
        expect.objectContaining({
          fields: expect.arrayContaining([
            expect.objectContaining({
              key: 'bindings.sql',
              editor: 'sql_source'
            })
          ])
        })
      ])
    );
  });
});
