import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeAll, beforeEach, expect, test, vi } from 'vitest';
import { resetAuthStore } from '../../../../../state/auth-store';
import { preferenceWriteCases, setPreferenceActor } from '../../../../../test/fixtures/preference-auth';
import { appI18n, loadApplicationI18nResources } from '../../../../../shared/i18n/app-i18n';
import { ApplicationLogsFloatingWindow } from '../ApplicationLogsFloatingWindow';

const key = 'applicationLogsFloatingWindowWidth:layout-permission';
beforeAll(async () => {
  await loadApplicationI18nResources();
  await appI18n.changeLanguage('zh_Hans');
});
beforeEach(() => {
  window.localStorage.clear();
  vi.spyOn(window, 'innerWidth', 'get').mockReturnValue(1280);
  vi.spyOn(window, 'innerHeight', 'get').mockReturnValue(900);
});
afterEach(() => {
  vi.restoreAllMocks();
  resetAuthStore();
});
function view() {
  return (
    <ApplicationLogsFloatingWindow active title="日志" testId="layout-permission"
      initialRect={() => ({ left: 100, top: 20, width: 400, height: 400 })}
      onActivate={vi.fn()}>
      <p>Readable log</p>
    </ApplicationLogsFloatingWindow>
  );
}
test.each(preferenceWriteCases)(
  'keeps floating log resizing usable and gates persistence: $name',
  ({ actor, canPersist }) => {
    setPreferenceActor(actor);
    window.localStorage.setItem(key, '500');
    const write = vi.spyOn(Storage.prototype, 'setItem');
    const { unmount } = render(view());
    expect(screen.getByText('Readable log')).toBeVisible();
    expect(screen.getByTestId('layout-permission')).toHaveStyle({ width: '500px' });
    fireEvent.keyDown(screen.getByRole('separator', { name: '从右侧调整日志宽度' }), { key: 'ArrowRight' });
    expect(screen.getByTestId('layout-permission')).toHaveStyle({ width: '510px' });
    expect(window.localStorage.getItem(key)).toBe(canPersist ? '510' : '500');
    if (canPersist) expect(write).toHaveBeenCalledWith(key, '510');
    else expect(write).not.toHaveBeenCalled();
    unmount();
    render(view());
    expect(screen.getByTestId('layout-permission')).toHaveStyle({ width: canPersist ? '510px' : '500px' });
  }
);
