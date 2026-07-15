import { render, screen } from '@testing-library/react';
import { describe, expect, test } from 'vitest';

import { AutosizeSelect } from '../AutosizeSelect';

describe('AutosizeSelect', () => {
  test('renders a select whose shell measures every option label', () => {
    const { container } = render(
      <AutosizeSelect
        aria-label="时间间隔"
        options={[
          { label: '今天', value: '1' },
          { label: '过去 7 天', value: '7' },
          { label: '过去 12 月', value: '365' }
        ]}
        value="7"
      />
    );

    expect(
      screen.getByRole('combobox', { name: '时间间隔' })
    ).toBeInTheDocument();

    const shell = container.querySelector('.autosize-select');
    const measureItems = Array.from(
      container.querySelectorAll('.autosize-select__measure-item')
    ).map((item) => item.getAttribute('data-measure-label'));

    expect(shell).not.toBeNull();
    expect(measureItems).toEqual(['今天', '过去 7 天', '过去 12 月']);
    expect(screen.getAllByText('过去 7 天')).toHaveLength(1);
  });

  test('allows explicit measurement labels when option labels are not plain text', () => {
    const { container } = render(
      <AutosizeSelect
        aria-label="状态"
        autosizeLabels={['短', '非常长的状态名称']}
        options={[
          { label: <span>短</span>, value: 'short' },
          { label: <span>长</span>, value: 'long' }
        ]}
        value="short"
      />
    );

    const measureItems = Array.from(
      container.querySelectorAll('.autosize-select__measure-item')
    ).map((item) => item.getAttribute('data-measure-label'));

    expect(measureItems).toEqual(['短', '非常长的状态名称']);
  });
});
