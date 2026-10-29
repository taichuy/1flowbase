import js from '@eslint/js';
import globals from 'globals';
import reactHooks from 'eslint-plugin-react-hooks';
import reactRefresh from 'eslint-plugin-react-refresh';
import testingLibrary from 'eslint-plugin-testing-library';
import jestDom from 'eslint-plugin-jest-dom';
import tseslint from 'typescript-eslint';
import lifecycleRules from '../scripts/node/testing-library-lifecycle-rules/index.js';

export default tseslint.config(
  {
    ignores: ['dist', 'coverage', 'node_modules']
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ['packages/rich-text/src/_tests/browser/*.cjs'],
    languageOptions: {
      sourceType: 'commonjs',
      globals: { ...globals.node, ...globals.browser }
    },
    rules: {
      // These Node browser probes intentionally load dependencies through CommonJS.
      '@typescript-eslint/no-require-imports': 'off'
    }
  },
  {
    files: ['**/*.{ts,tsx}'],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      globals: {
        ...globals.browser,
        ...globals.node
      }
    },
    plugins: {
      'react-hooks': lifecycleRules.refineReactHooksPlugin(reactHooks),
      'react-refresh': lifecycleRules.refineReactRefreshPlugin(reactRefresh)
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_' }
      ],
      'react-refresh/only-export-components': ['warn', { allowConstantExport: true }]
    }
  },
  {
    files: ['**/*.test.{ts,tsx}'],
    plugins: {
      'testing-library': lifecycleRules.refineTestingLibraryPlugin(testingLibrary),
      'jest-dom': jestDom
    },
    rules: {
      ...testingLibrary.configs.react.rules,
      ...jestDom.configs.recommended.rules,
      // Structural and style-boundary tests intentionally inspect DOM topology.
      'testing-library/no-container': 'off',
      'testing-library/no-node-access': 'off'
    }
  }
);
