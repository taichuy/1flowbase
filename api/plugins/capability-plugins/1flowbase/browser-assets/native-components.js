import React from "react";

export function Surface({ as = "section", children, ...props }) {
  return React.createElement(as, props, children);
}
