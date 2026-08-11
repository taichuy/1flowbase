import { findUnsupportedTailwindUtilityClasses } from '@1flowbase/tailwindcss-catalog/inventory';
import type { BlockProtocolError } from '@1flowbase/page-protocol';

export function diagnoseUnsupportedTailwindUtilities(
  source: string
): BlockProtocolError[] {
  return findUnsupportedTailwindUtilityClasses(source).map(
    ({ className, sourceLocation }, index) => ({
      code: 'transform_failed',
      path: `source.classNames[${index}]`,
      message: className.startsWith('<dynamic')
        ? 'Dynamic className expressions cannot be verified against the official low-code Tailwind inventory; use a static className literal.'
        : `Tailwind utility '${className}' is not available in the official low-code inventory.`,
      sourceLocation
    })
  );
}
