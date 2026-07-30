import { jsx as e } from "react/jsx-runtime";
//#region packages/native-components/src/index.tsx
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
//#endregion
export { n as ScrollableSurface, t as Surface };
