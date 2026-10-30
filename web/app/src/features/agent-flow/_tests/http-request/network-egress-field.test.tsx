import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useState } from 'react';
import { beforeEach, expect, test, vi } from 'vitest';
import type { HttpNodeNetworkEgress } from '@1flowbase/api-client';
import { HttpRequestNetworkEgressField } from '../../components/detail/fields/http-request/HttpRequestNetworkEgressField';
import { fetchHttpNodeProxyOptions } from '../../api/http-node-proxy-options';

vi.mock('../../api/http-node-proxy-options', () => ({
  httpNodeProxyOptionsQueryKey: ['http-node-proxy-options'],
  fetchHttpNodeProxyOptions: vi.fn()
}));

function Harness({ initial }: { initial?: HttpNodeNetworkEgress }) {
  const [value, setValue] = useState<unknown>(initial);
  return (
    <>
      <HttpRequestNetworkEgressField
        block={{
          kind: 'field',
          renderer: 'http_request_network_egress',
          path: 'config.network_egress',
          label: '网络出口'
        }}
        adapter={{
          getValue: () => value,
          setValue: (_, next) => setValue(next),
          getDerived: () => undefined,
          dispatch: () => undefined
        }}
      />
      <output aria-label="saved">{JSON.stringify(value)}</output>
    </>
  );
}
function mount(initial?: HttpNodeNetworkEgress) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  return render(
    <QueryClientProvider client={client}>
      <Harness initial={initial} />
    </QueryClientProvider>
  );
}
beforeEach(() => {
  vi.mocked(fetchHttpNodeProxyOptions)
    .mockReset()
    .mockResolvedValue([
      {
        pool_member_id: 'member-a',
        display_name: 'SOCKS5 出口',
        selectable: true
      },
      {
        pool_member_id: 'member-b',
        display_name: '停用出口',
        selectable: false
      }
    ]);
});

test('old nodes inherit workspace; choosing a proxy saves only its reference, then direct clears it', async () => {
  mount();
  expect(screen.getByText('跟随工作区设置')).toBeVisible();
  expect(fetchHttpNodeProxyOptions).not.toHaveBeenCalled();
  fireEvent.mouseDown(screen.getByRole('combobox', { name: '网络出口' }));
  fireEvent.click(await screen.findByText('指定代理'));
  await waitFor(() => expect(fetchHttpNodeProxyOptions).toHaveBeenCalled());
  fireEvent.mouseDown(screen.getByRole('combobox', { name: '指定代理' }));
  fireEvent.click(await screen.findByText('SOCKS5 出口'));
  expect(screen.getByLabelText('saved')).toHaveTextContent(
    '{"mode":"proxy","pool_member_id":"member-a"}'
  );
  fireEvent.mouseDown(screen.getByRole('combobox', { name: '网络出口' }));
  fireEvent.click(await screen.findByText('直接连接'));
  expect(screen.getByLabelText('saved')).toHaveTextContent('{"mode":"direct"}');
});

test('stored proxy selection is restored without credentials', async () => {
  mount({ mode: 'proxy', pool_member_id: 'member-a' });
  expect(await screen.findByText('SOCKS5 出口')).toBeVisible();
  expect(screen.getByLabelText('saved')).toHaveTextContent('member-a');
  expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
});

test('missing references remain selected and cannot silently become workspace or direct', async () => {
  mount({ mode: 'proxy', pool_member_id: 'deleted-member' });
  expect(await screen.findByText('代理不可用')).toBeVisible();
  expect(screen.getByLabelText('saved')).toHaveTextContent('deleted-member');
  expect(
    screen.getByText('指定代理不可用时请求失败，不会自动改为直连。')
  ).toBeVisible();
});

test('failed option loading exposes retry without clearing the stored reference', async () => {
  vi.mocked(fetchHttpNodeProxyOptions).mockRejectedValueOnce(
    new Error('denied')
  );
  mount({ mode: 'proxy', pool_member_id: 'member-a' });
  expect(await screen.findByText('无法加载代理列表')).toBeVisible();
  fireEvent.click(screen.getByRole('button', { name: '重新加载代理' }));
  expect(await screen.findByText('SOCKS5 出口')).toBeVisible();
  expect(screen.getByLabelText('saved')).toHaveTextContent('member-a');
});
