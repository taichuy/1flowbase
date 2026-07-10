import { describe, expect, test } from 'vitest';

import {
  buildMcpDirectoryTreeData,
  buildReadableToolId
} from '../mcp-management-view-model';

describe('mcp management view model', () => {
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
        title: 'Workspace Ops /',
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
            title: '/ops',
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
});
