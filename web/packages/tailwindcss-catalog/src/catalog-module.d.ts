declare module 'tailwindcss' {
  interface TailwindCompiler {
    build(candidates: string[]): string;
  }

  export function compile(css: string): Promise<TailwindCompiler>;

  const tailwindcss: Readonly<{
    name: 'tailwindcss';
    version: '4.3.3';
    mode: 'block-preset';
    compiler: Readonly<{
      name: '@1flowbase/tailwindcss-catalog';
      contract: 'block-preset-v1';
      tailwind_version: '4.3.3';
    }>;
  }>;
  export default tailwindcss;
}
