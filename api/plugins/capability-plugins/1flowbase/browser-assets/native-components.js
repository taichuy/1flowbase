import { jsx as e } from "react/jsx-runtime";
function t({ as: t = "section", children: n, className: r, ...i }) {
	return /* @__PURE__ */ e(t, {
		className: ["oneflow-surface", r].filter(Boolean).join(" "),
		...i,
		children: n
	});
}
function n({ children: n, className: r, ...i }) {
	return /* @__PURE__ */ e(t, {
		className: ["oneflow-scrollable-surface", r].filter(Boolean).join(" "),
		...i,
		children: n
	});
}
export { n as ScrollableSurface, t as Surface };
