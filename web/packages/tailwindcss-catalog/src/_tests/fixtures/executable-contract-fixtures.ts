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
    dependency_lock: sourceCode.includes("'tailwindcss'")
      ? [fetchedModule('tailwindcss', '4.3.3', 'a')]
      : [],
    compiler_identity: TAILWIND_4_3_3_COMPILER_IDENTITY,
    toolchain_lock: TAILWIND_4_3_3_TOOLCHAIN_LOCK
  };
}

function fetchedModule(source: string, version: string, digestSeed: string) {
  return {
    module_source: source,
    module_version: version,
    binding: 'fetched' as const,
    assets: [
      {
        role: 'browser_module' as const,
        media_type: 'text/javascript',
        sha256: digestSeed.repeat(64),
        url: `/fixture-assets/${source}`
      }
    ],
    exports: ['default']
  };
}

export const executableCompilerFixtures: readonly ExecutableCompilerFixture[] =
  [
    {
      name: 'official template catalog imports',
      request: {
        ...request(
          [
            "import { useState } from 'react';",
            "import 'tailwindcss';",
            "import type { BlockComponentProps } from '@1flowbase/block-sdk';",
            "import { Surface } from '@1flowbase/native-components';",
            'export default function Block({ ctx }: BlockComponentProps) {',
            '  const [count] = useState(0);',
            '  return <Surface className="grid gap-4 p-4">{ctx.workspace.id}{count}</Surface>;',
            '}'
          ].join('\n')
        ),
        dependency_lock: [
          {
            module_source: 'react',
            module_version: '19.2.5',
            binding: 'host',
            assets: [],
            exports: ['default', 'useState']
          },
          fetchedModule('tailwindcss', '4.3.3', 'a'),
          fetchedModule('@1flowbase/block-sdk', '1.0.0', 'b'),
          fetchedModule('@1flowbase/native-components', '0.3.4', 'c')
        ]
      },
      expectedCss: ['display: grid', 'gap:', 'padding:']
    },
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
        "import missing from '@not-in-lock/module'; export default () => <div />;"
      ),
      expectedDiagnostic: 'illegal_import',
      expectedCss: []
    }
  ];
