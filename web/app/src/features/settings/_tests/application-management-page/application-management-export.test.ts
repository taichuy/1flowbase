import { describe, expect, test } from 'vitest';

import { buildApplicationManagementCsv } from '../../components/application-management/application-management-export';

describe('application management CSV export', () => {
  test('AC-004 exports backend field names and escapes filtered result values', () => {
    const csv = buildApplicationManagementCsv([
      {
        id: 'app-1',
        application_type: 'workflow',
        workflow_trigger_type: 'extension',
        name: 'Orders, "Create"',
        description: 'Creates an order',
        icon: null,
        icon_type: null,
        icon_background: null,
        created_by: 'user-1',
        created_by_display_name: 'Root',
        created_at: '2026-07-18T08:00:00Z',
        updated_at: '2026-07-18T09:00:00Z',
        tags: [{ id: 'tag-1', name: 'Orders' }],
        publication_status: 'published'
      }
    ]);

    expect(csv).toContain('"application_type"');
    expect(csv).toContain('"workflow_trigger_type"');
    expect(csv).toContain('"Orders, ""Create"""');
    expect(csv).toContain('"extension"');
    expect(csv).toContain('"published"');
  });
});
