import type { ConsoleMcpExportPackage } from '@1flowbase/api-client';

export function parseJsonText(value: string, field: string) {
  try {
    return JSON.parse(value || '{}') as unknown;
  } catch {
    throw new Error(`${field} JSON`);
  }
}

export function stringifyJson(value: unknown) {
  return JSON.stringify(value ?? {}, null, 2);
}

export function riskColor(riskLevel: string) {
  switch (riskLevel) {
    case 'critical':
      return 'red';
    case 'high':
      return 'volcano';
    case 'medium':
      return 'gold';
    default:
      return 'green';
  }
}

export function statusColor(status: string) {
  return status === 'enabled' ? 'green' : status === 'disabled' ? 'default' : 'blue';
}

export function downloadMcpExportPackage(
  exportPackage: ConsoleMcpExportPackage
) {
  if (typeof window === 'undefined' || typeof document === 'undefined') {
    return;
  }

  const blob = new Blob([JSON.stringify(exportPackage, null, 2)], {
    type: 'application/json'
  });
  const url = window.URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = 'mcp-management-export.json';
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  window.URL.revokeObjectURL(url);
}
