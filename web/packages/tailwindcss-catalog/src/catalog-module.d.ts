declare module 'tailwindcss' {
  const tailwindcss: Readonly<{
    name: 'tailwindcss';
    version: '4.3.3';
    mode: 'theme-and-utilities';
    compiler: Readonly<{
      name: '@1flowbase/tailwindcss-catalog';
      contract: 'source-driven-utilities-v1';
      tailwind_version: '4.3.3';
    }>;
  }>;
  export default tailwindcss;
}
