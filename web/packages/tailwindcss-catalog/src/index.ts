const tailwindcss = Object.freeze({
  name: 'tailwindcss',
  version: '4.3.3',
  mode: 'theme-and-utilities',
  compiler: Object.freeze({
    name: '@1flowbase/tailwindcss-catalog',
    contract: 'source-driven-utilities-v1',
    tailwind_version: '4.3.3'
  })
} as const);

export default tailwindcss;

export * from './executable-contract.ts';
