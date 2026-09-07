import { afterEach, describe, expect, test, vi } from 'vitest';

import { acquireFrontstageScrollIntentArbiter } from '../scroll-intent-arbiter';

afterEach(() => {
  vi.useRealTimers();
  document.body.replaceChildren();
});

describe('frontstage scroll intent arbiter', () => {
  test('shares one writer policy across surfaces with the same scroll owner', () => {
    const fixture = createFixture();
    const first = acquireFrontstageScrollIntentArbiter(
      fixture.scrollOwner,
      window
    );
    const second = acquireFrontstageScrollIntentArbiter(
      fixture.scrollOwner,
      window
    );

    expect(second.arbiter).toBe(first.arbiter);

    first.release();
    expect(second.arbiter.applyProgrammaticScroll(60)).toBe(true);
    expect(fixture.scrollTo).toHaveBeenCalledOnce();

    second.release();
  });

  test('AC-001 suppresses programmatic scrolling while a user scroll is active', () => {
    vi.useFakeTimers();
    const fixture = createFixture();
    const lease = acquireFrontstageScrollIntentArbiter(
      fixture.scrollOwner,
      window
    );

    fixture.scrollOwner.scrollTop = 120;
    fixture.scrollOwner.dispatchEvent(new Event('scroll'));

    expect(lease.arbiter.applyProgrammaticScroll(40)).toBe(false);
    expect(fixture.scrollTo).not.toHaveBeenCalled();

    vi.advanceTimersByTime(150);
    expect(lease.arbiter.applyProgrammaticScroll(40)).toBe(true);
    expect(fixture.scrollTo).toHaveBeenCalledWith({
      top: 40,
      behavior: 'auto'
    });

    lease.release();
  });

  test('AC-002 recognizes its own scroll event without blocking a later reveal', () => {
    vi.useFakeTimers();
    const fixture = createFixture();
    const lease = acquireFrontstageScrollIntentArbiter(
      fixture.scrollOwner,
      window
    );

    expect(lease.arbiter.applyProgrammaticScroll(90)).toBe(true);
    fixture.scrollOwner.dispatchEvent(new Event('scroll'));
    fixture.scrollOwner.dispatchEvent(new Event('scroll'));

    expect(lease.arbiter.applyProgrammaticScroll(180)).toBe(true);
    expect(fixture.scrollTo).toHaveBeenCalledTimes(2);

    lease.release();
  });

  test('AC-003 applies layout compensation only while the captured user offset is unchanged', () => {
    const fixture = createFixture();
    const lease = acquireFrontstageScrollIntentArbiter(
      fixture.scrollOwner,
      window
    );

    fixture.scrollOwner.scrollTop = 80;
    expect(lease.arbiter.applyLayoutCompensation(80, 24)).toBe(true);
    expect(fixture.scrollTo).toHaveBeenLastCalledWith({
      top: 104,
      behavior: 'auto'
    });

    fixture.scrollOwner.scrollTop = 140;
    expect(lease.arbiter.applyLayoutCompensation(104, 24)).toBe(false);
    expect(fixture.scrollTo).toHaveBeenCalledTimes(1);

    lease.release();
  });
});

function createFixture() {
  const scrollOwner = document.createElement('div');
  Object.defineProperties(scrollOwner, {
    clientHeight: { configurable: true, value: 200 },
    scrollHeight: { configurable: true, value: 1_000 }
  });
  const scrollTo = vi.fn(({ top }: ScrollToOptions) => {
    scrollOwner.scrollTop = top ?? scrollOwner.scrollTop;
  });
  Object.defineProperty(scrollOwner, 'scrollTo', {
    configurable: true,
    value: scrollTo
  });
  document.body.append(scrollOwner);
  return { scrollOwner, scrollTo };
}
