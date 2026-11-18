import type { EventCatalogEntry } from '@1flowbase/api-client';
import { describe, expect, it } from 'vitest';
import { eventGroupTree } from '../event-groups';

function entry(group_path?: string[]): EventCatalogEntry {
  return {
    id: null,
    enabled: null,
    owner_id: null,
    scope: { kind: 'system' },
    contract_id: 'example.event',
    name: 'Event',
    description: '',
    payload_schema: {},
    origin: 'core',
    supported_phases: [],
    ...(group_path === undefined ? {} : { group_path })
  };
}

describe('event group path lookup', () => {
  it('reuses repeated prefixes while preserving first-seen sibling order and distinct parents', () => {
    expect(
      eventGroupTree([
        entry(['B', 'common']),
        entry(['A', 'common']),
        entry(['B', 'common', 'leaf']),
        entry(['B', 'second']),
        entry(['A', 'common'])
      ])
    ).toEqual([
      {
        title: 'B',
        value: '["B"]',
        children: [
          {
            title: 'common',
            value: '["B","common"]',
            children: [
              { title: 'leaf', value: '["B","common","leaf"]', children: [] }
            ]
          },
          { title: 'second', value: '["B","second"]', children: [] }
        ]
      },
      {
        title: 'A',
        value: '["A"]',
        children: [{ title: 'common', value: '["A","common"]', children: [] }]
      }
    ]);
  });

  it('keeps omitted or empty groups empty and does not merge paths with separator-like titles', () => {
    expect(eventGroupTree([entry(), entry([])])).toEqual([]);
    expect(
      eventGroupTree([entry(['a/b', 'c']), entry(['a', 'b/c'])]).map(
        (root) => root.children[0].value
      )
    ).toEqual(['["a/b","c"]', '["a","b/c"]']);
  });
});
