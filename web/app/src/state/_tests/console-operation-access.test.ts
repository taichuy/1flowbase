import { describe, expect, it } from 'vitest';

import { hasConsoleOperationAccess } from '../console-operation-access';

describe('console operation access', () => {
  it('keeps root access without requiring role grants', () => {
    expect(hasConsoleOperationAccess('root', [], 'mcp.instances.create')).toBe(true);
  });

  it('does not turn page visibility or a read into mutation authority', () => {
    const permissions = ['settings_feature.access.system.mcp-management', 'mcp.instances.view'];
    expect(hasConsoleOperationAccess('readonly', permissions, 'mcp.instances.view')).toBe(true);
    expect(hasConsoleOperationAccess('readonly', permissions, 'mcp.instances.create')).toBe(false);
  });

  it('does not expand one custom write grant to other operations', () => {
    const permissions = ['ui_management.templates.create'];
    expect(hasConsoleOperationAccess('custom', permissions, 'ui_management.templates.create')).toBe(true);
    expect(hasConsoleOperationAccess('custom', permissions, 'ui_management.templates.delete')).toBe(false);
    expect(hasConsoleOperationAccess(undefined, undefined, 'ui_management.templates.create')).toBe(false);
  });
});
