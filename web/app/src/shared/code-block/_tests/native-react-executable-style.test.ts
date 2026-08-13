import { describe, expect, test } from 'vitest';

import {
  compileNativeReactExecutableStyle,
  readLockedNativeReactExecutableStyle
} from '../native-react-executable-style';

describe('Native React executable style', () => {
  test('AC-001/002 compiles static Tailwind without a private inventory gate', async () => {
    const result = await compileNativeReactExecutableStyle(
      'import \'tailwindcss\'; export default () => <div className="hero bg-[#00ab73] md:grid-cols-2" />;'
    );
    expect(result.generated_css).toContain('#00ab73');
    expect(result.generated_css).toContain('@media');
  });

  test('AC-011 rejects legacy, incomplete, and digest-mismatched rows', () => {
    const ready = {
      source_code: 'export default null;',
      source_sha256:
        '6fcd1d591edc5697a4972c2cb3e83808f0656dbb077fd89eea085d0221601ee7',
      dependency_lock: [],
      tailwind_toolchain_lock: { package: 'tailwindcss' },
      generated_css: '',
      generated_css_sha256:
        'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
      compiler_identity: { name: 'tailwindcss' },
      executable_state: 'ready' as const
    };
    expect(() =>
      readLockedNativeReactExecutableStyle({
        ...ready,
        executable_state: 'legacy'
      })
    ).toThrow(/legacy or incomplete/u);
    expect(() =>
      readLockedNativeReactExecutableStyle({
        ...ready,
        generated_css_sha256: '0'.repeat(64)
      })
    ).toThrow(/generated_css_sha256/u);
  });
});
