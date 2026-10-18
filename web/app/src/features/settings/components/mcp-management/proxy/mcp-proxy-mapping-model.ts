import type {
  ConsoleMcpProxyInputMapping,
  ConsoleMcpProxyOutputMapping
} from '@1flowbase/api-client';

export const MCP_PROXY_PATH_PATTERN =
  /^[A-Za-z_][A-Za-z0-9_-]*(?:\.[A-Za-z_][A-Za-z0-9_-]*)*$/;

export type ProxyMapping =
  | ConsoleMcpProxyInputMapping
  | ConsoleMcpProxyOutputMapping;

export function mcpProxyMappingIsValid(value: ProxyMapping) {
  return value.mappings.every(
    (entry) =>
      MCP_PROXY_PATH_PATTERN.test(entry.local_path) &&
      MCP_PROXY_PATH_PATTERN.test(entry.remote_path)
  );
}
