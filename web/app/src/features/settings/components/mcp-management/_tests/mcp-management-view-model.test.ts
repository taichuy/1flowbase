import { describe, expect, test } from 'vitest';

import {
  buildMcpChildGroupPath,
  buildMcpDirectoryTreeData,
  buildReadableToolId,
  nextMcpDirectoryExpandedKeys
} from '../mcp-management-view-model';

describe('mcp management view model', () => {
  test('builds a direct child path from fixed root and nested parents', () => {
    expect(buildMcpChildGroupPath('/', 'system_mcp')).toBe('/system_mcp');
    expect(buildMcpChildGroupPath('/system_mcp', 'discovery')).toBe(
      '/system_mcp/discovery'
    );
    expect(buildMcpChildGroupPath('/system_mcp/discovery', 'search')).toBe(
      '/system_mcp/discovery/search'
    );
  });

  test('keeps descendants collapsed when reopening a parent directory', () => {
    expect(
      nextMcpDirectoryExpandedKeys(
        ['instance:ops:/', 'group:/ops', 'group:/ops/customer'],
        'group:/ops',
        true
      )
    ).toEqual(['instance:ops:/', 'group:/ops']);
    expect(
      nextMcpDirectoryExpandedKeys(
        ['instance:ops:/', 'group:/ops', 'group:/ops/customer'],
        'group:/ops',
        false
      )
    ).toEqual(['instance:ops:/']);
    expect(
      nextMcpDirectoryExpandedKeys(
        ['instance:ops:/', 'group:/ops'],
        'instance:ops:/',
        true
      )
    ).toEqual(['instance:ops:/']);
  });

  test('builds a readable tool id from name', () => {
    expect(buildReadableToolId('Create Customer')).toBe('create_customer');
    expect(buildReadableToolId('', 'A_b9Zx10')).toBe('A_b9Zx10');
    expect(buildReadableToolId('', 'too-long-random-value')).toHaveLength(8);
  });

  test('represents groups and multi-path tool bindings in the directory tree', () => {
    const treeData = buildMcpDirectoryTreeData({
      instance: {
        id: 'instance-record-1',
        instance_id: 'workspace_ops',
        name: 'Workspace Ops',
        default_entry_path: '/'
      },
      groups: [
        {
          id: 'group-1',
          instance_record_id: 'instance-record-1',
          path: '/crm',
          display_name: 'CRM',
          description_short: 'Customer operations',
          enabled: true,
          sort_order: 0
        }
      ],
      bindings: [
        {
          id: 'binding-1',
          instance_record_id: 'instance-record-1',
          tool_record_id: 'tool-record-1',
          group_path: '/crm',
          tool_id: 'customer_create',
          display_alias: null,
          visible: true,
          sort_order: 0
        },
        {
          id: 'binding-2',
          instance_record_id: 'instance-record-1',
          tool_record_id: 'tool-record-1',
          group_path: '/ops',
          tool_id: 'customer_create',
          display_alias: 'Customer Create Ops',
          visible: true,
          sort_order: 1
        }
      ],
      tools: [
        {
          id: 'tool-record-1',
          tool_id: 'customer_create',
          short_description: 'Create a customer account.'
        }
      ]
    });

    expect(treeData).toEqual([
      {
        key: 'instance:workspace_ops:/',
        title: '/',
        node_type: 'instance',
        path: '/',
        children: [
          {
            key: 'group:/crm',
            title: 'CRM',
            display_name: 'CRM',
            description_short: 'Customer operations',
            node_type: 'group',
            path: '/crm',
            children: [
              {
                key: 'binding:binding-1',
                title: 'customer_create',
                tool_short_description: 'Create a customer account.',
                node_type: 'binding',
                path: '/crm',
                binding_id: 'binding-1'
              }
            ]
          },
          {
            key: 'group:/ops',
            title: 'ops',
            display_name: undefined,
            description_short: undefined,
            node_type: 'group',
            path: '/ops',
            children: [
              {
                key: 'binding:binding-2',
                title: 'customer_create',
                tool_short_description: 'Create a customer account.',
                node_type: 'binding',
                path: '/ops',
                binding_id: 'binding-2'
              }
            ]
          }
        ]
      }
    ]);
  });

  test('shows bindings mounted directly on the instance root', () => {
    const treeData = buildMcpDirectoryTreeData({
      instance: {
        id: 'instance-record-1',
        instance_id: 'workspace_ops',
        name: 'Workspace Ops',
        default_entry_path: '/'
      },
      groups: [],
      bindings: [
        {
          id: 'binding-root',
          instance_record_id: 'instance-record-1',
          tool_record_id: 'tool-record-1',
          group_path: '/',
          tool_id: 'customer_search',
          display_alias: null,
          visible: true,
          sort_order: 0
        }
      ],
      tools: [
        {
          id: 'tool-record-1',
          tool_id: 'customer_search',
          short_description: 'Search customers.'
        }
      ]
    });

    expect(treeData[0]?.children).toEqual([
      expect.objectContaining({ key: 'binding:binding-root', path: '/' })
    ]);
  });

  test('nests child group paths under their parent group', () => {
    const treeData = buildMcpDirectoryTreeData({
      instance: {
        id: 'instance-record-1',
        instance_id: 'workspace_ops',
        name: 'Workspace Ops',
        default_entry_path: '/'
      },
      groups: [
        {
          id: 'group-1',
          instance_record_id: 'instance-record-1',
          path: '/ops',
          display_name: 'Ops',
          description_short: null,
          enabled: true,
          sort_order: 0
        },
        {
          id: 'group-2',
          instance_record_id: 'instance-record-1',
          path: '/ops/customer',
          display_name: 'Customer',
          description_short: null,
          enabled: true,
          sort_order: 0
        }
      ],
      bindings: [
        {
          id: 'binding-1',
          instance_record_id: 'instance-record-1',
          tool_record_id: 'tool-record-1',
          group_path: '/ops/customer',
          tool_id: 'customer_search',
          display_alias: null,
          visible: true,
          sort_order: 0
        }
      ],
      tools: [
        {
          id: 'tool-record-1',
          tool_id: 'customer_search',
          short_description: 'Search customers.'
        }
      ]
    });

    expect(treeData[0]?.children?.[0]).toMatchObject({
      key: 'group:/ops',
      children: [
        {
          key: 'group:/ops/customer',
          children: [{ key: 'binding:binding-1' }]
        }
      ]
    });
  });

  test('shares deep ancestors while preserving scoped roots, sibling order and every binding', () => {
    const input = {
      instance: { id: 'i', instance_id: 'ops', name: 'Ops', default_entry_path: '/ops' },
      groups: ['/ops/shared/z', '/ops/shared/a', '/outside', '/ops/shared'].map((path, index) => ({
        id: `g${index}`, instance_record_id: 'i', path, display_name: path,
        description_short: null, enabled: true, sort_order: -index
      })),
      bindings: Array.from({ length: 50 }, (_, index) => ({
        id: `b${index}`, instance_record_id: 'i', tool_record_id: 'missing',
        group_path: index % 2 ? '/ops/shared/a/deep' : '/ops/shared/z/deep',
        tool_id: `tool_${index}`, display_alias: null, visible: true, sort_order: index
      })),
      tools: []
    };
    const [root] = buildMcpDirectoryTreeData(input);
    expect(root?.children?.map((node) => node.path)).toEqual(['/ops/shared']);
    const branches = root?.children?.[0]?.children;
    expect(branches?.map((node) => node.path)).toEqual(['/ops/shared/a', '/ops/shared/z']);
    for (const [branch, parity] of [[0, 1], [1, 0]] as const) {
      expect(branches?.[branch]?.children?.[0]?.children?.map((node) => node.binding_id))
        .toEqual(input.bindings.filter((_, index) => index % 2 === parity).map((binding) => binding.id));
    }
  });

  test('keeps first appearance for collation ties and last group metadata for duplicate paths', () => {
    const paths = ['/ops/é', '/ops/e\u0301', '/ops/é'];
    const [root] = buildMcpDirectoryTreeData({
      instance: { id: 'i', instance_id: 'ops', name: 'Ops', default_entry_path: '/ops' },
      groups: paths.map((path, index) => ({
        id: `g${index}`, instance_record_id: 'i', path, display_name: `Group ${index}`,
        description_short: null, enabled: true, sort_order: 0
      })),
      bindings: [], tools: []
    });
    expect(root?.children?.map((node) => [node.path, node.title])).toEqual([
      ['/ops/é', 'Group 2'], ['/ops/e\u0301', 'Group 1']
    ]);
  });
});
