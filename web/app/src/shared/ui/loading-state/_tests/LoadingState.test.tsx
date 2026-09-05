import { render, screen } from '@testing-library/react';
import { afterEach, describe, expect, test } from 'vitest';

import { appI18n } from '../../../i18n/app-i18n';
import { LoadingState } from '../LoadingState';

describe('LoadingState', () => {
  afterEach(async () => {
    await appI18n.changeLanguage('en_US');
  });

  test('uses the fixed English loading label in every locale', async () => {
    await appI18n.changeLanguage('zh_Hans');
    render(<LoadingState />);

    expect(
      screen.getByRole('status', { name: 'thinking' })
    ).toBeInTheDocument();
    expect(screen.getByText('thinking')).toBeInTheDocument();
  });

  test('uses the large circular loading indicator', () => {
    render(<LoadingState />);

    expect(document.querySelector('.anticon-loading')).toHaveStyle({
      color: '#1677ff',
      fontSize: '48px'
    });
    expect(document.querySelector('.ant-spin-description')).toHaveStyle({
      color: '#1677ff'
    });
  });

  test('supports fullscreen and compact layout variants', () => {
    render(<LoadingState fullscreen compact />);

    expect(screen.getByRole('status', { name: 'thinking' })).toHaveClass(
      'loading-state',
      'loading-state--fullscreen',
      'loading-state--compact'
    );
  });
});
