import {
  TAILWIND_4_3_3_COMPILER_IDENTITY,
  TAILWIND_4_3_3_TOOLCHAIN_LOCK,
  type TailwindExecutableCompilerRequest
} from '../../executable-contract';

export interface ExecutableCompilerFixture {
  name: string;
  request: TailwindExecutableCompilerRequest;
  expectedDiagnostic?: 'tsx_transform_failed' | 'illegal_import';
  expectedCss: readonly string[];
  excludedCss?: readonly string[];
}

function request(sourceCode: string): TailwindExecutableCompilerRequest {
  return {
    source_code: sourceCode,
    compiler_identity: TAILWIND_4_3_3_COMPILER_IDENTITY,
    toolchain_lock: TAILWIND_4_3_3_TOOLCHAIN_LOCK
  };
}

export const executableCompilerFixtures: readonly ExecutableCompilerFixture[] =
  [
    {
      name: 'Tailwind variants and arbitrary values',
      request: request(
        [
          "import 'tailwindcss';",
          'export default function Block() {',
          '  return <div className="grid grid-cols-[200px_1fr] bg-[#00ab73] md:grid-cols-2 hover:[&>span]:opacity-80"><span /></div>;',
          '}'
        ].join('\n')
      ),
      expectedCss: ['200px 1fr', '#00ab73', '@media', 'span'],
      excludedCss: ['@layer base', 'button,input']
    },
    {
      name: 'authored CSS class beside a Tailwind utility',
      request: request(
        [
          "import 'tailwindcss';",
          "const CSS = '.hero { color: red; }';",
          'export default () => <section className="hero mt-3" />;'
        ].join('\n')
      ),
      expectedCss: ['margin-top'],
      excludedCss: ['.hero']
    },
    {
      name: 'source without Tailwind',
      request: request(
        'export default function Plain() { return <div className="plain" />; }'
      ),
      expectedCss: []
    },
    {
      name: 'invalid TSX',
      request: request(
        "import 'tailwindcss'; export default () => <div><span></div>;"
      ),
      expectedDiagnostic: 'tsx_transform_failed',
      expectedCss: []
    },
    {
      name: 'illegal import',
      request: request(
        "import fs from 'node:fs'; export default () => <div />;"
      ),
      expectedDiagnostic: 'illegal_import',
      expectedCss: []
    }
  ];
