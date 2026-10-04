import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { runInThisContext } from 'node:vm';
import * as React from 'react';
import { act, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
type CSSMotionRef = { inMotion(): boolean; enableMotion(): boolean };
type CSSMotionProps = {
  visible: boolean;
  motionName: string;
  motionDeadline: number;
  removeOnLeave: boolean;
  onAppearStart(): React.CSSProperties;
  onAppearActive(): React.CSSProperties;
  onEnterStart(): React.CSSProperties;
  onEnterActive(): React.CSSProperties;
  onLeaveStart(): React.CSSProperties;
  onLeaveActive(): React.CSSProperties;
  onVisibleChanged(visible: boolean): void;
  children(
    props: { className?: string; style?: React.CSSProperties },
    ref: React.Ref<HTMLElement>,
  ): React.ReactElement;
};

const require = createRequire(import.meta.url);
const requireFromAntd = createRequire(require.resolve('antd/package.json'));
const requireFromDialog = createRequire(
  requireFromAntd.resolve('@rc-component/dialog/package.json'),
);
const motionRoot = dirname(
  requireFromDialog.resolve('@rc-component/motion/package.json'),
);
const motionPath = join(motionRoot, 'lib/CSSMotion.js');
const requireFromMotion = createRequire(motionPath);

// A negative probe can replace only useStatus with its saved original source.
// All child rendering, step queues and React Activity behavior remain real.
function loadMotion() {
  const original = process.env.FRONTSTAGE_MOTION_ORIGINAL_USE_STATUS;
  if (!original) return requireFromMotion(motionPath);
  const hook = { exports: {} };
  const hookPath = join(motionRoot, 'lib/hooks/useStatus.js');
  runInThisContext(
    `(function(require,module,exports){${readFileSync(original, 'utf8')}\n})`,
    {
      filename: hookPath,
    },
  )(createRequire(hookPath), hook, hook.exports);
  const module = { exports: {} };
  runInThisContext(
    `(function(require,module,exports){${readFileSync(motionPath, 'utf8')}\n})`,
    {
      filename: motionPath,
    },
  )(
    (specifier: string) =>
      specifier === './hooks/useStatus'
        ? hook.exports
        : requireFromMotion(specifier),
    module,
    module.exports,
  );
  return module.exports;
}

const { genCSSMotion } = loadMotion() as {
  genCSSMotion(
    support: boolean,
  ): React.ForwardRefExoticComponent<
    CSSMotionProps & React.RefAttributes<CSSMotionRef>
  >;
};
const Motion = genCSSMotion(true);

beforeEach(() => vi.useFakeTimers());
const unmounts: Array<() => void> = [];
afterEach(() => {
  unmounts.splice(0).forEach((unmount) => unmount());
  vi.useRealTimers();
});

async function advance(milliseconds: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(milliseconds);
  });
}

function fixture(removeOnLeave = false) {
  const ref = React.createRef<CSSMotionRef>();
  const changed = vi.fn();
  const appear = vi.fn(() => ({ opacity: 0 }));
  const leave = vi.fn(() => ({ opacity: 0 }));
  const tree = (mode: 'visible' | 'hidden', visible: boolean) => (
    <React.Activity mode={mode}>
      <Motion
        ref={ref}
        visible={visible}
        motionName="retained-probe"
        motionDeadline={1000}
        removeOnLeave={removeOnLeave}
        onAppearStart={appear}
        onAppearActive={() => ({ opacity: 1 })}
        onEnterStart={() => ({ opacity: 0 })}
        onEnterActive={() => ({ opacity: 1 })}
        onLeaveStart={leave}
        onLeaveActive={() => ({ opacity: 0 })}
        onVisibleChanged={changed}
      >
        {({ className, style }, motionRef) => (
          <section
            ref={motionRef as React.Ref<HTMLElement>}
            className={className}
            style={style}
          >
            <input aria-label="retained input" defaultValue="initial" />
          </section>
        )}
      </Motion>
    </React.Activity>
  );
  const view = render(tree('visible', true));
  unmounts.push(view.unmount);
  return {
    ref,
    changed,
    appear,
    leave,
    view,
    show(mode: 'visible' | 'hidden', visible = true) {
      view.rerender(tree(mode, visible));
    },
    panel() {
      return view.container.querySelector('section')!;
    },
    input() {
      return view.container.querySelector('input')!;
    },
  };
}

describe('installed CSSMotion retains committed children across Activity effects', () => {
  test('keeps settled DOM and uncontrolled input without replaying appear or its notification', async () => {
    const f = fixture();
    await advance(100);
    await advance(1000);
    const panel = f.panel();
    const input = f.input();
    expect(panel).not.toBeNull();
    input.value = 'retained settled value';
    expect(f.ref.current!.inMotion()).toBe(false);
    expect(f.changed.mock.calls).toEqual([[true]]);
    const appearCount = f.appear.mock.calls.length;
    f.show('hidden');
    expect(f.panel()).toBe(panel);
    expect(f.ref.current).toBeNull();
    f.show('visible');
    expect(f.panel()).toBe(panel);
    expect(f.input()).toBe(input);
    expect(input.value).toBe('retained settled value');
    expect(f.ref.current!.inMotion()).toBe(false);
    expect(f.appear).toHaveBeenCalledTimes(appearCount);
    expect(f.changed.mock.calls).toEqual([[true]]);
  });

  test('settles interrupted appear on reconnect while keeping original DOM and child state', async () => {
    const f = fixture();
    await advance(100);
    const panel = f.panel();
    const input = f.input();
    expect(panel).not.toBeNull();
    input.value = 'interrupted animation value';
    expect(f.ref.current!.inMotion()).toBe(true);
    expect(f.changed).not.toHaveBeenCalled();
    f.show('hidden');
    await advance(2000);
    f.show('visible');
    expect(f.panel()).toBe(panel);
    expect(f.input()).toBe(input);
    expect(input.value).toBe('interrupted animation value');
    expect(f.ref.current!.inMotion()).toBe(false);
    expect(f.changed.mock.calls).toEqual([[true]]);
  });

  test('honors real visible changes made while hidden and notifies each resulting steady state', async () => {
    const f = fixture();
    await advance(100);
    await advance(1000);
    const panel = f.panel();
    const input = f.input();
    input.value = 'hidden change value';
    f.show('hidden', false);
    f.show('visible', false);
    expect(f.panel()).toBe(panel);
    expect(panel).toHaveStyle({ display: 'none' });
    expect(f.ref.current!.inMotion()).toBe(false);
    expect(f.changed.mock.calls).toEqual([[true], [false]]);
    f.show('hidden', true);
    f.show('visible', true);
    expect(f.panel()).toBe(panel);
    expect(f.input()).toBe(input);
    expect(input.value).toBe('hidden change value');
    expect(panel).not.toHaveStyle({ display: 'none' });
    expect(f.changed.mock.calls).toEqual([[true], [false], [true]]);
  });

  test('still animates ordinary first appear and close, then removes the child', async () => {
    const f = fixture(true);
    await advance(100);
    expect(f.appear).toHaveBeenCalledTimes(1);
    expect(f.ref.current!.inMotion()).toBe(true);
    await advance(1000);
    expect(f.changed.mock.calls).toEqual([[true]]);
    f.show('visible', false);
    await advance(100);
    expect(f.leave).toHaveBeenCalledTimes(1);
    expect(f.ref.current!.inMotion()).toBe(true);
    expect(f.panel()).not.toBeNull();
    await advance(1000);
    expect(f.panel()).toBeNull();
    expect(f.ref.current!.inMotion()).toBe(false);
    expect(f.changed.mock.calls).toEqual([[true], [false]]);
  });
});
