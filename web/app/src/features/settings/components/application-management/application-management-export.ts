import type { SettingsApplicationManagementItem } from '../../api/application-management';

function csvCell(value: string) {
  return `"${value.replaceAll('"', '""')}"`;
}

export function buildApplicationManagementCsv(
  applications: SettingsApplicationManagementItem[]
) {
  const rows = applications.map((application) => [
    application.id,
    application.name,
    application.application_type,
    application.workflow_trigger_type ?? '',
    application.publication_status,
    application.created_by_display_name,
    application.tags.map((tag) => tag.name).join(', '),
    application.created_at,
    application.updated_at
  ]);
  const header = [
    'id',
    'name',
    'application_type',
    'workflow_trigger_type',
    'publication_status',
    'created_by_display_name',
    'tags',
    'created_at',
    'updated_at'
  ];

  return [header, ...rows]
    .map((row) => row.map((value) => csvCell(String(value))).join(','))
    .join('\n');
}

export function downloadApplicationManagementCsv(csv: string) {
  const blob = new Blob([`\uFEFF${csv}`], { type: 'text/csv;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = `applications-${new Date().toISOString().slice(0, 10)}.csv`;
  link.click();
  URL.revokeObjectURL(url);
}
