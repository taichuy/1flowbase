import type {
  CommunicationCommandConfiguration,
  CommunicationCommandView
} from '@1flowbase/api-client';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { ConfigProvider } from 'antd';
import enUS from 'antd/locale/en_US';
import { I18nextProvider } from 'react-i18next';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { CommunicationCommandsTable } from '../communications/CommunicationCommandsTable';

vi.mock('../../../../../state/console-operation-access', () => ({
  useConsoleOperationAccess: () => () => true
}));

beforeEach(async () => {
  localStorage.clear();
  await appI18n.changeLanguage('en_US');
});
afterEach(async () => {
  cleanup();
  localStorage.clear();
  await appI18n.changeLanguage('zh_Hans');
});

function declaration(
  command_id: string,
  extra: Partial<CommunicationCommandView> = {}
): CommunicationCommandView {
  return {
    command_id,
    name: 'new',
    description: 'Command',
    usage: '/new',
    source_kind: 'builtin',
    source_name: 'Host',
    installation_id: null,
    enabled: true,
    available: true,
    unavailable_reason: null,
    ...extra
  };
}

test('preserves first declaration, last configured value, row order and missing-declaration fallbacks', () => {
  const commands = [
    declaration('host/new'),
    declaration('user/invoice', {
      source_kind: 'user',
      installation_id: 'first',
      available: false,
      unavailable_reason: 'first declaration unavailable'
    }),
    declaration('user/invoice', {
      source_kind: 'user',
      installation_id: 'second'
    })
  ];
  const value: CommunicationCommandConfiguration[] = [
    { kind: 'registered', command_id: 'plugin/missing', enabled: true },
    {
      kind: 'event',
      command_id: 'user/invoice',
      enabled: false,
      name: 'invoice',
      description: 'Invoice event',
      contract_id: 'invoice.created',
      input_mapping: {}
    },
    { kind: 'registered', command_id: 'host/new', enabled: true },
    { kind: 'registered', command_id: 'host/new', enabled: false },
    {
      kind: 'event',
      command_id: 'user/note',
      enabled: false,
      name: 'note',
      description: 'Uninstalled event',
      contract_id: 'note.created',
      input_mapping: {}
    }
  ];
  const onChange = vi.fn();
  render(
    <I18nextProvider i18n={appI18n}>
      <ConfigProvider locale={enUS}>
        <CommunicationCommandsTable
          value={value}
          commands={commands}
          loading={false}
          disabled={false}
          onChange={onChange}
          onEdit={vi.fn()}
          onDelete={vi.fn()}
        />
      </ConfigProvider>
    </I18nextProvider>
  );
  expect(
    screen.getAllByRole('switch').map((item) => item.getAttribute('aria-label'))
  ).toEqual([
    'Enable new',
    'Enable plugin/missing',
    'Enable invoice',
    'Enable note'
  ]);
  expect(screen.getByRole('switch', { name: 'Enable new' })).not.toBeChecked();
  expect(screen.getByRole('switch', { name: 'Enable invoice' })).toBeDisabled();
  expect(screen.getByText(/first declaration unavailable/)).toBeInTheDocument();
  expect(screen.getByRole('switch', { name: 'Enable note' })).toBeEnabled();
  fireEvent.click(
    screen.getByRole('switch', { name: 'Enable plugin/missing' })
  );
  expect(onChange).toHaveBeenCalledWith({
    command_id: 'plugin/missing',
    enabled: false
  });
});
