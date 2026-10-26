import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, test } from 'vitest';
import { MessageStatus } from '../communications/communication-presentation';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import type { CommunicationMessage } from '@1flowbase/api-client';
afterEach(async () => {
  cleanup();
  await appI18n.changeLanguage('zh_Hans');
});
const message: Pick<
  CommunicationMessage,
  'processing_status' | 'send_status' | 'failure_code'
> = {
  processing_status: 'succeeded',
  send_status: 'failed',
  failure_code: 'send_interrupted'
};
test('delivery failure remains visible after successful processing and translates failure reason', async () => {
  await appI18n.changeLanguage('en_US');
  render(<MessageStatus message={message} />);
  expect(screen.getByText('Processed')).toBeInTheDocument();
  expect(screen.getByText('Delivery failed')).toBeInTheDocument();
  expect(
    screen.getByText('Failure reason: Delivery was interrupted')
  ).toBeInTheDocument();
  expect(screen.queryByText('send_interrupted')).not.toBeInTheDocument();
});
test('unknown failure does not expose provider diagnostic codes', async () => {
  await appI18n.changeLanguage('en_US');
  render(
    <MessageStatus
      message={{ ...message, failure_code: 'private_provider_code' }}
    />
  );
  expect(
    screen.getByText('Failure reason: Failure reason unavailable')
  ).toBeInTheDocument();
  expect(screen.queryByText(/private_provider_code/)).not.toBeInTheDocument();
});
