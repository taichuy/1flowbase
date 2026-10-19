import { isInaccessible } from '@testing-library/react';

// HTML-ARIA maps <search> to the search landmark. The pinned aria-query table
// omits this native mapping, so keep the native element and the query's default
// accessibility/uniqueness constraints without changing the global role map.
export function getNativeSearchLandmark(): HTMLElement {
  const landmarks = Array.from(
    document.body.querySelectorAll<HTMLElement>(
      'search:not([role]), search[role="search"]'
    )
  ).filter((element) => !isInaccessible(element));
  const [landmark] = landmarks;

  if (!landmark || landmarks.length !== 1) {
    throw new Error(
      `Expected one accessible native <search> landmark; found ${landmarks.length}`
    );
  }

  return landmark;
}
