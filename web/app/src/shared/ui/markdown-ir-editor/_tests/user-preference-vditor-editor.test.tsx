import { render, screen } from '@testing-library/react';
import { afterEach, expect, test, vi } from 'vitest';
import { resetAuthStore } from '../../../../state/auth-store';
import { preferenceWriteCases, setPreferenceActor } from '../../../../test/fixtures/preference-auth';
import { UserPreferenceVditorEditor } from '../UserPreferenceVditorEditor';

const capture = vi.hoisted(() => ({ canPersist: undefined as (() => boolean) | undefined }));
vi.mock('@1flowbase/rich-text', () => ({
  VditorEditor: (props: { value: string; canPersistModePreference?: () => boolean }) => {
    capture.canPersist = props.canPersistModePreference;
    return <p>{props.value}</p>;
  }
}));
afterEach(resetAuthStore);
test.each(preferenceWriteCases)(
  'binds the editor capability to the host projection rather than a caller override: $name',
  ({ actor, canPersist }) => {
    setPreferenceActor(actor);
    render(<UserPreferenceVditorEditor value="Readable editor content" onChange={vi.fn()}
      canPersistModePreference={() => true} />);
    expect(screen.getByText('Readable editor content')).toBeVisible();
    expect(capture.canPersist?.()).toBe(canPersist);
  }
);
test('an existing editor capability follows revoked and newly granted operations', () => {
  setPreferenceActor({ role: 'root', permissions: [] });
  render(<UserPreferenceVditorEditor value="Draft" onChange={vi.fn()} />);
  const canPersist = capture.canPersist!;
  expect(canPersist()).toBe(true);
  setPreferenceActor({ role: 'member', permissions: ['patch_me'] });
  expect(canPersist()).toBe(false);
  setPreferenceActor({ role: 'member', permissions: ['patch_me_meta'] });
  expect(canPersist()).toBe(true);
});
