const INVENTORY_SOURCE = `
block inline-block inline flex inline-flex grid hidden contents
relative absolute fixed sticky static
inset-0 top-0 right-0 bottom-0 left-0 z-10 z-20 z-50
flex-row flex-row-reverse flex-col flex-col-reverse flex-wrap flex-nowrap
items-start items-center items-end items-stretch items-baseline
justify-start justify-center justify-end justify-between justify-around justify-evenly
content-start content-center content-end content-between self-start self-center self-end self-stretch
grow grow-0 shrink shrink-0 basis-0 basis-auto basis-1/2 basis-1/3 basis-2/3 basis-full
grid-cols-1 grid-cols-2 grid-cols-3 grid-cols-4 grid-cols-6 grid-cols-12
col-span-1 col-span-2 col-span-3 col-span-4 col-span-6 col-span-12 col-span-full
grid-rows-1 grid-rows-2 grid-rows-3 grid-flow-row grid-flow-col
gap-0 gap-1 gap-2 gap-3 gap-4 gap-5 gap-6 gap-8 gap-10 gap-12
gap-x-1 gap-x-2 gap-x-3 gap-x-4 gap-x-6 gap-x-8
gap-y-1 gap-y-2 gap-y-3 gap-y-4 gap-y-6 gap-y-8
p-0 p-1 p-2 p-3 p-4 p-5 p-6 p-8 p-10 p-12 p-16 p-20 p-24
px-0 px-1 px-2 px-3 px-4 px-5 px-6 px-8 px-10 px-12 px-16
py-0 py-1 py-2 py-3 py-4 py-5 py-6 py-8 py-10 py-12 py-16
pt-0 pt-1 pt-2 pt-3 pt-4 pt-6 pt-8 pb-0 pb-1 pb-2 pb-3 pb-4 pb-6 pb-8
pl-0 pl-1 pl-2 pl-3 pl-4 pl-6 pl-8 pr-0 pr-1 pr-2 pr-3 pr-4 pr-6 pr-8
m-0 m-1 m-2 m-3 m-4 m-6 m-8 m-auto
mx-0 mx-1 mx-2 mx-3 mx-4 mx-6 mx-8 mx-auto
my-0 my-1 my-2 my-3 my-4 my-6 my-8 my-auto
mt-0 mt-1 mt-2 mt-3 mt-4 mt-6 mt-8 mt-auto mb-0 mb-1 mb-2 mb-3 mb-4 mb-6 mb-8 mb-auto
ml-0 ml-1 ml-2 ml-3 ml-4 ml-6 ml-8 ml-auto mr-0 mr-1 mr-2 mr-3 mr-4 mr-6 mr-8 mr-auto
w-0 w-1 w-2 w-3 w-4 w-6 w-8 w-10 w-12 w-16 w-20 w-24 w-32 w-40 w-48 w-64
w-auto w-1/2 w-1/3 w-2/3 w-1/4 w-3/4 w-full w-screen
h-0 h-1 h-2 h-3 h-4 h-6 h-8 h-10 h-12 h-16 h-20 h-24 h-32 h-40 h-48 h-64
h-auto h-full h-screen min-w-0 min-w-full min-h-0 min-h-full min-h-screen
max-w-xs max-w-sm max-w-md max-w-lg max-w-xl max-w-2xl max-w-3xl max-w-4xl max-w-5xl max-w-6xl max-w-7xl max-w-full max-w-none
max-h-32 max-h-48 max-h-64 max-h-80 max-h-96 max-h-full max-h-screen
overflow-auto overflow-hidden overflow-visible overflow-scroll overflow-x-auto overflow-x-hidden overflow-y-auto overflow-y-hidden
text-left text-center text-right text-xs text-sm text-base text-lg text-xl text-2xl text-3xl
font-normal font-medium font-semibold font-bold italic not-italic
leading-none leading-tight leading-snug leading-normal leading-relaxed leading-loose
tracking-tight tracking-normal tracking-wide whitespace-normal whitespace-nowrap whitespace-pre-wrap
truncate break-words break-all select-none select-text
rounded-none rounded-sm rounded rounded-md rounded-lg rounded-xl rounded-2xl rounded-full
border border-0 border-2 border-t border-r border-b border-l
border-transparent border-slate-200 border-slate-300 border-gray-200 border-gray-300 border-red-500 border-amber-500 border-emerald-500 border-blue-500 border-sky-500
bg-transparent bg-white bg-black bg-slate-50 bg-slate-100 bg-slate-500 bg-slate-600 bg-gray-50 bg-gray-100 bg-gray-500 bg-gray-600
bg-red-50 bg-red-100 bg-red-500 bg-red-600 bg-amber-50 bg-amber-100 bg-amber-500 bg-amber-600
bg-emerald-50 bg-emerald-100 bg-emerald-500 bg-emerald-600 bg-blue-50 bg-blue-100 bg-blue-500 bg-blue-600 bg-sky-50 bg-sky-100 bg-sky-500 bg-sky-600
text-white text-black text-slate-500 text-slate-600 text-slate-700 text-slate-900 text-gray-500 text-gray-600 text-gray-700 text-gray-900
text-red-500 text-red-600 text-red-700 text-amber-500 text-amber-600 text-amber-700 text-emerald-500 text-emerald-600 text-emerald-700 text-blue-500 text-blue-600 text-blue-700 text-sky-500 text-sky-600 text-sky-700
shadow-none shadow-sm shadow shadow-md shadow-lg shadow-xl
opacity-0 opacity-25 opacity-50 opacity-75 opacity-100
cursor-default cursor-pointer cursor-not-allowed pointer-events-none pointer-events-auto
transition transition-colors transition-opacity duration-150 duration-200 duration-300 ease-in ease-out ease-in-out
hover:bg-slate-100 hover:bg-gray-100 hover:bg-red-600 hover:bg-amber-600 hover:bg-emerald-600 hover:bg-blue-600 hover:bg-sky-600
hover:text-slate-900 hover:text-gray-900 hover:text-white focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-emerald-500
disabled:cursor-not-allowed disabled:opacity-50
sm:block sm:flex sm:grid sm:hidden sm:flex-row sm:flex-col sm:grid-cols-2 sm:grid-cols-3 sm:gap-4 sm:p-4
md:block md:flex md:grid md:hidden md:flex-row md:flex-col md:grid-cols-2 md:grid-cols-3 md:grid-cols-4 md:gap-4 md:gap-6 md:p-6
lg:block lg:flex lg:grid lg:hidden lg:flex-row lg:flex-col lg:grid-cols-2 lg:grid-cols-3 lg:grid-cols-4 lg:grid-cols-6 lg:gap-6 lg:gap-8 lg:p-8
`;

export const TAILWIND_UTILITY_CLASS_NAMES = Object.freeze(
  INVENTORY_SOURCE.trim().split(/\s+/u)
);

const utilityClassNames = new Set(TAILWIND_UTILITY_CLASS_NAMES);
const tailwindImportPattern = /(?:import|export)\s+(?:[^'";]+?\s+from\s+)?['"]tailwindcss['"]/u;
const staticClassNamePattern =
  /\bclassName\s*=\s*(?:\{\s*)?(['"`])([^'"`]*?)\1\s*\}?/gu;
const classNameAssignmentPattern = /\bclassName\s*=/gu;

export interface UnsupportedTailwindUtilityClass {
  className: string;
  sourceLocation: {
    line: number;
    column: number;
    endLine: number;
    endColumn: number;
  };
}

export function findUnsupportedTailwindUtilityClasses(
  source: string
): UnsupportedTailwindUtilityClass[] {
  if (!tailwindImportPattern.test(source)) return [];

  const unsupported: UnsupportedTailwindUtilityClass[] = [];
  const staticMatches = [...source.matchAll(staticClassNamePattern)];
  const staticAssignmentOffsets = new Set(
    staticMatches.map((match) => match.index ?? 0)
  );
  for (const match of staticMatches) {
    const value = match[2] ?? '';
    const valueOffset = (match.index ?? 0) + match[0].indexOf(value);
    for (const classMatch of value.matchAll(/\S+/gu)) {
      const className = classMatch[0];
      if (utilityClassNames.has(className)) continue;
      const start = valueOffset + (classMatch.index ?? 0);
      const startLocation = offsetToLocation(source, start);
      unsupported.push({
        className,
        sourceLocation: {
          ...startLocation,
          endLine: startLocation.line,
          endColumn: startLocation.column + className.length
        }
      });
    }
  }
  for (const match of source.matchAll(classNameAssignmentPattern)) {
    const start = match.index ?? 0;
    if (staticAssignmentOffsets.has(start)) continue;
    const startLocation = offsetToLocation(source, start);
    unsupported.push({
      className: '<dynamic className expression>',
      sourceLocation: {
        ...startLocation,
        endLine: startLocation.line,
        endColumn: startLocation.column + 'className'.length
      }
    });
  }
  return unsupported;
}

function offsetToLocation(source: string, offset: number) {
  const prefix = source.slice(0, offset);
  const lines = prefix.split('\n');
  return {
    line: lines.length,
    column: (lines.at(-1)?.length ?? 0) + 1
  };
}
