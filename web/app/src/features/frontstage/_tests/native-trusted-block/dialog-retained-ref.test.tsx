import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { runInNewContext } from 'node:vm';
import * as React from 'react';
import { render } from '@testing-library/react';
import ts from 'typescript';
import { afterEach, describe, expect, test } from 'vitest';

type PanelHandle = { focus(): void };
type MotionHandle = { inMotion(): boolean; enableMotion(): boolean };
type ContentHandle = PanelHandle & MotionHandle;

const require = createRequire(import.meta.url);
const requireFromAntd = createRequire(require.resolve('antd/package.json'));
const dialogRoot = dirname(
  requireFromAntd.resolve('@rc-component/dialog/package.json')
);
const requireFromDialog = createRequire(join(dialogRoot, 'package.json'));
const hosts: HTMLElement[] = [];
const unmounts: Array<() => void> = [];

afterEach(() => {
  // Unmount owned fixtures before restoring timers or removing their external hosts.
  unmounts.splice(0).forEach((unmount) => unmount());
  for (const host of hosts.splice(0)) host.remove();
});

// Execute the installed Content in both shipped formats. Only its child ref
// boundaries are controlled: Activity's child/parent reconnect ordering must
// be reproducible without depending on an intermittent browser schedule.
function fixture(
  format: 'es' | 'lib',
  initialPanel: PanelHandle | null,
  initialMotion: MotionHandle | null
) {
  let panelRef: React.RefObject<PanelHandle | null>;
  let motionRef: React.RefObject<MotionHandle | null>;
  const Panel = React.forwardRef<PanelHandle>((_props, ref) => {
    React.useLayoutEffect(() => {
      panelRef = ref as React.RefObject<PanelHandle | null>;
      panelRef.current = initialPanel;
      return () => {
        panelRef.current = null;
      };
    }, []);
    return null;
  });
  const Motion = React.forwardRef<
    MotionHandle,
    {
      children: (state: object, ref: () => void) => React.ReactNode;
    }
  >((props, ref) => {
    React.useLayoutEffect(() => {
      motionRef = ref as React.RefObject<MotionHandle | null>;
      motionRef.current = initialMotion;
      return () => {
        motionRef.current = null;
      };
    }, []);
    return props.children({}, () => {});
  });
  const path = join(dialogRoot, format, 'Dialog/Content/index.js');
  const source = readFileSync(path, 'utf8');
  const executable =
    format === 'lib'
      ? source
      : ts.transpileModule(source, {
          compilerOptions: {
            module: ts.ModuleKind.CommonJS,
            target: ts.ScriptTarget.ES2022
          }
        }).outputText;
  const module = {
    exports: {} as {
      default: React.ForwardRefExoticComponent<
        {
          visible: boolean;
        } & React.RefAttributes<ContentHandle>
      >;
    }
  };
  runInNewContext(
    executable,
    {
      module,
      exports: module.exports,
      process,
      require: (specifier: string) => {
        if (specifier === 'react') return React;
        if (specifier === '@rc-component/motion')
          return { __esModule: true, default: Motion };
        if (specifier === './Panel')
          return { __esModule: true, default: Panel };
        return requireFromDialog(
          specifier.startsWith('.') ? join(dirname(path), specifier) : specifier
        );
      }
    },
    { filename: path }
  );
  return {
    Content: module.exports.default,
    reconnect(panel: PanelHandle | null, motion: MotionHandle | null) {
      panelRef.current = panel;
      motionRef.current = motion;
    }
  };
}

function focusTarget() {
  const input = document.createElement('input');
  document.body.append(input);
  hosts.push(input);
  return input;
}

describe.each(['es', 'lib'] as const)(
  'installed dialog Content %s retained refs',
  (format) => {
    test('exposes a stable callable handle before the panel or motion reconnects', () => {
      const { Content, reconnect } = fixture(format, null, null);
      const ref = React.createRef<ContentHandle>();
      const view = render(<Content ref={ref} visible />);
      unmounts.push(view.unmount);
      const handle = ref.current!;
      expect(typeof handle.focus).toBe('function');
      expect(() => handle.focus()).not.toThrow();
      expect(handle.inMotion()).toBe(false);
      expect(handle.enableMotion()).toBe(false);
      const input = focusTarget();
      reconnect(
        { focus: () => input.focus() },
        {
          inMotion: () => true,
          enableMotion: () => true
        }
      );
      handle.focus();
      expect(input).toHaveFocus();
      expect(handle.inMotion()).toBe(true);
      expect(handle.enableMotion()).toBe(true);
      view.rerender(<Content ref={ref} visible={false} />);
      expect(ref.current).toBe(handle);
    });

    test('forwards focus and motion to current children after old refs disappear', () => {
      const oldPanel = focusTarget();
      const oldMotion = { inMotion: () => true, enableMotion: () => false };
      const { Content, reconnect } = fixture(
        format,
        {
          focus: () => oldPanel.focus()
        },
        oldMotion
      );
      const ref = React.createRef<ContentHandle>();
      const view = render(<Content ref={ref} visible />);
      unmounts.push(view.unmount);
      const handle = ref.current!;
      handle.focus();
      expect(oldPanel).toHaveFocus();
      expect(handle.inMotion()).toBe(true);
      expect(handle.enableMotion()).toBe(false);

      oldPanel.remove();
      reconnect(null, null);
      expect(() => handle.focus()).not.toThrow();
      expect(handle.inMotion()).toBe(false);
      expect(handle.enableMotion()).toBe(false);

      const currentPanel = focusTarget();
      const currentMotion = {
        moving: false,
        enabled: true,
        inMotion() {
          return this.moving;
        },
        enableMotion() {
          return this.enabled;
        }
      };
      reconnect({ focus: () => currentPanel.focus() }, currentMotion);
      handle.focus();
      expect(currentPanel).toHaveFocus();
      expect(handle.inMotion()).toBe(false);
      expect(handle.enableMotion()).toBe(true);
      currentMotion.moving = true;
      currentMotion.enabled = false;
      expect(handle.inMotion()).toBe(true);
      expect(handle.enableMotion()).toBe(false);
      expect(ref.current).toBe(handle);
    });
  }
);
