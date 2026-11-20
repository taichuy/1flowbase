import { readFileSync } from 'node:fs';
import getDesignToken from 'antd/es/theme/getDesignToken';
import ArrowLeftOutlined from '@ant-design/icons-svg/es/asn/ArrowLeftOutlined';
import CloseCircleFilled from '@ant-design/icons-svg/es/asn/CloseCircleFilled';
import LoadingOutlined from '@ant-design/icons-svg/es/asn/LoadingOutlined';
import { expect, test } from 'vitest';
import { emeraldLightTheme } from '../../../../../../../packages/ui/src/theme';

const css = readFileSync(
  new URL('../default-password.css', import.meta.url),
  'utf8'
);
const primitives = readFileSync(
  new URL('../primitives.tsx', import.meta.url),
  'utf8'
);
const token = getDesignToken(emeraldLightTheme);
function declaration(selector: string, property: string) {
  const start = css.indexOf(`${selector} {`);
  expect(start, selector).toBeGreaterThanOrEqual(0);
  const rule = css.slice(start, css.indexOf('}', start));
  return rule.match(new RegExp(`(?:^|[;{])\\s*${property}:\\s*([^;]+);`))?.[1];
}
function rule(suffix: string, property: string) {
  return declaration(
    `.auth-default-password .auth-password-${suffix}`,
    property
  );
}

test('the scoped primitive metrics/colors follow the installed Emerald Antd token values (not a visual assertion)', () => {
  expect(rule('input', 'padding')).toBe('4px 11px'); // Input initComponentToken: (32 - 14 * 22/14)/2 - 1; paddingSM - 1.
  expect(rule('input', 'border-radius')).toBe(`${token.borderRadius}px`);
  expect(rule('input:focus', 'box-shadow')).toBe(
    emeraldLightTheme.components?.Input?.activeShadow
  );
  expect(rule('button', 'height')).toBe(`${token.controlHeight}px`);
  expect(rule('button', 'font-weight')).toBe(
    String(emeraldLightTheme.components?.Button?.fontWeight)
  );
  expect(rule('button-primary', 'color')).toBe(
    emeraldLightTheme.components?.Button?.primaryColor
  );
  expect(rule('button-primary', 'background')).toBe(
    emeraldLightTheme.components?.Button?.colorPrimary
  );
  expect(rule('button-primary:not(:disabled):hover', 'background')).toBe(
    emeraldLightTheme.components?.Button?.colorPrimaryHover
  );
  expect(rule('button-primary:not(:disabled):active', 'background')).toBe(
    emeraldLightTheme.components?.Button?.colorPrimaryActive
  );
  expect(rule('button-link', 'color')).toBe(token.colorLink);
  expect(rule('button-link:not(:disabled):hover', 'color')).toBe(
    token.colorLinkHover
  );
  expect(rule('button-link:not(:disabled):active', 'color')).toBe(
    token.colorLinkActive
  );
  expect(rule('button:focus-visible', 'outline')).toBe(
    `${token.lineWidthFocus}px solid ${token.colorPrimaryBorder}`
  );
  expect(rule('space-middle', 'gap')).toBe(`${token.padding}px`);
  expect(rule('alert', 'padding')).toBe(
    `${token.paddingContentVerticalSM}px 12px`
  );
  expect(rule('alert', 'border')).toBe(
    `${token.lineWidth}px solid ${token.colorErrorBorder}`
  );
  expect(rule('alert', 'border-radius')).toBe(`${token.borderRadiusLG}px`);
  expect(rule('alert', 'background')).toBe(token.colorErrorBg);
  expect(css.replace(/\/\*[\s\S]*?\*\//g, '')).not.toMatch(
    /\.ant-(?:btn|input|alert|space)/
  );
});

test('auth icons keep the actual installed Antd SVG geometry without loading its icon framework', () => {
  for (const icon of [ArrowLeftOutlined, CloseCircleFilled, LoadingOutlined]) {
    if (typeof icon.icon === 'function')
      throw new Error('Expected a static icon');
    for (const child of icon.icon.children ?? []) {
      expect(primitives).toContain(child.attrs.d);
    }
  }
});
