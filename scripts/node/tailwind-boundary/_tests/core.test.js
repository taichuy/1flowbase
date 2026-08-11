const assert = require('node:assert/strict');
const test = require('node:test');

const {
  collectTailwindBoundaryViolations,
  readStaticClassNameTokens
} = require('../core.js');

const utilities = new Set(['flex', 'gap-4', 'p-4']);

test('AC-006 rejects host-global Tailwind and non-module @apply authoring', () => {
  const violations = collectTailwindBoundaryViolations(
    [
      {
        path: 'web/app/src/styles/globals.css',
        content: '@import "tailwindcss";'
      },
      {
        path: 'web/app/src/features/demo/demo.css',
        content: '.demo { @apply flex; }'
      }
    ],
    utilities
  );

  assert.deepEqual(
    violations.map(({ code }) => code),
    ['global-tailwind-entry', 'tailwind-owner-required']
  );
});

test('AC-007 allows CSS Modules theme references and rejects direct TSX utilities', () => {
  const files = [
    {
      path: 'web/app/src/features/demo/demo.module.css',
      content:
        '@reference "tailwindcss/theme.css";\n.root { @apply flex gap-4 p-4; }'
    },
    {
      path: 'web/app/src/features/demo/Demo.tsx',
      content: 'export const Demo = () => <div className="flex demo" />;'
    }
  ];

  assert.deepEqual(
    collectTailwindBoundaryViolations(
      files,
      utilities,
      new Set(['web/app/src/features/demo/demo.module.css'])
    ).map(({ code }) => code),
    ['direct-tailwind-utility']
  );
  assert.deepEqual(readStaticClassNameTokens(files[1].content), [
    'flex',
    'demo'
  ]);
});

test('AC-008 requires every Tailwind CSS Module to map to a style-boundary owner', () => {
  const file = {
    path: 'web/app/src/features/demo/demo.module.css',
    content: '@reference "tailwindcss/theme.css";\n.root { @apply flex; }'
  };

  assert.deepEqual(
    collectTailwindBoundaryViolations([file], utilities).map(({ code }) => code),
    ['style-boundary-owner-missing']
  );
  assert.deepEqual(
    collectTailwindBoundaryViolations(
      [file],
      utilities,
      new Set([file.path])
    ),
    []
  );
});

test('AC-007 rejects full Tailwind imports inside CSS Modules', () => {
  const violations = collectTailwindBoundaryViolations(
    [
      {
        path: 'web/app/src/features/demo/demo.module.css',
        content: '@import "tailwindcss";'
      }
    ],
    utilities
  );
  assert.deepEqual(violations.map(({ code }) => code), [
    'module-full-tailwind-import',
    'style-boundary-owner-missing'
  ]);
});
