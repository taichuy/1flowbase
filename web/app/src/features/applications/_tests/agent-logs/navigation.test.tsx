import { expect, test } from 'vitest';
import type { ConsoleApplicationDetail } from '@1flowbase/api-client';
import {
  getApplicationDefaultSection,
  getApplicationSections
} from '../../lib/application-sections';

test('Agent Logs defaults to logs and exposes precisely logs/API/collection/statistics', () => {
  const application = {
    application_type: 'agent_logs',
    sections: { api: { status: 'available' } }
  } as ConsoleApplicationDetail;
  const sections = getApplicationSections(
    'app-logs',
    (key) => key,
    application
  );
  expect(sections.map((item) => item.key)).toEqual([
    'logs',
    'api',
    'collector',
    'statistics'
  ]);
  expect(sections.map((item) => item.to)).toEqual([
    '/applications/app-logs/logs',
    '/applications/app-logs/api',
    '/applications/app-logs/collector',
    '/applications/app-logs/statistics'
  ]);
  expect(getApplicationDefaultSection('agent_logs')).toBe('logs');
  expect(getApplicationDefaultSection('agent_flow')).toBe('orchestration');
  expect(getApplicationDefaultSection('workflow')).toBe('orchestration');
});
