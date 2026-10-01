import { describe, expect, test } from 'vitest';

import {
  buildInputMappingFromInterface,
  MCP_CALL_PARAMETERS,
  withLegacyCallMappings
} from '../mcp-input-mapping-model';

describe('MCP input mapping model', () => {
  test('provides short descriptions and a JSON Pointer example for call parameters', () => {
    expect(
      MCP_CALL_PARAMETERS.map((parameter) => parameter.description)
    ).toEqual([
      '描述 ID；留空使用 Tool 默认值，例如 report_detail。',
      '返回字符预算；正整数，留空使用 Tool 默认值，例如 4000。',
      '返回字段；JSON Pointer 数组，例如 ["/title", "/items/0/body"]。留空使用 Tool 默认值（未设置则沿用映射输出）；[] 不返回业务字段。'
    ]);
  });

  test('moves an existing call allowlist into the shared mappings table', () => {
    const mapping = withLegacyCallMappings({
      interface_parameters: [
        {
          name: 'title',
          field_type: 'string',
          parameter_type: 'json_body',
          required: true
        }
      ],
      mappings: [
        { interface_param: 'title', mcp_param: 'body.title', required: true }
      ],
      call_parameters: ['response_fields']
    });

    expect(mapping).not.toHaveProperty('call_parameters');
    expect(
      mapping.interface_parameters.map((parameter) => parameter.name)
    ).toEqual(['des_id', 'max_inline_chars', 'response_fields', 'title']);
    expect(mapping.mappings).toEqual([
      expect.objectContaining({
        interface_param: 'response_fields',
        source: { kind: 'mcp_call', path: 'response_fields' }
      }),
      expect.objectContaining({
        interface_param: 'title',
        mcp_param: 'body.title'
      })
    ]);
  });

  test('fills empty call descriptions on load without replacing custom text', () => {
    const mapping = withLegacyCallMappings({
      interface_parameters: MCP_CALL_PARAMETERS.map((parameter) => ({
        ...parameter,
        description: ''
      })),
      mappings: [
        {
          interface_param: 'response_fields',
          mcp_param: 'response_fields',
          description: '',
          source: { kind: 'mcp_call', path: 'response_fields' }
        },
        {
          interface_param: 'max_inline_chars',
          mcp_param: 'max_inline_chars',
          description: '自定义预算说明',
          source: { kind: 'mcp_call', path: 'max_inline_chars' }
        }
      ]
    });

    expect(
      mapping.interface_parameters.map((parameter) => parameter.description)
    ).toEqual(MCP_CALL_PARAMETERS.map((parameter) => parameter.description));
    expect(mapping.mappings[0]?.description).toBe(
      MCP_CALL_PARAMETERS[2]?.description
    );
    expect(mapping.mappings[1]?.description).toBe('自定义预算说明');
  });

  test('fills empty call descriptions when refreshing interface parameters', () => {
    const mapping = buildInputMappingFromInterface(
      { parameter_descriptors: [] } as unknown as Parameters<
        typeof buildInputMappingFromInterface
      >[0],
      {
        mappings: [
          {
            interface_param: 'des_id',
            mcp_param: 'des_id',
            description: '',
            source: { kind: 'mcp_call', path: 'des_id' }
          }
        ]
      }
    );

    expect(mapping.mappings[0]?.description).toBe(
      MCP_CALL_PARAMETERS[0]?.description
    );
  });
});
