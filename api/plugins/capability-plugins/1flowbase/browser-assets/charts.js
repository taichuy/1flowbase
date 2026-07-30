import { useEffect as e, useRef as t } from "react";
import { jsx as n } from "react/jsx-runtime";
//#region \0rolldown/runtime.js
var r = Object.defineProperty, i = (e, t) => {
	let n = {};
	for (var i in e) r(n, i, {
		get: e[i],
		enumerable: !0
	});
	return t || r(n, Symbol.toStringTag, { value: "Module" }), n;
}, a = function(e, t) {
	return a = Object.setPrototypeOf || { __proto__: [] } instanceof Array && function(e, t) {
		e.__proto__ = t;
	} || function(e, t) {
		for (var n in t) Object.prototype.hasOwnProperty.call(t, n) && (e[n] = t[n]);
	}, a(e, t);
};
function o(e, t) {
	if (typeof t != "function" && t !== null) throw TypeError("Class extends value " + String(t) + " is not a constructor or null");
	a(e, t);
	function n() {
		this.constructor = e;
	}
	e.prototype = t === null ? Object.create(t) : (n.prototype = t.prototype, new n());
}
var s = "12px sans-serif", c = 20, l = 100, u = "007LLmW'55;N0500LLLLLLLLLL00NNNLzWW\\\\WQb\\0FWLg\\bWb\\WQ\\WrWWQ000CL5LLFLL0LL**F*gLLLL5F0LF\\FFF5.5N";
function d(e) {
	var t = {};
	if (typeof JSON > "u") return t;
	for (var n = 0; n < e.length; n++) {
		var r = String.fromCharCode(n + 32);
		t[r] = (e.charCodeAt(n) - c) / l;
	}
	return t;
}
var f = d(u), p = {
	createCanvas: function() {
		return typeof document < "u" && document.createElement("canvas");
	},
	measureText: (function() {
		var e, t;
		return function(n, r) {
			if (!e) {
				var i = p.createCanvas();
				e = i && i.getContext("2d");
			}
			if (e) return t !== r && (t = e.font = r || "12px sans-serif"), e.measureText(n);
			n ||= "", r ||= "12px sans-serif";
			var a = /((?:\d+)?\.?\d*)px/.exec(r), o = a && +a[1] || 12, s = 0;
			if (r.indexOf("mono") >= 0) s = o * n.length;
			else for (var c = 0; c < n.length; c++) {
				var l = f[n[c]];
				s += l == null ? o : l * o;
			}
			return { width: s };
		};
	})(),
	loadImage: function(e, t, n) {
		var r = new Image();
		return r.onload = t, r.onerror = n, r.src = e, r;
	},
	getTime: function() {
		return Date.now ? Date.now() : +/* @__PURE__ */ new Date();
	}
}, m = ne([
	"Function",
	"RegExp",
	"Date",
	"Error",
	"CanvasGradient",
	"CanvasPattern",
	"Image",
	"Canvas"
], function(e, t) {
	return e["[object " + t + "]"] = !0, e;
}, {}), h = ne([
	"Int8",
	"Uint8",
	"Uint8Clamped",
	"Int16",
	"Uint16",
	"Int32",
	"Uint32",
	"Float32",
	"Float64"
], function(e, t) {
	return e["[object " + t + "Array]"] = !0, e;
}, {}), g = Object.prototype.toString, _ = Array.prototype, v = _.forEach, y = _.filter, b = _.slice, x = _.map, S = function() {}.constructor, C = S ? S.prototype : null, w = "__proto__", T = 2311, E = 2 ** 53 - 1;
function D() {
	return T >= E && (T = 0), T++;
}
function O() {
	var e = [...arguments];
	typeof console < "u" && console.error.apply(console, e);
}
function k(e) {
	if (typeof e != "object" || !e) return e;
	var t = e, n = g.call(e);
	if (n === "[object Array]") {
		if (!Se(e)) {
			t = [];
			for (var r = 0, i = e.length; r < i; r++) t[r] = k(e[r]);
		}
	} else if (h[n]) {
		if (!Se(e)) {
			var a = e.constructor;
			if (a.from) t = a.from(e);
			else {
				t = new a(e.length);
				for (var r = 0, i = e.length; r < i; r++) t[r] = e[r];
			}
		}
	} else if (!m[n] && !Se(e) && !ue(e)) for (var o in t = {}, e) e.hasOwnProperty(o) && o !== w && (t[o] = k(e[o]));
	return t;
}
function A(e, t, n) {
	if (!W(t) || !W(e)) return n ? k(t) : e;
	for (var r in t) if (t.hasOwnProperty(r) && r !== w) {
		var i = e[r], a = t[r];
		W(a) && W(i) && !V(a) && !V(i) && !ue(a) && !ue(i) && !ce(a) && !ce(i) && !Se(a) && !Se(i) ? A(i, a, n) : (n || !(r in e)) && (e[r] = k(t[r]));
	}
	return e;
}
function j(e, t) {
	if (Object.assign) Object.assign(e, t);
	else for (var n in t) t.hasOwnProperty(n) && n !== w && (e[n] = t[n]);
	return e;
}
function ee(e, t, n) {
	e ||= {};
	for (var r = 0; r < n.length; r++) {
		var i = n[r];
		e[i] = t[i];
	}
	return e;
}
function M(e, t, n) {
	for (var r = R(t), i = 0, a = r.length; i < a; i++) {
		var o = r[i];
		(n ? t[o] != null : e[o] == null) && (e[o] = t[o]);
	}
	return e;
}
p.createCanvas;
function N(e, t) {
	if (e) {
		if (e.indexOf) return e.indexOf(t);
		for (var n = 0, r = e.length; n < r; n++) if (e[n] === t) return n;
	}
	return -1;
}
function te(e, t) {
	var n = e.prototype;
	function r() {}
	for (var i in r.prototype = t.prototype, e.prototype = new r(), n) n.hasOwnProperty(i) && (e.prototype[i] = n[i]);
	e.prototype.constructor = e, e.superClass = t;
}
function P(e, t, n) {
	if (e = "prototype" in e ? e.prototype : e, t = "prototype" in t ? t.prototype : t, Object.getOwnPropertyNames) for (var r = Object.getOwnPropertyNames(t), i = 0; i < r.length; i++) {
		var a = r[i];
		a !== "constructor" && (n ? t[a] != null : e[a] == null) && (e[a] = t[a]);
	}
	else M(e, t, n);
}
function F(e) {
	return !e || typeof e == "string" ? !1 : typeof e.length == "number";
}
function I(e, t, n) {
	if (e && t) if (e.forEach && e.forEach === v) e.forEach(t, n);
	else if (e.length === +e.length) for (var r = 0, i = e.length; r < i; r++) t.call(n, e[r], r, e);
	else for (var a in e) e.hasOwnProperty(a) && t.call(n, e[a], a, e);
}
function L(e, t, n) {
	if (!e) return [];
	if (!t) return ge(e);
	if (e.map && e.map === x) return e.map(t, n);
	for (var r = [], i = 0, a = e.length; i < a; i++) r.push(t.call(n, e[i], i, e));
	return r;
}
function ne(e, t, n, r) {
	if (e && t) {
		for (var i = 0, a = e.length; i < a; i++) n = t.call(r, n, e[i], i, e);
		return n;
	}
}
function re(e, t, n) {
	if (!e) return [];
	if (!t) return ge(e);
	if (e.filter && e.filter === y) return e.filter(t, n);
	for (var r = [], i = 0, a = e.length; i < a; i++) t.call(n, e[i], i, e) && r.push(e[i]);
	return r;
}
function ie(e, t, n) {
	if (e && t) {
		for (var r = 0, i = e.length; r < i; r++) if (t.call(n, e[r], r, e)) return e[r];
	}
}
function R(e) {
	if (!e) return [];
	if (Object.keys) return Object.keys(e);
	var t = [];
	for (var n in e) e.hasOwnProperty(n) && t.push(n);
	return t;
}
function ae(e, t) {
	var n = [...arguments].slice(2);
	return function() {
		return e.apply(t, n.concat(b.call(arguments)));
	};
}
var z = C && H(C.bind) ? C.call.bind(C.bind) : ae;
function B(e) {
	var t = [...arguments].slice(1);
	return function() {
		return e.apply(this, t.concat(b.call(arguments)));
	};
}
function V(e) {
	return Array.isArray ? Array.isArray(e) : g.call(e) === "[object Array]";
}
function H(e) {
	return typeof e == "function";
}
function U(e) {
	return typeof e == "string";
}
function oe(e) {
	return g.call(e) === "[object String]";
}
function se(e) {
	return typeof e == "number";
}
function W(e) {
	var t = typeof e;
	return t === "function" || !!e && t === "object";
}
function ce(e) {
	return !!m[g.call(e)];
}
function le(e) {
	return !!h[g.call(e)];
}
function ue(e) {
	return typeof e == "object" && typeof e.nodeType == "number" && typeof e.ownerDocument == "object";
}
function de(e) {
	return e.colorStops != null;
}
function fe(e) {
	return e.image != null;
}
function pe(e) {
	return e !== e;
}
function me() {
	for (var e = [...arguments], t = 0, n = e.length; t < n; t++) if (e[t] != null) return e[t];
}
function G(e, t) {
	return e ?? t;
}
function he(e, t, n) {
	return e ?? t ?? n;
}
function ge(e) {
	var t = [...arguments].slice(1);
	return b.apply(e, t);
}
function _e(e) {
	if (typeof e == "number") return [
		e,
		e,
		e,
		e
	];
	var t = e.length;
	return t === 2 ? [
		e[0],
		e[1],
		e[0],
		e[1]
	] : t === 3 ? [
		e[0],
		e[1],
		e[2],
		e[1]
	] : e;
}
function ve(e, t) {
	if (!e) throw Error(t);
}
function ye(e) {
	return e == null ? null : typeof e.trim == "function" ? e.trim() : e.replace(/^[\s\uFEFF\xA0]+|[\s\uFEFF\xA0]+$/g, "");
}
var be = "__ec_primitive__";
function xe(e) {
	e[be] = !0;
}
function Se(e) {
	return e[be];
}
var Ce = function() {
	function e() {
		this.data = {};
	}
	return e.prototype.delete = function(e) {
		var t = this.has(e);
		return t && delete this.data[e], t;
	}, e.prototype.has = function(e) {
		return this.data.hasOwnProperty(e);
	}, e.prototype.get = function(e) {
		return this.data[e];
	}, e.prototype.set = function(e, t) {
		return this.data[e] = t, this;
	}, e.prototype.keys = function() {
		return R(this.data);
	}, e.prototype.forEach = function(e) {
		var t = this.data;
		for (var n in t) t.hasOwnProperty(n) && e(t[n], n);
	}, e;
}(), we = typeof Map == "function";
function Te() {
	return we ? /* @__PURE__ */ new Map() : new Ce();
}
var Ee = function() {
	function e(t) {
		var n = V(t);
		this.data = Te();
		var r = this;
		t instanceof e ? t.each(i) : t && I(t, i);
		function i(e, t) {
			n ? r.set(e, t) : r.set(t, e);
		}
	}
	return e.prototype.hasKey = function(e) {
		return this.data.has(e);
	}, e.prototype.get = function(e) {
		return this.data.get(e);
	}, e.prototype.set = function(e, t) {
		return this.data.set(e, t), t;
	}, e.prototype.each = function(e, t) {
		this.data.forEach(function(n, r) {
			e.call(t, n, r);
		});
	}, e.prototype.keys = function() {
		var e = this.data.keys();
		return we ? Array.from(e) : e;
	}, e.prototype.removeKey = function(e) {
		this.data.delete(e);
	}, e;
}();
function K(e) {
	return new Ee(e);
}
function De(e, t) {
	for (var n = new e.constructor(e.length + t.length), r = 0; r < e.length; r++) n[r] = e[r];
	for (var i = e.length, r = 0; r < t.length; r++) n[r + i] = t[r];
	return n;
}
function Oe(e, t) {
	var n;
	if (Object.create) n = Object.create(e);
	else {
		var r = function() {};
		r.prototype = e, n = new r();
	}
	return t && j(n, t), n;
}
function ke(e) {
	var t = e.style;
	t.webkitUserSelect = "none", t.userSelect = "none", t.webkitTapHighlightColor = "rgba(0,0,0,0)", t["-webkit-touch-callout"] = "none";
}
function Ae(e, t) {
	return e.hasOwnProperty(t);
}
function je() {}
var Me = 180 / Math.PI, Ne = function() {
	function e() {
		this.firefox = !1, this.ie = !1, this.edge = !1, this.newEdge = !1, this.weChat = !1;
	}
	return e;
}(), q = new (function() {
	function e() {
		this.browser = new Ne(), this.node = !1, this.wxa = !1, this.worker = !1, this.svgSupported = !1, this.touchEventsSupported = !1, this.pointerEventsSupported = !1, this.domSupported = !1, this.transformSupported = !1, this.transform3dSupported = !1, this.hasGlobalWindow = typeof window < "u";
	}
	return e;
}())();
typeof wx == "object" && typeof wx.getSystemInfoSync == "function" ? (q.wxa = !0, q.touchEventsSupported = !0) : typeof document > "u" && typeof self < "u" ? q.worker = !0 : !q.hasGlobalWindow || "Deno" in window || typeof navigator < "u" && typeof navigator.userAgent == "string" && navigator.userAgent.indexOf("Node.js") > -1 ? (q.node = !0, q.svgSupported = !0) : Pe(navigator.userAgent, q);
function Pe(e, t) {
	var n = t.browser, r = e.match(/Firefox\/([\d.]+)/), i = e.match(/MSIE\s([\d.]+)/) || e.match(/Trident\/.+?rv:(([\d.]+))/), a = e.match(/Edge?\/([\d.]+)/), o = /micromessenger/i.test(e);
	if (r && (n.firefox = !0, n.version = r[1]), i && (n.ie = !0, n.version = i[1]), a && (n.edge = !0, n.version = a[1], n.newEdge = +a[1].split(".")[0] > 18), o && (n.weChat = !0), t.svgSupported = typeof SVGRect < "u", t.touchEventsSupported = "ontouchstart" in window && !n.ie && !n.edge, t.pointerEventsSupported = "onpointerdown" in window && (n.edge || n.ie && +n.version >= 11), t.domSupported = typeof document < "u") {
		var s = document.documentElement.style;
		t.transform3dSupported = (n.ie && "transition" in s || n.edge || "WebKitCSSMatrix" in window && "m11" in new WebKitCSSMatrix() || "MozPerspective" in s) && !("OTransition" in s), t.transformSupported = t.transform3dSupported || n.ie && +n.version >= 9;
	}
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/util/clazz.js
var Fe = ".", Ie = "___EC__COMPONENT__CONTAINER___", Le = "___EC__EXTENDED_CLASS___";
function Re(e) {
	var t = {
		main: "",
		sub: ""
	};
	if (e) {
		var n = e.split(Fe);
		t.main = n[0] || "", t.sub = n[1] || "";
	}
	return t;
}
function ze(e) {
	ve(/^[a-zA-Z0-9_]+([.][a-zA-Z0-9_]+)?$/.test(e), "componentType \"" + e + "\" illegal");
}
function Be(e) {
	return !!(e && e[Le]);
}
function Ve(e, t) {
	e.$constructor = e, e.extend = function(e) {
		var t = this, n;
		return He(t) ? n = function(e) {
			o(t, e);
			function t() {
				return e.apply(this, arguments) || this;
			}
			return t;
		}(t) : (n = function() {
			(e.$constructor || t).apply(this, arguments);
		}, te(n, this)), j(n.prototype, e), n[Le] = !0, n.extend = this.extend, n.superCall = Ke, n.superApply = qe, n.superClass = t, n;
	};
}
function He(e) {
	return H(e) && /^class\s/.test(Function.prototype.toString.call(e));
}
function Ue(e, t) {
	e.extend = t.extend;
}
var We = Math.round(Math.random() * 10);
function Ge(e) {
	var t = ["__\0is_clz", We++].join("_");
	e.prototype[t] = !0, e.isInstance = function(e) {
		return !!(e && e[t]);
	};
}
function Ke(e, t) {
	var n = [...arguments].slice(2);
	return this.superClass.prototype[t].apply(e, n);
}
function qe(e, t, n) {
	return this.superClass.prototype[t].apply(e, n);
}
function Je(e) {
	var t = {};
	e.registerClass = function(e) {
		var r = e.type || e.prototype.type;
		if (r) {
			ze(r), e.prototype.type = r;
			var i = Re(r);
			if (!i.sub) t[i.main] = e;
			else if (i.sub !== Ie) {
				var a = n(i);
				a[i.sub] = e;
			}
		}
		return e;
	}, e.getClass = function(e, n, r) {
		var i = t[e];
		if (i && i[Ie] && (i = n ? i[n] : null), r && !i) throw Error(n ? "Component " + e + "." + (n || "") + " is used but not imported." : e + ".type should be specified.");
		return i;
	}, e.getClassesByMainType = function(e) {
		var n = Re(e), r = [], i = t[n.main];
		return i && i[Ie] ? I(i, function(e, t) {
			t !== Ie && r.push(e);
		}) : r.push(i), r;
	}, e.hasClass = function(e) {
		return !!t[Re(e).main];
	}, e.getAllClassMainTypes = function() {
		var e = [];
		return I(t, function(t, n) {
			e.push(n);
		}), e;
	}, e.hasSubTypes = function(e) {
		var n = t[Re(e).main];
		return n && n[Ie];
	};
	function n(e) {
		var n = t[e.main];
		return (!n || !n[Ie]) && (n = t[e.main] = {}, n[Ie] = !0), n;
	}
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/model/mixin/makeStyleMapper.js
function Ye(e, t) {
	for (var n = 0; n < e.length; n++) e[n][1] || (e[n][1] = e[n][0]);
	return t ||= !1, function(n, r, i) {
		for (var a = {}, o = 0; o < e.length; o++) {
			var s = e[o][1];
			if (!(r && N(r, s) >= 0 || i && N(i, s) < 0)) {
				var c = n.getShallow(s, t);
				c != null && (a[e[o][0]] = c);
			}
		}
		return a;
	};
}
var Xe = Ye([
	["fill", "color"],
	["shadowBlur"],
	["shadowOffsetX"],
	["shadowOffsetY"],
	["opacity"],
	["shadowColor"]
]), Ze = function() {
	function e() {}
	return e.prototype.getAreaStyle = function(e, t) {
		return Xe(this, e, t);
	}, e;
}(), Qe = function() {
	function e(e) {
		this.value = e;
	}
	return e;
}(), $e = function() {
	function e() {
		this._len = 0;
	}
	return e.prototype.insert = function(e) {
		var t = new Qe(e);
		return this.insertEntry(t), t;
	}, e.prototype.insertEntry = function(e) {
		this.head ? (this.tail.next = e, e.prev = this.tail, e.next = null, this.tail = e) : this.head = this.tail = e, this._len++;
	}, e.prototype.remove = function(e) {
		var t = e.prev, n = e.next;
		t ? t.next = n : this.head = n, n ? n.prev = t : this.tail = t, e.next = e.prev = null, this._len--;
	}, e.prototype.len = function() {
		return this._len;
	}, e.prototype.clear = function() {
		this.head = this.tail = null, this._len = 0;
	}, e;
}(), et = function() {
	function e(e) {
		this._list = new $e(), this._maxSize = 10, this._map = {}, this._maxSize = e;
	}
	return e.prototype.put = function(e, t) {
		var n = this._list, r = this._map, i = null;
		if (r[e] == null) {
			var a = n.len(), o = this._lastRemovedEntry;
			if (a >= this._maxSize && a > 0) {
				var s = n.head;
				n.remove(s), delete r[s.key], i = s.value, this._lastRemovedEntry = s;
			}
			o ? o.value = t : o = new Qe(t), o.key = e, n.insertEntry(o), r[e] = o;
		}
		return i;
	}, e.prototype.get = function(e) {
		var t = this._map[e], n = this._list;
		if (t != null) return t !== n.tail && (n.remove(t), n.insertEntry(t)), t.value;
	}, e.prototype.clear = function() {
		this._list.clear(), this._map = {};
	}, e.prototype.len = function() {
		return this._list.len();
	}, e;
}(), tt = new et(50);
function nt(e) {
	if (typeof e == "string") {
		var t = tt.get(e);
		return t && t.image;
	} else return e;
}
function rt(e, t, n, r, i) {
	if (!e) return t;
	if (typeof e == "string") {
		if (t && t.__zrImageSrc === e || !n) return t;
		var a = tt.get(e), o = {
			hostEl: n,
			cb: r,
			cbPayload: i
		};
		return a ? (t = a.image, !at(t) && a.pending.push(o)) : (t = p.loadImage(e, it, it), t.__zrImageSrc = e, tt.put(e, t.__cachedImgObj = {
			image: t,
			pending: [o]
		})), t;
	} else return e;
}
function it() {
	var e = this.__cachedImgObj;
	this.onload = this.onerror = this.__cachedImgObj = null;
	for (var t = 0; t < e.pending.length; t++) {
		var n = e.pending[t], r = n.cb;
		r && r(this, n.cbPayload), n.hostEl.dirty();
	}
	e.pending.length = 0;
}
function at(e) {
	return e && e.width && e.height;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/core/matrix.js
function ot() {
	return [
		1,
		0,
		0,
		1,
		0,
		0
	];
}
function st(e) {
	return e[0] = 1, e[1] = 0, e[2] = 0, e[3] = 1, e[4] = 0, e[5] = 0, e;
}
function ct(e, t) {
	return e[0] = t[0], e[1] = t[1], e[2] = t[2], e[3] = t[3], e[4] = t[4], e[5] = t[5], e;
}
function lt(e, t, n) {
	var r = t[0] * n[0] + t[2] * n[1], i = t[1] * n[0] + t[3] * n[1], a = t[0] * n[2] + t[2] * n[3], o = t[1] * n[2] + t[3] * n[3], s = t[0] * n[4] + t[2] * n[5] + t[4], c = t[1] * n[4] + t[3] * n[5] + t[5];
	return e[0] = r, e[1] = i, e[2] = a, e[3] = o, e[4] = s, e[5] = c, e;
}
function ut(e, t, n) {
	return e[0] = t[0], e[1] = t[1], e[2] = t[2], e[3] = t[3], e[4] = t[4] + n[0], e[5] = t[5] + n[1], e;
}
function dt(e, t, n, r) {
	r === void 0 && (r = [0, 0]);
	var i = t[0], a = t[2], o = t[4], s = t[1], c = t[3], l = t[5], u = Math.sin(n), d = Math.cos(n);
	return e[0] = i * d + s * u, e[1] = -i * u + s * d, e[2] = a * d + c * u, e[3] = -a * u + d * c, e[4] = d * (o - r[0]) + u * (l - r[1]) + r[0], e[5] = d * (l - r[1]) - u * (o - r[0]) + r[1], e;
}
function ft(e, t, n) {
	var r = n[0], i = n[1];
	return e[0] = t[0] * r, e[1] = t[1] * i, e[2] = t[2] * r, e[3] = t[3] * i, e[4] = t[4] * r, e[5] = t[5] * i, e;
}
function pt(e, t) {
	var n = t[0], r = t[2], i = t[4], a = t[1], o = t[3], s = t[5], c = n * o - a * r;
	return c ? (c = 1 / c, e[0] = o * c, e[1] = -a * c, e[2] = -r * c, e[3] = n * c, e[4] = (r * s - o * i) * c, e[5] = (a * i - n * s) * c, e) : null;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/core/vector.js
function mt(e, t) {
	return e ??= 0, t ??= 0, [e, t];
}
function ht(e) {
	return [e[0], e[1]];
}
function gt(e, t, n) {
	return e[0] = t, e[1] = n, e;
}
function _t(e, t, n) {
	return e[0] = t[0] + n[0], e[1] = t[1] + n[1], e;
}
function vt(e, t, n) {
	return e[0] = t[0] - n[0], e[1] = t[1] - n[1], e;
}
function yt(e) {
	return Math.sqrt(bt(e));
}
function bt(e) {
	return e[0] * e[0] + e[1] * e[1];
}
function xt(e, t, n) {
	return e[0] = t[0] * n, e[1] = t[1] * n, e;
}
function St(e, t) {
	var n = yt(t);
	return n === 0 ? (e[0] = 0, e[1] = 0) : (e[0] = t[0] / n, e[1] = t[1] / n), e;
}
function Ct(e, t) {
	return Math.sqrt((e[0] - t[0]) * (e[0] - t[0]) + (e[1] - t[1]) * (e[1] - t[1]));
}
var wt = Ct;
function Tt(e, t) {
	return (e[0] - t[0]) * (e[0] - t[0]) + (e[1] - t[1]) * (e[1] - t[1]);
}
var Et = Tt;
function Dt(e, t, n, r) {
	return e[0] = t[0] + r * (n[0] - t[0]), e[1] = t[1] + r * (n[1] - t[1]), e;
}
function Ot(e, t, n) {
	var r = t[0], i = t[1];
	return e[0] = n[0] * r + n[2] * i + n[4], e[1] = n[1] * r + n[3] * i + n[5], e;
}
function kt(e, t, n) {
	return e[0] = Math.min(t[0], n[0]), e[1] = Math.min(t[1], n[1]), e;
}
function At(e, t, n) {
	return e[0] = Math.max(t[0], n[0]), e[1] = Math.max(t[1], n[1]), e;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/core/Point.js
var J = function() {
	function e(e, t) {
		this.x = e || 0, this.y = t || 0;
	}
	return e.prototype.copy = function(e) {
		return this.x = e.x, this.y = e.y, this;
	}, e.prototype.clone = function() {
		return new e(this.x, this.y);
	}, e.prototype.set = function(e, t) {
		return this.x = e, this.y = t, this;
	}, e.prototype.equal = function(e) {
		return e.x === this.x && e.y === this.y;
	}, e.prototype.add = function(e) {
		return this.x += e.x, this.y += e.y, this;
	}, e.prototype.scale = function(e) {
		this.x *= e, this.y *= e;
	}, e.prototype.scaleAndAdd = function(e, t) {
		this.x += e.x * t, this.y += e.y * t;
	}, e.prototype.sub = function(e) {
		return this.x -= e.x, this.y -= e.y, this;
	}, e.prototype.dot = function(e) {
		return this.x * e.x + this.y * e.y;
	}, e.prototype.len = function() {
		return Math.sqrt(this.x * this.x + this.y * this.y);
	}, e.prototype.lenSquare = function() {
		return this.x * this.x + this.y * this.y;
	}, e.prototype.normalize = function() {
		var e = this.len();
		return this.x /= e, this.y /= e, this;
	}, e.prototype.distance = function(e) {
		var t = this.x - e.x, n = this.y - e.y;
		return Math.sqrt(t * t + n * n);
	}, e.prototype.distanceSquare = function(e) {
		var t = this.x - e.x, n = this.y - e.y;
		return t * t + n * n;
	}, e.prototype.negate = function() {
		return this.x = -this.x, this.y = -this.y, this;
	}, e.prototype.transform = function(e) {
		if (e) {
			var t = this.x, n = this.y;
			return this.x = e[0] * t + e[2] * n + e[4], this.y = e[1] * t + e[3] * n + e[5], this;
		}
	}, e.prototype.toArray = function(e) {
		return e[0] = this.x, e[1] = this.y, e;
	}, e.prototype.fromArray = function(e) {
		this.x = e[0], this.y = e[1];
	}, e.set = function(e, t, n) {
		e.x = t, e.y = n;
	}, e.copy = function(e, t) {
		e.x = t.x, e.y = t.y;
	}, e.len = function(e) {
		return Math.sqrt(e.x * e.x + e.y * e.y);
	}, e.lenSquare = function(e) {
		return e.x * e.x + e.y * e.y;
	}, e.dot = function(e, t) {
		return e.x * t.x + e.y * t.y;
	}, e.add = function(e, t, n) {
		e.x = t.x + n.x, e.y = t.y + n.y;
	}, e.sub = function(e, t, n) {
		e.x = t.x - n.x, e.y = t.y - n.y;
	}, e.scale = function(e, t, n) {
		e.x = t.x * n, e.y = t.y * n;
	}, e.scaleAndAdd = function(e, t, n, r) {
		e.x = t.x + n.x * r, e.y = t.y + n.y * r;
	}, e.lerp = function(e, t, n, r) {
		var i = 1 - r;
		e.x = i * t.x + r * n.x, e.y = i * t.y + r * n.y;
	}, e;
}(), jt = Math.min, Mt = Math.max, Nt = Math.abs, Pt = ["x", "y"], Ft = ["width", "height"], It = new J(), Lt = new J(), Rt = new J(), zt = new J(), Bt = Zt(), Vt = Bt.minTv, Ht = Bt.maxTv, Ut = [0, 0], Y = function() {
	function e(e, t, n, r) {
		Wt(this, e, t, n, r);
	}
	return e.set = function(e, t, n, r, i) {
		return r < 0 && (t += r, r = -r), i < 0 && (n += i, i = -i), e.x = t, e.y = n, e.width = r, e.height = i, e;
	}, e.prototype.union = function(e) {
		var t = jt(e.x, this.x), n = jt(e.y, this.y);
		isFinite(this.x) && isFinite(this.width) ? this.width = Mt(e.x + e.width, this.x + this.width) - t : this.width = e.width, isFinite(this.y) && isFinite(this.height) ? this.height = Mt(e.y + e.height, this.y + this.height) - n : this.height = e.height, this.x = t, this.y = n;
	}, e.prototype.applyTransform = function(t) {
		e.applyTransform(this, this, t);
	}, e.prototype.calculateTransform = function(e) {
		return Kt(ot(), this, e);
	}, e.prototype.intersect = function(t, n, r) {
		return e.intersect(this, t, n, r);
	}, e.intersect = function(t, n, r, i) {
		r && J.set(r, 0, 0);
		var a = i && i.outIntersectRect || null, o = i && i.clamp;
		if (a && (a.x = a.y = a.width = a.height = NaN), !t || !n) return !1;
		t instanceof e || (t = Wt(qt, t.x, t.y, t.width, t.height)), n instanceof e || (n = Wt(Jt, n.x, n.y, n.width, n.height));
		var s = !!r;
		Bt.reset(i, s);
		var c = Bt.touchThreshold, l = t.x + c, u = t.x + t.width - c, d = t.y + c, f = t.y + t.height - c, p = n.x + c, m = n.x + n.width - c, h = n.y + c, g = n.y + n.height - c;
		if (l > u || d > f || p > m || h > g) return !1;
		var _ = !(u < p || m < l || f < h || g < d);
		return (s || a) && (Ut[0] = Infinity, Ut[1] = 0, Xt(l, u, p, m, 0, s, a, o), Xt(d, f, h, g, 1, s, a, o), s && J.copy(r, _ ? Bt.useDir ? Bt.dirMinTv : Vt : Ht)), _;
	}, e.contain = function(e, t, n) {
		return t >= e.x && t <= e.x + e.width && n >= e.y && n <= e.y + e.height;
	}, e.prototype.contain = function(t, n) {
		return e.contain(this, t, n);
	}, e.prototype.clone = function() {
		return new e(this.x, this.y, this.width, this.height);
	}, e.prototype.copy = function(e) {
		Gt(this, e);
	}, e.prototype.plain = function() {
		return {
			x: this.x,
			y: this.y,
			width: this.width,
			height: this.height
		};
	}, e.prototype.isFinite = function() {
		return isFinite(this.x) && isFinite(this.y) && isFinite(this.width) && isFinite(this.height);
	}, e.prototype.isZero = function() {
		return this.width === 0 || this.height === 0;
	}, e.create = function(t) {
		return new e(t ? t.x : 0, t ? t.y : 0, t ? t.width : 0, t ? t.height : 0);
	}, e.copy = function(e, t) {
		return e.x = t.x, e.y = t.y, e.width = t.width, e.height = t.height, e;
	}, e.applyTransform = function(e, t, n) {
		if (!n) {
			e !== t && Gt(e, t);
			return;
		}
		if (n[1] < 1e-5 && n[1] > -1e-5 && n[2] < 1e-5 && n[2] > -1e-5) {
			var r = n[0], i = n[3], a = n[4], o = n[5];
			e.x = t.x * r + a, e.y = t.y * i + o, e.width = t.width * r, e.height = t.height * i, e.width < 0 && (e.x += e.width, e.width = -e.width), e.height < 0 && (e.y += e.height, e.height = -e.height);
			return;
		}
		It.x = Rt.x = t.x, It.y = zt.y = t.y, Lt.x = zt.x = t.x + t.width, Lt.y = Rt.y = t.y + t.height, It.transform(n), zt.transform(n), Lt.transform(n), Rt.transform(n), e.x = jt(It.x, Lt.x, Rt.x, zt.x), e.y = jt(It.y, Lt.y, Rt.y, zt.y);
		var s = Mt(It.x, Lt.x, Rt.x, zt.x), c = Mt(It.y, Lt.y, Rt.y, zt.y);
		e.width = s - e.x, e.height = c - e.y;
	}, e.calculateTransform = function(e, t, n) {
		var r = n.width / t.width, i = n.height / t.height;
		return e = st(e || []), ut(e, e, gt(Yt, -t.x, -t.y)), ft(e, e, gt(Yt, r, i)), ut(e, e, gt(Yt, n.x, n.y)), e;
	}, e;
}();
Y.create;
var Wt = Y.set, Gt = Y.copy, Kt = Y.calculateTransform;
Y.applyTransform, Y.contain;
var qt = new Y(0, 0, 0, 0), Jt = new Y(0, 0, 0, 0), Yt = [];
function Xt(e, t, n, r, i, a, o, s) {
	var c = Nt(t - n), l = Nt(r - e), u = jt(c, l), d = Pt[i], f = Pt[1 - i], p = Ft[i];
	t < n || r < e ? c < l ? (a && (Ht[d] = -c), s && (o[d] = t, o[p] = 0)) : (a && (Ht[d] = l), s && (o[d] = e, o[p] = 0)) : (o && (o[d] = Mt(e, n), o[p] = jt(t, r) - o[d]), a && (u < Ut[0] || Bt.useDir) && (Ut[0] = jt(u, Ut[0]), (c < l || !Bt.bidirectional) && (Vt[d] = c, Vt[f] = 0, Bt.useDir && Bt.calcDirMTV()), (c >= l || !Bt.bidirectional) && (Vt[d] = -l, Vt[f] = 0, Bt.useDir && Bt.calcDirMTV())));
}
function Zt() {
	var e = 0, t = new J(), n = new J(), r = {
		minTv: new J(),
		maxTv: new J(),
		useDir: !1,
		dirMinTv: new J(),
		touchThreshold: 0,
		bidirectional: !0,
		negativeSize: !1,
		reset: function(i, a) {
			r.touchThreshold = 0, i && i.touchThreshold != null && (r.touchThreshold = Mt(0, i.touchThreshold)), r.negativeSize = !1, a && (r.minTv.set(Infinity, Infinity), r.maxTv.set(0, 0), r.useDir = !1, i && i.direction != null && (r.useDir = !0, r.dirMinTv.copy(r.minTv), n.copy(r.minTv), e = i.direction, r.bidirectional = i.bidirectional == null || !!i.bidirectional, r.bidirectional || t.set(Math.cos(e), Math.sin(e))));
		},
		calcDirMTV: function() {
			var a = r.minTv, o = r.dirMinTv, s = a.y * a.y + a.x * a.x, c = Math.sin(e), l = Math.cos(e), u = c * a.y + l * a.x;
			if (i(u)) {
				i(a.x) && i(a.y) && o.set(0, 0);
				return;
			}
			if (n.x = s * l / u, n.y = s * c / u, i(n.x) && i(n.y)) {
				o.set(0, 0);
				return;
			}
			(r.bidirectional || t.dot(n) > 0) && n.len() < o.len() && o.copy(n);
		}
	};
	function i(e) {
		return Nt(e) < 1e-10;
	}
	return r;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/contain/text.js
function Qt(e) {
	$t ||= new et(100), e ||= "12px sans-serif";
	var t = $t.get(e);
	return t || (t = {
		font: e,
		strWidthCache: new et(500),
		asciiWidthMap: null,
		asciiWidthMapTried: !1,
		stWideCharWidth: p.measureText("国", e).width,
		asciiCharWidth: p.measureText("a", e).width
	}, $t.put(e, t)), t;
}
var $t;
function en(e) {
	if (!(tn >= nn)) {
		e ||= "12px sans-serif";
		for (var t = [], n = +/* @__PURE__ */ new Date(), r = 0; r <= 127; r++) t[r] = p.measureText(String.fromCharCode(r), e).width;
		var i = +/* @__PURE__ */ new Date() - n;
		return i > 16 ? tn = nn : i > 2 && tn++, t;
	}
}
var tn = 0, nn = 5;
function rn(e, t) {
	return e.asciiWidthMapTried ||= (e.asciiWidthMap = en(e.font), !0), 0 <= t && t <= 127 ? e.asciiWidthMap == null ? e.asciiCharWidth : e.asciiWidthMap[t] : e.stWideCharWidth;
}
function an(e, t) {
	var n = e.strWidthCache, r = n.get(t);
	return r ?? (r = p.measureText(t, e.font).width, n.put(t, r)), r;
}
function on(e, t, n, r) {
	var i = an(Qt(t), e), a = un(t);
	return new Y(cn(0, i, n), ln(0, a, r), i, a);
}
function sn(e, t, n, r) {
	var i = ((e || "") + "").split("\n");
	if (i.length === 1) return on(i[0], t, n, r);
	for (var a = new Y(0, 0, 0, 0), o = 0; o < i.length; o++) {
		var s = on(i[o], t, n, r);
		o === 0 ? a.copy(s) : a.union(s);
	}
	return a;
}
function cn(e, t, n, r) {
	return n === "right" ? r ? e += t : e -= t : n === "center" && (r ? e += t / 2 : e -= t / 2), e;
}
function ln(e, t, n, r) {
	return n === "middle" ? r ? e += t / 2 : e -= t / 2 : n === "bottom" && (r ? e += t : e -= t), e;
}
function un(e) {
	return Qt(e).stWideCharWidth;
}
function dn(e, t) {
	return typeof e == "string" ? e.lastIndexOf("%") >= 0 ? parseFloat(e) / 100 * t : parseFloat(e) : e;
}
function fn(e, t, n) {
	var r = t.position || "inside", i = t.distance == null ? 5 : t.distance, a = n.height, o = n.width, s = a / 2, c = n.x, l = n.y, u = "left", d = "top";
	if (r instanceof Array) c += dn(r[0], n.width), l += dn(r[1], n.height), u = null, d = null;
	else switch (r) {
		case "left":
			c -= i, l += s, u = "right", d = "middle";
			break;
		case "right":
			c += i + o, l += s, d = "middle";
			break;
		case "top":
			c += o / 2, l -= i, u = "center", d = "bottom";
			break;
		case "bottom":
			c += o / 2, l += a + i, u = "center";
			break;
		case "inside":
			c += o / 2, l += s, u = "center", d = "middle";
			break;
		case "insideLeft":
			c += i, l += s, d = "middle";
			break;
		case "insideRight":
			c += o - i, l += s, u = "right", d = "middle";
			break;
		case "insideTop":
			c += o / 2, l += i, u = "center";
			break;
		case "insideBottom":
			c += o / 2, l += a - i, u = "center", d = "bottom";
			break;
		case "insideTopLeft":
			c += i, l += i;
			break;
		case "insideTopRight":
			c += o - i, l += i, u = "right";
			break;
		case "insideBottomLeft":
			c += i, l += a - i, d = "bottom";
			break;
		case "insideBottomRight":
			c += o - i, l += a - i, u = "right", d = "bottom";
			break;
	}
	return e ||= {}, e.x = c, e.y = l, e.align = u, e.verticalAlign = d, e;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/helper/parseText.js
var pn = /\{([a-zA-Z0-9_]+)\|([^}]*)\}/g;
function mn(e, t, n, r, i, a) {
	if (!n) {
		e.text = "", e.isTruncated = !1;
		return;
	}
	var o = (t + "").split("\n");
	a = hn(n, r, i, a);
	for (var s = !1, c = {}, l = 0, u = o.length; l < u; l++) gn(c, o[l], a), o[l] = c.textLine, s ||= c.isTruncated;
	e.text = o.join("\n"), e.isTruncated = s;
}
function hn(e, t, n, r) {
	r ||= {};
	var i = j({}, r);
	n = G(n, "..."), i.maxIterations = G(r.maxIterations, 2);
	var a = i.minChar = G(r.minChar, 0), o = i.fontMeasureInfo = Qt(t), s = o.asciiCharWidth;
	i.placeholder = G(r.placeholder, "");
	for (var c = e = Math.max(0, e - 1), l = 0; l < a && c >= s; l++) c -= s;
	var u = an(o, n);
	return u > c && (n = "", u = 0), c = e - u, i.ellipsis = n, i.ellipsisWidth = u, i.contentWidth = c, i.containerWidth = e, i;
}
function gn(e, t, n) {
	var r = n.containerWidth, i = n.contentWidth, a = n.fontMeasureInfo;
	if (!r) {
		e.textLine = "", e.isTruncated = !1;
		return;
	}
	var o = an(a, t);
	if (o <= r) {
		e.textLine = t, e.isTruncated = !1;
		return;
	}
	for (var s = 0;; s++) {
		if (o <= i || s >= n.maxIterations) {
			t += n.ellipsis;
			break;
		}
		var c = s === 0 ? _n(t, i, a) : o > 0 ? Math.floor(t.length * i / o) : 0;
		t = t.substr(0, c), o = an(a, t);
	}
	t === "" && (t = n.placeholder), e.textLine = t, e.isTruncated = !0;
}
function _n(e, t, n) {
	for (var r = 0, i = 0, a = e.length; i < a && r < t; i++) r += rn(n, e.charCodeAt(i));
	return i;
}
function vn(e, t, n, r) {
	var i = jn(e), a = t.overflow, o = t.padding, s = o ? o[1] + o[3] : 0, c = o ? o[0] + o[2] : 0, l = t.font, u = a === "truncate", d = un(l), f = G(t.lineHeight, d), p = t.lineOverflow === "truncate", m = !1, h = t.width;
	h == null && n != null && (h = n - s);
	var g = t.height;
	g == null && r != null && (g = r - c);
	var _ = h != null && (a === "break" || a === "breakAll") ? i ? Dn(i, t.font, h, a === "breakAll", 0).lines : [] : i ? i.split("\n") : [], v = _.length * f;
	if (g ??= v, v > g && p) {
		var y = Math.floor(g / f);
		m ||= _.length > y, _ = _.slice(0, y), v = _.length * f;
	}
	if (i && u && h != null) for (var b = hn(h, l, t.ellipsis, {
		minChar: t.truncateMinChar,
		placeholder: t.placeholder
	}), x = {}, S = 0; S < _.length; S++) gn(x, _[S], b), _[S] = x.textLine, m ||= x.isTruncated;
	for (var C = g, w = 0, T = Qt(l), S = 0; S < _.length; S++) w = Math.max(an(T, _[S]), w);
	h ??= w;
	var E = h;
	return C += c, E += s, {
		lines: _,
		height: g,
		outerWidth: E,
		outerHeight: C,
		lineHeight: f,
		calculatedLineHeight: d,
		contentWidth: w,
		contentHeight: v,
		width: h,
		isTruncated: m
	};
}
var yn = function() {
	function e() {}
	return e;
}(), bn = function() {
	function e(e) {
		this.tokens = [], e && (this.tokens = e);
	}
	return e;
}(), xn = function() {
	function e() {
		this.width = 0, this.height = 0, this.contentWidth = 0, this.contentHeight = 0, this.outerWidth = 0, this.outerHeight = 0, this.lines = [], this.isTruncated = !1;
	}
	return e;
}();
function Sn(e, t, n, r, i) {
	var a = new xn(), o = jn(e);
	if (!o) return a;
	var s = t.padding, c = s ? s[1] + s[3] : 0, l = s ? s[0] + s[2] : 0, u = t.width;
	u == null && n != null && (u = n - c);
	var d = t.height;
	d == null && r != null && (d = r - l);
	for (var f = t.overflow, p = (f === "break" || f === "breakAll") && u != null ? {
		width: u,
		accumWidth: 0,
		breakAll: f === "breakAll"
	} : null, m = pn.lastIndex = 0, h; (h = pn.exec(o)) != null;) {
		var g = h.index;
		g > m && Cn(a, o.substring(m, g), t, p), Cn(a, h[2], t, p, h[1]), m = pn.lastIndex;
	}
	m < o.length && Cn(a, o.substring(m, o.length), t, p);
	var _ = [], v = 0, y = 0, b = f === "truncate", x = t.lineOverflow === "truncate", S = {};
	function C(e, t, n) {
		e.width = t, e.lineHeight = n, v += n, y = Math.max(y, t);
	}
	outer: for (var w = 0; w < a.lines.length; w++) {
		for (var T = a.lines[w], E = 0, D = 0, O = 0; O < T.tokens.length; O++) {
			var k = T.tokens[O], A = k.styleName && t.rich[k.styleName] || {}, j = k.textPadding = A.padding, ee = j ? j[1] + j[3] : 0, M = k.font = A.font || t.font;
			k.contentHeight = un(M);
			var N = G(A.height, k.contentHeight);
			if (k.innerHeight = N, j && (N += j[0] + j[2]), k.height = N, k.lineHeight = he(A.lineHeight, t.lineHeight, N), k.align = A && A.align || i, k.verticalAlign = A && A.verticalAlign || "middle", x && d != null && v + k.lineHeight > d) {
				var te = a.lines.length;
				O > 0 ? (T.tokens = T.tokens.slice(0, O), C(T, D, E), a.lines = a.lines.slice(0, w + 1)) : a.lines = a.lines.slice(0, w), a.isTruncated = a.isTruncated || a.lines.length < te;
				break outer;
			}
			var P = A.width, F = P == null || P === "auto";
			if (typeof P == "string" && P.charAt(P.length - 1) === "%") k.percentWidth = P, _.push(k), k.contentWidth = an(Qt(M), k.text);
			else {
				if (F) {
					var I = A.backgroundColor, L = I && I.image;
					L && (L = nt(L), at(L) && (k.width = Math.max(k.width, L.width * N / L.height)));
				}
				var ne = b && u != null ? u - D : null;
				ne != null && ne < k.width ? !F || ne < ee ? (k.text = "", k.width = k.contentWidth = 0) : (mn(S, k.text, ne - ee, M, t.ellipsis, { minChar: t.truncateMinChar }), k.text = S.text, a.isTruncated = a.isTruncated || S.isTruncated, k.width = k.contentWidth = an(Qt(M), k.text)) : k.contentWidth = an(Qt(M), k.text);
			}
			k.width += ee, D += k.width, A && (E = Math.max(E, k.lineHeight));
		}
		C(T, D, E);
	}
	a.outerWidth = a.width = G(u, y), a.outerHeight = a.height = G(d, v), a.contentHeight = v, a.contentWidth = y, a.outerWidth += c, a.outerHeight += l;
	for (var w = 0; w < _.length; w++) {
		var k = _[w], re = k.percentWidth;
		k.width = parseInt(re, 10) / 100 * a.width;
	}
	return a;
}
function Cn(e, t, n, r, i) {
	var a = t === "", o = i && n.rich[i] || {}, s = e.lines, c = o.font || n.font, l = !1, u, d;
	if (r) {
		var f = o.padding, p = f ? f[1] + f[3] : 0;
		if (o.width != null && o.width !== "auto") {
			var m = dn(o.width, r.width) + p;
			s.length > 0 && m + r.accumWidth > r.width && (u = t.split("\n"), l = !0), r.accumWidth = m;
		} else {
			var h = Dn(t, c, r.width, r.breakAll, r.accumWidth);
			r.accumWidth = h.accumWidth + p, d = h.linesWidths, u = h.lines;
		}
	}
	u ||= t.split("\n");
	for (var g = Qt(c), _ = 0; _ < u.length; _++) {
		var v = u[_], y = new yn();
		if (y.styleName = i, y.text = v, y.isLineHolder = !v && !a, typeof o.width == "number" ? y.width = o.width : y.width = d ? d[_] : an(g, v), !_ && !l) {
			var b = (s[s.length - 1] || (s[0] = new bn())).tokens, x = b.length;
			x === 1 && b[0].isLineHolder ? b[0] = y : (v || !x || a) && b.push(y);
		} else s.push(new bn([y]));
	}
}
function wn(e) {
	var t = e.charCodeAt(0);
	return t >= 32 && t <= 591 || t >= 880 && t <= 4351 || t >= 4608 && t <= 5119 || t >= 7680 && t <= 8303;
}
var Tn = ne(",&?/;] ".split(""), function(e, t) {
	return e[t] = !0, e;
}, {});
function En(e) {
	return wn(e) ? !!Tn[e] : !0;
}
function Dn(e, t, n, r, i) {
	for (var a = [], o = [], s = "", c = "", l = 0, u = 0, d = Qt(t), f = 0; f < e.length; f++) {
		var p = e.charAt(f);
		if (p === "\n") {
			c && (s += c, u += l), a.push(s), o.push(u), s = "", c = "", l = 0, u = 0;
			continue;
		}
		var m = rn(d, p.charCodeAt(0)), h = r ? !1 : !En(p);
		if (a.length ? u + m > n : i + u + m > n) {
			u ? (s || c) && (h ? (s || (s = c, c = "", l = 0, u = l), a.push(s), o.push(u - l), c += p, l += m, s = "", u = l) : (c && (s += c, c = "", l = 0), a.push(s), o.push(u), s = p, u = m)) : h ? (a.push(c), o.push(l), c = p, l = m) : (a.push(p), o.push(m));
			continue;
		}
		u += m, h ? (c += p, l += m) : (c && (s += c, c = "", l = 0), s += p);
	}
	return c && (s += c), s && (a.push(s), o.push(u)), a.length === 1 && (u += i), {
		accumWidth: u,
		lines: a,
		linesWidths: o
	};
}
function On(e, t, n, r, i, a) {
	if (e.baseX = n, e.baseY = r, e.outerWidth = e.outerHeight = null, t) {
		var o = t.width * 2, s = t.height * 2;
		Y.set(kn, cn(n, o, i), ln(r, s, a), o, s), Y.intersect(t, kn, null, An);
		var c = An.outIntersectRect;
		e.outerWidth = c.width, e.outerHeight = c.height, e.baseX = cn(c.x, c.width, i, !0), e.baseY = ln(c.y, c.height, a, !0);
	}
}
var kn = new Y(0, 0, 0, 0), An = {
	outIntersectRect: {},
	clamp: !0
};
function jn(e) {
	return e == null ? e = "" : e += "";
}
function Mn(e) {
	var t = jn(e.text), n = e.font;
	return Nn(e, an(Qt(n), t), un(n), null);
}
function Nn(e, t, n, r) {
	var i = new Y(cn(e.x || 0, t, e.textAlign), ln(e.y || 0, n, e.textBaseline), t, n), a = r ?? (Pn(e) ? e.lineWidth : 0);
	return a > 0 && (i.x -= a / 2, i.y -= a / 2, i.width += a, i.height += a), i;
}
function Pn(e) {
	var t = e.stroke;
	return t != null && t !== "none" && e.lineWidth > 0;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/core/Transformable.js
var Fn = st, In = 5e-5;
function Ln(e) {
	return e > In || e < -In;
}
var Rn = [], zn = [], Bn = ot(), Vn = Math.abs, Hn = function() {
	function e() {}
	return e.prototype.getLocalTransform = function(e) {
		return Un(this, e);
	}, e.prototype.setPosition = function(e) {
		this.x = e[0], this.y = e[1];
	}, e.prototype.setScale = function(e) {
		this.scaleX = e[0], this.scaleY = e[1];
	}, e.prototype.setSkew = function(e) {
		this.skewX = e[0], this.skewY = e[1];
	}, e.prototype.setOrigin = function(e) {
		this.originX = e[0], this.originY = e[1];
	}, e.prototype.needLocalTransform = function() {
		return Ln(this.rotation) || Ln(this.x) || Ln(this.y) || Ln(this.scaleX - 1) || Ln(this.scaleY - 1) || Ln(this.skewX) || Ln(this.skewY);
	}, e.prototype.updateTransform = function() {
		var e = this.parent && this.parent.transform, t = this.needLocalTransform(), n = this.transform;
		if (!(t || e)) {
			n && (Fn(n), this.invTransform = null);
			return;
		}
		n ||= ot(), t ? this.getLocalTransform(n) : Fn(n), e && (t ? lt(n, e, n) : ct(n, e)), this.transform = n, this._resolveGlobalScaleRatio(n), this.invTransform = this.invTransform || ot(), pt(this.invTransform, n);
	}, e.prototype._resolveGlobalScaleRatio = function(e) {
		var t = this.globalScaleRatio;
		if (t != null && t !== 1) {
			this.getGlobalScale(Rn);
			var n = Rn[0] < 0 ? -1 : 1, r = Rn[1] < 0 ? -1 : 1, i = ((Rn[0] - n) * t + n) / Rn[0] || 0, a = ((Rn[1] - r) * t + r) / Rn[1] || 0;
			e[0] *= i, e[1] *= i, e[2] *= a, e[3] *= a;
		}
	}, e.prototype.getComputedTransform = function() {
		for (var e = this, t = []; e;) t.push(e), e = e.parent;
		for (; e = t.pop();) e.updateTransform();
		return this.transform;
	}, e.prototype.setLocalTransform = function(e) {
		if (e) {
			var t = e[0] * e[0] + e[1] * e[1], n = e[2] * e[2] + e[3] * e[3], r = Math.atan2(e[1], e[0]), i = Math.PI / 2 + r - Math.atan2(e[3], e[2]);
			n = Math.sqrt(n) * Math.cos(i), t = Math.sqrt(t), this.skewX = i, this.skewY = 0, this.rotation = -r, this.x = +e[4], this.y = +e[5], this.scaleX = t, this.scaleY = n, this.originX = 0, this.originY = 0;
		}
	}, e.prototype.decomposeTransform = function() {
		if (this.transform) {
			var e = this.parent, t = this.transform;
			e && e.transform && (e.invTransform = e.invTransform || ot(), lt(zn, e.invTransform, t), t = zn);
			var n = this.originX, r = this.originY;
			(n || r) && (Bn[4] = n, Bn[5] = r, lt(zn, t, Bn), zn[4] -= n, zn[5] -= r, t = zn), this.setLocalTransform(t);
		}
	}, e.prototype.getGlobalScale = function(e) {
		var t = this.transform;
		return e ||= [], t ? (e[0] = Math.sqrt(t[0] * t[0] + t[1] * t[1]), e[1] = Math.sqrt(t[2] * t[2] + t[3] * t[3]), t[0] < 0 && (e[0] = -e[0]), t[3] < 0 && (e[1] = -e[1]), e) : (e[0] = 1, e[1] = 1, e);
	}, e.prototype.transformCoordToLocal = function(e, t) {
		var n = [e, t], r = this.invTransform;
		return r && Ot(n, n, r), n;
	}, e.prototype.transformCoordToGlobal = function(e, t) {
		var n = [e, t], r = this.transform;
		return r && Ot(n, n, r), n;
	}, e.prototype.getLineScale = function() {
		var e = this.transform;
		return e && Vn(e[0] - 1) > 1e-10 && Vn(e[3] - 1) > 1e-10 ? Math.sqrt(Vn(e[0] * e[3] - e[2] * e[1])) : 1;
	}, e.prototype.copyTransform = function(e) {
		Gn(this, e);
	}, e.getLocalTransform = function(e, t) {
		t ||= [];
		var n = e.originX || 0, r = e.originY || 0, i = e.scaleX, a = e.scaleY, o = e.anchorX, s = e.anchorY, c = e.rotation || 0, l = e.x, u = e.y, d = e.skewX ? Math.tan(e.skewX) : 0, f = e.skewY ? Math.tan(-e.skewY) : 0;
		if (n || r || o || s) {
			var p = n + o, m = r + s;
			t[4] = -p * i - d * m * a, t[5] = -m * a - f * p * i;
		} else t[4] = t[5] = 0;
		return t[0] = i, t[3] = a, t[1] = f * i, t[2] = d * a, c && dt(t, t, c), t[4] += n + l, t[5] += r + u, t;
	}, e.initDefaultProps = (function() {
		var t = e.prototype;
		t.scaleX = t.scaleY = t.globalScaleRatio = 1, t.x = t.y = t.originX = t.originY = t.skewX = t.skewY = t.rotation = t.anchorX = t.anchorY = 0;
	})(), e;
}(), Un = Hn.getLocalTransform, Wn = [
	"x",
	"y",
	"originX",
	"originY",
	"anchorX",
	"anchorY",
	"rotation",
	"scaleX",
	"scaleY",
	"skewX",
	"skewY"
];
function Gn(e, t) {
	return ee(e, t, Wn);
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/animation/easing.js
var Kn = {
	linear: function(e) {
		return e;
	},
	quadraticIn: function(e) {
		return e * e;
	},
	quadraticOut: function(e) {
		return e * (2 - e);
	},
	quadraticInOut: function(e) {
		return (e *= 2) < 1 ? .5 * e * e : -.5 * (--e * (e - 2) - 1);
	},
	cubicIn: function(e) {
		return e * e * e;
	},
	cubicOut: function(e) {
		return --e * e * e + 1;
	},
	cubicInOut: function(e) {
		return (e *= 2) < 1 ? .5 * e * e * e : .5 * ((e -= 2) * e * e + 2);
	},
	quarticIn: function(e) {
		return e * e * e * e;
	},
	quarticOut: function(e) {
		return 1 - --e * e * e * e;
	},
	quarticInOut: function(e) {
		return (e *= 2) < 1 ? .5 * e * e * e * e : -.5 * ((e -= 2) * e * e * e - 2);
	},
	quinticIn: function(e) {
		return e * e * e * e * e;
	},
	quinticOut: function(e) {
		return --e * e * e * e * e + 1;
	},
	quinticInOut: function(e) {
		return (e *= 2) < 1 ? .5 * e * e * e * e * e : .5 * ((e -= 2) * e * e * e * e + 2);
	},
	sinusoidalIn: function(e) {
		return 1 - Math.cos(e * Math.PI / 2);
	},
	sinusoidalOut: function(e) {
		return Math.sin(e * Math.PI / 2);
	},
	sinusoidalInOut: function(e) {
		return .5 * (1 - Math.cos(Math.PI * e));
	},
	exponentialIn: function(e) {
		return e === 0 ? 0 : 1024 ** (e - 1);
	},
	exponentialOut: function(e) {
		return e === 1 ? 1 : 1 - 2 ** (-10 * e);
	},
	exponentialInOut: function(e) {
		return e === 0 ? 0 : e === 1 ? 1 : (e *= 2) < 1 ? .5 * 1024 ** (e - 1) : .5 * (-(2 ** (-10 * (e - 1))) + 2);
	},
	circularIn: function(e) {
		return 1 - Math.sqrt(1 - e * e);
	},
	circularOut: function(e) {
		return Math.sqrt(1 - --e * e);
	},
	circularInOut: function(e) {
		return (e *= 2) < 1 ? -.5 * (Math.sqrt(1 - e * e) - 1) : .5 * (Math.sqrt(1 - (e -= 2) * e) + 1);
	},
	elasticIn: function(e) {
		var t, n = .1, r = .4;
		return e === 0 ? 0 : e === 1 ? 1 : (!n || n < 1 ? (n = 1, t = r / 4) : t = r * Math.asin(1 / n) / (2 * Math.PI), -(n * 2 ** (10 * --e) * Math.sin((e - t) * (2 * Math.PI) / r)));
	},
	elasticOut: function(e) {
		var t, n = .1, r = .4;
		return e === 0 ? 0 : e === 1 ? 1 : (!n || n < 1 ? (n = 1, t = r / 4) : t = r * Math.asin(1 / n) / (2 * Math.PI), n * 2 ** (-10 * e) * Math.sin((e - t) * (2 * Math.PI) / r) + 1);
	},
	elasticInOut: function(e) {
		var t, n = .1, r = .4;
		return e === 0 ? 0 : e === 1 ? 1 : (!n || n < 1 ? (n = 1, t = r / 4) : t = r * Math.asin(1 / n) / (2 * Math.PI), (e *= 2) < 1 ? -.5 * (n * 2 ** (10 * --e) * Math.sin((e - t) * (2 * Math.PI) / r)) : n * 2 ** (-10 * --e) * Math.sin((e - t) * (2 * Math.PI) / r) * .5 + 1);
	},
	backIn: function(e) {
		var t = 1.70158;
		return e * e * ((t + 1) * e - t);
	},
	backOut: function(e) {
		var t = 1.70158;
		return --e * e * ((t + 1) * e + t) + 1;
	},
	backInOut: function(e) {
		var t = 1.70158 * 1.525;
		return (e *= 2) < 1 ? .5 * (e * e * ((t + 1) * e - t)) : .5 * ((e -= 2) * e * ((t + 1) * e + t) + 2);
	},
	bounceIn: function(e) {
		return 1 - Kn.bounceOut(1 - e);
	},
	bounceOut: function(e) {
		return e < 1 / 2.75 ? 7.5625 * e * e : e < 2 / 2.75 ? 7.5625 * (e -= 1.5 / 2.75) * e + .75 : e < 2.5 / 2.75 ? 7.5625 * (e -= 2.25 / 2.75) * e + .9375 : 7.5625 * (e -= 2.625 / 2.75) * e + .984375;
	},
	bounceInOut: function(e) {
		return e < .5 ? Kn.bounceIn(e * 2) * .5 : Kn.bounceOut(e * 2 - 1) * .5 + .5;
	}
}, qn = Math.pow, Jn = Math.sqrt, Yn = 1e-8, Xn = 1e-4, Zn = Jn(3), Qn = 1 / 3, $n = mt(), er = mt(), tr = mt();
function nr(e) {
	return e > -Yn && e < Yn;
}
function rr(e) {
	return e > Yn || e < -Yn;
}
function ir(e, t, n, r, i) {
	var a = 1 - i;
	return a * a * (a * e + 3 * i * t) + i * i * (i * r + 3 * a * n);
}
function ar(e, t, n, r, i) {
	var a = 1 - i;
	return 3 * (((t - e) * a + 2 * (n - t) * i) * a + (r - n) * i * i);
}
function or(e, t, n, r, i, a) {
	var o = r + 3 * (t - n) - e, s = 3 * (n - t * 2 + e), c = 3 * (t - e), l = e - i, u = s * s - 3 * o * c, d = s * c - 9 * o * l, f = c * c - 3 * s * l, p = 0;
	if (nr(u) && nr(d)) if (nr(s)) a[0] = 0;
	else {
		var m = -c / s;
		m >= 0 && m <= 1 && (a[p++] = m);
	}
	else {
		var h = d * d - 4 * u * f;
		if (nr(h)) {
			var g = d / u, m = -s / o + g, _ = -g / 2;
			m >= 0 && m <= 1 && (a[p++] = m), _ >= 0 && _ <= 1 && (a[p++] = _);
		} else if (h > 0) {
			var v = Jn(h), y = u * s + 1.5 * o * (-d + v), b = u * s + 1.5 * o * (-d - v);
			y = y < 0 ? -qn(-y, Qn) : qn(y, Qn), b = b < 0 ? -qn(-b, Qn) : qn(b, Qn);
			var m = (-s - (y + b)) / (3 * o);
			m >= 0 && m <= 1 && (a[p++] = m);
		} else {
			var x = (2 * u * s - 3 * o * d) / (2 * Jn(u * u * u)), S = Math.acos(x) / 3, C = Jn(u), w = Math.cos(S), m = (-s - 2 * C * w) / (3 * o), _ = (-s + C * (w + Zn * Math.sin(S))) / (3 * o), T = (-s + C * (w - Zn * Math.sin(S))) / (3 * o);
			m >= 0 && m <= 1 && (a[p++] = m), _ >= 0 && _ <= 1 && (a[p++] = _), T >= 0 && T <= 1 && (a[p++] = T);
		}
	}
	return p;
}
function sr(e, t, n, r, i) {
	var a = 6 * n - 12 * t + 6 * e, o = 9 * t + 3 * r - 3 * e - 9 * n, s = 3 * t - 3 * e, c = 0;
	if (nr(o)) {
		if (rr(a)) {
			var l = -s / a;
			l >= 0 && l <= 1 && (i[c++] = l);
		}
	} else {
		var u = a * a - 4 * o * s;
		if (nr(u)) i[0] = -a / (2 * o);
		else if (u > 0) {
			var d = Jn(u), l = (-a + d) / (2 * o), f = (-a - d) / (2 * o);
			l >= 0 && l <= 1 && (i[c++] = l), f >= 0 && f <= 1 && (i[c++] = f);
		}
	}
	return c;
}
function cr(e, t, n, r, i, a) {
	var o = (t - e) * i + e, s = (n - t) * i + t, c = (r - n) * i + n, l = (s - o) * i + o, u = (c - s) * i + s, d = (u - l) * i + l;
	a[0] = e, a[1] = o, a[2] = l, a[3] = d, a[4] = d, a[5] = u, a[6] = c, a[7] = r;
}
function lr(e, t, n, r, i, a, o, s, c, l, u) {
	var d, f = .005, p = Infinity, m, h, g, _;
	$n[0] = c, $n[1] = l;
	for (var v = 0; v < 1; v += .05) er[0] = ir(e, n, i, o, v), er[1] = ir(t, r, a, s, v), g = Et($n, er), g < p && (d = v, p = g);
	p = Infinity;
	for (var y = 0; y < 32 && !(f < Xn); y++) m = d - f, h = d + f, er[0] = ir(e, n, i, o, m), er[1] = ir(t, r, a, s, m), g = Et(er, $n), m >= 0 && g < p ? (d = m, p = g) : (tr[0] = ir(e, n, i, o, h), tr[1] = ir(t, r, a, s, h), _ = Et(tr, $n), h <= 1 && _ < p ? (d = h, p = _) : f *= .5);
	return u && (u[0] = ir(e, n, i, o, d), u[1] = ir(t, r, a, s, d)), Jn(p);
}
function ur(e, t, n, r, i, a, o, s, c) {
	for (var l = e, u = t, d = 0, f = 1 / c, p = 1; p <= c; p++) {
		var m = p * f, h = ir(e, n, i, o, m), g = ir(t, r, a, s, m), _ = h - l, v = g - u;
		d += Math.sqrt(_ * _ + v * v), l = h, u = g;
	}
	return d;
}
function dr(e, t, n, r) {
	var i = 1 - r;
	return i * (i * e + 2 * r * t) + r * r * n;
}
function fr(e, t, n, r) {
	return 2 * ((1 - r) * (t - e) + r * (n - t));
}
function pr(e, t, n, r, i) {
	var a = e - 2 * t + n, o = 2 * (t - e), s = e - r, c = 0;
	if (nr(a)) {
		if (rr(o)) {
			var l = -s / o;
			l >= 0 && l <= 1 && (i[c++] = l);
		}
	} else {
		var u = o * o - 4 * a * s;
		if (nr(u)) {
			var l = -o / (2 * a);
			l >= 0 && l <= 1 && (i[c++] = l);
		} else if (u > 0) {
			var d = Jn(u), l = (-o + d) / (2 * a), f = (-o - d) / (2 * a);
			l >= 0 && l <= 1 && (i[c++] = l), f >= 0 && f <= 1 && (i[c++] = f);
		}
	}
	return c;
}
function mr(e, t, n) {
	var r = e + n - 2 * t;
	return r === 0 ? .5 : (e - t) / r;
}
function hr(e, t, n, r, i) {
	var a = (t - e) * r + e, o = (n - t) * r + t, s = (o - a) * r + a;
	i[0] = e, i[1] = a, i[2] = s, i[3] = s, i[4] = o, i[5] = n;
}
function gr(e, t, n, r, i, a, o, s, c) {
	var l, u = .005, d = Infinity;
	$n[0] = o, $n[1] = s;
	for (var f = 0; f < 1; f += .05) {
		er[0] = dr(e, n, i, f), er[1] = dr(t, r, a, f);
		var p = Et($n, er);
		p < d && (l = f, d = p);
	}
	d = Infinity;
	for (var m = 0; m < 32 && !(u < Xn); m++) {
		var h = l - u, g = l + u;
		er[0] = dr(e, n, i, h), er[1] = dr(t, r, a, h);
		var p = Et(er, $n);
		if (h >= 0 && p < d) l = h, d = p;
		else {
			tr[0] = dr(e, n, i, g), tr[1] = dr(t, r, a, g);
			var _ = Et(tr, $n);
			g <= 1 && _ < d ? (l = g, d = _) : u *= .5;
		}
	}
	return c && (c[0] = dr(e, n, i, l), c[1] = dr(t, r, a, l)), Jn(d);
}
function _r(e, t, n, r, i, a, o) {
	for (var s = e, c = t, l = 0, u = 1 / o, d = 1; d <= o; d++) {
		var f = d * u, p = dr(e, n, i, f), m = dr(t, r, a, f), h = p - s, g = m - c;
		l += Math.sqrt(h * h + g * g), s = p, c = m;
	}
	return l;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/animation/cubicEasing.js
var vr = /cubic-bezier\(([0-9,\.e ]+)\)/;
function yr(e) {
	var t = e && vr.exec(e);
	if (t) {
		var n = t[1].split(","), r = +ye(n[0]), i = +ye(n[1]), a = +ye(n[2]), o = +ye(n[3]);
		if (isNaN(r + i + a + o)) return;
		var s = [];
		return function(e) {
			return e <= 0 ? 0 : e >= 1 ? 1 : or(0, r, a, 1, e, s) && ir(0, i, o, 1, s[0]);
		};
	}
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/animation/Clip.js
var br = function() {
	function e(e) {
		this._inited = !1, this._startTime = 0, this._pausedTime = 0, this._paused = !1, this._life = e.life || 1e3, this._delay = e.delay || 0, this.loop = e.loop || !1, this.onframe = e.onframe || je, this.ondestroy = e.ondestroy || je, this.onrestart = e.onrestart || je, e.easing && this.setEasing(e.easing);
	}
	return e.prototype.step = function(e, t) {
		if (this._inited ||= (this._startTime = e + this._delay, !0), this._paused) {
			this._pausedTime += t;
			return;
		}
		var n = this._life, r = e - this._startTime - this._pausedTime, i = r / n;
		i < 0 && (i = 0), i = Math.min(i, 1);
		var a = this.easingFunc, o = a ? a(i) : i;
		if (this.onframe(o), i === 1) if (this.loop) {
			var s = r % n;
			this._startTime = e - s, this._pausedTime = 0, this.onrestart();
		} else return !0;
		return !1;
	}, e.prototype.pause = function() {
		this._paused = !0;
	}, e.prototype.resume = function() {
		this._paused = !1;
	}, e.prototype.setEasing = function(e) {
		this.easing = e, this.easingFunc = H(e) ? e : Kn[e] || yr(e);
	}, e;
}(), xr = {
	transparent: [
		0,
		0,
		0,
		0
	],
	aliceblue: [
		240,
		248,
		255,
		1
	],
	antiquewhite: [
		250,
		235,
		215,
		1
	],
	aqua: [
		0,
		255,
		255,
		1
	],
	aquamarine: [
		127,
		255,
		212,
		1
	],
	azure: [
		240,
		255,
		255,
		1
	],
	beige: [
		245,
		245,
		220,
		1
	],
	bisque: [
		255,
		228,
		196,
		1
	],
	black: [
		0,
		0,
		0,
		1
	],
	blanchedalmond: [
		255,
		235,
		205,
		1
	],
	blue: [
		0,
		0,
		255,
		1
	],
	blueviolet: [
		138,
		43,
		226,
		1
	],
	brown: [
		165,
		42,
		42,
		1
	],
	burlywood: [
		222,
		184,
		135,
		1
	],
	cadetblue: [
		95,
		158,
		160,
		1
	],
	chartreuse: [
		127,
		255,
		0,
		1
	],
	chocolate: [
		210,
		105,
		30,
		1
	],
	coral: [
		255,
		127,
		80,
		1
	],
	cornflowerblue: [
		100,
		149,
		237,
		1
	],
	cornsilk: [
		255,
		248,
		220,
		1
	],
	crimson: [
		220,
		20,
		60,
		1
	],
	cyan: [
		0,
		255,
		255,
		1
	],
	darkblue: [
		0,
		0,
		139,
		1
	],
	darkcyan: [
		0,
		139,
		139,
		1
	],
	darkgoldenrod: [
		184,
		134,
		11,
		1
	],
	darkgray: [
		169,
		169,
		169,
		1
	],
	darkgreen: [
		0,
		100,
		0,
		1
	],
	darkgrey: [
		169,
		169,
		169,
		1
	],
	darkkhaki: [
		189,
		183,
		107,
		1
	],
	darkmagenta: [
		139,
		0,
		139,
		1
	],
	darkolivegreen: [
		85,
		107,
		47,
		1
	],
	darkorange: [
		255,
		140,
		0,
		1
	],
	darkorchid: [
		153,
		50,
		204,
		1
	],
	darkred: [
		139,
		0,
		0,
		1
	],
	darksalmon: [
		233,
		150,
		122,
		1
	],
	darkseagreen: [
		143,
		188,
		143,
		1
	],
	darkslateblue: [
		72,
		61,
		139,
		1
	],
	darkslategray: [
		47,
		79,
		79,
		1
	],
	darkslategrey: [
		47,
		79,
		79,
		1
	],
	darkturquoise: [
		0,
		206,
		209,
		1
	],
	darkviolet: [
		148,
		0,
		211,
		1
	],
	deeppink: [
		255,
		20,
		147,
		1
	],
	deepskyblue: [
		0,
		191,
		255,
		1
	],
	dimgray: [
		105,
		105,
		105,
		1
	],
	dimgrey: [
		105,
		105,
		105,
		1
	],
	dodgerblue: [
		30,
		144,
		255,
		1
	],
	firebrick: [
		178,
		34,
		34,
		1
	],
	floralwhite: [
		255,
		250,
		240,
		1
	],
	forestgreen: [
		34,
		139,
		34,
		1
	],
	fuchsia: [
		255,
		0,
		255,
		1
	],
	gainsboro: [
		220,
		220,
		220,
		1
	],
	ghostwhite: [
		248,
		248,
		255,
		1
	],
	gold: [
		255,
		215,
		0,
		1
	],
	goldenrod: [
		218,
		165,
		32,
		1
	],
	gray: [
		128,
		128,
		128,
		1
	],
	green: [
		0,
		128,
		0,
		1
	],
	greenyellow: [
		173,
		255,
		47,
		1
	],
	grey: [
		128,
		128,
		128,
		1
	],
	honeydew: [
		240,
		255,
		240,
		1
	],
	hotpink: [
		255,
		105,
		180,
		1
	],
	indianred: [
		205,
		92,
		92,
		1
	],
	indigo: [
		75,
		0,
		130,
		1
	],
	ivory: [
		255,
		255,
		240,
		1
	],
	khaki: [
		240,
		230,
		140,
		1
	],
	lavender: [
		230,
		230,
		250,
		1
	],
	lavenderblush: [
		255,
		240,
		245,
		1
	],
	lawngreen: [
		124,
		252,
		0,
		1
	],
	lemonchiffon: [
		255,
		250,
		205,
		1
	],
	lightblue: [
		173,
		216,
		230,
		1
	],
	lightcoral: [
		240,
		128,
		128,
		1
	],
	lightcyan: [
		224,
		255,
		255,
		1
	],
	lightgoldenrodyellow: [
		250,
		250,
		210,
		1
	],
	lightgray: [
		211,
		211,
		211,
		1
	],
	lightgreen: [
		144,
		238,
		144,
		1
	],
	lightgrey: [
		211,
		211,
		211,
		1
	],
	lightpink: [
		255,
		182,
		193,
		1
	],
	lightsalmon: [
		255,
		160,
		122,
		1
	],
	lightseagreen: [
		32,
		178,
		170,
		1
	],
	lightskyblue: [
		135,
		206,
		250,
		1
	],
	lightslategray: [
		119,
		136,
		153,
		1
	],
	lightslategrey: [
		119,
		136,
		153,
		1
	],
	lightsteelblue: [
		176,
		196,
		222,
		1
	],
	lightyellow: [
		255,
		255,
		224,
		1
	],
	lime: [
		0,
		255,
		0,
		1
	],
	limegreen: [
		50,
		205,
		50,
		1
	],
	linen: [
		250,
		240,
		230,
		1
	],
	magenta: [
		255,
		0,
		255,
		1
	],
	maroon: [
		128,
		0,
		0,
		1
	],
	mediumaquamarine: [
		102,
		205,
		170,
		1
	],
	mediumblue: [
		0,
		0,
		205,
		1
	],
	mediumorchid: [
		186,
		85,
		211,
		1
	],
	mediumpurple: [
		147,
		112,
		219,
		1
	],
	mediumseagreen: [
		60,
		179,
		113,
		1
	],
	mediumslateblue: [
		123,
		104,
		238,
		1
	],
	mediumspringgreen: [
		0,
		250,
		154,
		1
	],
	mediumturquoise: [
		72,
		209,
		204,
		1
	],
	mediumvioletred: [
		199,
		21,
		133,
		1
	],
	midnightblue: [
		25,
		25,
		112,
		1
	],
	mintcream: [
		245,
		255,
		250,
		1
	],
	mistyrose: [
		255,
		228,
		225,
		1
	],
	moccasin: [
		255,
		228,
		181,
		1
	],
	navajowhite: [
		255,
		222,
		173,
		1
	],
	navy: [
		0,
		0,
		128,
		1
	],
	oldlace: [
		253,
		245,
		230,
		1
	],
	olive: [
		128,
		128,
		0,
		1
	],
	olivedrab: [
		107,
		142,
		35,
		1
	],
	orange: [
		255,
		165,
		0,
		1
	],
	orangered: [
		255,
		69,
		0,
		1
	],
	orchid: [
		218,
		112,
		214,
		1
	],
	palegoldenrod: [
		238,
		232,
		170,
		1
	],
	palegreen: [
		152,
		251,
		152,
		1
	],
	paleturquoise: [
		175,
		238,
		238,
		1
	],
	palevioletred: [
		219,
		112,
		147,
		1
	],
	papayawhip: [
		255,
		239,
		213,
		1
	],
	peachpuff: [
		255,
		218,
		185,
		1
	],
	peru: [
		205,
		133,
		63,
		1
	],
	pink: [
		255,
		192,
		203,
		1
	],
	plum: [
		221,
		160,
		221,
		1
	],
	powderblue: [
		176,
		224,
		230,
		1
	],
	purple: [
		128,
		0,
		128,
		1
	],
	red: [
		255,
		0,
		0,
		1
	],
	rosybrown: [
		188,
		143,
		143,
		1
	],
	royalblue: [
		65,
		105,
		225,
		1
	],
	saddlebrown: [
		139,
		69,
		19,
		1
	],
	salmon: [
		250,
		128,
		114,
		1
	],
	sandybrown: [
		244,
		164,
		96,
		1
	],
	seagreen: [
		46,
		139,
		87,
		1
	],
	seashell: [
		255,
		245,
		238,
		1
	],
	sienna: [
		160,
		82,
		45,
		1
	],
	silver: [
		192,
		192,
		192,
		1
	],
	skyblue: [
		135,
		206,
		235,
		1
	],
	slateblue: [
		106,
		90,
		205,
		1
	],
	slategray: [
		112,
		128,
		144,
		1
	],
	slategrey: [
		112,
		128,
		144,
		1
	],
	snow: [
		255,
		250,
		250,
		1
	],
	springgreen: [
		0,
		255,
		127,
		1
	],
	steelblue: [
		70,
		130,
		180,
		1
	],
	tan: [
		210,
		180,
		140,
		1
	],
	teal: [
		0,
		128,
		128,
		1
	],
	thistle: [
		216,
		191,
		216,
		1
	],
	tomato: [
		255,
		99,
		71,
		1
	],
	turquoise: [
		64,
		224,
		208,
		1
	],
	violet: [
		238,
		130,
		238,
		1
	],
	wheat: [
		245,
		222,
		179,
		1
	],
	white: [
		255,
		255,
		255,
		1
	],
	whitesmoke: [
		245,
		245,
		245,
		1
	],
	yellow: [
		255,
		255,
		0,
		1
	],
	yellowgreen: [
		154,
		205,
		50,
		1
	]
};
function Sr(e) {
	return e = Math.round(e), e < 0 ? 0 : e > 255 ? 255 : e;
}
function Cr(e) {
	return e = Math.round(e), e < 0 ? 0 : e > 360 ? 360 : e;
}
function wr(e) {
	return e < 0 ? 0 : e > 1 ? 1 : e;
}
function Tr(e) {
	var t = e;
	return t.length && t.charAt(t.length - 1) === "%" ? Sr(parseFloat(t) / 100 * 255) : Sr(parseInt(t, 10));
}
function Er(e) {
	var t = e;
	return t.length && t.charAt(t.length - 1) === "%" ? wr(parseFloat(t) / 100) : wr(parseFloat(t));
}
function Dr(e, t, n) {
	return n < 0 ? n += 1 : n > 1 && --n, n * 6 < 1 ? e + (t - e) * n * 6 : n * 2 < 1 ? t : n * 3 < 2 ? e + (t - e) * (2 / 3 - n) * 6 : e;
}
function Or(e, t, n) {
	return e + (t - e) * n;
}
function kr(e, t, n, r, i) {
	return e[0] = t, e[1] = n, e[2] = r, e[3] = i, e;
}
function Ar(e, t) {
	return e[0] = t[0], e[1] = t[1], e[2] = t[2], e[3] = t[3], e;
}
var jr = new et(20), Mr = null;
function Nr(e, t) {
	Mr && Ar(Mr, t), Mr = jr.put(e, Mr || t.slice());
}
function Pr(e, t) {
	if (e) {
		t ||= [];
		var n = jr.get(e);
		if (n) return Ar(t, n);
		e += "";
		var r = e.replace(/ /g, "").toLowerCase();
		if (r in xr) return Ar(t, xr[r]), Nr(e, t), t;
		var i = r.length;
		if (r.charAt(0) === "#") {
			if (i === 4 || i === 5) {
				var a = parseInt(r.slice(1, 4), 16);
				if (!(a >= 0 && a <= 4095)) {
					kr(t, 0, 0, 0, 1);
					return;
				}
				return kr(t, (a & 3840) >> 4 | (a & 3840) >> 8, a & 240 | (a & 240) >> 4, a & 15 | (a & 15) << 4, i === 5 ? parseInt(r.slice(4), 16) / 15 : 1), Nr(e, t), t;
			} else if (i === 7 || i === 9) {
				var a = parseInt(r.slice(1, 7), 16);
				if (!(a >= 0 && a <= 16777215)) {
					kr(t, 0, 0, 0, 1);
					return;
				}
				return kr(t, (a & 16711680) >> 16, (a & 65280) >> 8, a & 255, i === 9 ? parseInt(r.slice(7), 16) / 255 : 1), Nr(e, t), t;
			}
			return;
		}
		var o = r.indexOf("("), s = r.indexOf(")");
		if (o !== -1 && s + 1 === i) {
			var c = r.substr(0, o), l = r.substr(o + 1, s - (o + 1)).split(","), u = 1;
			switch (c) {
				case "rgba":
					if (l.length !== 4) return l.length === 3 ? kr(t, +l[0], +l[1], +l[2], 1) : kr(t, 0, 0, 0, 1);
					u = Er(l.pop());
				case "rgb":
					if (l.length >= 3) return kr(t, Tr(l[0]), Tr(l[1]), Tr(l[2]), l.length === 3 ? u : Er(l[3])), Nr(e, t), t;
					kr(t, 0, 0, 0, 1);
					return;
				case "hsla":
					if (l.length !== 4) {
						kr(t, 0, 0, 0, 1);
						return;
					}
					return l[3] = Er(l[3]), Fr(l, t), Nr(e, t), t;
				case "hsl":
					if (l.length !== 3) {
						kr(t, 0, 0, 0, 1);
						return;
					}
					return Fr(l, t), Nr(e, t), t;
				default: return;
			}
		}
		kr(t, 0, 0, 0, 1);
	}
}
function Fr(e, t) {
	var n = (parseFloat(e[0]) % 360 + 360) % 360 / 360, r = Er(e[1]), i = Er(e[2]), a = i <= .5 ? i * (r + 1) : i + r - i * r, o = i * 2 - a;
	return t ||= [], kr(t, Sr(Dr(o, a, n + 1 / 3) * 255), Sr(Dr(o, a, n) * 255), Sr(Dr(o, a, n - 1 / 3) * 255), 1), e.length === 4 && (t[3] = e[3]), t;
}
function Ir(e) {
	if (e) {
		var t = e[0] / 255, n = e[1] / 255, r = e[2] / 255, i = Math.min(t, n, r), a = Math.max(t, n, r), o = a - i, s = (a + i) / 2, c, l;
		if (o === 0) c = 0, l = 0;
		else {
			l = s < .5 ? o / (a + i) : o / (2 - a - i);
			var u = ((a - t) / 6 + o / 2) / o, d = ((a - n) / 6 + o / 2) / o, f = ((a - r) / 6 + o / 2) / o;
			t === a ? c = f - d : n === a ? c = 1 / 3 + u - f : r === a && (c = 2 / 3 + d - u), c < 0 && (c += 1), c > 1 && --c;
		}
		var p = [
			c * 360,
			l,
			s
		];
		return e[3] != null && p.push(e[3]), p;
	}
}
function Lr(e, t) {
	var n = Pr(e);
	if (n) {
		for (var r = 0; r < 3; r++) t < 0 ? n[r] = n[r] * (1 - t) | 0 : n[r] = (255 - n[r]) * t + n[r] | 0, n[r] > 255 ? n[r] = 255 : n[r] < 0 && (n[r] = 0);
		return Br(n, n.length === 4 ? "rgba" : "rgb");
	}
}
function Rr(e, t, n) {
	if (!(!(t && t.length) || !(e >= 0 && e <= 1))) {
		var r = e * (t.length - 1), i = Math.floor(r), a = Math.ceil(r), o = Pr(t[i]), s = Pr(t[a]), c = r - i, l = Br([
			Sr(Or(o[0], s[0], c)),
			Sr(Or(o[1], s[1], c)),
			Sr(Or(o[2], s[2], c)),
			wr(Or(o[3], s[3], c))
		], "rgba");
		return n ? {
			color: l,
			leftIndex: i,
			rightIndex: a,
			value: r
		} : l;
	}
}
function zr(e, t, n, r) {
	var i = Pr(e);
	if (e) return i = Ir(i), t != null && (i[0] = Cr(H(t) ? t(i[0]) : t)), n != null && (i[1] = Er(H(n) ? n(i[1]) : n)), r != null && (i[2] = Er(H(r) ? r(i[2]) : r)), Br(Fr(i), "rgba");
}
function Br(e, t) {
	if (!(!e || !e.length)) {
		var n = e[0] + "," + e[1] + "," + e[2];
		return (t === "rgba" || t === "hsva" || t === "hsla") && (n += "," + e[3]), t + "(" + n + ")";
	}
}
function Vr(e, t) {
	var n = Pr(e);
	return n ? (.299 * n[0] + .587 * n[1] + .114 * n[2]) * n[3] / 255 + (1 - n[3]) * t : 0;
}
var Hr = new et(100);
function Ur(e) {
	if (U(e)) {
		var t = Hr.get(e);
		return t || (t = Lr(e, -.1), Hr.put(e, t)), t;
	} else if (de(e)) {
		var n = j({}, e);
		return n.colorStops = L(e.colorStops, function(e) {
			return {
				offset: e.offset,
				color: Lr(e.color, -.1)
			};
		}), n;
	}
	return e;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/svg/helper.js
function Wr(e) {
	return e.type === "linear";
}
function Gr(e) {
	return e.type === "radial";
}
(function() {
	return typeof Buffer < "u" && typeof Buffer.from == "function" ? function(e) {
		return Buffer.from(e).toString("base64");
	} : typeof btoa == "function" && typeof unescape == "function" && typeof encodeURIComponent == "function" ? function(e) {
		return btoa(unescape(encodeURIComponent(e)));
	} : function(e) {
		return null;
	};
})();
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/animation/Animator.js
var Kr = Array.prototype.slice;
function qr(e, t, n) {
	return (t - e) * n + e;
}
function Jr(e, t, n, r) {
	for (var i = t.length, a = 0; a < i; a++) e[a] = qr(t[a], n[a], r);
	return e;
}
function Yr(e, t, n, r) {
	for (var i = t.length, a = i && t[0].length, o = 0; o < i; o++) {
		e[o] || (e[o] = []);
		for (var s = 0; s < a; s++) e[o][s] = qr(t[o][s], n[o][s], r);
	}
	return e;
}
function Xr(e, t, n, r) {
	for (var i = t.length, a = 0; a < i; a++) e[a] = t[a] + n[a] * r;
	return e;
}
function Zr(e, t, n, r) {
	for (var i = t.length, a = i && t[0].length, o = 0; o < i; o++) {
		e[o] || (e[o] = []);
		for (var s = 0; s < a; s++) e[o][s] = t[o][s] + n[o][s] * r;
	}
	return e;
}
function Qr(e, t) {
	for (var n = e.length, r = t.length, i = n > r ? t : e, a = Math.min(n, r), o = i[a - 1] || {
		color: [
			0,
			0,
			0,
			0
		],
		offset: 0
	}, s = a; s < Math.max(n, r); s++) i.push({
		offset: o.offset,
		color: o.color.slice()
	});
}
function $r(e, t, n) {
	var r = e, i = t;
	if (!(!r.push || !i.push)) {
		var a = r.length, o = i.length;
		if (a !== o) if (a > o) r.length = o;
		else for (var s = a; s < o; s++) r.push(n === 1 ? i[s] : Kr.call(i[s]));
		for (var c = r[0] && r[0].length, s = 0; s < r.length; s++) if (n === 1) isNaN(r[s]) && (r[s] = i[s]);
		else for (var l = 0; l < c; l++) isNaN(r[s][l]) && (r[s][l] = i[s][l]);
	}
}
function ei(e) {
	if (F(e)) {
		var t = e.length;
		if (F(e[0])) {
			for (var n = [], r = 0; r < t; r++) n.push(Kr.call(e[r]));
			return n;
		}
		return Kr.call(e);
	}
	return e;
}
function ti(e) {
	return e[0] = Math.floor(e[0]) || 0, e[1] = Math.floor(e[1]) || 0, e[2] = Math.floor(e[2]) || 0, e[3] = e[3] == null ? 1 : e[3], "rgba(" + e.join(",") + ")";
}
function ni(e) {
	return F(e && e[0]) ? 2 : 1;
}
var ri = 0, ii = 1, ai = 2, oi = 3, si = 4, ci = 5, li = 6;
function ui(e) {
	return e === si || e === ci;
}
function di(e) {
	return e === ii || e === ai;
}
var fi = [
	0,
	0,
	0,
	0
], pi = function() {
	function e(e) {
		this.keyframes = [], this.discrete = !1, this._invalid = !1, this._needsSort = !1, this._lastFr = 0, this._lastFrP = 0, this.propName = e;
	}
	return e.prototype.isFinished = function() {
		return this._finished;
	}, e.prototype.setFinished = function() {
		this._finished = !0, this._additiveTrack && this._additiveTrack.setFinished();
	}, e.prototype.needsAnimate = function() {
		return this.keyframes.length >= 1;
	}, e.prototype.getAdditiveTrack = function() {
		return this._additiveTrack;
	}, e.prototype.addKeyframe = function(e, t, n) {
		this._needsSort = !0;
		var r = this.keyframes, i = r.length, a = !1, o = li, s = t;
		if (F(t)) {
			var c = ni(t);
			o = c, (c === 1 && !se(t[0]) || c === 2 && !se(t[0][0])) && (a = !0);
		} else if (se(t) && !pe(t)) o = ri;
		else if (U(t)) if (!isNaN(+t)) o = ri;
		else {
			var l = Pr(t);
			l && (s = l, o = oi);
		}
		else if (de(t)) {
			var u = j({}, s);
			u.colorStops = L(t.colorStops, function(e) {
				return {
					offset: e.offset,
					color: Pr(e.color)
				};
			}), Wr(t) ? o = si : Gr(t) && (o = ci), s = u;
		}
		i === 0 ? this.valType = o : (o !== this.valType || o === li) && (a = !0), this.discrete = this.discrete || a;
		var d = {
			time: e,
			value: s,
			rawValue: t,
			percent: 0
		};
		return n && (d.easing = n, d.easingFunc = H(n) ? n : Kn[n] || yr(n)), r.push(d), d;
	}, e.prototype.prepare = function(e, t) {
		var n = this.keyframes;
		this._needsSort && n.sort(function(e, t) {
			return e.time - t.time;
		});
		for (var r = this.valType, i = n.length, a = n[i - 1], o = this.discrete, s = di(r), c = ui(r), l = 0; l < i; l++) {
			var u = n[l], d = u.value, f = a.value;
			u.percent = u.time / e, o || (s && l !== i - 1 ? $r(d, f, r) : c && Qr(d.colorStops, f.colorStops));
		}
		if (!o && r !== ci && t && this.needsAnimate() && t.needsAnimate() && r === t.valType && !t._finished) {
			this._additiveTrack = t;
			for (var p = n[0].value, l = 0; l < i; l++) r === ri ? n[l].additiveValue = n[l].value - p : r === oi ? n[l].additiveValue = Xr([], n[l].value, p, -1) : di(r) && (n[l].additiveValue = r === ii ? Xr([], n[l].value, p, -1) : Zr([], n[l].value, p, -1));
		}
	}, e.prototype.step = function(e, t) {
		if (!this._finished) {
			this._additiveTrack && this._additiveTrack._finished && (this._additiveTrack = null);
			var n = this._additiveTrack != null, r = n ? "additiveValue" : "value", i = this.valType, a = this.keyframes, o = a.length, s = this.propName, c = i === oi, l, u = this._lastFr, d = Math.min, f, p;
			if (o === 1) f = p = a[0];
			else {
				if (t < 0) l = 0;
				else if (t < this._lastFrP) {
					for (l = d(u + 1, o - 1); l >= 0 && !(a[l].percent <= t); l--);
					l = d(l, o - 2);
				} else {
					for (l = u; l < o && !(a[l].percent > t); l++);
					l = d(l - 1, o - 2);
				}
				p = a[l + 1], f = a[l];
			}
			if (f && p) {
				this._lastFr = l, this._lastFrP = t;
				var m = p.percent - f.percent, h = m === 0 ? 1 : d((t - f.percent) / m, 1);
				p.easingFunc && (h = p.easingFunc(h));
				var g = n ? this._additiveValue : c ? fi : e[s];
				if ((di(i) || c) && !g && (g = this._additiveValue = []), this.discrete) e[s] = h < 1 ? f.rawValue : p.rawValue;
				else if (di(i)) i === ii ? Jr(g, f[r], p[r], h) : Yr(g, f[r], p[r], h);
				else if (ui(i)) {
					var _ = f[r], v = p[r], y = i === si;
					e[s] = {
						type: y ? "linear" : "radial",
						x: qr(_.x, v.x, h),
						y: qr(_.y, v.y, h),
						colorStops: L(_.colorStops, function(e, t) {
							var n = v.colorStops[t];
							return {
								offset: qr(e.offset, n.offset, h),
								color: ti(Jr([], e.color, n.color, h))
							};
						}),
						global: v.global
					}, y ? (e[s].x2 = qr(_.x2, v.x2, h), e[s].y2 = qr(_.y2, v.y2, h)) : e[s].r = qr(_.r, v.r, h);
				} else if (c) Jr(g, f[r], p[r], h), n || (e[s] = ti(g));
				else {
					var b = qr(f[r], p[r], h);
					n ? this._additiveValue = b : e[s] = b;
				}
				n && this._addToTarget(e);
			}
		}
	}, e.prototype._addToTarget = function(e) {
		var t = this.valType, n = this.propName, r = this._additiveValue;
		t === ri ? e[n] = e[n] + r : t === oi ? (Pr(e[n], fi), Xr(fi, fi, r, 1), e[n] = ti(fi)) : t === ii ? Xr(e[n], e[n], r, 1) : t === ai && Zr(e[n], e[n], r, 1);
	}, e;
}(), mi = function() {
	function e(e, t, n, r) {
		if (this._tracks = {}, this._trackKeys = [], this._maxTime = 0, this._started = 0, this._clip = null, this._target = e, this._loop = t, t && r) {
			O("Can' use additive animation on looped animation.");
			return;
		}
		this._additiveAnimators = r, this._allowDiscrete = n;
	}
	return e.prototype.getMaxTime = function() {
		return this._maxTime;
	}, e.prototype.getDelay = function() {
		return this._delay;
	}, e.prototype.getLoop = function() {
		return this._loop;
	}, e.prototype.getTarget = function() {
		return this._target;
	}, e.prototype.changeTarget = function(e) {
		this._target = e;
	}, e.prototype.when = function(e, t, n) {
		return this.whenWithKeys(e, t, R(t), n);
	}, e.prototype.whenWithKeys = function(e, t, n, r) {
		for (var i = this._tracks, a = 0; a < n.length; a++) {
			var o = n[a], s = i[o];
			if (!s) {
				s = i[o] = new pi(o);
				var c = void 0, l = this._getAdditiveTrack(o);
				if (l) {
					var u = l.keyframes, d = u[u.length - 1];
					c = d && d.value, l.valType === oi && c && (c = ti(c));
				} else c = this._target[o];
				if (c == null) continue;
				e > 0 && s.addKeyframe(0, ei(c), r), this._trackKeys.push(o);
			}
			s.addKeyframe(e, ei(t[o]), r);
		}
		return this._maxTime = Math.max(this._maxTime, e), this;
	}, e.prototype.pause = function() {
		this._clip.pause(), this._paused = !0;
	}, e.prototype.resume = function() {
		this._clip.resume(), this._paused = !1;
	}, e.prototype.isPaused = function() {
		return !!this._paused;
	}, e.prototype.duration = function(e) {
		return this._maxTime = e, this._force = !0, this;
	}, e.prototype._doneCallback = function() {
		this._setTracksFinished(), this._clip = null;
		var e = this._doneCbs;
		if (e) for (var t = e.length, n = 0; n < t; n++) e[n].call(this);
	}, e.prototype._abortedCallback = function() {
		this._setTracksFinished();
		var e = this.animation, t = this._abortedCbs;
		if (e && e.removeClip(this._clip), this._clip = null, t) for (var n = 0; n < t.length; n++) t[n].call(this);
	}, e.prototype._setTracksFinished = function() {
		for (var e = this._tracks, t = this._trackKeys, n = 0; n < t.length; n++) e[t[n]].setFinished();
	}, e.prototype._getAdditiveTrack = function(e) {
		var t, n = this._additiveAnimators;
		if (n) for (var r = 0; r < n.length; r++) {
			var i = n[r].getTrack(e);
			i && (t = i);
		}
		return t;
	}, e.prototype.start = function(e) {
		if (!(this._started > 0)) {
			this._started = 1;
			for (var t = this, n = [], r = this._maxTime || 0, i = 0; i < this._trackKeys.length; i++) {
				var a = this._trackKeys[i], o = this._tracks[a], s = this._getAdditiveTrack(a), c = o.keyframes, l = c.length;
				if (o.prepare(r, s), o.needsAnimate()) if (!this._allowDiscrete && o.discrete) {
					var u = c[l - 1];
					u && (t._target[o.propName] = u.rawValue), o.setFinished();
				} else n.push(o);
			}
			if (n.length || this._force) {
				var d = new br({
					life: r,
					loop: this._loop,
					delay: this._delay || 0,
					onframe: function(e) {
						t._started = 2;
						var r = t._additiveAnimators;
						if (r) {
							for (var i = !1, a = 0; a < r.length; a++) if (r[a]._clip) {
								i = !0;
								break;
							}
							i || (t._additiveAnimators = null);
						}
						for (var a = 0; a < n.length; a++) n[a].step(t._target, e);
						var o = t._onframeCbs;
						if (o) for (var a = 0; a < o.length; a++) o[a](t._target, e);
					},
					ondestroy: function() {
						t._doneCallback();
					}
				});
				this._clip = d, this.animation && this.animation.addClip(d), e && d.setEasing(e);
			} else this._doneCallback();
			return this;
		}
	}, e.prototype.stop = function(e) {
		if (this._clip) {
			var t = this._clip;
			e && t.onframe(1), this._abortedCallback();
		}
	}, e.prototype.delay = function(e) {
		return this._delay = e, this;
	}, e.prototype.during = function(e) {
		return e && (this._onframeCbs ||= [], this._onframeCbs.push(e)), this;
	}, e.prototype.done = function(e) {
		return e && (this._doneCbs ||= [], this._doneCbs.push(e)), this;
	}, e.prototype.aborted = function(e) {
		return e && (this._abortedCbs ||= [], this._abortedCbs.push(e)), this;
	}, e.prototype.getClip = function() {
		return this._clip;
	}, e.prototype.getTrack = function(e) {
		return this._tracks[e];
	}, e.prototype.getTracks = function() {
		var e = this;
		return L(this._trackKeys, function(t) {
			return e._tracks[t];
		});
	}, e.prototype.stopTracks = function(e, t) {
		if (!e.length || !this._clip) return !0;
		for (var n = this._tracks, r = this._trackKeys, i = 0; i < e.length; i++) {
			var a = n[e[i]];
			a && !a.isFinished() && (t ? a.step(this._target, 1) : this._started === 1 && a.step(this._target, 0), a.setFinished());
		}
		for (var o = !0, i = 0; i < r.length; i++) if (!n[r[i]].isFinished()) {
			o = !1;
			break;
		}
		return o && this._abortedCallback(), o;
	}, e.prototype.saveTo = function(e, t, n) {
		if (e) {
			t ||= this._trackKeys;
			for (var r = 0; r < t.length; r++) {
				var i = t[r], a = this._tracks[i];
				if (!(!a || a.isFinished())) {
					var o = a.keyframes, s = o[n ? 0 : o.length - 1];
					s && (e[i] = ei(s.rawValue));
				}
			}
		}
	}, e.prototype.__changeFinalValue = function(e, t) {
		t ||= R(e);
		for (var n = 0; n < t.length; n++) {
			var r = t[n], i = this._tracks[r];
			if (i) {
				var a = i.keyframes;
				if (a.length > 1) {
					var o = a.pop();
					i.addKeyframe(o.time, e[r]), i.prepare(this._maxTime, i.getAdditiveTrack());
				}
			}
		}
	}, e;
}(), hi = function() {
	function e(e) {
		e && (this._$eventProcessor = e);
	}
	return e.prototype.on = function(e, t, n, r) {
		this._$handlers ||= {};
		var i = this._$handlers;
		if (typeof t == "function" && (r = n, n = t, t = null), !n || !e) return this;
		var a = this._$eventProcessor;
		t != null && a && a.normalizeQuery && (t = a.normalizeQuery(t)), i[e] || (i[e] = []);
		for (var o = 0; o < i[e].length; o++) if (i[e][o].h === n) return this;
		var s = {
			h: n,
			query: t,
			ctx: r || this,
			callAtLast: n.zrEventfulCallAtLast
		}, c = i[e].length - 1, l = i[e][c];
		return l && l.callAtLast ? i[e].splice(c, 0, s) : i[e].push(s), this;
	}, e.prototype.isSilent = function(e) {
		var t = this._$handlers;
		return !t || !t[e] || !t[e].length;
	}, e.prototype.off = function(e, t) {
		var n = this._$handlers;
		if (!n) return this;
		if (!e) return this._$handlers = {}, this;
		if (t) {
			if (n[e]) {
				for (var r = [], i = 0, a = n[e].length; i < a; i++) n[e][i].h !== t && r.push(n[e][i]);
				n[e] = r;
			}
			n[e] && n[e].length === 0 && delete n[e];
		} else delete n[e];
		return this;
	}, e.prototype.trigger = function(e) {
		var t = [...arguments].slice(1);
		if (!this._$handlers) return this;
		var n = this._$handlers[e], r = this._$eventProcessor;
		if (n) for (var i = t.length, a = n.length, o = 0; o < a; o++) {
			var s = n[o];
			if (!(r && r.filter && s.query != null && !r.filter(e, s.query))) switch (i) {
				case 0:
					s.h.call(s.ctx);
					break;
				case 1:
					s.h.call(s.ctx, t[0]);
					break;
				case 2:
					s.h.call(s.ctx, t[0], t[1]);
					break;
				default:
					s.h.apply(s.ctx, t);
					break;
			}
		}
		return r && r.afterTrigger && r.afterTrigger(e), this;
	}, e.prototype.triggerWithContext = function(e) {
		var t = [...arguments].slice(1);
		if (!this._$handlers) return this;
		var n = this._$handlers[e], r = this._$eventProcessor;
		if (n) for (var i = t.length, a = t[i - 1], o = n.length, s = 0; s < o; s++) {
			var c = n[s];
			if (!(r && r.filter && c.query != null && !r.filter(e, c.query))) switch (i) {
				case 0:
					c.h.call(a);
					break;
				case 1:
					c.h.call(a, t[0]);
					break;
				case 2:
					c.h.call(a, t[0], t[1]);
					break;
				default:
					c.h.apply(a, t.slice(1, i - 1));
					break;
			}
		}
		return r && r.afterTrigger && r.afterTrigger(e), this;
	}, e;
}(), gi = 1;
q.hasGlobalWindow && (gi = Math.max(window.devicePixelRatio || window.screen && window.screen.deviceXDPI / window.screen.logicalXDPI || 1, 1));
var _i = gi, vi = .4, yi = "#333", bi = "#ccc", xi = "#eee", Si = "__zr_normal__", Ci = Wn.concat(["ignore"]), wi = ne(Wn, function(e, t) {
	return e[t] = !0, e;
}, { ignore: !1 }), Ti = {}, Ei = new Y(0, 0, 0, 0), Di = [], Oi = function() {
	function e(e) {
		this.id = D(), this.animators = [], this.currentStates = [], this.states = {}, this._init(e);
	}
	return e.prototype._init = function(e) {
		this.attr(e);
	}, e.prototype.drift = function(e, t, n) {
		switch (this.draggable) {
			case "horizontal":
				t = 0;
				break;
			case "vertical":
				e = 0;
				break;
		}
		var r = this.transform;
		r ||= this.transform = [
			1,
			0,
			0,
			1,
			0,
			0
		], r[4] += e, r[5] += t, this.decomposeTransform(), this.markRedraw();
	}, e.prototype.beforeUpdate = function() {}, e.prototype.afterUpdate = function() {}, e.prototype.update = function() {
		this.updateTransform(), this.__dirty && this.updateInnerText();
	}, e.prototype.updateInnerText = function(e) {
		var t = this._textContent;
		if (t && (!t.ignore || e)) {
			this.textConfig ||= {};
			var n = this.textConfig, r = n.local, i = t.innerTransformable, a = void 0, o = void 0, s = !1;
			i.parent = r ? this : null;
			var c = !1;
			i.copyTransform(t);
			var l = n.position != null, u = n.autoOverflowArea, d = void 0;
			if ((u || l) && (d = Ei, n.layoutRect ? d.copy(n.layoutRect) : d.copy(this.getBoundingRect()), r || d.applyTransform(this.transform)), l) {
				this.calculateTextPosition ? this.calculateTextPosition(Ti, n, d) : fn(Ti, n, d), i.x = Ti.x, i.y = Ti.y, a = Ti.align, o = Ti.verticalAlign;
				var f = n.origin;
				if (f && n.rotation != null) {
					var p = void 0, m = void 0;
					f === "center" ? (p = d.width * .5, m = d.height * .5) : (p = dn(f[0], d.width), m = dn(f[1], d.height)), c = !0, i.originX = -i.x + p + (r ? 0 : d.x), i.originY = -i.y + m + (r ? 0 : d.y);
				}
			}
			n.rotation != null && (i.rotation = n.rotation);
			var h = n.offset;
			h && (i.x += h[0], i.y += h[1], c || (i.originX = -h[0], i.originY = -h[1]));
			var g = this._innerTextDefaultStyle ||= {};
			if (u) {
				var _ = g.overflowRect = g.overflowRect || new Y(0, 0, 0, 0);
				i.getLocalTransform(Di), pt(Di, Di), Y.copy(_, d), _.applyTransform(Di);
			} else g.overflowRect = null;
			var v = n.inside == null ? typeof n.position == "string" && n.position.indexOf("inside") >= 0 : n.inside, y = void 0, b = void 0, x = void 0;
			v && this.canBeInsideText() ? (y = n.insideFill, b = n.insideStroke, (y == null || y === "auto") && (y = this.getInsideTextFill()), (b == null || b === "auto") && (b = this.getInsideTextStroke(y), x = !0)) : (y = n.outsideFill, b = n.outsideStroke, (y == null || y === "auto") && (y = this.getOutsideFill()), (b == null || b === "auto") && (b = this.getOutsideStroke(y), x = !0)), y ||= "#000", (y !== g.fill || b !== g.stroke || x !== g.autoStroke || a !== g.align || o !== g.verticalAlign) && (s = !0, g.fill = y, g.stroke = b, g.autoStroke = x, g.align = a, g.verticalAlign = o, t.setDefaultTextStyle(g)), t.__dirty |= 1, s && t.dirtyStyle(!0);
		}
	}, e.prototype.canBeInsideText = function() {
		return !0;
	}, e.prototype.getInsideTextFill = function() {
		return "#fff";
	}, e.prototype.getInsideTextStroke = function(e) {
		return "#000";
	}, e.prototype.getOutsideFill = function() {
		return this.__zr && this.__zr.isDarkMode() ? bi : yi;
	}, e.prototype.getOutsideStroke = function(e) {
		var t = this.__zr && this.__zr.getBackgroundColor(), n = typeof t == "string" && Pr(t);
		n ||= [
			255,
			255,
			255,
			1
		];
		for (var r = n[3], i = this.__zr.isDarkMode(), a = 0; a < 3; a++) n[a] = n[a] * r + (i ? 0 : 255) * (1 - r);
		return n[3] = 1, Br(n, "rgba");
	}, e.prototype.traverse = function(e, t) {}, e.prototype.attrKV = function(e, t) {
		e === "textConfig" ? this.setTextConfig(t) : e === "textContent" ? this.setTextContent(t) : e === "clipPath" ? this.setClipPath(t) : e === "extra" ? (this.extra = this.extra || {}, j(this.extra, t)) : this[e] = t;
	}, e.prototype.hide = function() {
		this.ignore = !0, this.markRedraw();
	}, e.prototype.show = function() {
		this.ignore = !1, this.markRedraw();
	}, e.prototype.attr = function(e, t) {
		if (typeof e == "string") this.attrKV(e, t);
		else if (W(e)) for (var n = R(e), r = 0; r < n.length; r++) {
			var i = n[r];
			this.attrKV(i, e[i]);
		}
		return this.markRedraw(), this;
	}, e.prototype.saveCurrentToNormalState = function(e) {
		this._innerSaveToNormal(e);
		for (var t = this._normalState, n = 0; n < this.animators.length; n++) {
			var r = this.animators[n], i = r.__fromStateTransition;
			if (!(r.getLoop() || i && i !== "__zr_normal__")) {
				var a = r.targetName, o = a ? t[a] : t;
				r.saveTo(o);
			}
		}
	}, e.prototype._innerSaveToNormal = function(e) {
		var t = this._normalState;
		t ||= this._normalState = {}, e.textConfig && !t.textConfig && (t.textConfig = this.textConfig), this._savePrimaryToNormal(e, t, Ci);
	}, e.prototype._savePrimaryToNormal = function(e, t, n) {
		for (var r = 0; r < n.length; r++) {
			var i = n[r];
			e[i] != null && !(i in t) && (t[i] = this[i]);
		}
	}, e.prototype.hasState = function() {
		return this.currentStates.length > 0;
	}, e.prototype.getState = function(e) {
		return this.states[e];
	}, e.prototype.ensureState = function(e) {
		var t = this.states;
		return t[e] || (t[e] = {}), t[e];
	}, e.prototype.clearStates = function(e) {
		this.useState(Si, !1, e);
	}, e.prototype.useState = function(e, t, n, r) {
		var i = e === Si;
		if (!(!this.hasState() && i)) {
			var a = this.currentStates, o = this.stateTransition;
			if (!(N(a, e) >= 0 && (t || a.length === 1))) {
				var s;
				if (this.stateProxy && !i && (s = this.stateProxy(e)), s ||= this.states && this.states[e], !s && !i) {
					O("State " + e + " not exists.");
					return;
				}
				i || this.saveCurrentToNormalState(s);
				var c = this._textContent, l = Ii(this, c, s, r);
				l && !this.__inHover && (this.__inHover = l), this._applyStateObj(e, s, this._normalState, t, Ri(this, n, o), o);
				var u = this._textGuide;
				return c && c.useState(e, t, n, !!l), u && u.useState(e, t, n, !!l), i ? (this.currentStates = [], this._normalState = {}) : t ? this.currentStates.push(e) : this.currentStates = [e], this._updateAnimationTargets(), this.markRedraw(), !l && this.__inHover && (this.__inHover = 0, this.__dirty &= -2), s;
			}
		}
	}, e.prototype.useStates = function(e, t, n) {
		if (!e.length) this.clearStates();
		else {
			var r = [], i = this.currentStates, a = e.length, o = a === i.length;
			if (o) {
				for (var s = 0; s < a; s++) if (e[s] !== i[s]) {
					o = !1;
					break;
				}
			}
			if (o) return;
			for (var s = 0; s < a; s++) {
				var c = e[s], l = void 0;
				this.stateProxy && (l = this.stateProxy(c, e)), l ||= this.states[c], l && r.push(l);
			}
			var u = r[a - 1], d = this._textContent, f = Ii(this, d, u, n);
			f && !this.__inHover && (this.__inHover = f);
			var p = this._mergeStates(r), m = this.stateTransition;
			this.saveCurrentToNormalState(p), this._applyStateObj(e.join(","), p, this._normalState, !1, Ri(this, t, m), m);
			var h = this._textGuide;
			d && d.useStates(e, t, !!f), h && h.useStates(e, t, !!f), this._updateAnimationTargets(), this.currentStates = e.slice(), this.markRedraw(), !f && this.__inHover && (this.__inHover = 0, this.__dirty &= -2);
		}
	}, e.prototype.isSilent = function() {
		for (var e = this; e;) {
			if (e.silent) return !0;
			var t = e.__hostTarget;
			e = t ? e.ignoreHostSilent ? null : t : e.parent;
		}
		return !1;
	}, e.prototype._updateAnimationTargets = function() {
		for (var e = 0; e < this.animators.length; e++) {
			var t = this.animators[e];
			t.targetName && t.changeTarget(this[t.targetName]);
		}
	}, e.prototype.removeState = function(e) {
		var t = N(this.currentStates, e);
		if (t >= 0) {
			var n = this.currentStates.slice();
			n.splice(t, 1), this.useStates(n);
		}
	}, e.prototype.replaceState = function(e, t, n) {
		var r = this.currentStates.slice(), i = N(r, e), a = N(r, t) >= 0;
		i >= 0 ? a ? r.splice(i, 1) : r[i] = t : n && !a && r.push(t), this.useStates(r);
	}, e.prototype.toggleState = function(e, t) {
		t ? this.useState(e, !0) : this.removeState(e);
	}, e.prototype._mergeStates = function(e) {
		for (var t = {}, n, r = 0; r < e.length; r++) {
			var i = e[r];
			j(t, i), i.textConfig && (n ||= {}, j(n, i.textConfig));
		}
		return n && (t.textConfig = n), t;
	}, e.prototype._applyStateObj = function(e, t, n, r, i, a) {
		if (this.__inHover !== 1) {
			var o = !(t && r);
			t && t.textConfig ? (this.textConfig = j({}, r ? this.textConfig : n.textConfig), j(this.textConfig, t.textConfig)) : o && n.textConfig && (this.textConfig = n.textConfig);
			for (var s = {}, c = !1, l = 0; l < Ci.length; l++) {
				var u = Ci[l], d = i && wi[u];
				t && t[u] != null ? d ? (c = !0, s[u] = t[u]) : this[u] = t[u] : o && n[u] != null && (d ? (c = !0, s[u] = n[u]) : this[u] = n[u]);
			}
			if (!i) for (var l = 0; l < this.animators.length; l++) {
				var f = this.animators[l], p = f.targetName;
				f.getLoop() || f.__changeFinalValue(p ? (t || n)[p] : t || n);
			}
			c && this._transitionState(e, s, a);
		}
	}, e.prototype._attachComponent = function(e) {
		if (!(e.__zr && !e.__hostTarget) && e !== this) {
			var t = this.__zr;
			t && e.addSelfToZr(t), e.__zr = t, e.__hostTarget = this;
		}
	}, e.prototype._detachComponent = function(e) {
		e.__zr && e.removeSelfFromZr(e.__zr), e.__zr = null, e.__hostTarget = null;
	}, e.prototype.getClipPath = function() {
		return this._clipPath;
	}, e.prototype.setClipPath = function(e) {
		this._clipPath && this._clipPath !== e && this.removeClipPath(), this._attachComponent(e), this._clipPath = e, this.markRedraw();
	}, e.prototype.removeClipPath = function() {
		var e = this._clipPath;
		e && (this._detachComponent(e), this._clipPath = null, this.markRedraw());
	}, e.prototype.getTextContent = function() {
		return this._textContent;
	}, e.prototype.setTextContent = function(e) {
		var t = this._textContent;
		t !== e && (t && t !== e && this.removeTextContent(), e.innerTransformable = new Hn(), this._attachComponent(e), this._textContent = e, this.markRedraw());
	}, e.prototype.setTextConfig = function(e) {
		this.textConfig ||= {}, j(this.textConfig, e), this.markRedraw();
	}, e.prototype.removeTextConfig = function() {
		this.textConfig = null, this.markRedraw();
	}, e.prototype.removeTextContent = function() {
		var e = this._textContent;
		e && (e.innerTransformable = null, this._detachComponent(e), this._textContent = null, this._innerTextDefaultStyle = null, this.markRedraw());
	}, e.prototype.getTextGuideLine = function() {
		return this._textGuide;
	}, e.prototype.setTextGuideLine = function(e) {
		this._textGuide && this._textGuide !== e && this.removeTextGuideLine(), this._attachComponent(e), this._textGuide = e, this.markRedraw();
	}, e.prototype.removeTextGuideLine = function() {
		var e = this._textGuide;
		e && (this._detachComponent(e), this._textGuide = null, this.markRedraw());
	}, e.prototype.markRedraw = function() {
		this.__dirty |= 1;
		var e = this.__zr;
		e && (this.__inHover ? e.refreshHover() : e.refresh()), this.__hostTarget && this.__hostTarget.markRedraw();
	}, e.prototype.dirty = function() {
		this.markRedraw();
	}, e.prototype.addSelfToZr = function(e) {
		if (this.__zr !== e) {
			this.__zr = e;
			var t = this.animators;
			if (t) for (var n = 0; n < t.length; n++) e.animation.addAnimator(t[n]);
			this._clipPath && this._clipPath.addSelfToZr(e), this._textContent && this._textContent.addSelfToZr(e), this._textGuide && this._textGuide.addSelfToZr(e);
		}
	}, e.prototype.removeSelfFromZr = function(e) {
		if (this.__zr) {
			this.__zr = null;
			var t = this.animators;
			if (t) for (var n = 0; n < t.length; n++) e.animation.removeAnimator(t[n]);
			this._clipPath && this._clipPath.removeSelfFromZr(e), this._textContent && this._textContent.removeSelfFromZr(e), this._textGuide && this._textGuide.removeSelfFromZr(e);
		}
	}, e.prototype.animate = function(e, t, n) {
		var r = new mi(e ? this[e] : this, t, n);
		return e && (r.targetName = e), this.addAnimator(r, e), r;
	}, e.prototype.addAnimator = function(e, t) {
		var n = this.__zr, r = this;
		e.during(function() {
			r.updateDuringAnimation(t);
		}).done(function() {
			var t = r.animators, n = N(t, e);
			n >= 0 && t.splice(n, 1);
		}), this.animators.push(e), n && n.animation.addAnimator(e), n && n.wakeUp();
	}, e.prototype.updateDuringAnimation = function(e) {
		this.markRedraw();
	}, e.prototype.stopAnimation = function(e, t) {
		for (var n = this.animators, r = n.length, i = [], a = 0; a < r; a++) {
			var o = n[a];
			!e || e === o.scope ? o.stop(t) : i.push(o);
		}
		return this.animators = i, this;
	}, e.prototype.animateTo = function(e, t, n) {
		ki(this, e, t, n);
	}, e.prototype.animateFrom = function(e, t, n) {
		ki(this, e, t, n, !0);
	}, e.prototype._transitionState = function(e, t, n, r) {
		for (var i = ki(this, t, n, r), a = 0; a < i.length; a++) i[a].__fromStateTransition = e;
	}, e.prototype.getBoundingRect = function() {
		return null;
	}, e.prototype.getPaintRect = function() {
		return null;
	}, e.initDefaultProps = (function() {
		var t = e.prototype;
		t.type = "element", t.name = "", t.ignore = t.silent = t.ignoreHostSilent = t.isGroup = t.draggable = t.dragging = t.ignoreClip = !1, t.__inHover = 0, t.__dirty = 1;
		function n(e, n, r, i) {
			Object.defineProperty(t, e, {
				get: function() {
					if (!this[n]) {
						var e = this[n] = [];
						a(this, e);
					}
					return this[n];
				},
				set: function(e) {
					this[r] = e[0], this[i] = e[1], this[n] = e, a(this, e);
				}
			});
			function a(e, t) {
				Object.defineProperty(t, 0, {
					get: function() {
						return e[r];
					},
					set: function(t) {
						e[r] = t;
					}
				}), Object.defineProperty(t, 1, {
					get: function() {
						return e[i];
					},
					set: function(t) {
						e[i] = t;
					}
				});
			}
		}
		Object.defineProperty && (n("position", "_legacyPos", "x", "y"), n("scale", "_legacyScale", "scaleX", "scaleY"), n("origin", "_legacyOrigin", "originX", "originY"));
	})(), e;
}();
P(Oi, hi), P(Oi, Hn);
function ki(e, t, n, r, i) {
	n ||= {};
	var a = [];
	Fi(e, "", e, t, n, r, a, i);
	var o = a.length, s = !1, c = n.done, l = n.aborted, u = function() {
		s = !0, o--, o <= 0 && (s ? c && c() : l && l());
	}, d = function() {
		o--, o <= 0 && (s ? c && c() : l && l());
	};
	o || c && c(), a.length > 0 && n.during && a[0].during(function(e, t) {
		n.during(t);
	});
	for (var f = 0; f < a.length; f++) {
		var p = a[f];
		u && p.done(u), d && p.aborted(d), n.force && p.duration(n.duration), p.start(n.easing);
	}
	return a;
}
function Ai(e, t, n) {
	for (var r = 0; r < n; r++) e[r] = t[r];
}
function ji(e) {
	return F(e[0]);
}
function Mi(e, t, n) {
	if (F(t[n])) if (F(e[n]) || (e[n] = []), le(t[n])) {
		var r = t[n].length;
		e[n].length !== r && (e[n] = new t[n].constructor(r), Ai(e[n], t[n], r));
	} else {
		var i = t[n], a = e[n], o = i.length;
		if (ji(i)) for (var s = i[0].length, c = 0; c < o; c++) a[c] ? Ai(a[c], i[c], s) : a[c] = Array.prototype.slice.call(i[c]);
		else Ai(a, i, o);
		a.length = i.length;
	}
	else e[n] = t[n];
}
function Ni(e, t) {
	return e === t || F(e) && F(t) && Pi(e, t);
}
function Pi(e, t) {
	var n = e.length;
	if (n !== t.length) return !1;
	for (var r = 0; r < n; r++) if (e[r] !== t[r]) return !1;
	return !0;
}
function Fi(e, t, n, r, i, a, o, s) {
	for (var c = R(r), l = i.duration, u = i.delay, d = i.additive, f = i.setToFinal, p = !W(a), m = e.animators, h = [], g = 0; g < c.length; g++) {
		var _ = c[g], v = r[_];
		if (v != null && n[_] != null && (p || a[_])) if (W(v) && !F(v) && !de(v)) {
			if (t) {
				s || (n[_] = v, e.updateDuringAnimation(t));
				continue;
			}
			Fi(e, _, n[_], v, i, a && a[_], o, s);
		} else h.push(_);
		else s || (n[_] = v, e.updateDuringAnimation(t), h.push(_));
	}
	var y = h.length;
	if (!d && y) for (var b = 0; b < m.length; b++) {
		var x = m[b];
		if (x.targetName === t && x.stopTracks(h)) {
			var S = N(m, x);
			m.splice(S, 1);
		}
	}
	if (i.force || (h = re(h, function(e) {
		return !Ni(r[e], n[e]);
	}), y = h.length), y > 0 || i.force && !o.length) {
		var C = void 0, w = void 0, T = void 0;
		if (s) {
			w = {}, f && (C = {});
			for (var b = 0; b < y; b++) {
				var _ = h[b];
				w[_] = n[_], f ? C[_] = r[_] : n[_] = r[_];
			}
		} else if (f) {
			T = {};
			for (var b = 0; b < y; b++) {
				var _ = h[b];
				T[_] = ei(n[_]), Mi(n, r, _);
			}
		}
		var x = new mi(n, !1, !1, d ? re(m, function(e) {
			return e.targetName === t;
		}) : null);
		x.targetName = t, i.scope && (x.scope = i.scope), f && C && x.whenWithKeys(0, C, h), T && x.whenWithKeys(0, T, h), x.whenWithKeys(l ?? 500, s ? w : r, h).delay(u || 0), e.addAnimator(x, t), o.push(x);
	}
}
function Ii(e, t, n, r) {
	return !(n && n.hoverLayer || r) || Li(e) || t && Li(t) ? 0 : 1;
}
function Li(e) {
	return e.type === "text" || e.type === "tspan";
}
function Ri(e, t, n) {
	return !t && !e.__inHover && n && n.duration > 0;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/Displayable.js
var zi = "__zr_style_" + Math.round(Math.random() * 10), Bi = {
	shadowBlur: 0,
	shadowOffsetX: 0,
	shadowOffsetY: 0,
	shadowColor: "#000",
	opacity: 1,
	blend: "source-over"
}, Vi = { style: {
	shadowBlur: !0,
	shadowOffsetX: !0,
	shadowOffsetY: !0,
	shadowColor: !0,
	opacity: !0
} };
Bi[zi] = !0;
var Hi = [
	"z",
	"z2",
	"invisible"
], Ui = ["invisible"], Wi = function(e) {
	o(t, e);
	function t(t) {
		return e.call(this, t) || this;
	}
	return t.prototype._init = function(t) {
		for (var n = R(t), r = 0; r < n.length; r++) {
			var i = n[r];
			i === "style" ? this.useStyle(t[i]) : e.prototype.attrKV.call(this, i, t[i]);
		}
		this.style || this.useStyle({});
	}, t.prototype.beforeBrush = function(e) {}, t.prototype.afterBrush = function() {}, t.prototype.innerBeforeBrush = function() {}, t.prototype.innerAfterBrush = function() {}, t.prototype.shouldBePainted = function(e, t, n, r) {
		var i = this.transform;
		if (this.ignore || this.invisible || this.style.opacity === 0 || this.culling && qi(this, e, t) || i && !i[0] && !i[3]) return !1;
		if (n && this.__clipPaths && this.__clipPaths.length) {
			for (var a = 0; a < this.__clipPaths.length; ++a) if (this.__clipPaths[a].isZeroArea()) return !1;
		}
		if (r && this.parent) for (var o = this.parent; o;) {
			if (o.ignore) return !1;
			o = o.parent;
		}
		return !0;
	}, t.prototype.contain = function(e, t) {
		return this.rectContain(e, t);
	}, t.prototype.traverse = function(e, t) {
		e.call(t, this);
	}, t.prototype.rectContain = function(e, t) {
		var n = this.transformCoordToLocal(e, t);
		return this.getBoundingRect().contain(n[0], n[1]);
	}, t.prototype.getPaintRect = function() {
		var e = this._paintRect;
		if (!this._paintRect || this.__dirty) {
			var t = this.transform, n = this.getBoundingRect(), r = this.style, i = r.shadowBlur || 0, a = r.shadowOffsetX || 0, o = r.shadowOffsetY || 0;
			e = this._paintRect ||= new Y(0, 0, 0, 0), t ? Y.applyTransform(e, n, t) : e.copy(n), (i || a || o) && (e.width += i * 2 + Math.abs(a), e.height += i * 2 + Math.abs(o), e.x = Math.min(e.x, e.x + a - i), e.y = Math.min(e.y, e.y + o - i));
			var s = this.dirtyRectTolerance;
			e.isZero() || (e.x = Math.floor(e.x - s), e.y = Math.floor(e.y - s), e.width = Math.ceil(e.width + 1 + s * 2), e.height = Math.ceil(e.height + 1 + s * 2));
		}
		return e;
	}, t.prototype.setPrevPaintRect = function(e) {
		e ? (this._prevPaintRect = this._prevPaintRect || new Y(0, 0, 0, 0), this._prevPaintRect.copy(e)) : this._prevPaintRect = null;
	}, t.prototype.getPrevPaintRect = function() {
		return this._prevPaintRect;
	}, t.prototype.animateStyle = function(e) {
		return this.animate("style", e);
	}, t.prototype.updateDuringAnimation = function(e) {
		e === "style" ? this.dirtyStyle() : this.markRedraw();
	}, t.prototype.attrKV = function(t, n) {
		t === "style" ? this.style ? this.setStyle(n) : this.useStyle(n) : e.prototype.attrKV.call(this, t, n);
	}, t.prototype.setStyle = function(e, t) {
		return typeof e == "string" ? this.style[e] = t : j(this.style, e), this.dirtyStyle(), this;
	}, t.prototype.dirtyStyle = function(e) {
		e || this.markRedraw(), this.__dirty |= 2, this._rect &&= null;
	}, t.prototype.dirty = function() {
		this.dirtyStyle();
	}, t.prototype.styleChanged = function() {
		return !!(this.__dirty & 2);
	}, t.prototype.styleUpdated = function() {
		this.__dirty &= -3;
	}, t.prototype.createStyle = function(e) {
		return Oe(Bi, e);
	}, t.prototype.useStyle = function(e) {
		e[zi] || (e = this.createStyle(e)), this.style = e, this.dirtyStyle();
	}, t.prototype._useHoverStyle = function(e) {
		this.__hoverStyle = e;
	}, t.prototype.isStyleObject = function(e) {
		return e[zi];
	}, t.prototype._innerSaveToNormal = function(t) {
		e.prototype._innerSaveToNormal.call(this, t);
		var n = this._normalState;
		t.style && !n.style && (n.style = this._mergeStyle(this.createStyle(), this.style)), this._savePrimaryToNormal(t, n, Hi);
	}, t.prototype._applyStateObj = function(t, n, r, i, a, o) {
		e.prototype._applyStateObj.call(this, t, n, r, i, a, o);
		var s = !(n && i), c = this.__inHover === 1, l;
		if (n && n.style ? a ? i ? l = n.style : (l = this._mergeStyle(this.createStyle(), r.style), this._mergeStyle(l, n.style)) : (l = this._mergeStyle(this.createStyle(), i ? this.style : r.style), this._mergeStyle(l, n.style)) : s && (l = r.style), l) if (a) {
			var u = this.style;
			if (this.style = this.createStyle(s ? {} : u), s) for (var d = R(u), f = 0; f < d.length; f++) {
				var p = d[f];
				p in l && (l[p] = l[p], this.style[p] = u[p]);
			}
			for (var m = R(l), f = 0; f < m.length; f++) {
				var p = m[f];
				this.style[p] = this.style[p];
			}
			this._transitionState(t, { style: l }, o, this.getAnimationStyleProps());
		} else c ? this._useHoverStyle(l) : this.useStyle(l);
		if (!c) for (var h = this.__inHover ? Ui : Hi, f = 0; f < h.length; f++) {
			var p = h[f];
			n && n[p] != null ? this[p] = n[p] : s && r[p] != null && (this[p] = r[p]);
		}
	}, t.prototype._mergeStates = function(t) {
		for (var n = e.prototype._mergeStates.call(this, t), r, i = 0; i < t.length; i++) {
			var a = t[i];
			a.style && (r ||= {}, this._mergeStyle(r, a.style));
		}
		return r && (n.style = r), n;
	}, t.prototype._mergeStyle = function(e, t) {
		return j(e, t), e;
	}, t.prototype.getAnimationStyleProps = function() {
		return Vi;
	}, t.initDefaultProps = (function() {
		var e = t.prototype;
		e.type = "displayable", e.invisible = !1, e.z = 0, e.z2 = 0, e.zlevel = 0, e.culling = !1, e.cursor = "pointer", e.rectHover = !1, e.incremental = 0, e._rect = null, e.dirtyRectTolerance = 0, e.__dirty = 3;
	})(), t;
}(Oi), Gi = new Y(0, 0, 0, 0), Ki = new Y(0, 0, 0, 0);
function qi(e, t, n) {
	return Gi.copy(e.getBoundingRect()), e.transform && Gi.applyTransform(e.transform), Ki.width = t, Ki.height = n, !Gi.intersect(Ki);
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/core/bbox.js
var Ji = Math.min, Yi = Math.max, Xi = Math.sin, Zi = Math.cos, Qi = Math.PI * 2, $i = mt(), ea = mt(), ta = mt();
function na(e, t, n, r, i, a) {
	i[0] = Ji(e, n), i[1] = Ji(t, r), a[0] = Yi(e, n), a[1] = Yi(t, r);
}
var ra = [], ia = [];
function aa(e, t, n, r, i, a, o, s, c, l) {
	var u = sr, d = ir, f = u(e, n, i, o, ra);
	c[0] = Infinity, c[1] = Infinity, l[0] = -Infinity, l[1] = -Infinity;
	for (var p = 0; p < f; p++) {
		var m = d(e, n, i, o, ra[p]);
		c[0] = Ji(m, c[0]), l[0] = Yi(m, l[0]);
	}
	f = u(t, r, a, s, ia);
	for (var p = 0; p < f; p++) {
		var h = d(t, r, a, s, ia[p]);
		c[1] = Ji(h, c[1]), l[1] = Yi(h, l[1]);
	}
	c[0] = Ji(e, c[0]), l[0] = Yi(e, l[0]), c[0] = Ji(o, c[0]), l[0] = Yi(o, l[0]), c[1] = Ji(t, c[1]), l[1] = Yi(t, l[1]), c[1] = Ji(s, c[1]), l[1] = Yi(s, l[1]);
}
function oa(e, t, n, r, i, a, o, s) {
	var c = mr, l = dr, u = Yi(Ji(c(e, n, i), 1), 0), d = Yi(Ji(c(t, r, a), 1), 0), f = l(e, n, i, u), p = l(t, r, a, d);
	o[0] = Ji(e, i, f), o[1] = Ji(t, a, p), s[0] = Yi(e, i, f), s[1] = Yi(t, a, p);
}
function sa(e, t, n, r, i, a, o, s, c) {
	var l = kt, u = At, d = Math.abs(i - a);
	if (d % Qi < 1e-4 && d > 1e-4) {
		s[0] = e - n, s[1] = t - r, c[0] = e + n, c[1] = t + r;
		return;
	}
	if ($i[0] = Zi(i) * n + e, $i[1] = Xi(i) * r + t, ea[0] = Zi(a) * n + e, ea[1] = Xi(a) * r + t, l(s, $i, ea), u(c, $i, ea), i %= Qi, i < 0 && (i += Qi), a %= Qi, a < 0 && (a += Qi), i > a && !o ? a += Qi : i < a && o && (i += Qi), o) {
		var f = a;
		a = i, i = f;
	}
	for (var p = 0; p < a; p += Math.PI / 2) p > i && (ta[0] = Zi(p) * n + e, ta[1] = Xi(p) * r + t, l(s, ta, s), u(c, ta, c));
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/core/PathProxy.js
var ca = {
	M: 1,
	L: 2,
	C: 3,
	Q: 4,
	A: 5,
	Z: 6,
	R: 7
}, la = [], ua = [], da = [], fa = [], pa = [], ma = [], ha = Math.min, ga = Math.max, _a = Math.cos, va = Math.sin, ya = Math.abs, ba = Math.PI, xa = ba * 2, Sa = typeof Float32Array < "u", Ca = [];
function wa(e) {
	return Math.round(e / ba * 1e8) / 1e8 % 2 * ba;
}
function Ta(e, t) {
	var n = wa(e[0]);
	n < 0 && (n += xa);
	var r = n - e[0], i = e[1];
	i += r, !t && i - n >= xa ? i = n + xa : t && n - i >= xa ? i = n - xa : !t && n > i ? i = n + (xa - wa(n - i)) : t && n < i && (i = n - (xa - wa(i - n))), e[0] = n, e[1] = i;
}
var Ea = function() {
	function e(e) {
		this.dpr = 1, this._xi = 0, this._yi = 0, this._x0 = 0, this._y0 = 0, this._len = 0, e && (this._saveData = !1), this._saveData && (this.data = []);
	}
	return e.prototype.increaseVersion = function() {
		this._version++;
	}, e.prototype.getVersion = function() {
		return this._version;
	}, e.prototype.setScale = function(e, t, n) {
		n ||= 0, n > 0 && (this._ux = ya(n / _i / e) || 0, this._uy = ya(n / _i / t) || 0);
	}, e.prototype.setDPR = function(e) {
		this.dpr = e;
	}, e.prototype.setContext = function(e) {
		this._ctx = e;
	}, e.prototype.getContext = function() {
		return this._ctx;
	}, e.prototype.beginPath = function() {
		return this._ctx && this._ctx.beginPath(), this.reset(), this;
	}, e.prototype.reset = function() {
		this._saveData && (this._len = 0), this._pathSegLen && (this._pathSegLen = null, this._pathLen = 0), this._version++;
	}, e.prototype.moveTo = function(e, t) {
		return this._drawPendingPt(), this.addData(ca.M, e, t), this._ctx && this._ctx.moveTo(e, t), this._x0 = e, this._y0 = t, this._xi = e, this._yi = t, this;
	}, e.prototype.lineTo = function(e, t) {
		var n = ya(e - this._xi), r = ya(t - this._yi), i = n > this._ux || r > this._uy;
		if (this.addData(ca.L, e, t), this._ctx && i && this._ctx.lineTo(e, t), i) this._xi = e, this._yi = t, this._pendingPtDist = 0;
		else {
			var a = n * n + r * r;
			a > this._pendingPtDist && (this._pendingPtX = e, this._pendingPtY = t, this._pendingPtDist = a);
		}
		return this;
	}, e.prototype.bezierCurveTo = function(e, t, n, r, i, a) {
		return this._drawPendingPt(), this.addData(ca.C, e, t, n, r, i, a), this._ctx && this._ctx.bezierCurveTo(e, t, n, r, i, a), this._xi = i, this._yi = a, this;
	}, e.prototype.quadraticCurveTo = function(e, t, n, r) {
		return this._drawPendingPt(), this.addData(ca.Q, e, t, n, r), this._ctx && this._ctx.quadraticCurveTo(e, t, n, r), this._xi = n, this._yi = r, this;
	}, e.prototype.arc = function(e, t, n, r, i, a) {
		this._drawPendingPt(), Ca[0] = r, Ca[1] = i, Ta(Ca, a), r = Ca[0], i = Ca[1];
		var o = i - r;
		return this.addData(ca.A, e, t, n, n, r, o, 0, +!a), this._ctx && this._ctx.arc(e, t, n, r, i, a), this._xi = _a(i) * n + e, this._yi = va(i) * n + t, this;
	}, e.prototype.arcTo = function(e, t, n, r, i) {
		return this._drawPendingPt(), this._ctx && this._ctx.arcTo(e, t, n, r, i), this;
	}, e.prototype.rect = function(e, t, n, r) {
		return this._drawPendingPt(), this._ctx && this._ctx.rect(e, t, n, r), this.addData(ca.R, e, t, n, r), this;
	}, e.prototype.closePath = function() {
		this._drawPendingPt(), this.addData(ca.Z);
		var e = this._ctx, t = this._x0, n = this._y0;
		return e && e.closePath(), this._xi = t, this._yi = n, this;
	}, e.prototype.fill = function(e) {
		e && e.fill(), this.toStatic();
	}, e.prototype.stroke = function(e) {
		e && e.stroke(), this.toStatic();
	}, e.prototype.len = function() {
		return this._len;
	}, e.prototype.setData = function(e) {
		if (this._saveData) {
			var t = e.length;
			!(this.data && this.data.length === t) && Sa && (this.data = new Float32Array(t));
			for (var n = 0; n < t; n++) this.data[n] = e[n];
			this._len = t;
		}
	}, e.prototype.appendPath = function(e) {
		if (this._saveData) {
			e instanceof Array || (e = [e]);
			for (var t = e.length, n = 0, r = this._len, i = 0; i < t; i++) n += e[i].len();
			var a = this.data;
			if (Sa && (a instanceof Float32Array || !a) && (this.data = new Float32Array(r + n), r > 0 && a)) for (var o = 0; o < r; o++) this.data[o] = a[o];
			for (var i = 0; i < t; i++) for (var s = e[i].data, o = 0; o < s.length; o++) this.data[r++] = s[o];
			this._len = r;
		}
	}, e.prototype.addData = function(e, t, n, r, i, a, o, s, c) {
		if (this._saveData) {
			var l = this.data;
			this._len + arguments.length > l.length && (this._expandData(), l = this.data);
			for (var u = 0; u < arguments.length; u++) l[this._len++] = arguments[u];
		}
	}, e.prototype._drawPendingPt = function() {
		this._pendingPtDist > 0 && (this._ctx && this._ctx.lineTo(this._pendingPtX, this._pendingPtY), this._pendingPtDist = 0);
	}, e.prototype._expandData = function() {
		if (!(this.data instanceof Array)) {
			for (var e = [], t = 0; t < this._len; t++) e[t] = this.data[t];
			this.data = e;
		}
	}, e.prototype.toStatic = function() {
		if (this._saveData) {
			this._drawPendingPt();
			var e = this.data;
			e instanceof Array && (e.length = this._len, Sa && this._len > 11 && (this.data = new Float32Array(e)));
		}
	}, e.prototype.getBoundingRect = function() {
		da[0] = da[1] = pa[0] = pa[1] = Number.MAX_VALUE, fa[0] = fa[1] = ma[0] = ma[1] = -Number.MAX_VALUE;
		var e = this.data, t = 0, n = 0, r = 0, i = 0, a;
		for (a = 0; a < this._len;) {
			var o = e[a++], s = a === 1;
			switch (s && (t = e[a], n = e[a + 1], r = t, i = n), o) {
				case ca.M:
					t = r = e[a++], n = i = e[a++], pa[0] = r, pa[1] = i, ma[0] = r, ma[1] = i;
					break;
				case ca.L:
					na(t, n, e[a], e[a + 1], pa, ma), t = e[a++], n = e[a++];
					break;
				case ca.C:
					aa(t, n, e[a++], e[a++], e[a++], e[a++], e[a], e[a + 1], pa, ma), t = e[a++], n = e[a++];
					break;
				case ca.Q:
					oa(t, n, e[a++], e[a++], e[a], e[a + 1], pa, ma), t = e[a++], n = e[a++];
					break;
				case ca.A:
					var c = e[a++], l = e[a++], u = e[a++], d = e[a++], f = e[a++], p = e[a++] + f;
					a += 1;
					var m = !e[a++];
					s && (r = _a(f) * u + c, i = va(f) * d + l), sa(c, l, u, d, f, p, m, pa, ma), t = _a(p) * u + c, n = va(p) * d + l;
					break;
				case ca.R:
					r = t = e[a++], i = n = e[a++];
					var h = e[a++], g = e[a++];
					na(r, i, r + h, i + g, pa, ma);
					break;
				case ca.Z:
					t = r, n = i;
					break;
			}
			kt(da, da, pa), At(fa, fa, ma);
		}
		return a === 0 && (da[0] = da[1] = fa[0] = fa[1] = 0), new Y(da[0], da[1], fa[0] - da[0], fa[1] - da[1]);
	}, e.prototype._calculateLength = function() {
		var e = this.data, t = this._len, n = this._ux, r = this._uy, i = 0, a = 0, o = 0, s = 0;
		this._pathSegLen ||= [];
		for (var c = this._pathSegLen, l = 0, u = 0, d = 0; d < t;) {
			var f = e[d++], p = d === 1;
			p && (i = e[d], a = e[d + 1], o = i, s = a);
			var m = -1;
			switch (f) {
				case ca.M:
					i = o = e[d++], a = s = e[d++];
					break;
				case ca.L:
					var h = e[d++], g = e[d++], _ = h - i, v = g - a;
					(ya(_) > n || ya(v) > r || d === t - 1) && (m = Math.sqrt(_ * _ + v * v), i = h, a = g);
					break;
				case ca.C:
					var y = e[d++], b = e[d++], h = e[d++], g = e[d++], x = e[d++], S = e[d++];
					m = ur(i, a, y, b, h, g, x, S, 10), i = x, a = S;
					break;
				case ca.Q:
					var y = e[d++], b = e[d++], h = e[d++], g = e[d++];
					m = _r(i, a, y, b, h, g, 10), i = h, a = g;
					break;
				case ca.A:
					var C = e[d++], w = e[d++], T = e[d++], E = e[d++], D = e[d++], O = e[d++], k = O + D;
					d += 1, p && (o = _a(D) * T + C, s = va(D) * E + w), m = ga(T, E) * ha(xa, Math.abs(O)), i = _a(k) * T + C, a = va(k) * E + w;
					break;
				case ca.R:
					o = i = e[d++], s = a = e[d++];
					var A = e[d++], j = e[d++];
					m = A * 2 + j * 2;
					break;
				case ca.Z:
					var _ = o - i, v = s - a;
					m = Math.sqrt(_ * _ + v * v), i = o, a = s;
					break;
			}
			m >= 0 && (c[u++] = m, l += m);
		}
		return this._pathLen = l, l;
	}, e.prototype.rebuildPath = function(e, t) {
		var n = this.data, r = this._ux, i = this._uy, a = this._len, o, s, c, l, u, d, f = t < 1, p, m, h = 0, g = 0, _, v = 0, y, b;
		if (!(f && (this._pathSegLen || this._calculateLength(), p = this._pathSegLen, m = this._pathLen, _ = t * m, !_))) lo: for (var x = 0; x < a;) {
			var S = n[x++], C = x === 1;
			switch (C && (c = n[x], l = n[x + 1], o = c, s = l), S !== ca.L && v > 0 && (e.lineTo(y, b), v = 0), S) {
				case ca.M:
					o = c = n[x++], s = l = n[x++], e.moveTo(c, l);
					break;
				case ca.L:
					u = n[x++], d = n[x++];
					var w = ya(u - c), T = ya(d - l);
					if (w > r || T > i) {
						if (f) {
							var E = p[g++];
							if (h + E > _) {
								var D = (_ - h) / E;
								e.lineTo(c * (1 - D) + u * D, l * (1 - D) + d * D);
								break lo;
							}
							h += E;
						}
						e.lineTo(u, d), c = u, l = d, v = 0;
					} else {
						var O = w * w + T * T;
						O > v && (y = u, b = d, v = O);
					}
					break;
				case ca.C:
					var k = n[x++], A = n[x++], j = n[x++], ee = n[x++], M = n[x++], N = n[x++];
					if (f) {
						var E = p[g++];
						if (h + E > _) {
							var D = (_ - h) / E;
							cr(c, k, j, M, D, la), cr(l, A, ee, N, D, ua), e.bezierCurveTo(la[1], ua[1], la[2], ua[2], la[3], ua[3]);
							break lo;
						}
						h += E;
					}
					e.bezierCurveTo(k, A, j, ee, M, N), c = M, l = N;
					break;
				case ca.Q:
					var k = n[x++], A = n[x++], j = n[x++], ee = n[x++];
					if (f) {
						var E = p[g++];
						if (h + E > _) {
							var D = (_ - h) / E;
							hr(c, k, j, D, la), hr(l, A, ee, D, ua), e.quadraticCurveTo(la[1], ua[1], la[2], ua[2]);
							break lo;
						}
						h += E;
					}
					e.quadraticCurveTo(k, A, j, ee), c = j, l = ee;
					break;
				case ca.A:
					var te = n[x++], P = n[x++], F = n[x++], I = n[x++], L = n[x++], ne = n[x++], re = n[x++], ie = !n[x++], R = F > I ? F : I, ae = ya(F - I) > .001, z = L + ne, B = !1;
					if (f) {
						var E = p[g++];
						h + E > _ && (z = L + ne * (_ - h) / E, B = !0), h += E;
					}
					if (ae && e.ellipse ? e.ellipse(te, P, F, I, re, L, z, ie) : e.arc(te, P, R, L, z, ie), B) break lo;
					C && (o = _a(L) * F + te, s = va(L) * I + P), c = _a(z) * F + te, l = va(z) * I + P;
					break;
				case ca.R:
					o = c = n[x], s = l = n[x + 1], u = n[x++], d = n[x++];
					var V = n[x++], H = n[x++];
					if (f) {
						var E = p[g++];
						if (h + E > _) {
							var U = _ - h;
							e.moveTo(u, d), e.lineTo(u + ha(U, V), d), U -= V, U > 0 && e.lineTo(u + V, d + ha(U, H)), U -= H, U > 0 && e.lineTo(u + ga(V - U, 0), d + H), U -= V, U > 0 && e.lineTo(u, d + ga(H - U, 0));
							break lo;
						}
						h += E;
					}
					e.rect(u, d, V, H);
					break;
				case ca.Z:
					if (f) {
						var E = p[g++];
						if (h + E > _) {
							var D = (_ - h) / E;
							e.lineTo(c * (1 - D) + o * D, l * (1 - D) + s * D);
							break lo;
						}
						h += E;
					}
					e.closePath(), c = o, l = s;
			}
		}
	}, e.prototype.clone = function() {
		var t = new e(), n = this.data;
		return t.data = n.slice ? n.slice() : Array.prototype.slice.call(n), t._len = this._len, t;
	}, e.prototype.canSave = function() {
		return !!this._saveData;
	}, e.CMD = ca, e.initDefaultProps = (function() {
		var t = e.prototype;
		t._saveData = !0, t._ux = 0, t._uy = 0, t._pendingPtDist = 0, t._version = 0;
	})(), e;
}();
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/contain/line.js
function Da(e, t, n, r, i, a, o) {
	if (i === 0) return !1;
	var s = i, c = 0, l = e;
	if (o > t + s && o > r + s || o < t - s && o < r - s || a > e + s && a > n + s || a < e - s && a < n - s) return !1;
	if (e !== n) c = (t - r) / (e - n), l = (e * r - n * t) / (e - n);
	else return Math.abs(a - e) <= s / 2;
	var u = c * a - o + l;
	return u * u / (c * c + 1) <= s / 2 * s / 2;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/contain/cubic.js
function Oa(e, t, n, r, i, a, o, s, c, l, u) {
	if (c === 0) return !1;
	var d = c;
	return u > t + d && u > r + d && u > a + d && u > s + d || u < t - d && u < r - d && u < a - d && u < s - d || l > e + d && l > n + d && l > i + d && l > o + d || l < e - d && l < n - d && l < i - d && l < o - d ? !1 : lr(e, t, n, r, i, a, o, s, l, u, null) <= d / 2;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/contain/quadratic.js
function ka(e, t, n, r, i, a, o, s, c) {
	if (o === 0) return !1;
	var l = o;
	return c > t + l && c > r + l && c > a + l || c < t - l && c < r - l && c < a - l || s > e + l && s > n + l && s > i + l || s < e - l && s < n - l && s < i - l ? !1 : gr(e, t, n, r, i, a, s, c, null) <= l / 2;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/contain/util.js
var Aa = Math.PI * 2;
function ja(e) {
	return e %= Aa, e < 0 && (e += Aa), e;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/contain/arc.js
var Ma = Math.PI * 2;
function Na(e, t, n, r, i, a, o, s, c) {
	if (o === 0) return !1;
	var l = o;
	s -= e, c -= t;
	var u = Math.sqrt(s * s + c * c);
	if (u - l > n || u + l < n) return !1;
	if (Math.abs(r - i) % Ma < 1e-4) return !0;
	if (a) {
		var d = r;
		r = ja(i), i = ja(d);
	} else r = ja(r), i = ja(i);
	r > i && (i += Ma);
	var f = Math.atan2(c, s);
	return f < 0 && (f += Ma), f >= r && f <= i || f + Ma >= r && f + Ma <= i;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/contain/windingLine.js
function Pa(e, t, n, r, i, a) {
	if (a > t && a > r || a < t && a < r || r === t) return 0;
	var o = (a - t) / (r - t), s = r < t ? 1 : -1;
	(o === 1 || o === 0) && (s = r < t ? .5 : -.5);
	var c = o * (n - e) + e;
	return c === i ? Infinity : c > i ? s : 0;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/contain/path.js
var Fa = Ea.CMD, Ia = Math.PI * 2, La = 1e-4;
function Ra(e, t) {
	return Math.abs(e - t) < La;
}
var za = [
	-1,
	-1,
	-1
], Ba = [-1, -1];
function Va() {
	var e = Ba[0];
	Ba[0] = Ba[1], Ba[1] = e;
}
function Ha(e, t, n, r, i, a, o, s, c, l) {
	if (l > t && l > r && l > a && l > s || l < t && l < r && l < a && l < s) return 0;
	var u = or(t, r, a, s, l, za);
	if (u === 0) return 0;
	for (var d = 0, f = -1, p = void 0, m = void 0, h = 0; h < u; h++) {
		var g = za[h], _ = g === 0 || g === 1 ? .5 : 1;
		ir(e, n, i, o, g) < c || (f < 0 && (f = sr(t, r, a, s, Ba), Ba[1] < Ba[0] && f > 1 && Va(), p = ir(t, r, a, s, Ba[0]), f > 1 && (m = ir(t, r, a, s, Ba[1]))), f === 2 ? g < Ba[0] ? d += p < t ? _ : -_ : g < Ba[1] ? d += m < p ? _ : -_ : d += s < m ? _ : -_ : g < Ba[0] ? d += p < t ? _ : -_ : d += s < p ? _ : -_);
	}
	return d;
}
function Ua(e, t, n, r, i, a, o, s) {
	if (s > t && s > r && s > a || s < t && s < r && s < a) return 0;
	var c = pr(t, r, a, s, za);
	if (c === 0) return 0;
	var l = mr(t, r, a);
	if (l >= 0 && l <= 1) {
		for (var u = 0, d = dr(t, r, a, l), f = 0; f < c; f++) {
			var p = za[f] === 0 || za[f] === 1 ? .5 : 1, m = dr(e, n, i, za[f]);
			m < o || (za[f] < l ? u += d < t ? p : -p : u += a < d ? p : -p);
		}
		return u;
	} else {
		var p = za[0] === 0 || za[0] === 1 ? .5 : 1, m = dr(e, n, i, za[0]);
		return m < o ? 0 : a < t ? p : -p;
	}
}
function Wa(e, t, n, r, i, a, o, s) {
	if (s -= t, s > n || s < -n) return 0;
	var c = Math.sqrt(n * n - s * s);
	za[0] = -c, za[1] = c;
	var l = Math.abs(r - i);
	if (l < 1e-4) return 0;
	if (l >= Ia - 1e-4) {
		r = 0, i = Ia;
		var u = a ? 1 : -1;
		return o >= za[0] + e && o <= za[1] + e ? u : 0;
	}
	if (r > i) {
		var d = r;
		r = i, i = d;
	}
	r < 0 && (r += Ia, i += Ia);
	for (var f = 0, p = 0; p < 2; p++) {
		var m = za[p];
		if (m + e > o) {
			var h = Math.atan2(s, m), u = a ? 1 : -1;
			h < 0 && (h = Ia + h), (h >= r && h <= i || h + Ia >= r && h + Ia <= i) && (h > Math.PI / 2 && h < Math.PI * 1.5 && (u = -u), f += u);
		}
	}
	return f;
}
function Ga(e, t, n, r, i) {
	for (var a = e.data, o = e.len(), s = 0, c = 0, l = 0, u = 0, d = 0, f, p, m = 0; m < o;) {
		var h = a[m++], g = m === 1;
		switch (h === Fa.M && m > 1 && (n || (s += Pa(c, l, u, d, r, i))), g && (c = a[m], l = a[m + 1], u = c, d = l), h) {
			case Fa.M:
				u = a[m++], d = a[m++], c = u, l = d;
				break;
			case Fa.L:
				if (n) {
					if (Da(c, l, a[m], a[m + 1], t, r, i)) return !0;
				} else s += Pa(c, l, a[m], a[m + 1], r, i) || 0;
				c = a[m++], l = a[m++];
				break;
			case Fa.C:
				if (n) {
					if (Oa(c, l, a[m++], a[m++], a[m++], a[m++], a[m], a[m + 1], t, r, i)) return !0;
				} else s += Ha(c, l, a[m++], a[m++], a[m++], a[m++], a[m], a[m + 1], r, i) || 0;
				c = a[m++], l = a[m++];
				break;
			case Fa.Q:
				if (n) {
					if (ka(c, l, a[m++], a[m++], a[m], a[m + 1], t, r, i)) return !0;
				} else s += Ua(c, l, a[m++], a[m++], a[m], a[m + 1], r, i) || 0;
				c = a[m++], l = a[m++];
				break;
			case Fa.A:
				var _ = a[m++], v = a[m++], y = a[m++], b = a[m++], x = a[m++], S = a[m++];
				m += 1;
				var C = !!(1 - a[m++]);
				f = Math.cos(x) * y + _, p = Math.sin(x) * b + v, g ? (u = f, d = p) : s += Pa(c, l, f, p, r, i);
				var w = (r - _) * b / y + _;
				if (n) {
					if (Na(_, v, b, x, x + S, C, t, w, i)) return !0;
				} else s += Wa(_, v, b, x, x + S, C, w, i);
				c = Math.cos(x + S) * y + _, l = Math.sin(x + S) * b + v;
				break;
			case Fa.R:
				u = c = a[m++], d = l = a[m++];
				var T = a[m++], E = a[m++];
				if (f = u + T, p = d + E, n) {
					if (Da(u, d, f, d, t, r, i) || Da(f, d, f, p, t, r, i) || Da(f, p, u, p, t, r, i) || Da(u, p, u, d, t, r, i)) return !0;
				} else s += Pa(f, d, f, p, r, i), s += Pa(u, p, u, d, r, i);
				break;
			case Fa.Z:
				if (n) {
					if (Da(c, l, u, d, t, r, i)) return !0;
				} else s += Pa(c, l, u, d, r, i);
				c = u, l = d;
				break;
		}
	}
	return !n && !Ra(l, d) && (s += Pa(c, l, u, d, r, i) || 0), s !== 0;
}
function Ka(e, t, n) {
	return Ga(e, 0, !1, t, n);
}
function qa(e, t, n, r) {
	return Ga(e, t, !0, n, r);
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/Path.js
var Ja = M({
	fill: "#000",
	stroke: null,
	strokePercent: 1,
	fillOpacity: 1,
	strokeOpacity: 1,
	lineDashOffset: 0,
	lineWidth: 1,
	lineCap: "butt",
	miterLimit: 10,
	strokeNoScale: !1,
	strokeFirst: !1
}, Bi), Ya = { style: M({
	fill: !0,
	stroke: !0,
	strokePercent: !0,
	fillOpacity: !0,
	strokeOpacity: !0,
	lineDashOffset: !0,
	lineWidth: !0,
	miterLimit: !0
}, Vi.style) }, Xa = Wn.concat([
	"invisible",
	"culling",
	"z",
	"z2",
	"zlevel",
	"parent"
]), Za = function(e) {
	o(t, e);
	function t(t) {
		return e.call(this, t) || this;
	}
	return t.prototype.update = function() {
		var n = this;
		e.prototype.update.call(this);
		var r = this.style;
		if (r.decal) {
			var i = this._decalEl = this._decalEl || new t();
			i.buildPath === t.prototype.buildPath && (i.buildPath = function(e) {
				n.buildPath(e, n.shape);
			}), i.silent = !0;
			var a = i.style;
			for (var o in r) a[o] !== r[o] && (a[o] = r[o]);
			a.fill = r.fill ? r.decal : null, a.decal = null, a.shadowColor = null, r.strokeFirst && (a.stroke = null);
			for (var s = 0; s < Xa.length; ++s) i[Xa[s]] = this[Xa[s]];
			i.__dirty |= 1;
		} else this._decalEl &&= null;
	}, t.prototype.getDecalElement = function() {
		return this._decalEl;
	}, t.prototype._init = function(t) {
		var n = R(t);
		this.shape = this.getDefaultShape();
		var r = this.getDefaultStyle();
		r && this.useStyle(r);
		for (var i = 0; i < n.length; i++) {
			var a = n[i], o = t[a];
			a === "style" ? this.style ? j(this.style, o) : this.useStyle(o) : a === "shape" ? j(this.shape, o) : e.prototype.attrKV.call(this, a, o);
		}
		this.style || this.useStyle({});
	}, t.prototype.getDefaultStyle = function() {
		return null;
	}, t.prototype.getDefaultShape = function() {
		return {};
	}, t.prototype.canBeInsideText = function() {
		return this.hasFill();
	}, t.prototype.getInsideTextFill = function() {
		var e = this.style.fill;
		if (e !== "none") {
			if (U(e)) {
				var t = Vr(e, 0);
				return t > .5 ? yi : t > .2 ? xi : bi;
			} else if (e) return bi;
		}
		return yi;
	}, t.prototype.getInsideTextStroke = function(e) {
		var t = this.style.fill;
		if (U(t)) {
			var n = this.__zr;
			if (!!(n && n.isDarkMode()) == Vr(e, 0) < .4) return t;
		}
	}, t.prototype.buildPath = function(e, t, n) {}, t.prototype.pathUpdated = function() {
		this.__dirty &= -5;
	}, t.prototype.getUpdatedPathProxy = function(e) {
		return !this.path && this.createPathProxy(), this.path.beginPath(), this.buildPath(this.path, this.shape, e), this.path;
	}, t.prototype.createPathProxy = function() {
		this.path = new Ea(!1);
	}, t.prototype.hasStroke = function() {
		var e = this.style, t = e.stroke;
		return !(t == null || t === "none" || !(e.lineWidth > 0));
	}, t.prototype.hasFill = function() {
		var e = this.style.fill;
		return e != null && e !== "none";
	}, t.prototype.getBoundingRect = function() {
		var e = this._rect, t = this.style, n = !e;
		if (n) {
			var r = !1;
			this.path || (r = !0, this.createPathProxy());
			var i = this.path;
			(r || this.__dirty & 4) && (i.beginPath(), this.buildPath(i, this.shape, !1), this.pathUpdated()), e = i.getBoundingRect();
		}
		if (this._rect = e, this.hasStroke() && this.path && this.path.len() > 0) {
			var a = this._rectStroke ||= e.clone();
			if (this.__dirty || n) {
				a.copy(e);
				var o = t.strokeNoScale ? this.getLineScale() : 1, s = t.lineWidth;
				if (!this.hasFill()) {
					var c = this.strokeContainThreshold;
					s = Math.max(s, c ?? 4);
				}
				o > 1e-10 && (a.width += s / o, a.height += s / o, a.x -= s / o / 2, a.y -= s / o / 2);
			}
			return a;
		}
		return e;
	}, t.prototype.contain = function(e, t) {
		var n = this.transformCoordToLocal(e, t), r = this.getBoundingRect(), i = this.style;
		if (e = n[0], t = n[1], r.contain(e, t)) {
			var a = this.path;
			if (this.hasStroke()) {
				var o = i.lineWidth, s = i.strokeNoScale ? this.getLineScale() : 1;
				if (s > 1e-10 && (this.hasFill() || (o = Math.max(o, this.strokeContainThreshold)), qa(a, o / s, e, t))) return !0;
			}
			if (this.hasFill()) return Ka(a, e, t);
		}
		return !1;
	}, t.prototype.dirtyShape = function() {
		this.__dirty |= 4, this._rect &&= null, this._decalEl && this._decalEl.dirtyShape(), this.markRedraw();
	}, t.prototype.dirty = function() {
		this.dirtyStyle(), this.dirtyShape();
	}, t.prototype.animateShape = function(e) {
		return this.animate("shape", e);
	}, t.prototype.updateDuringAnimation = function(e) {
		e === "style" ? this.dirtyStyle() : e === "shape" ? this.dirtyShape() : this.markRedraw();
	}, t.prototype.attrKV = function(t, n) {
		t === "shape" ? this.setShape(n) : e.prototype.attrKV.call(this, t, n);
	}, t.prototype.setShape = function(e, t) {
		var n = this.shape;
		return n ||= this.shape = {}, typeof e == "string" ? n[e] = t : j(n, e), this.dirtyShape(), this;
	}, t.prototype.shapeChanged = function() {
		return !!(this.__dirty & 4);
	}, t.prototype.createStyle = function(e) {
		return Oe(Ja, e);
	}, t.prototype._innerSaveToNormal = function(t) {
		e.prototype._innerSaveToNormal.call(this, t);
		var n = this._normalState;
		t.shape && !n.shape && (n.shape = j({}, this.shape));
	}, t.prototype._applyStateObj = function(t, n, r, i, a, o) {
		if (e.prototype._applyStateObj.call(this, t, n, r, i, a, o), this.__inHover !== 1) {
			var s = !(n && i), c;
			if (n && n.shape ? a ? i ? c = n.shape : (c = j({}, r.shape), j(c, n.shape)) : (c = j({}, i ? this.shape : r.shape), j(c, n.shape)) : s && (c = r.shape), c) if (a) {
				this.shape = j({}, this.shape);
				for (var l = {}, u = R(c), d = 0; d < u.length; d++) {
					var f = u[d];
					typeof c[f] == "object" ? this.shape[f] = c[f] : l[f] = c[f];
				}
				this._transitionState(t, { shape: l }, o);
			} else this.shape = c, this.dirtyShape();
		}
	}, t.prototype._mergeStates = function(t) {
		for (var n = e.prototype._mergeStates.call(this, t), r, i = 0; i < t.length; i++) {
			var a = t[i];
			a.shape && (r ||= {}, this._mergeStyle(r, a.shape));
		}
		return r && (n.shape = r), n;
	}, t.prototype.getAnimationStyleProps = function() {
		return Ya;
	}, t.prototype.isZeroArea = function() {
		return !1;
	}, t.extend = function(e) {
		var n = function(t) {
			o(n, t);
			function n(n) {
				var r = t.call(this, n) || this;
				return e.init && e.init.call(r, n), r;
			}
			return n.prototype.getDefaultStyle = function() {
				return k(e.style);
			}, n.prototype.getDefaultShape = function() {
				return k(e.shape);
			}, n;
		}(t);
		for (var r in e) typeof e[r] == "function" && (n.prototype[r] = e[r]);
		return n;
	}, t.initDefaultProps = (function() {
		var e = t.prototype;
		e.type = "path", e.strokeContainThreshold = 5, e.segmentIgnoreThreshold = 0, e.subPixelOptimize = !1, e.autoBatch = !1, e.__dirty = 7;
	})(), t;
}(Wi), Qa = M({
	strokeFirst: !0,
	font: s,
	x: 0,
	y: 0,
	textAlign: "left",
	textBaseline: "top",
	miterLimit: 2
}, Ja), $a = function(e) {
	o(t, e);
	function t() {
		return e !== null && e.apply(this, arguments) || this;
	}
	return t.prototype.hasStroke = function() {
		return Pn(this.style);
	}, t.prototype.hasFill = function() {
		var e = this.style.fill;
		return e != null && e !== "none";
	}, t.prototype.createStyle = function(e) {
		return Oe(Qa, e);
	}, t.prototype.setBoundingRect = function(e) {
		this._rect = e;
	}, t.prototype.getBoundingRect = function() {
		return this._rect ||= Mn(this.style), this._rect;
	}, t.initDefaultProps = (function() {
		var e = t.prototype;
		e.dirtyRectTolerance = 10;
	})(), t;
}(Wi);
$a.prototype.type = "tspan";
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/Image.js
var eo = M({
	x: 0,
	y: 0
}, Bi), to = { style: M({
	x: !0,
	y: !0,
	width: !0,
	height: !0,
	sx: !0,
	sy: !0,
	sWidth: !0,
	sHeight: !0
}, Vi.style) };
function no(e) {
	return !!(e && typeof e != "string" && e.width && e.height);
}
var ro = function(e) {
	o(t, e);
	function t() {
		return e !== null && e.apply(this, arguments) || this;
	}
	return t.prototype.createStyle = function(e) {
		return Oe(eo, e);
	}, t.prototype._getSize = function(e) {
		var t = this.style, n = t[e];
		if (n != null) return n;
		var r = no(t.image) ? t.image : this.__image;
		if (!r) return 0;
		var i = e === "width" ? "height" : "width", a = t[i];
		return a == null ? r[e] : r[e] / r[i] * a;
	}, t.prototype.getWidth = function() {
		return this._getSize("width");
	}, t.prototype.getHeight = function() {
		return this._getSize("height");
	}, t.prototype.getAnimationStyleProps = function() {
		return to;
	}, t.prototype.getBoundingRect = function() {
		var e = this.style;
		return this._rect ||= new Y(e.x || 0, e.y || 0, this.getWidth(), this.getHeight()), this._rect;
	}, t;
}(Wi);
ro.prototype.type = "image";
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/helper/roundRect.js
function io(e, t) {
	var n = t.x, r = t.y, i = t.width, a = t.height, o = t.r, s, c, l, u;
	i < 0 && (n += i, i = -i), a < 0 && (r += a, a = -a), typeof o == "number" ? s = c = l = u = o : o instanceof Array ? o.length === 1 ? s = c = l = u = o[0] : o.length === 2 ? (s = l = o[0], c = u = o[1]) : o.length === 3 ? (s = o[0], c = u = o[1], l = o[2]) : (s = o[0], c = o[1], l = o[2], u = o[3]) : s = c = l = u = 0;
	var d;
	s + c > i && (d = s + c, s *= i / d, c *= i / d), l + u > i && (d = l + u, l *= i / d, u *= i / d), c + l > a && (d = c + l, c *= a / d, l *= a / d), s + u > a && (d = s + u, s *= a / d, u *= a / d), e.moveTo(n + s, r), e.lineTo(n + i - c, r), c !== 0 && e.arc(n + i - c, r + c, c, -Math.PI / 2, 0), e.lineTo(n + i, r + a - l), l !== 0 && e.arc(n + i - l, r + a - l, l, 0, Math.PI / 2), e.lineTo(n + u, r + a), u !== 0 && e.arc(n + u, r + a - u, u, Math.PI / 2, Math.PI), e.lineTo(n, r + s), s !== 0 && e.arc(n + s, r + s, s, Math.PI, Math.PI * 1.5), e.closePath();
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/helper/subPixelOptimize.js
var ao = Math.round;
function oo(e, t, n) {
	if (t) {
		var r = t.x1, i = t.x2, a = t.y1, o = t.y2;
		e.x1 = r, e.x2 = i, e.y1 = a, e.y2 = o;
		var s = n && n.lineWidth;
		return s ? (ao(r * 2) === ao(i * 2) && (e.x1 = e.x2 = co(r, s, !0)), ao(a * 2) === ao(o * 2) && (e.y1 = e.y2 = co(a, s, !0)), e) : e;
	}
}
function so(e, t, n) {
	if (t) {
		var r = t.x, i = t.y, a = t.width, o = t.height;
		e.x = r, e.y = i, e.width = a, e.height = o;
		var s = n && n.lineWidth;
		return s ? (e.x = co(r, s, !0), e.y = co(i, s, !0), e.width = Math.max(co(r + a, s, !1) - e.x, a === 0 ? 0 : 1), e.height = Math.max(co(i + o, s, !1) - e.y, o === 0 ? 0 : 1), e) : e;
	}
}
function co(e, t, n) {
	if (!t) return e;
	var r = ao(e * 2);
	return (r + ao(t)) % 2 == 0 ? r / 2 : (r + (n ? 1 : -1)) / 2;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/shape/Rect.js
var lo = function() {
	function e() {
		this.x = 0, this.y = 0, this.width = 0, this.height = 0;
	}
	return e;
}(), uo = {}, fo = function(e) {
	o(t, e);
	function t(t) {
		return e.call(this, t) || this;
	}
	return t.prototype.getDefaultShape = function() {
		return new lo();
	}, t.prototype.buildPath = function(e, t) {
		var n, r, i, a;
		if (this.subPixelOptimize) {
			var o = so(uo, t, this.style);
			n = o.x, r = o.y, i = o.width, a = o.height, o.r = t.r, t = o;
		} else n = t.x, r = t.y, i = t.width, a = t.height;
		t.r ? io(e, t) : e.rect(n, r, i, a);
	}, t.prototype.isZeroArea = function() {
		return !this.shape.width || !this.shape.height;
	}, t;
}(Za);
fo.prototype.type = "rect";
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/Text.js
var po = { fill: "#000" }, mo = 2, ho = {}, go = { style: M({
	fill: !0,
	stroke: !0,
	fillOpacity: !0,
	strokeOpacity: !0,
	lineWidth: !0,
	fontSize: !0,
	lineHeight: !0,
	width: !0,
	height: !0,
	textShadowColor: !0,
	textShadowBlur: !0,
	textShadowOffsetX: !0,
	textShadowOffsetY: !0,
	backgroundColor: !0,
	padding: !0,
	borderColor: !0,
	borderWidth: !0,
	borderRadius: !0
}, Vi.style) }, _o = function(e) {
	o(t, e);
	function t(t) {
		var n = e.call(this) || this;
		return n.type = "text", n._children = [], n._defaultStyle = po, n.attr(t), n;
	}
	return t.prototype.childrenRef = function() {
		return this._children;
	}, t.prototype.update = function() {
		e.prototype.update.call(this), this.styleChanged() && this._updateSubTexts();
		for (var t = 0; t < this._children.length; t++) {
			var n = this._children[t];
			n.zlevel = this.zlevel, n.z = this.z, n.z2 = this.z2, n.culling = this.culling, n.cursor = this.cursor, n.invisible = this.invisible;
		}
	}, t.prototype.updateTransform = function() {
		var t = this.innerTransformable;
		t ? (t.updateTransform(), t.transform && (this.transform = t.transform)) : e.prototype.updateTransform.call(this);
	}, t.prototype.getLocalTransform = function(t) {
		var n = this.innerTransformable;
		return n ? n.getLocalTransform(t) : e.prototype.getLocalTransform.call(this, t);
	}, t.prototype.getComputedTransform = function() {
		return this.__hostTarget && (this.__hostTarget.getComputedTransform(), this.__hostTarget.updateInnerText(!0)), e.prototype.getComputedTransform.call(this);
	}, t.prototype._updateSubTexts = function() {
		this._childCursor = 0, wo(this.style), this.style.rich ? this._updateRichTexts() : this._updatePlainTexts(), this._children.length = this._childCursor, this.styleUpdated();
	}, t.prototype.addSelfToZr = function(t) {
		e.prototype.addSelfToZr.call(this, t);
		for (var n = 0; n < this._children.length; n++) this._children[n].__zr = t;
	}, t.prototype.removeSelfFromZr = function(t) {
		e.prototype.removeSelfFromZr.call(this, t);
		for (var n = 0; n < this._children.length; n++) this._children[n].__zr = null;
	}, t.prototype.getBoundingRect = function() {
		if (this.styleChanged() && this._updateSubTexts(), !this._rect) {
			for (var e = new Y(0, 0, 0, 0), t = this._children, n = [], r = null, i = 0; i < t.length; i++) {
				var a = t[i], o = a.getBoundingRect(), s = a.getLocalTransform(n);
				s ? (e.copy(o), e.applyTransform(s), r ||= e.clone(), r.union(e)) : (r ||= o.clone(), r.union(o));
			}
			this._rect = r || e;
		}
		return this._rect;
	}, t.prototype.setDefaultTextStyle = function(e) {
		this._defaultStyle = e || po;
	}, t.prototype.setTextContent = function(e) {}, t.prototype._mergeStyle = function(e, t) {
		if (!t) return e;
		var n = t.rich, r = e.rich || n && {};
		return j(e, t), n && r ? (this._mergeRich(r, n), e.rich = r) : r && (e.rich = r), e;
	}, t.prototype._mergeRich = function(e, t) {
		for (var n = R(t), r = 0; r < n.length; r++) {
			var i = n[r];
			e[i] = e[i] || {}, j(e[i], t[i]);
		}
	}, t.prototype.getAnimationStyleProps = function() {
		return go;
	}, t.prototype._getOrCreateChild = function(e) {
		var t = this._children[this._childCursor];
		return (!t || !(t instanceof e)) && (t = new e()), this._children[this._childCursor++] = t, t.__zr = this.__zr, t.parent = this, t;
	}, t.prototype._updatePlainTexts = function() {
		var e = this.style, t = e.font || "12px sans-serif", n = e.padding, r = this._defaultStyle, i = e.x || 0, a = e.y || 0, o = e.align || r.align || "left", s = e.verticalAlign || r.verticalAlign || "top";
		On(ho, r.overflowRect, i, a, o, s), i = ho.baseX, a = ho.baseY;
		var c = vn(ko(e), e, ho.outerWidth, ho.outerHeight), l = Ao(e), u = !!e.backgroundColor, d = c.outerHeight, f = c.outerWidth, p = c.lines, m = c.lineHeight;
		this.isTruncated = !!c.isTruncated;
		var h = i, g = ln(a, c.contentHeight, s);
		if (l || n) {
			var _ = cn(i, f, o), v = ln(a, d, s);
			l && this._renderBackground(e, e, _, v, f, d);
		}
		g += m / 2, n && (h = Oo(i, o, n), s === "top" ? g += n[0] : s === "bottom" && (g -= n[2]));
		for (var y = 0, b = !1, x = !1, S = Do("fill" in e ? e.fill : (x = !0, r.fill)), C = Eo("stroke" in e ? e.stroke : !u && (!r.autoStroke || x) ? (y = mo, b = !0, r.stroke) : null), w = e.textShadowBlur > 0, T = 0; T < p.length; T++) {
			var E = this._getOrCreateChild($a), D = E.createStyle();
			E.useStyle(D), D.text = p[T], D.x = h, D.y = g, o && (D.textAlign = o), D.textBaseline = "middle", D.opacity = e.opacity, D.strokeFirst = !0, w && (D.shadowBlur = e.textShadowBlur || 0, D.shadowColor = e.textShadowColor || "transparent", D.shadowOffsetX = e.textShadowOffsetX || 0, D.shadowOffsetY = e.textShadowOffsetY || 0), D.stroke = C, D.fill = S, C && (D.lineWidth = e.lineWidth || y, D.lineDash = e.lineDash, D.lineDashOffset = e.lineDashOffset || 0), D.font = t, So(D, e), g += m, E.setBoundingRect(Nn(D, c.contentWidth, c.calculatedLineHeight, b ? 0 : null));
		}
	}, t.prototype._updateRichTexts = function() {
		var e = this.style, t = this._defaultStyle, n = e.align || t.align, r = e.verticalAlign || t.verticalAlign, i = e.x || 0, a = e.y || 0;
		On(ho, t.overflowRect, i, a, n, r), i = ho.baseX, a = ho.baseY;
		var o = Sn(ko(e), e, ho.outerWidth, ho.outerHeight, n), s = o.width, c = o.outerWidth, l = o.outerHeight, u = e.padding;
		this.isTruncated = !!o.isTruncated;
		var d = cn(i, c, n), f = ln(a, l, r), p = d, m = f;
		u && (p += u[3], m += u[0]);
		var h = p + s;
		Ao(e) && this._renderBackground(e, e, d, f, c, l);
		for (var g = !!e.backgroundColor, _ = 0; _ < o.lines.length; _++) {
			for (var v = o.lines[_], y = v.tokens, b = y.length, x = v.lineHeight, S = v.width, C = 0, w = p, T = h, E = b - 1, D = void 0; C < b && (D = y[C], !D.align || D.align === "left");) this._placeToken(D, e, x, m, w, "left", g), S -= D.width, w += D.width, C++;
			for (; E >= 0 && (D = y[E], D.align === "right");) this._placeToken(D, e, x, m, T, "right", g), S -= D.width, T -= D.width, E--;
			for (w += (s - (w - p) - (h - T) - S) / 2; C <= E;) D = y[C], this._placeToken(D, e, x, m, w + D.width / 2, "center", g), w += D.width, C++;
			m += x;
		}
	}, t.prototype._placeToken = function(e, t, n, r, i, a, o) {
		var s = t.rich[e.styleName] || {};
		s.text = e.text;
		var c = e.verticalAlign, l = r + n / 2;
		c === "top" ? l = r + e.height / 2 : c === "bottom" && (l = r + n - e.height / 2), !e.isLineHolder && Ao(s) && this._renderBackground(s, t, a === "right" ? i - e.width : a === "center" ? i - e.width / 2 : i, l - e.height / 2, e.width, e.height);
		var u = !!s.backgroundColor, d = e.textPadding;
		d && (i = Oo(i, a, d), l -= e.height / 2 - d[0] - e.innerHeight / 2);
		var f = this._getOrCreateChild($a), p = f.createStyle();
		f.useStyle(p);
		var m = this._defaultStyle, h = !1, g = 0, _ = !1, v = Do("fill" in s ? s.fill : "fill" in t ? t.fill : (h = !0, m.fill)), y = Eo("stroke" in s ? s.stroke : "stroke" in t ? t.stroke : !u && !o && (!m.autoStroke || h) ? (g = mo, _ = !0, m.stroke) : null), b = s.textShadowBlur > 0 || t.textShadowBlur > 0;
		p.text = e.text, p.x = i, p.y = l, b && (p.shadowBlur = s.textShadowBlur || t.textShadowBlur || 0, p.shadowColor = s.textShadowColor || t.textShadowColor || "transparent", p.shadowOffsetX = s.textShadowOffsetX || t.textShadowOffsetX || 0, p.shadowOffsetY = s.textShadowOffsetY || t.textShadowOffsetY || 0), p.textAlign = a, p.textBaseline = "middle", p.font = e.font || "12px sans-serif", p.opacity = he(s.opacity, t.opacity, 1), So(p, s), y && (p.lineWidth = he(s.lineWidth, t.lineWidth, g), p.lineDash = G(s.lineDash, t.lineDash), p.lineDashOffset = t.lineDashOffset || 0, p.stroke = y), v && (p.fill = v), f.setBoundingRect(Nn(p, e.contentWidth, e.contentHeight, _ ? 0 : null));
	}, t.prototype._renderBackground = function(e, t, n, r, i, a) {
		var o = e.backgroundColor, s = e.borderWidth, c = e.borderColor, l = o && o.image, u = o && !l, d = e.borderRadius, f = this, p, m;
		if (u || e.lineHeight || s && c) {
			p = this._getOrCreateChild(fo), p.useStyle(p.createStyle()), p.style.fill = null;
			var h = p.shape;
			h.x = n, h.y = r, h.width = i, h.height = a, h.r = d, p.dirtyShape();
		}
		if (u) {
			var g = p.style;
			g.fill = o || null, g.fillOpacity = G(e.fillOpacity, 1);
		} else if (l) {
			m = this._getOrCreateChild(ro), m.onload = function() {
				f.dirtyStyle();
			};
			var _ = m.style;
			_.image = o.image, _.x = n, _.y = r, _.width = i, _.height = a;
		}
		if (s && c) {
			var g = p.style;
			g.lineWidth = s, g.stroke = c, g.strokeOpacity = G(e.strokeOpacity, 1), g.lineDash = e.borderDash, g.lineDashOffset = e.borderDashOffset || 0, p.strokeContainThreshold = 0, p.hasFill() && p.hasStroke() && (g.strokeFirst = !0, g.lineWidth *= 2);
		}
		var v = (p || m).style;
		v.shadowBlur = e.shadowBlur || 0, v.shadowColor = e.shadowColor || "transparent", v.shadowOffsetX = e.shadowOffsetX || 0, v.shadowOffsetY = e.shadowOffsetY || 0, v.opacity = he(e.opacity, t.opacity, 1);
	}, t.makeFont = function(e) {
		var t = "";
		return Co(e) && (t = [
			e.fontStyle,
			e.fontWeight,
			xo(e.fontSize),
			e.fontFamily || "sans-serif"
		].join(" ")), t && ye(t) || e.textFont || e.font;
	}, t;
}(Wi), vo = {
	left: !0,
	right: 1,
	center: 1
}, yo = {
	top: 1,
	bottom: 1,
	middle: 1
}, bo = [
	"fontStyle",
	"fontWeight",
	"fontSize",
	"fontFamily"
];
function xo(e) {
	return typeof e == "string" && (e.indexOf("px") !== -1 || e.indexOf("rem") !== -1 || e.indexOf("em") !== -1) ? e : isNaN(+e) ? "12px" : e + "px";
}
function So(e, t) {
	for (var n = 0; n < bo.length; n++) {
		var r = bo[n], i = t[r];
		i != null && (e[r] = i);
	}
}
function Co(e) {
	return e.fontSize != null || e.fontFamily || e.fontWeight;
}
function wo(e) {
	return To(e), I(e.rich, To), e;
}
function To(e) {
	if (e) {
		e.font = _o.makeFont(e);
		var t = e.align;
		t === "middle" && (t = "center"), e.align = t == null || vo[t] ? t : "left";
		var n = e.verticalAlign;
		n === "center" && (n = "middle"), e.verticalAlign = n == null || yo[n] ? n : "top", e.padding &&= _e(e.padding);
	}
}
function Eo(e, t) {
	return e == null || t <= 0 || e === "transparent" || e === "none" ? null : e.image || e.colorStops ? "#000" : e;
}
function Do(e) {
	return e == null || e === "none" ? null : e.image || e.colorStops ? "#000" : e;
}
function Oo(e, t, n) {
	return t === "right" ? e - n[1] : t === "center" ? e + n[3] / 2 - n[1] / 2 : e + n[3];
}
function ko(e) {
	var t = e.text;
	return t != null && (t += ""), t;
}
function Ao(e) {
	return !!(e.backgroundColor || e.lineHeight || e.borderWidth && e.borderColor);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/util/number.js
var jo = 1e-4, Mo = 20;
function No(e) {
	return e.replace(/^\s+|\s+$/g, "");
}
var Po = Math.min, Fo = Math.max, Io = Math.abs, Lo = Math.round, Ro = Math.floor, zo = Math.ceil, Bo = Math.pow, Vo = Math.log, Ho = Math.LN10, Uo = Math.PI, Wo = Math.random;
function Go(e, t, n, r) {
	var i = t[0], a = t[1], o = n[0], s = n[1], c = a - i, l = s - o;
	if (c === 0) return l === 0 ? o : (o + s) / 2;
	if (r) {
		if (c > 0) {
			if (e <= i) return o;
			if (e >= a) return s;
		} else if (e >= i) return o;
		else if (e <= a) return s;
	} else {
		if (e === i) return o;
		if (e === a) return s;
	}
	return (e - i) / c * l + o;
}
var X = Ko;
function Ko(e, t, n) {
	switch (e) {
		case "center":
		case "middle":
			e = "50%";
			break;
		case "left":
		case "top":
			e = "0%";
			break;
		case "right":
		case "bottom":
			e = "100%";
			break;
	}
	return qo(e, t, n);
}
function qo(e, t, n) {
	return U(e) ? Jo(e) ? parseFloat(e) / 100 * t + (n || 0) : parseFloat(e) : e == null ? NaN : +e;
}
function Jo(e) {
	return !!No(e).match(/%$/);
}
function Z(e, t, n) {
	return isNaN(t) ? n ? "" + e : +e : (t = Po(Fo(0, t), Mo), e = (+e).toFixed(t), n ? e : +e);
}
function Yo(e) {
	return e.sort(function(e, t) {
		return e - t;
	}), e;
}
function Xo(e) {
	if (e = +e, isNaN(e)) return 0;
	if (e > 1e-14) {
		for (var t = 1, n = 0; n < 15; n++, t *= 10) if (Lo(e * t) / t === e) return n;
	}
	return Zo(e);
}
function Zo(e) {
	var t = e.toString().toLowerCase(), n = t.indexOf("e"), r = n > 0 ? +t.slice(n + 1) : 0, i = n > 0 ? n : t.length, a = t.indexOf(".");
	return Fo(0, (a < 0 ? 0 : i - 1 - a) - r);
}
function Qo(e, t, n) {
	var r = Io(e[1] - e[0]);
	if (!isFinite(r) || r === 0) return NaN;
	var i = Vo(2 * Io(n || 1) * Io(r)) / Ho, a = Vo(Io(t)) / Ho, o = Fo(0, zo(-i + a));
	return isFinite(o) || (o = NaN), o;
}
function $o(e, t) {
	var n = ne(e, function(e, t) {
		return e + (isNaN(t) ? 0 : t);
	}, 0);
	if (n === 0) return [];
	for (var r = Bo(10, t), i = L(e, function(e) {
		return (isNaN(e) ? 0 : e) / n * r * 100;
	}), a = r * 100, o = L(i, function(e) {
		return Ro(e);
	}), s = ne(o, function(e, t) {
		return e + t;
	}, 0), c = L(i, function(e, t) {
		return e - o[t];
	}); s < a;) {
		for (var l = -Infinity, u = null, d = 0, f = c.length; d < f; ++d) c[d] > l && (l = c[d], u = d);
		++o[u], c[u] = 0, ++s;
	}
	return L(o, function(e) {
		return e / r;
	});
}
function es(e, t) {
	var n = Fo(Xo(e), Xo(t)), r = e + t;
	return n > Mo ? r : Z(r, n);
}
var ts = Bo(2, 53) - 1;
function ns(e) {
	var t = Uo * 2;
	return (e % t + t) % t;
}
function rs(e) {
	return e > -jo && e < jo;
}
var is = /^(?:(\d{4})(?:[-\/](\d{1,2})(?:[-\/](\d{1,2})(?:[T ](\d{1,2})(?::(\d{1,2})(?::(\d{1,2})(?:[.,](\d+))?)?)?(Z|[\+\-]\d\d:?\d\d)?)?)?)?)?$/;
function as(e) {
	if (e instanceof Date) return e;
	if (U(e)) {
		var t = is.exec(e);
		if (!t) return /* @__PURE__ */ new Date(NaN);
		if (t[8]) {
			var n = +t[4] || 0;
			return t[8].toUpperCase() !== "Z" && (n -= +t[8].slice(0, 3)), new Date(Date.UTC(+t[1], (t[2] || 1) - 1, +t[3] || 1, n, +(t[5] || 0), +t[6] || 0, t[7] ? +t[7].substring(0, 3) : 0));
		} else return new Date(+t[1], (t[2] || 1) - 1, +t[3] || 1, +t[4] || 0, +(t[5] || 0), +t[6] || 0, t[7] ? +t[7].substring(0, 3) : 0);
	} else if (e == null) return /* @__PURE__ */ new Date(NaN);
	return new Date(Lo(e));
}
function os(e) {
	return Bo(10, ss(e));
}
function ss(e) {
	if (e === 0) return 0;
	var t = Ro(Vo(e) / Ho);
	return e / Bo(10, t) >= 10 && t++, t;
}
function cs(e, t) {
	var n = ss(e), r = Bo(10, n), i = e / r;
	return e = (t === 2 ? 1 : t ? i < 1.5 ? 1 : i < 2.5 ? 2 : i < 4 ? 3 : i < 7 ? 5 : 10 : i < 1 ? 1 : i < 2 ? 2 : i < 3 ? 3 : i < 5 ? 5 : 10) * r, Z(e, -n);
}
function ls(e) {
	var t = parseFloat(e);
	return t == e && (t !== 0 || !U(e) || e.indexOf("x") <= 0) ? t : NaN;
}
function us(e) {
	return !isNaN(ls(e));
}
function ds() {
	return Lo(Wo() * 9);
}
function fs(e, t) {
	return t === 0 ? e : fs(t, e % t);
}
function ps(e, t) {
	return e == null ? t : t == null ? e : e * t / fs(e, t);
}
function ms(e) {
	return e != null && isFinite(e);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/util/log.js
var hs = "[ECharts] ", gs = {}, _s = typeof console < "u" && console.warn && console.log;
function vs(e, t, n) {
	if (_s) {
		if (n) {
			if (gs[t]) return;
			gs[t] = !0;
		}
		console[e](hs + t);
	}
}
function ys(e, t) {
	vs("error", e, t);
}
function bs(e) {
	throw Error(e);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/util/model.js
function xs(e, t, n) {
	return (t - e) * n + e;
}
var Ss = "series\0", Cs = "\0_ec_\0";
function ws(e) {
	return e instanceof Array ? e : e == null ? [] : [e];
}
function Ts(e, t, n) {
	if (e) {
		e[t] = e[t] || {}, e.emphasis = e.emphasis || {}, e.emphasis[t] = e.emphasis[t] || {};
		for (var r = 0, i = n.length; r < i; r++) {
			var a = n[r];
			!e.emphasis[t].hasOwnProperty(a) && e[t].hasOwnProperty(a) && (e.emphasis[t][a] = e[t][a]);
		}
	}
}
var Es = /* @__PURE__ */ "fontStyle.fontWeight.fontSize.fontFamily.rich.tag.color.textBorderColor.textBorderWidth.width.height.lineHeight.align.verticalAlign.baseline.shadowColor.shadowBlur.shadowOffsetX.shadowOffsetY.textShadowColor.textShadowBlur.textShadowOffsetX.textShadowOffsetY.backgroundColor.borderColor.borderWidth.borderRadius.padding".split(".");
function Ds(e) {
	return W(e) && !V(e) && !(e instanceof Date) ? e.value : e;
}
function Os(e) {
	return W(e) && !(e instanceof Array);
}
function ks(e, t, n) {
	var r = n === "normalMerge", i = n === "replaceMerge", a = n === "replaceAll";
	e ||= [], t = (t || []).slice();
	var o = K();
	I(t, function(e, n) {
		if (!W(e)) {
			t[n] = null;
			return;
		}
	});
	var s = As(e, o, n);
	return (r || i) && js(s, e, o, t), r && Ms(s, t), r || i ? Ns(s, t, i) : a && Ps(s, t), Fs(s), s;
}
function As(e, t, n) {
	var r = [];
	if (n === "replaceAll") return r;
	for (var i = 0; i < e.length; i++) {
		var a = e[i];
		a && a.id != null && t.set(a.id, i), r.push({
			existing: n === "replaceMerge" || Bs(a) ? null : a,
			newOption: null,
			keyInfo: null,
			brandNew: null
		});
	}
	return r;
}
function js(e, t, n, r) {
	I(r, function(i, a) {
		if (!(!i || i.id == null)) {
			var o = Ls(i.id), s = n.get(o);
			if (s != null) {
				var c = e[s];
				ve(!c.newOption, "Duplicated option on id \"" + o + "\"."), c.newOption = i, c.existing = t[s], r[a] = null;
			}
		}
	});
}
function Ms(e, t) {
	I(t, function(n, r) {
		if (!(!n || n.name == null)) for (var i = 0; i < e.length; i++) {
			var a = e[i].existing;
			if (!e[i].newOption && a && (a.id == null || n.id == null) && !Bs(n) && !Bs(a) && Is("name", a, n)) {
				e[i].newOption = n, t[r] = null;
				return;
			}
		}
	});
}
function Ns(e, t, n) {
	I(t, function(t) {
		if (t) {
			for (var r, i = 0; (r = e[i]) && (r.newOption || Bs(r.existing) || r.existing && t.id != null && !Is("id", t, r.existing));) i++;
			r ? (r.newOption = t, r.brandNew = n) : e.push({
				newOption: t,
				brandNew: n,
				existing: null,
				keyInfo: null
			}), i++;
		}
	});
}
function Ps(e, t) {
	I(t, function(t) {
		e.push({
			newOption: t,
			brandNew: !0,
			existing: null,
			keyInfo: null
		});
	});
}
function Fs(e) {
	var t = K();
	I(e, function(e) {
		var n = e.existing;
		n && t.set(n.id, e);
	}), I(e, function(e) {
		var n = e.newOption;
		ve(!n || n.id == null || !t.get(n.id) || t.get(n.id) === e, "id duplicates: " + (n && n.id)), n && n.id != null && t.set(n.id, e), !e.keyInfo && (e.keyInfo = {});
	}), I(e, function(e, n) {
		var r = e.existing, i = e.newOption, a = e.keyInfo;
		if (W(i)) {
			if (a.name = i.name == null ? r ? r.name : Ss + n : Ls(i.name), r) a.id = Ls(r.id);
			else if (i.id != null) a.id = Ls(i.id);
			else {
				var o = 0;
				do
					a.id = "\0" + a.name + "\0" + o++;
				while (t.get(a.id));
			}
			t.set(a.id, e);
		}
	});
}
function Is(e, t, n) {
	var r = Rs(t[e], null), i = Rs(n[e], null);
	return r != null && i != null && r === i;
}
function Ls(e) {
	return Rs(e, "");
}
function Rs(e, t) {
	return e == null ? t : U(e) ? e : se(e) || oe(e) ? e + "" : t;
}
function zs(e) {
	var t = e.name;
	return !!(t && t.indexOf(Ss));
}
function Bs(e) {
	return e && e.id != null && Ls(e.id).indexOf(Cs) === 0;
}
function Vs(e, t, n) {
	I(e, function(e) {
		var r = e.newOption;
		W(r) && (e.keyInfo.mainType = t, e.keyInfo.subType = Hs(t, r, e.existing, n));
	});
}
function Hs(e, t, n, r) {
	return t.type ? t.type : n ? n.subType : r.determineSubType(e, t);
}
function Us(e, t) {
	if (t.dataIndexInside != null) return t.dataIndexInside;
	if (t.dataIndex != null) return V(t.dataIndex) ? L(t.dataIndex, function(t) {
		return e.indexOfRawIndex(t);
	}) : e.indexOfRawIndex(t.dataIndex);
	if (t.name != null) return V(t.name) ? L(t.name, function(t) {
		return e.indexOfName(t);
	}) : e.indexOfName(t.name);
}
function Ws() {
	var e = "__ec_inner_" + Gs++;
	return function(t) {
		return t[e] || (t[e] = {});
	};
}
var Gs = ds();
function Ks(e, t, n) {
	var r = qs(t, n), i = r.mainTypeSpecified, a = r.queryOptionMap, o = r.others, s = n ? n.defaultMainType : null;
	return !i && s && a.set(s, {}), a.each(function(t, r) {
		var i = Ys(e, r, t, {
			useDefault: s === r,
			enableAll: n && n.enableAll != null ? n.enableAll : !0,
			enableNone: n && n.enableNone != null ? n.enableNone : !0
		});
		o[r + "Models"] = i.models, o[r + "Model"] = i.models[0];
	}), o;
}
function qs(e, t) {
	var n;
	if (U(e)) {
		var r = {};
		r[e + "Index"] = 0, n = r;
	} else n = e;
	var i = K(), a = {}, o = !1;
	return I(n, function(e, n) {
		if (n === "dataIndex" || n === "dataIndexInside") {
			a[n] = e;
			return;
		}
		var r = n.match(/^(\w+)(Index|Id|Name)$/) || [], s = r[1], c = (r[2] || "").toLowerCase();
		if (!(!s || !c || t && t.includeMainTypes && N(t.includeMainTypes, s) < 0)) {
			o ||= !!s;
			var l = i.get(s) || i.set(s, {});
			l[c] = e;
		}
	}), {
		mainTypeSpecified: o,
		queryOptionMap: i,
		others: a
	};
}
var Js = {
	useDefault: !0,
	enableAll: !1,
	enableNone: !1
};
function Ys(e, t, n, r) {
	r ||= Js;
	var i = n.index, a = n.id, o = n.name, s = {
		models: null,
		specified: i != null || a != null || o != null
	};
	if (!s.specified) {
		var c = void 0;
		return s.models = r.useDefault && (c = e.getComponent(t)) ? [c] : [], s;
	}
	if (i === "none" || i === !1) {
		if (r.enableNone) return s.models = [], s;
		i = -1;
	}
	return i === "all" && (i = r.enableAll ? a = o = null : -1), s.models = e.queryComponents({
		mainType: t,
		index: i,
		id: a,
		name: o
	}), s;
}
function Xs(e, t, n) {
	var r = {};
	r[t + "Id"] = e[t + "Id"], r[t + "Index"] = e[t + "Index"], r[t + "Name"] = e[t + "Name"];
	var i = {
		mainType: t,
		query: r
	};
	return n && (i.subType = n), i;
}
function Zs(e, t, n) {
	e.setAttribute ? e.setAttribute(t, n) : e[t] = n;
}
function Qs(e, t) {
	return e.getAttribute ? e.getAttribute(t) : e[t];
}
function $s(e) {
	return e === "auto" ? q.domSupported ? "html" : "richText" : e || "html";
}
function ec(e, t, n, r, i) {
	var a = t == null || t === "auto";
	if (r == null) return r;
	if (se(r)) {
		var o = xs(n || 0, r, i);
		return Z(o, a ? Math.max(Xo(n || 0), Xo(r)) : t);
	} else if (U(r)) return i < 1 ? n : r;
	else {
		for (var s = [], c = n, l = r, u = Math.max(c ? c.length : 0, l.length), d = 0; d < u; ++d) {
			var f = e.getDimensionInfo(d);
			if (f && f.type === "ordinal") s[d] = (i < 1 && c ? c : l)[d];
			else {
				var p = c && c[d] ? c[d] : 0, m = l[d], o = xs(p, m, i);
				s[d] = Z(o, a ? Math.max(Xo(p), Xo(m)) : t);
			}
		}
		return s;
	}
}
(function() {
	function e() {}
	return e.prototype.reset = function(e, t, n, r) {
		return this._list = e, this._step = r ||= 1, this._idx = t, this._end = n ?? (r > 0 ? e.length : 0), this.item = null, this.key = NaN, this;
	}, e.prototype.next = function() {
		return (this._step > 0 ? this._idx < this._end : this._idx >= this._end) ? (this.item = this._list[this._idx], this.key = this._idx += this._step, !0) : !1;
	}, e;
})();
function tc() {
	return [Infinity, -Infinity];
}
function nc(e, t) {
	oc(t) && (t < e[0] && (e[0] = t), t > e[1] && (e[1] = t));
}
function rc(e, t) {
	oc(t) && t < e[0] && (e[0] = t);
}
function ic(e, t) {
	oc(t) && t > e[1] && (e[1] = t);
}
function ac(e, t) {
	sc(t[0], t[1]) && (t[0] < e[0] && (e[0] = t[0]), t[1] > e[1] && (e[1] = t[1]));
}
function oc(e) {
	return e != null && isFinite(e);
}
function sc(e, t) {
	return oc(e) && oc(t) && e <= t;
}
function cc(e) {
	var t = e[1] - e[0];
	return isFinite(t) && t >= 0;
}
function lc(e) {
	sc(e[0], e[1]) && e[0] > e[1] && (e[0] = e[1]);
}
function uc() {
	var e = "__ec_once_" + dc++;
	return function(t, n) {
		Ae(t, e) || (t[e] = 1, n());
	};
}
var dc = ds();
function fc(e, t, n) {
	var r = K(), i = 0;
	I(e, function(a) {
		var o = t(a), s = r.get(o) || 0;
		n && n(a, s), !s && !n && (e[i++] = a), r.set(o, s + 1);
	}), n || (e.length = i);
}
function pc(e) {
	return e.value + "";
}
function mc(e) {
	return e + "";
}
function hc(e, t) {
	return G(t, !0) ? e.seriesIndex + 2 : 0;
}
function gc(e, t, n) {
	var r = e.getData().count();
	return {
		progressiveRender: n.progressiveEnabled && t.incrementalPrepareRender && r >= n.threshold,
		large: e.get("large") && r >= e.get("largeThreshold"),
		modDataCount: e.get("progressiveChunkMode") === "mod" ? e.getData().count() : null
	};
}
function _c(e, t) {
	return {
		seriesType: e,
		overallReset: t
	};
}
function vc(e) {
	return { overallReset: e };
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/util/innerStore.js
var yc = Ws(), bc = function(e, t, n, r) {
	if (r) {
		var i = yc(r);
		i.dataIndex = n, i.dataType = t, i.seriesIndex = e, i.ssrType = "chart", r.type === "group" && r.traverse(function(r) {
			var i = yc(r);
			i.seriesIndex = e, i.dataIndex = n, i.dataType = t, i.ssrType = "chart";
		});
	}
}, xc = K([
	"tooltip",
	"label",
	"itemName",
	"itemId",
	"itemGroupId",
	"itemChildGroupId",
	"seriesName"
]), Sc = "original", Cc = "arrayRows", wc = "objectRows", Tc = "keyedColumns", Ec = "typedArray", Dc = "unknown", Oc = "column", kc = [
	"getDom",
	"getZr",
	"getWidth",
	"getHeight",
	"getDevicePixelRatio",
	"dispatchAction",
	"isSSR",
	"isDisposed",
	"on",
	"off",
	"getDataURL",
	"getConnectedDataURL",
	"getOption",
	"getId",
	"updateLabelLayout"
], Ac = function() {
	function e(e) {
		I(kc, function(t) {
			this[t] = z(e[t], e);
		}, this);
	}
	return e;
}();
function jc(e, t) {
	return t.mainType === "series" ? e.getViewOfSeriesModel(t) : e.getViewOfComponentModel(t);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/util/states.js
var Mc = 1, Nc = {}, Pc = Ws(), Fc = Ws(), Ic = [
	"emphasis",
	"blur",
	"select"
], Lc = [
	"normal",
	"emphasis",
	"blur",
	"select"
], Rc = "highlight", zc = "downplay", Bc = "select", Vc = "unselect", Hc = "toggleSelect", Uc = "selectchanged";
function Wc(e) {
	return e != null && e !== "none";
}
function Gc(e, t, n) {
	e.onHoverStateChange && (e.hoverState || 0) !== n && e.onHoverStateChange(t), e.hoverState = n;
}
function Kc(e) {
	Gc(e, "emphasis", 2);
}
function qc(e) {
	e.hoverState === 2 && Gc(e, "normal", 0);
}
function Jc(e) {
	Gc(e, "blur", 1);
}
function Yc(e) {
	e.hoverState === 1 && Gc(e, "normal", 0);
}
function Xc(e) {
	e.selected = !0;
}
function Zc(e) {
	e.selected = !1;
}
function Qc(e, t, n) {
	t(e, n);
}
function $c(e, t, n) {
	Qc(e, t, n), e.isGroup && e.traverse(function(e) {
		Qc(e, t, n);
	});
}
function el(e, t) {
	switch (t) {
		case "emphasis":
			e.hoverState = 2;
			break;
		case "normal":
			e.hoverState = 0;
			break;
		case "blur":
			e.hoverState = 1;
			break;
		case "select": e.selected = !0;
	}
}
function tl(e, t, n, r) {
	for (var i = e.style, a = {}, o = 0; o < t.length; o++) {
		var s = t[o];
		a[s] = i[s] ?? (r && r[s]);
	}
	for (var o = 0; o < e.animators.length; o++) {
		var c = e.animators[o];
		c.__fromStateTransition && c.__fromStateTransition.indexOf(n) < 0 && c.targetName === "style" && c.saveTo(a, t);
	}
	return a;
}
function nl(e, t, n, r) {
	var i = n && N(n, "select") >= 0, a = !1;
	if (e instanceof Za) {
		var o = Pc(e), s = i && o.selectFill || o.normalFill, c = i && o.selectStroke || o.normalStroke;
		if (Wc(s) || Wc(c)) {
			r ||= {};
			var l = r.style || {};
			l.fill === "inherit" ? (a = !0, r = j({}, r), l = j({}, l), l.fill = s) : !Wc(l.fill) && Wc(s) ? (a = !0, r = j({}, r), l = j({}, l), l.fill = Ur(s)) : !Wc(l.stroke) && Wc(c) && (a || (r = j({}, r), l = j({}, l)), l.stroke = Ur(c)), r.style = l;
		}
	}
	if (r && r.z2 == null) {
		a || (r = j({}, r));
		var u = e.z2EmphasisLift;
		r.z2 = e.z2 + (u ?? 10);
	}
	return r;
}
function rl(e, t, n) {
	if (n && n.z2 == null) {
		n = j({}, n);
		var r = e.z2SelectLift;
		n.z2 = e.z2 + (r ?? 9);
	}
	return n;
}
function il(e, t, n) {
	var r = N(e.currentStates, t) >= 0, i = e.style.opacity, a = r ? null : tl(e, ["opacity"], t, { opacity: 1 });
	n ||= {};
	var o = n.style || {};
	return o.opacity ?? (n = j({}, n), o = j({ opacity: r ? i : a.opacity * .1 }, o), n.style = o), n;
}
function al(e, t) {
	var n = this.states[e];
	if (this.style) {
		if (e === "emphasis") return nl(this, e, t, n);
		if (e === "blur") return il(this, e, n);
		if (e === "select") return rl(this, e, n);
	}
	return n;
}
function ol(e) {
	e.stateProxy = al;
	var t = e.getTextContent(), n = e.getTextGuideLine();
	t && (t.stateProxy = al), n && (n.stateProxy = al);
}
function sl(e, t) {
	!hl(e, t) && !e.__highByOuter && $c(e, Kc);
}
function cl(e, t) {
	!hl(e, t) && !e.__highByOuter && $c(e, qc);
}
function ll(e, t) {
	e.__highByOuter |= 1 << (t || 0), $c(e, Kc);
}
function ul(e, t) {
	!(e.__highByOuter &= ~(1 << (t || 0))) && $c(e, qc);
}
function dl(e) {
	$c(e, Jc);
}
function fl(e) {
	$c(e, Yc);
}
function pl(e) {
	$c(e, Xc);
}
function ml(e) {
	$c(e, Zc);
}
function hl(e, t) {
	return e.__highDownSilentOnTouch && t.zrByTouch;
}
function gl(e) {
	var t = e.getModel(), n = [], r = [];
	t.eachComponent(function(t, i) {
		var a = Fc(i), o = jc(e, i), s = t === "series";
		!s && r.push(o), a.isBlured && (o.group.traverse(function(e) {
			Yc(e);
		}), s && n.push(i)), a.isBlured = !1;
	}), I(r, function(e) {
		e && e.toggleBlurSeries && e.toggleBlurSeries(n, !1, t);
	});
}
function _l(e, t, n, r) {
	var i = r.getModel();
	n ||= "coordinateSystem";
	function a(e, t) {
		for (var n = 0; n < t.length; n++) {
			var r = e.getItemGraphicEl(t[n]);
			r && fl(r);
		}
	}
	if (e != null && !(!t || t === "none")) {
		var o = i.getSeriesByIndex(e), s = o.coordinateSystem;
		s && s.master && (s = s.master);
		var c = [];
		i.eachSeries(function(e) {
			var i = o === e, l = e.coordinateSystem;
			if (l && l.master && (l = l.master), !(n === "series" && !i || n === "coordinateSystem" && !(l && s ? l === s : i) || t === "series" && i)) {
				if (r.getViewOfSeriesModel(e).group.traverse(function(e) {
					e.__highByOuter && i && t === "self" || Jc(e);
				}), F(t)) a(e.getData(), t);
				else if (W(t)) for (var u = R(t), d = 0; d < u.length; d++) a(e.getData(u[d]), t[u[d]]);
				c.push(e), Fc(e).isBlured = !0;
			}
		}), i.eachComponent(function(e, t) {
			if (e !== "series") {
				var n = r.getViewOfComponentModel(t);
				n && n.toggleBlurSeries && n.toggleBlurSeries(c, !0, i);
			}
		});
	}
}
function vl(e, t, n) {
	if (!(e == null || t == null)) {
		var r = n.getModel().getComponent(e, t);
		if (r) {
			Fc(r).isBlured = !0;
			var i = n.getViewOfComponentModel(r);
			!i || !i.focusBlurEnabled || i.group.traverse(function(e) {
				Jc(e);
			});
		}
	}
}
function yl(e, t, n) {
	var r = e.seriesIndex, i = e.getData(t.dataType);
	if (i) {
		var a = Us(i, t);
		a = (V(a) ? a[0] : a) || 0;
		var o = i.getItemGraphicEl(a);
		if (!o) for (var s = i.count(), c = 0; !o && c < s;) o = i.getItemGraphicEl(c++);
		if (o) {
			var l = yc(o);
			_l(r, l.focus, l.blurScope, n);
		} else {
			var u = e.get(["emphasis", "focus"]), d = e.get(["emphasis", "blurScope"]);
			u != null && _l(r, u, d, n);
		}
	}
}
function bl(e, t, n, r) {
	var i = {
		focusSelf: !1,
		dispatchers: null
	};
	if (e == null || e === "series" || t == null || n == null) return i;
	var a = r.getModel().getComponent(e, t);
	if (!a) return i;
	var o = r.getViewOfComponentModel(a);
	if (!o || !o.findHighDownDispatchers) return i;
	for (var s = o.findHighDownDispatchers(n), c, l = 0; l < s.length; l++) if (yc(s[l]).focus === "self") {
		c = !0;
		break;
	}
	return {
		focusSelf: c,
		dispatchers: s
	};
}
function xl(e, t, n) {
	var r = yc(e), i = bl(r.componentMainType, r.componentIndex, r.componentHighDownName, n), a = i.dispatchers, o = i.focusSelf;
	a ? (o && vl(r.componentMainType, r.componentIndex, n), I(a, function(e) {
		return sl(e, t);
	})) : (_l(r.seriesIndex, r.focus, r.blurScope, n), r.focus === "self" && vl(r.componentMainType, r.componentIndex, n), sl(e, t));
}
function Sl(e, t, n) {
	gl(n);
	var r = yc(e), i = bl(r.componentMainType, r.componentIndex, r.componentHighDownName, n).dispatchers;
	i ? I(i, function(e) {
		return cl(e, t);
	}) : cl(e, t);
}
function Cl(e, t, n) {
	if (Il(t)) {
		var r = t.dataType, i = Us(e.getData(r), t);
		V(i) || (i = [i]), e[t.type === "toggleSelect" ? "toggleSelect" : t.type === "select" ? "select" : "unselect"](i, r);
	}
}
function wl(e) {
	I(e.getAllData(), function(t) {
		var n = t.data, r = t.type;
		n.eachItemGraphicEl(function(t, n) {
			e.isSelected(n, r) ? pl(t) : ml(t);
		});
	});
}
function Tl(e) {
	var t = [];
	return e.eachSeries(function(e) {
		I(e.getAllData(), function(n) {
			n.data;
			var r = n.type, i = e.getSelectedDataIndices();
			if (i.length > 0) {
				var a = {
					dataIndex: i,
					seriesIndex: e.seriesIndex
				};
				r != null && (a.dataType = r), t.push(a);
			}
		});
	}), t;
}
function El(e, t, n) {
	Nl(e, !0), $c(e, ol), kl(e, t, n);
}
function Dl(e) {
	Nl(e, !1);
}
function Ol(e, t, n, r) {
	r ? Dl(e) : El(e, t, n);
}
function kl(e, t, n) {
	var r = yc(e);
	t == null ? r.focus &&= null : (r.focus = t, r.blurScope = n);
}
var Al = [
	"emphasis",
	"blur",
	"select"
], jl = {
	itemStyle: "getItemStyle",
	lineStyle: "getLineStyle",
	areaStyle: "getAreaStyle"
};
function Ml(e, t, n, r) {
	n ||= "itemStyle";
	for (var i = 0; i < Al.length; i++) {
		var a = Al[i], o = t.getModel([a, n]), s = e.ensureState(a);
		s.style = r ? r(o) : o[jl[n]]();
	}
}
function Nl(e, t) {
	var n = t === !1, r = e;
	e.highDownSilentOnTouch && (r.__highDownSilentOnTouch = e.highDownSilentOnTouch), (!n || r.__highDownDispatcher) && (r.__highByOuter = r.__highByOuter || 0, r.__highDownDispatcher = !n);
}
function Pl(e) {
	return !!(e && e.__highDownDispatcher);
}
function Fl(e) {
	var t = Nc[e];
	return t == null && Mc <= 32 && (t = Nc[e] = Mc++), t;
}
function Il(e) {
	var t = e.type;
	return t === "select" || t === "unselect" || t === "toggleSelect";
}
function Ll(e) {
	var t = e.type;
	return t === "highlight" || t === "downplay";
}
function Rl(e) {
	var t = Pc(e);
	t.normalFill = e.style.fill, t.normalStroke = e.style.stroke;
	var n = e.states.select || {};
	t.selectFill = n.style && n.style.fill || null, t.selectStroke = n.style && n.style.stroke || null;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/tool/transformPath.js
var zl = Ea.CMD, Bl = [
	[],
	[],
	[]
], Vl = Math.sqrt, Hl = Math.atan2;
function Ul(e, t) {
	if (t) {
		var n = e.data, r = e.len(), i, a, o, s, c, l, u = zl.M, d = zl.C, f = zl.L, p = zl.R, m = zl.A, h = zl.Q;
		for (o = 0, s = 0; o < r;) {
			switch (i = n[o++], s = o, a = 0, i) {
				case u:
					a = 1;
					break;
				case f:
					a = 1;
					break;
				case d:
					a = 3;
					break;
				case h:
					a = 2;
					break;
				case m:
					var g = t[4], _ = t[5], v = Vl(t[0] * t[0] + t[1] * t[1]), y = Vl(t[2] * t[2] + t[3] * t[3]), b = Hl(-t[1] / y, t[0] / v);
					n[o] *= v, n[o++] += g, n[o] *= y, n[o++] += _, n[o++] *= v, n[o++] *= y, n[o++] += b, n[o++] += b, o += 2, s = o;
					break;
				case p: l[0] = n[o++], l[1] = n[o++], Ot(l, l, t), n[s++] = l[0], n[s++] = l[1], l[0] += n[o++], l[1] += n[o++], Ot(l, l, t), n[s++] = l[0], n[s++] = l[1];
			}
			for (c = 0; c < a; c++) {
				var x = Bl[c];
				x[0] = n[o++], x[1] = n[o++], Ot(x, x, t), n[s++] = x[0], n[s++] = x[1];
			}
		}
		e.increaseVersion();
	}
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/tool/path.js
var Wl = Math.sqrt, Gl = Math.sin, Kl = Math.cos, ql = Math.PI;
function Jl(e) {
	return Math.sqrt(e[0] * e[0] + e[1] * e[1]);
}
function Yl(e, t) {
	return (e[0] * t[0] + e[1] * t[1]) / (Jl(e) * Jl(t));
}
function Xl(e, t) {
	return (e[0] * t[1] < e[1] * t[0] ? -1 : 1) * Math.acos(Yl(e, t));
}
function Zl(e, t, n, r, i, a, o, s, c, l, u) {
	var d = ql / 180 * c, f = Kl(d) * (e - n) / 2 + Gl(d) * (t - r) / 2, p = -1 * Gl(d) * (e - n) / 2 + Kl(d) * (t - r) / 2, m = f * f / (o * o) + p * p / (s * s);
	m > 1 && (o *= Wl(m), s *= Wl(m));
	var h = (i === a ? -1 : 1) * Wl((o * o * (s * s) - o * o * (p * p) - s * s * (f * f)) / (o * o * (p * p) + s * s * (f * f))) || 0, g = h * o * p / s, _ = h * -s * f / o, v = (e + n) / 2 + Kl(d) * g - Gl(d) * _, y = (t + r) / 2 + Gl(d) * g + Kl(d) * _, b = Xl([1, 0], [(f - g) / o, (p - _) / s]), x = [(f - g) / o, (p - _) / s], S = [(-1 * f - g) / o, (-1 * p - _) / s], C = Xl(x, S);
	if (Yl(x, S) <= -1 && (C = ql), Yl(x, S) >= 1 && (C = 0), C < 0) {
		var w = Math.round(C / ql * 1e6) / 1e6;
		C = ql * 2 + w % 2 * ql;
	}
	u.addData(l, v, y, o, s, b, C, d, a);
}
var Ql = /([mlvhzcqtsa])([^mlvhzcqtsa]*)/gi, $l = /-?([0-9]*\.)?[0-9]+([eE]-?[0-9]+)?/g;
function eu(e) {
	var t = new Ea();
	if (!e) return t;
	var n = 0, r = 0, i = n, a = r, o, s = Ea.CMD, c = e.match(Ql);
	if (!c) return t;
	for (var l = 0; l < c.length; l++) {
		for (var u = c[l], d = u.charAt(0), f = void 0, p = u.match($l) || [], m = p.length, h = 0; h < m; h++) p[h] = parseFloat(p[h]);
		for (var g = 0; g < m;) {
			var _ = void 0, v = void 0, y = void 0, b = void 0, x = void 0, S = void 0, C = void 0, w = n, T = r, E = void 0, D = void 0;
			switch (d) {
				case "l":
					n += p[g++], r += p[g++], f = s.L, t.addData(f, n, r);
					break;
				case "L":
					n = p[g++], r = p[g++], f = s.L, t.addData(f, n, r);
					break;
				case "m":
					n += p[g++], r += p[g++], f = s.M, t.addData(f, n, r), i = n, a = r, d = "l";
					break;
				case "M":
					n = p[g++], r = p[g++], f = s.M, t.addData(f, n, r), i = n, a = r, d = "L";
					break;
				case "h":
					n += p[g++], f = s.L, t.addData(f, n, r);
					break;
				case "H":
					n = p[g++], f = s.L, t.addData(f, n, r);
					break;
				case "v":
					r += p[g++], f = s.L, t.addData(f, n, r);
					break;
				case "V":
					r = p[g++], f = s.L, t.addData(f, n, r);
					break;
				case "C":
					f = s.C, t.addData(f, p[g++], p[g++], p[g++], p[g++], p[g++], p[g++]), n = p[g - 2], r = p[g - 1];
					break;
				case "c":
					f = s.C, t.addData(f, p[g++] + n, p[g++] + r, p[g++] + n, p[g++] + r, p[g++] + n, p[g++] + r), n += p[g - 2], r += p[g - 1];
					break;
				case "S":
					_ = n, v = r, E = t.len(), D = t.data, o === s.C && (_ += n - D[E - 4], v += r - D[E - 3]), f = s.C, w = p[g++], T = p[g++], n = p[g++], r = p[g++], t.addData(f, _, v, w, T, n, r);
					break;
				case "s":
					_ = n, v = r, E = t.len(), D = t.data, o === s.C && (_ += n - D[E - 4], v += r - D[E - 3]), f = s.C, w = n + p[g++], T = r + p[g++], n += p[g++], r += p[g++], t.addData(f, _, v, w, T, n, r);
					break;
				case "Q":
					w = p[g++], T = p[g++], n = p[g++], r = p[g++], f = s.Q, t.addData(f, w, T, n, r);
					break;
				case "q":
					w = p[g++] + n, T = p[g++] + r, n += p[g++], r += p[g++], f = s.Q, t.addData(f, w, T, n, r);
					break;
				case "T":
					_ = n, v = r, E = t.len(), D = t.data, o === s.Q && (_ += n - D[E - 4], v += r - D[E - 3]), n = p[g++], r = p[g++], f = s.Q, t.addData(f, _, v, n, r);
					break;
				case "t":
					_ = n, v = r, E = t.len(), D = t.data, o === s.Q && (_ += n - D[E - 4], v += r - D[E - 3]), n += p[g++], r += p[g++], f = s.Q, t.addData(f, _, v, n, r);
					break;
				case "A":
					y = p[g++], b = p[g++], x = p[g++], S = p[g++], C = p[g++], w = n, T = r, n = p[g++], r = p[g++], f = s.A, Zl(w, T, n, r, S, C, y, b, x, f, t);
					break;
				case "a":
					y = p[g++], b = p[g++], x = p[g++], S = p[g++], C = p[g++], w = n, T = r, n += p[g++], r += p[g++], f = s.A, Zl(w, T, n, r, S, C, y, b, x, f, t);
					break;
			}
		}
		(d === "z" || d === "Z") && (f = s.Z, t.addData(f), n = i, r = a), o = f;
	}
	return t.toStatic(), t;
}
var tu = function(e) {
	o(t, e);
	function t() {
		return e !== null && e.apply(this, arguments) || this;
	}
	return t.prototype.applyTransform = function(e) {}, t;
}(Za);
function nu(e) {
	return e.setData != null;
}
function ru(e, t) {
	var n = eu(e), r = j({}, t);
	return r.buildPath = function(e) {
		var t = nu(e);
		if (t && e.canSave()) {
			e.appendPath(n);
			var r = e.getContext();
			r && e.rebuildPath(r, 1);
		} else {
			var r = t ? e.getContext() : e;
			r && n.rebuildPath(r, 1);
		}
	}, r.applyTransform = function(e) {
		Ul(n, e), this.dirtyShape();
	}, r;
}
function iu(e, t) {
	return new tu(ru(e, t));
}
function au(e, t) {
	var n = ru(e, t);
	return function(e) {
		o(t, e);
		function t(t) {
			var r = e.call(this, t) || this;
			return r.applyTransform = n.applyTransform, r.buildPath = n.buildPath, r;
		}
		return t;
	}(tu);
}
function ou(e, t) {
	for (var n = [], r = e.length, i = 0; i < r; i++) {
		var a = e[i];
		n.push(a.getUpdatedPathProxy(!0));
	}
	var o = new Za(t);
	return o.createPathProxy(), o.buildPath = function(e) {
		if (nu(e)) {
			e.appendPath(n);
			var t = e.getContext();
			t && e.rebuildPath(t, 1);
		}
	}, o;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/Group.js
var su = function(e) {
	o(t, e);
	function t(t) {
		var n = e.call(this) || this;
		return n.isGroup = !0, n._children = [], n.attr(t), n;
	}
	return t.prototype.childrenRef = function() {
		return this._children;
	}, t.prototype.children = function() {
		return this._children.slice();
	}, t.prototype.childAt = function(e) {
		return this._children[e];
	}, t.prototype.childOfName = function(e) {
		for (var t = this._children, n = 0; n < t.length; n++) if (t[n].name === e) return t[n];
	}, t.prototype.childCount = function() {
		return this._children.length;
	}, t.prototype.add = function(e) {
		return e && e !== this && e.parent !== this && (this._children.push(e), this._doAdd(e)), this;
	}, t.prototype.addBefore = function(e, t) {
		if (e && e !== this && e.parent !== this && t && t.parent === this) {
			var n = this._children, r = n.indexOf(t);
			r >= 0 && (n.splice(r, 0, e), this._doAdd(e));
		}
		return this;
	}, t.prototype.replace = function(e, t) {
		var n = N(this._children, e);
		return n >= 0 && this.replaceAt(t, n), this;
	}, t.prototype.replaceAt = function(e, t) {
		var n = this._children, r = n[t];
		if (e && e !== this && e.parent !== this && e !== r) {
			n[t] = e, r.parent = null;
			var i = this.__zr;
			i && r.removeSelfFromZr(i), this._doAdd(e);
		}
		return this;
	}, t.prototype._doAdd = function(e) {
		e.parent && e.parent.remove(e), e.parent = this;
		var t = this.__zr;
		t && t !== e.__zr && e.addSelfToZr(t), t && t.refresh();
	}, t.prototype.remove = function(e) {
		var t = this.__zr, n = this._children, r = N(n, e);
		return r < 0 ? this : (n.splice(r, 1), e.parent = null, t && e.removeSelfFromZr(t), t && t.refresh(), this);
	}, t.prototype.removeAll = function() {
		for (var e = this._children, t = this.__zr, n = 0; n < e.length; n++) {
			var r = e[n];
			t && r.removeSelfFromZr(t), r.parent = null;
		}
		return e.length = 0, this;
	}, t.prototype.eachChild = function(e, t) {
		for (var n = this._children, r = 0; r < n.length; r++) {
			var i = n[r];
			e.call(t, i, r);
		}
		return this;
	}, t.prototype.traverse = function(e, t) {
		for (var n = 0; n < this._children.length; n++) {
			var r = this._children[n], i = e.call(t, r);
			r.isGroup && !i && r.traverse(e, t);
		}
		return this;
	}, t.prototype.addSelfToZr = function(t) {
		e.prototype.addSelfToZr.call(this, t);
		for (var n = 0; n < this._children.length; n++) this._children[n].addSelfToZr(t);
	}, t.prototype.removeSelfFromZr = function(t) {
		e.prototype.removeSelfFromZr.call(this, t);
		for (var n = 0; n < this._children.length; n++) this._children[n].removeSelfFromZr(t);
	}, t.prototype.getBoundingRect = function(e) {
		for (var t = new Y(0, 0, 0, 0), n = e || this._children, r = [], i = null, a = 0; a < n.length; a++) {
			var o = n[a];
			if (!(o.ignore || o.invisible)) {
				var s = o.getBoundingRect(), c = o.getLocalTransform(r);
				c ? (Y.applyTransform(t, s, c), i ||= t.clone(), i.union(t)) : (i ||= s.clone(), i.union(s));
			}
		}
		return i || t;
	}, t;
}(Oi);
su.prototype.type = "group";
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/shape/Circle.js
var cu = function() {
	function e() {
		this.cx = 0, this.cy = 0, this.r = 0;
	}
	return e;
}(), lu = function(e) {
	o(t, e);
	function t(t) {
		return e.call(this, t) || this;
	}
	return t.prototype.getDefaultShape = function() {
		return new cu();
	}, t.prototype.buildPath = function(e, t) {
		e.moveTo(t.cx + t.r, t.cy), e.arc(t.cx, t.cy, t.r, 0, Math.PI * 2);
	}, t;
}(Za);
lu.prototype.type = "circle";
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/shape/Ellipse.js
var uu = function() {
	function e() {
		this.cx = 0, this.cy = 0, this.rx = 0, this.ry = 0;
	}
	return e;
}(), du = function(e) {
	o(t, e);
	function t(t) {
		return e.call(this, t) || this;
	}
	return t.prototype.getDefaultShape = function() {
		return new uu();
	}, t.prototype.buildPath = function(e, t) {
		var n = .5522848, r = t.cx, i = t.cy, a = t.rx, o = t.ry, s = a * n, c = o * n;
		e.moveTo(r - a, i), e.bezierCurveTo(r - a, i - c, r - s, i - o, r, i - o), e.bezierCurveTo(r + s, i - o, r + a, i - c, r + a, i), e.bezierCurveTo(r + a, i + c, r + s, i + o, r, i + o), e.bezierCurveTo(r - s, i + o, r - a, i + c, r - a, i), e.closePath();
	}, t;
}(Za);
du.prototype.type = "ellipse";
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/helper/roundSector.js
var fu = Math.PI, pu = fu * 2, mu = Math.sin, hu = Math.cos, gu = Math.acos, _u = Math.atan2, vu = Math.abs, yu = Math.sqrt, bu = Math.max, xu = Math.min, Su = 1e-4;
function Cu(e, t, n, r, i, a, o, s) {
	var c = n - e, l = r - t, u = o - i, d = s - a, f = d * c - u * l;
	if (!(f * f < Su)) return f = (u * (t - a) - d * (e - i)) / f, [e + f * c, t + f * l];
}
function wu(e, t, n, r, i, a, o) {
	var s = e - n, c = t - r, l = (o ? a : -a) / yu(s * s + c * c), u = l * c, d = -l * s, f = e + u, p = t + d, m = n + u, h = r + d, g = (f + m) / 2, _ = (p + h) / 2, v = m - f, y = h - p, b = v * v + y * y, x = i - a, S = f * h - m * p, C = (y < 0 ? -1 : 1) * yu(bu(0, x * x * b - S * S)), w = (S * y - v * C) / b, T = (-S * v - y * C) / b, E = (S * y + v * C) / b, D = (-S * v + y * C) / b, O = w - g, k = T - _, A = E - g, j = D - _;
	return O * O + k * k > A * A + j * j && (w = E, T = D), {
		cx: w,
		cy: T,
		x0: -u,
		y0: -d,
		x1: w * (i / x - 1),
		y1: T * (i / x - 1)
	};
}
function Tu(e) {
	var t;
	if (V(e)) {
		var n = e.length;
		if (!n) return e;
		t = n === 1 ? [
			e[0],
			e[0],
			0,
			0
		] : n === 2 ? [
			e[0],
			e[0],
			e[1],
			e[1]
		] : n === 3 ? e.concat(e[2]) : e;
	} else t = [
		e,
		e,
		e,
		e
	];
	return t;
}
function Eu(e, t) {
	var n, r = bu(t.r, 0), i = bu(t.r0 || 0, 0), a = r > 0;
	if (!(!a && !(i > 0))) {
		if (a || (r = i, i = 0), i > r) {
			var o = r;
			r = i, i = o;
		}
		var s = t.startAngle, c = t.endAngle;
		if (!(isNaN(s) || isNaN(c))) {
			var l = t.cx, u = t.cy, d = !!t.clockwise, f = vu(c - s), p = f > pu && f % pu;
			if (p > Su && (f = p), !(r > Su)) e.moveTo(l, u);
			else if (f > pu - Su) e.moveTo(l + r * hu(s), u + r * mu(s)), e.arc(l, u, r, s, c, !d), i > Su && (e.moveTo(l + i * hu(c), u + i * mu(c)), e.arc(l, u, i, c, s, d));
			else {
				var m = void 0, h = void 0, g = void 0, _ = void 0, v = void 0, y = void 0, b = void 0, x = void 0, S = void 0, C = void 0, w = void 0, T = void 0, E = void 0, D = void 0, O = void 0, k = void 0, A = r * hu(s), j = r * mu(s), ee = i * hu(c), M = i * mu(c), N = f > Su;
				if (N) {
					var te = t.cornerRadius;
					te && (n = Tu(te), m = n[0], h = n[1], g = n[2], _ = n[3]);
					var P = vu(r - i) / 2;
					if (v = xu(P, g), y = xu(P, _), b = xu(P, m), x = xu(P, h), w = S = bu(v, y), T = C = bu(b, x), (S > Su || C > Su) && (E = r * hu(c), D = r * mu(c), O = i * hu(s), k = i * mu(s), f < fu)) {
						var F = Cu(A, j, O, k, E, D, ee, M);
						if (F) {
							var I = A - F[0], L = j - F[1], ne = E - F[0], re = D - F[1], ie = 1 / mu(gu((I * ne + L * re) / (yu(I * I + L * L) * yu(ne * ne + re * re))) / 2), R = yu(F[0] * F[0] + F[1] * F[1]);
							w = xu(S, (r - R) / (ie + 1)), T = xu(C, (i - R) / (ie - 1));
						}
					}
				}
				if (!N) e.moveTo(l + A, u + j);
				else if (w > Su) {
					var ae = xu(g, w), z = xu(_, w), B = wu(O, k, A, j, r, ae, d), V = wu(E, D, ee, M, r, z, d);
					e.moveTo(l + B.cx + B.x0, u + B.cy + B.y0), w < S && ae === z ? e.arc(l + B.cx, u + B.cy, w, _u(B.y0, B.x0), _u(V.y0, V.x0), !d) : (ae > 0 && e.arc(l + B.cx, u + B.cy, ae, _u(B.y0, B.x0), _u(B.y1, B.x1), !d), e.arc(l, u, r, _u(B.cy + B.y1, B.cx + B.x1), _u(V.cy + V.y1, V.cx + V.x1), !d), z > 0 && e.arc(l + V.cx, u + V.cy, z, _u(V.y1, V.x1), _u(V.y0, V.x0), !d));
				} else e.moveTo(l + A, u + j), e.arc(l, u, r, s, c, !d);
				if (!(i > Su) || !N) e.lineTo(l + ee, u + M);
				else if (T > Su) {
					var ae = xu(m, T), z = xu(h, T), B = wu(ee, M, E, D, i, -z, d), V = wu(A, j, O, k, i, -ae, d);
					e.lineTo(l + B.cx + B.x0, u + B.cy + B.y0), T < C && ae === z ? e.arc(l + B.cx, u + B.cy, T, _u(B.y0, B.x0), _u(V.y0, V.x0), !d) : (z > 0 && e.arc(l + B.cx, u + B.cy, z, _u(B.y0, B.x0), _u(B.y1, B.x1), !d), e.arc(l, u, i, _u(B.cy + B.y1, B.cx + B.x1), _u(V.cy + V.y1, V.cx + V.x1), d), ae > 0 && e.arc(l + V.cx, u + V.cy, ae, _u(V.y1, V.x1), _u(V.y0, V.x0), !d));
				} else e.lineTo(l + ee, u + M), e.arc(l, u, i, c, s, d);
			}
			e.closePath();
		}
	}
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/shape/Sector.js
var Du = function() {
	function e() {
		this.cx = 0, this.cy = 0, this.r0 = 0, this.r = 0, this.startAngle = 0, this.endAngle = Math.PI * 2, this.clockwise = !0, this.cornerRadius = 0;
	}
	return e;
}(), Ou = function(e) {
	o(t, e);
	function t(t) {
		return e.call(this, t) || this;
	}
	return t.prototype.getDefaultShape = function() {
		return new Du();
	}, t.prototype.buildPath = function(e, t) {
		Eu(e, t);
	}, t.prototype.isZeroArea = function() {
		return this.shape.startAngle === this.shape.endAngle || this.shape.r === this.shape.r0;
	}, t;
}(Za);
Ou.prototype.type = "sector";
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/shape/Ring.js
var ku = function() {
	function e() {
		this.cx = 0, this.cy = 0, this.r = 0, this.r0 = 0;
	}
	return e;
}(), Au = function(e) {
	o(t, e);
	function t(t) {
		return e.call(this, t) || this;
	}
	return t.prototype.getDefaultShape = function() {
		return new ku();
	}, t.prototype.buildPath = function(e, t) {
		var n = t.cx, r = t.cy, i = Math.PI * 2;
		e.moveTo(n + t.r, r), e.arc(n, r, t.r, 0, i, !1), e.moveTo(n + t.r0, r), e.arc(n, r, t.r0, 0, i, !0);
	}, t;
}(Za);
Au.prototype.type = "ring";
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/helper/smoothBezier.js
function ju(e, t, n, r) {
	var i = [], a = [], o = [], s = [], c, l, u, d;
	if (r) {
		u = [Infinity, Infinity], d = [-Infinity, -Infinity];
		for (var f = 0, p = e.length; f < p; f++) kt(u, u, e[f]), At(d, d, e[f]);
		kt(u, u, r[0]), At(d, d, r[1]);
	}
	for (var f = 0, p = e.length; f < p; f++) {
		var m = e[f];
		if (n) c = e[f ? f - 1 : p - 1], l = e[(f + 1) % p];
		else if (f === 0 || f === p - 1) {
			i.push(ht(e[f]));
			continue;
		} else c = e[f - 1], l = e[f + 1];
		vt(a, l, c), xt(a, a, t);
		var h = Ct(m, c), g = Ct(m, l), _ = h + g;
		_ !== 0 && (h /= _, g /= _), xt(o, a, -h), xt(s, a, g);
		var v = _t([], m, o), y = _t([], m, s);
		r && (At(v, v, u), kt(v, v, d), At(y, y, u), kt(y, y, d)), i.push(v), i.push(y);
	}
	return n && i.push(i.shift()), i;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/helper/poly.js
function Mu(e, t, n) {
	var r = t.smooth, i = t.points;
	if (i && i.length >= 2) {
		if (r) {
			var a = ju(i, r, n, t.smoothConstraint);
			e.moveTo(i[0][0], i[0][1]);
			for (var o = i.length, s = 0; s < (n ? o : o - 1); s++) {
				var c = a[s * 2], l = a[s * 2 + 1], u = i[(s + 1) % o];
				e.bezierCurveTo(c[0], c[1], l[0], l[1], u[0], u[1]);
			}
		} else {
			e.moveTo(i[0][0], i[0][1]);
			for (var s = 1, d = i.length; s < d; s++) e.lineTo(i[s][0], i[s][1]);
		}
		n && e.closePath();
	}
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/shape/Polygon.js
var Nu = function() {
	function e() {
		this.points = null, this.smooth = 0, this.smoothConstraint = null;
	}
	return e;
}(), Pu = function(e) {
	o(t, e);
	function t(t) {
		return e.call(this, t) || this;
	}
	return t.prototype.getDefaultShape = function() {
		return new Nu();
	}, t.prototype.buildPath = function(e, t) {
		Mu(e, t, !0);
	}, t;
}(Za);
Pu.prototype.type = "polygon";
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/shape/Polyline.js
var Fu = function() {
	function e() {
		this.points = null, this.percent = 1, this.smooth = 0, this.smoothConstraint = null;
	}
	return e;
}(), Iu = function(e) {
	o(t, e);
	function t(t) {
		return e.call(this, t) || this;
	}
	return t.prototype.getDefaultStyle = function() {
		return {
			stroke: "#000",
			fill: null
		};
	}, t.prototype.getDefaultShape = function() {
		return new Fu();
	}, t.prototype.buildPath = function(e, t) {
		Mu(e, t, !1);
	}, t;
}(Za);
Iu.prototype.type = "polyline";
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/shape/Line.js
var Lu = {}, Ru = function() {
	function e() {
		this.x1 = 0, this.y1 = 0, this.x2 = 0, this.y2 = 0, this.percent = 1;
	}
	return e;
}(), zu = function(e) {
	o(t, e);
	function t(t) {
		return e.call(this, t) || this;
	}
	return t.prototype.getDefaultStyle = function() {
		return {
			stroke: "#000",
			fill: null
		};
	}, t.prototype.getDefaultShape = function() {
		return new Ru();
	}, t.prototype.buildPath = function(e, t) {
		var n, r, i, a;
		if (this.subPixelOptimize) {
			var o = oo(Lu, t, this.style);
			n = o.x1, r = o.y1, i = o.x2, a = o.y2;
		} else n = t.x1, r = t.y1, i = t.x2, a = t.y2;
		var s = t.percent;
		s !== 0 && (e.moveTo(n, r), s < 1 && (i = n * (1 - s) + i * s, a = r * (1 - s) + a * s), e.lineTo(i, a));
	}, t.prototype.pointAt = function(e) {
		var t = this.shape;
		return [t.x1 * (1 - e) + t.x2 * e, t.y1 * (1 - e) + t.y2 * e];
	}, t;
}(Za);
zu.prototype.type = "line";
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/shape/BezierCurve.js
var Bu = [], Vu = function() {
	function e() {
		this.x1 = 0, this.y1 = 0, this.x2 = 0, this.y2 = 0, this.cpx1 = 0, this.cpy1 = 0, this.percent = 1;
	}
	return e;
}();
function Hu(e, t, n) {
	var r = e.cpx2, i = e.cpy2;
	return r != null || i != null ? [(n ? ar : ir)(e.x1, e.cpx1, e.cpx2, e.x2, t), (n ? ar : ir)(e.y1, e.cpy1, e.cpy2, e.y2, t)] : [(n ? fr : dr)(e.x1, e.cpx1, e.x2, t), (n ? fr : dr)(e.y1, e.cpy1, e.y2, t)];
}
var Uu = function(e) {
	o(t, e);
	function t(t) {
		return e.call(this, t) || this;
	}
	return t.prototype.getDefaultStyle = function() {
		return {
			stroke: "#000",
			fill: null
		};
	}, t.prototype.getDefaultShape = function() {
		return new Vu();
	}, t.prototype.buildPath = function(e, t) {
		var n = t.x1, r = t.y1, i = t.x2, a = t.y2, o = t.cpx1, s = t.cpy1, c = t.cpx2, l = t.cpy2, u = t.percent;
		u !== 0 && (e.moveTo(n, r), c == null || l == null ? (u < 1 && (hr(n, o, i, u, Bu), o = Bu[1], i = Bu[2], hr(r, s, a, u, Bu), s = Bu[1], a = Bu[2]), e.quadraticCurveTo(o, s, i, a)) : (u < 1 && (cr(n, o, c, i, u, Bu), o = Bu[1], c = Bu[2], i = Bu[3], cr(r, s, l, a, u, Bu), s = Bu[1], l = Bu[2], a = Bu[3]), e.bezierCurveTo(o, s, c, l, i, a)));
	}, t.prototype.pointAt = function(e) {
		return Hu(this.shape, e, !1);
	}, t.prototype.tangentAt = function(e) {
		var t = Hu(this.shape, e, !0);
		return St(t, t);
	}, t;
}(Za);
Uu.prototype.type = "bezier-curve";
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/shape/Arc.js
var Wu = function() {
	function e() {
		this.cx = 0, this.cy = 0, this.r = 0, this.startAngle = 0, this.endAngle = Math.PI * 2, this.clockwise = !0;
	}
	return e;
}(), Gu = function(e) {
	o(t, e);
	function t(t) {
		return e.call(this, t) || this;
	}
	return t.prototype.getDefaultStyle = function() {
		return {
			stroke: "#000",
			fill: null
		};
	}, t.prototype.getDefaultShape = function() {
		return new Wu();
	}, t.prototype.buildPath = function(e, t) {
		var n = t.cx, r = t.cy, i = Math.max(t.r, 0), a = t.startAngle, o = t.endAngle, s = t.clockwise, c = Math.cos(a), l = Math.sin(a);
		e.moveTo(c * i + n, l * i + r), e.arc(n, r, i, a, o, !s);
	}, t;
}(Za);
Gu.prototype.type = "arc";
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/graphic/CompoundPath.js
var Ku = function(e) {
	o(t, e);
	function t() {
		var t = e !== null && e.apply(this, arguments) || this;
		return t.type = "compound", t;
	}
	return t.prototype._updatePathDirty = function() {
		for (var e = this.shape.paths, t = this.shapeChanged(), n = 0; n < e.length; n++) t ||= e[n].shapeChanged();
		t && this.dirtyShape();
	}, t.prototype.beforeBrush = function() {
		this._updatePathDirty();
		for (var e = this.shape.paths || [], t = this.getGlobalScale(), n = 0; n < e.length; n++) e[n].path || e[n].createPathProxy(), e[n].path.setScale(t[0], t[1], e[n].segmentIgnoreThreshold);
	}, t.prototype.buildPath = function(e, t) {
		for (var n = t.paths || [], r = 0; r < n.length; r++) n[r].buildPath(e, n[r].shape, !0);
	}, t.prototype.afterBrush = function() {
		for (var e = this.shape.paths || [], t = 0; t < e.length; t++) e[t].pathUpdated();
	}, t.prototype.getBoundingRect = function() {
		return this._updatePathDirty.call(this), Za.prototype.getBoundingRect.call(this);
	}, t;
}(Za), qu = function() {
	function e(e) {
		this.colorStops = e || [];
	}
	return e.prototype.addColorStop = function(e, t) {
		this.colorStops.push({
			offset: e,
			color: t
		});
	}, e;
}(), Ju = function(e) {
	o(t, e);
	function t(t, n, r, i, a, o) {
		var s = e.call(this, a) || this;
		return s.x = t ?? 0, s.y = n ?? 0, s.x2 = r ?? 1, s.y2 = i ?? 0, s.type = "linear", s.global = o || !1, s;
	}
	return t;
}(qu), Yu = function(e) {
	o(t, e);
	function t(t, n, r, i, a) {
		var o = e.call(this, i) || this;
		return o.x = t ?? .5, o.y = n ?? .5, o.r = r ?? .5, o.type = "radial", o.global = a || !1, o;
	}
	return t;
}(qu), Xu = Math.min, Zu = Math.max, Qu = Math.abs, $u = [0, 0], ed = [0, 0], td = Zt(), nd = td.minTv, rd = td.maxTv, id = function() {
	function e(e, t) {
		this._corners = [], this._axes = [], this._origin = [0, 0];
		for (var n = 0; n < 4; n++) this._corners[n] = new J();
		for (var n = 0; n < 2; n++) this._axes[n] = new J();
		e && this.fromBoundingRect(e, t);
	}
	return e.prototype.fromBoundingRect = function(e, t) {
		var n = this._corners, r = this._axes, i = e.x, a = e.y, o = i + e.width, s = a + e.height;
		if (n[0].set(i, a), n[1].set(o, a), n[2].set(o, s), n[3].set(i, s), t) for (var c = 0; c < 4; c++) n[c].transform(t);
		J.sub(r[0], n[1], n[0]), J.sub(r[1], n[3], n[0]), r[0].normalize(), r[1].normalize();
		for (var c = 0; c < 2; c++) this._origin[c] = r[c].dot(n[0]);
	}, e.prototype.intersect = function(e, t, n) {
		var r = !0, i = !t;
		return t && J.set(t, 0, 0), td.reset(n, !i), !this._intersectCheckOneSide(this, e, i, 1) && (r = !1, i) || !this._intersectCheckOneSide(e, this, i, -1) && (r = !1, i) || !i && !td.negativeSize && J.copy(t, r ? td.useDir ? td.dirMinTv : nd : rd), r;
	}, e.prototype._intersectCheckOneSide = function(e, t, n, r) {
		for (var i = !0, a = 0; a < 2; a++) {
			var o = e._axes[a];
			if (e._getProjMinMaxOnAxis(a, e._corners, $u), e._getProjMinMaxOnAxis(a, t._corners, ed), td.negativeSize || $u[1] < ed[0] || $u[0] > ed[1]) {
				if (i = !1, td.negativeSize || n) return i;
				var s = Qu(ed[0] - $u[1]), c = Qu($u[0] - ed[1]);
				Xu(s, c) > rd.len() && (s < c ? J.scale(rd, o, -s * r) : J.scale(rd, o, c * r));
			} else if (!n) {
				var s = Qu(ed[0] - $u[1]), c = Qu($u[0] - ed[1]);
				(td.useDir || Xu(s, c) < nd.len()) && ((s < c || !td.bidirectional) && (J.scale(nd, o, s * r), td.useDir && td.calcDirMTV()), (s >= c || !td.bidirectional) && (J.scale(nd, o, -c * r), td.useDir && td.calcDirMTV()));
			}
		}
		return i;
	}, e.prototype._getProjMinMaxOnAxis = function(e, t, n) {
		for (var r = this._axes[e], i = this._origin, a = t[0].dot(r) + i[e], o = a, s = a, c = 1; c < t.length; c++) {
			var l = t[c].dot(r) + i[e];
			o = Xu(l, o), s = Zu(l, s);
		}
		n[0] = o + td.touchThreshold, n[1] = s - td.touchThreshold, td.negativeSize = n[1] < n[0];
	}, e;
}(), ad = [], od = function(e) {
	o(t, e);
	function t() {
		var t = e !== null && e.apply(this, arguments) || this;
		return t.notClear = !0, t.incremental = 1, t._displayables = [], t._temporaryDisplayables = [], t._cursor = 0, t;
	}
	return t.prototype.traverse = function(e, t) {
		e.call(t, this);
	}, t.prototype.useStyle = function() {
		this.style = {};
	}, t.prototype._useHoverStyle = function() {
		this.__hoverStyle = null;
	}, t.prototype.getCursor = function() {
		return this._cursor;
	}, t.prototype.innerAfterBrush = function() {
		this._cursor = this._displayables.length;
	}, t.prototype.clearDisplaybles = function() {
		this._displayables = [], this._temporaryDisplayables = [], this._cursor = 0, this.markRedraw(), this.notClear = !1;
	}, t.prototype.clearTemporalDisplayables = function() {
		this._temporaryDisplayables = [];
	}, t.prototype.addDisplayable = function(e, t) {
		t ? this._temporaryDisplayables.push(e) : this._displayables.push(e), this.markRedraw();
	}, t.prototype.addDisplayables = function(e, t) {
		t ||= !1;
		for (var n = 0; n < e.length; n++) this.addDisplayable(e[n], t);
	}, t.prototype.getDisplayables = function() {
		return this._displayables;
	}, t.prototype.getTemporalDisplayables = function() {
		return this._temporaryDisplayables;
	}, t.prototype.eachPendingDisplayable = function(e) {
		for (var t = this._cursor; t < this._displayables.length; t++) e && e(this._displayables[t]);
		for (var t = 0; t < this._temporaryDisplayables.length; t++) e && e(this._temporaryDisplayables[t]);
	}, t.prototype.update = function() {
		this.updateTransform();
		for (var e = this._cursor; e < this._displayables.length; e++) {
			var t = this._displayables[e];
			t.parent = this, t.update(), t.parent = null;
		}
		for (var e = 0; e < this._temporaryDisplayables.length; e++) {
			var t = this._temporaryDisplayables[e];
			t.parent = this, t.update(), t.parent = null;
		}
	}, t.prototype.getBoundingRect = function() {
		if (!this._rect) {
			for (var e = new Y(Infinity, Infinity, -Infinity, -Infinity), t = 0; t < this._displayables.length; t++) {
				var n = this._displayables[t], r = n.getBoundingRect().clone();
				n.needLocalTransform() && r.applyTransform(n.getLocalTransform(ad)), e.union(r);
			}
			this._rect = e;
		}
		return this._rect;
	}, t.prototype.contain = function(e, t) {
		var n = this.transformCoordToLocal(e, t);
		if (this.getBoundingRect().contain(n[0], n[1])) {
			for (var r = 0; r < this._displayables.length; r++) if (this._displayables[r].contain(e, t)) return !0;
		}
		return !1;
	}, t;
}(Wi), sd = Ws();
function cd(e, t, n, r, i) {
	var a;
	if (t && t.ecModel) {
		var o = t.ecModel.getUpdatePayload();
		a = o && o.animation;
	}
	var s = t && t.isAnimationEnabled(), c = e === "update";
	if (s) {
		var l = void 0, u = void 0, d = void 0;
		return r ? (l = G(r.duration, 200), u = G(r.easing, "cubicOut"), d = 0) : (l = t.getShallow(c ? "animationDurationUpdate" : "animationDuration"), u = t.getShallow(c ? "animationEasingUpdate" : "animationEasing"), d = t.getShallow(c ? "animationDelayUpdate" : "animationDelay")), a && (a.duration != null && (l = a.duration), a.easing != null && (u = a.easing), a.delay != null && (d = a.delay)), H(d) && (d = d(n, i)), H(l) && (l = l(n)), {
			duration: l || 0,
			delay: d,
			easing: u
		};
	} else return null;
}
function ld(e, t, n, r, i, a, o) {
	var s = !1, c;
	H(i) ? (o = a, a = i, i = null) : W(i) && (a = i.cb, o = i.during, s = i.isFrom, c = i.removeOpt, i = i.dataIndex);
	var l = e === "leave";
	l || t.stopAnimation("leave");
	var u = cd(e, r, i, l ? c || {} : null, r && r.getAnimationDelayParams ? r.getAnimationDelayParams(t, i) : null);
	if (u && u.duration > 0) {
		var d = u.duration, f = u.delay, p = u.easing, m = {
			duration: d,
			delay: f || 0,
			easing: p,
			done: a,
			force: !!a || !!o,
			setToFinal: !l,
			scope: e,
			during: o
		};
		s ? t.animateFrom(n, m) : t.animateTo(n, m);
	} else t.stopAnimation(), !s && t.attr(n), o && o(1), a && a();
}
function ud(e, t, n, r, i, a) {
	ld("update", e, t, n, r, i, a);
}
function dd(e, t, n, r, i, a) {
	ld("enter", e, t, n, r, i, a);
}
function fd(e) {
	if (!e.__zr) return !0;
	for (var t = 0; t < e.animators.length; t++) if (e.animators[t].scope === "leave") return !0;
	return !1;
}
function pd(e, t, n, r, i, a) {
	fd(e) || ld("leave", e, t, n, r, i, a);
}
function md(e, t, n, r) {
	e.removeTextContent(), e.removeTextGuideLine(), pd(e, { style: { opacity: 0 } }, t, n, r);
}
function hd(e, t, n) {
	function r() {
		e.parent && e.parent.remove(e);
	}
	e.isGroup ? e.traverse(function(e) {
		e.isGroup || md(e, t, n, r);
	}) : md(e, t, n, r);
}
function gd(e) {
	sd(e).oldStyle = e.style;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/util/graphic.js
var _d = /* @__PURE__ */ i({
	Arc: () => Gu,
	BezierCurve: () => Uu,
	BoundingRect: () => Y,
	Circle: () => lu,
	CompoundPath: () => Ku,
	Ellipse: () => du,
	Group: () => su,
	HOVER_LAYER_FOR_INCREMENTAL: () => 2,
	HOVER_LAYER_FROM_THRESHOLD: () => 1,
	HOVER_LAYER_NO: () => 0,
	Image: () => ro,
	IncrementalDisplayable: () => od,
	Line: () => zu,
	LinearGradient: () => Ju,
	OrientedBoundingRect: () => id,
	Path: () => Za,
	Point: () => J,
	Polygon: () => Pu,
	Polyline: () => Iu,
	RadialGradient: () => Yu,
	Rect: () => fo,
	Ring: () => Au,
	Sector: () => Ou,
	Text: () => _o,
	WH: () => bd,
	XY: () => yd,
	applyTransform: () => Fd,
	calcZ2Range: () => af,
	clipPointsByRect: () => Bd,
	clipRectByRect: () => Vd,
	createIcon: () => Hd,
	decomposeTransform: () => lf,
	ensureCopyRect: () => tf,
	ensureCopyTransform: () => nf,
	expandOrShrinkRect: () => qd,
	extendPath: () => Cd,
	extendShape: () => xd,
	getCurrentCanvasPainter: () => df,
	getShapeClass: () => Td,
	getTransform: () => Pd,
	groupTransition: () => zd,
	initProps: () => dd,
	isBoundingRectAxisAligned: () => $d,
	isElementRemoved: () => fd,
	lineLineIntersect: () => Wd,
	linePolygonIntersect: () => Ud,
	makeImage: () => Dd,
	makePath: () => Ed,
	mergePath: () => kd,
	payloadDisableAnimation: () => cf,
	registerShape: () => wd,
	removeElement: () => pd,
	removeElementWithFadeOut: () => hd,
	resizePath: () => Ad,
	retrieveZInfo: () => rf,
	setTooltipConfig: () => Xd,
	subPixelOptimize: () => Nd,
	subPixelOptimizeLine: () => jd,
	subPixelOptimizeRect: () => Md,
	transformDirection: () => Id,
	traverseElements: () => Qd,
	traverseUpdateZ: () => of,
	updateProps: () => ud
}), vd = {}, yd = ["x", "y"], bd = ["width", "height"];
function xd(e) {
	return Za.extend(e);
}
var Sd = au;
function Cd(e, t) {
	return Sd(e, t);
}
function wd(e, t) {
	vd[e] = t;
}
function Td(e) {
	if (vd.hasOwnProperty(e)) return vd[e];
}
function Ed(e, t, n, r) {
	var i = iu(e, t);
	return n && (r === "center" && (n = Od(n, i.getBoundingRect())), Ad(i, n)), i;
}
function Dd(e, t, n) {
	var r = new ro({
		style: {
			image: e,
			x: t.x,
			y: t.y,
			width: t.width,
			height: t.height
		},
		onload: function(e) {
			if (n === "center") {
				var i = {
					width: e.width,
					height: e.height
				};
				r.setStyle(Od(t, i));
			}
		}
	});
	return r;
}
function Od(e, t) {
	var n = t.width / t.height, r = e.height * n, i;
	r <= e.width ? i = e.height : (r = e.width, i = r / n);
	var a = e.x + e.width / 2, o = e.y + e.height / 2;
	return {
		x: a - r / 2,
		y: o - i / 2,
		width: r,
		height: i
	};
}
var kd = ou;
function Ad(e, t) {
	if (e.applyTransform) {
		var n = e.getBoundingRect().calculateTransform(t);
		e.applyTransform(n);
	}
}
function jd(e, t) {
	return oo(e, e, { lineWidth: t }), e;
}
function Md(e, t) {
	return so(e, e, t), e;
}
var Nd = co;
function Pd(e, t) {
	for (var n = st([]); e && e !== t;) lt(n, e.getLocalTransform(), n), e = e.parent;
	return n;
}
function Fd(e, t, n) {
	return t && !F(t) && (t = Hn.getLocalTransform(t)), n && (t = pt([], t)), Ot([], e, t);
}
function Id(e, t, n) {
	var r = t[4] === 0 || t[5] === 0 || t[0] === 0 ? 1 : Io(2 * t[4] / t[0]), i = t[4] === 0 || t[5] === 0 || t[2] === 0 ? 1 : Io(2 * t[4] / t[2]), a = [e === "left" ? -r : e === "right" ? r : 0, e === "top" ? -i : e === "bottom" ? i : 0];
	return a = Fd(a, t, n), Io(a[0]) > Io(a[1]) ? a[0] > 0 ? "right" : "left" : a[1] > 0 ? "bottom" : "top";
}
function Ld(e) {
	return !e.isGroup;
}
function Rd(e) {
	return e.shape != null;
}
function zd(e, t, n) {
	if (!e || !t) return;
	function r(e) {
		var t = {};
		return e.traverse(function(e) {
			Ld(e) && e.anid && (t[e.anid] = e);
		}), t;
	}
	function i(e) {
		var t = {
			x: e.x,
			y: e.y,
			rotation: e.rotation
		};
		return Rd(e) && (t.shape = k(e.shape)), t;
	}
	var a = r(e);
	t.traverse(function(e) {
		if (Ld(e) && e.anid) {
			var t = a[e.anid];
			if (t) {
				var r = i(e);
				e.attr(i(t)), ud(e, r, n, yc(e).dataIndex);
			}
		}
	});
}
function Bd(e, t) {
	return L(e, function(e) {
		var n = e[0];
		n = Fo(n, t.x), n = Po(n, t.x + t.width);
		var r = e[1];
		return r = Fo(r, t.y), r = Po(r, t.y + t.height), [n, r];
	});
}
function Vd(e, t) {
	var n = Fo(e.x, t.x), r = Po(e.x + e.width, t.x + t.width), i = Fo(e.y, t.y), a = Po(e.y + e.height, t.y + t.height);
	if (r >= n && a >= i) return {
		x: n,
		y: i,
		width: r - n,
		height: a - i
	};
}
function Hd(e, t, n) {
	var r = j({ rectHover: !0 }, t), i = r.style = { strokeNoScale: !0 };
	if (n ||= {
		x: -1,
		y: -1,
		width: 2,
		height: 2
	}, e) return e.indexOf("image://") === 0 ? (i.image = e.slice(8), M(i, n), new ro(r)) : Ed(e.replace("path://", ""), r, n, "center");
}
function Ud(e, t, n, r, i) {
	for (var a = 0, o = i[i.length - 1]; a < i.length; a++) {
		var s = i[a];
		if (Wd(e, t, n, r, s[0], s[1], o[0], o[1])) return !0;
		o = s;
	}
}
function Wd(e, t, n, r, i, a, o, s) {
	var c = n - e, l = r - t, u = o - i, d = s - a, f = Gd(u, d, c, l);
	if (Kd(f)) return !1;
	var p = e - i, m = t - a, h = Gd(p, m, c, l) / f;
	if (h < 0 || h > 1) return !1;
	var g = Gd(p, m, u, d) / f;
	return !(g < 0 || g > 1);
}
function Gd(e, t, n, r) {
	return e * r - n * t;
}
function Kd(e) {
	return e <= 1e-6 && e >= -1e-6;
}
function qd(e, t, n, r, i) {
	return t == null ? e : (se(t) ? Jd[0] = Jd[1] = Jd[2] = Jd[3] = t : (Jd[0] = t[0], Jd[1] = t[1], Jd[2] = t[2], Jd[3] = t[3]), r && (Jd[0] = Fo(0, Jd[0]), Jd[1] = Fo(0, Jd[1]), Jd[2] = Fo(0, Jd[2]), Jd[3] = Fo(0, Jd[3])), n && (Jd[0] = -Jd[0], Jd[1] = -Jd[1], Jd[2] = -Jd[2], Jd[3] = -Jd[3]), Yd(e, Jd, "x", "width", 3, 1, i && i[0] || 0), Yd(e, Jd, "y", "height", 0, 2, i && i[1] || 0), e);
}
var Jd = [
	0,
	0,
	0,
	0
];
function Yd(e, t, n, r, i, a, o) {
	var s = t[a] + t[i], c = e[r];
	e[r] += s, o = Fo(0, Po(o, c)), e[r] < o ? (e[r] = o, e[n] += t[i] >= 0 ? -t[i] : t[a] >= 0 ? c + t[a] : Io(s) > 1e-8 ? (c - o) * t[i] / s : 0) : e[n] -= t[i];
}
function Xd(e) {
	var t = e.itemTooltipOption, n = e.componentModel, r = e.itemName, i = U(t) ? { formatter: t } : t, a = n.mainType, o = n.componentIndex, s = {
		componentType: a,
		name: r,
		$vars: ["name"]
	};
	s[a + "Index"] = o;
	var c = e.formatterParamsExtra;
	c && I(R(c), function(e) {
		Ae(s, e) || (s[e] = c[e], s.$vars.push(e));
	});
	var l = yc(e.el);
	l.componentMainType = a, l.componentIndex = o, l.tooltipConfig = {
		name: r,
		option: M({
			content: r,
			encodeHTMLContent: !0,
			formatterParams: s
		}, i)
	};
}
function Zd(e, t) {
	var n;
	e.isGroup && (n = t(e)), n || e.traverse(t);
}
function Qd(e, t) {
	if (e) if (V(e)) for (var n = 0; n < e.length; n++) Zd(e[n], t);
	else Zd(e, t);
}
function $d(e) {
	return !e || Io(e[1]) < ef && Io(e[2]) < ef || Io(e[0]) < ef && Io(e[3]) < ef;
}
var ef = 1e-5;
function tf(e, t) {
	return e ? Y.copy(e, t) : t.clone();
}
function nf(e, t) {
	return t ? ct(e || ot(), t) : void 0;
}
function rf(e) {
	return {
		z: e.get("z") || 0,
		zlevel: e.get("zlevel") || 0
	};
}
function af(e) {
	var t = -Infinity, n = Infinity;
	Zd(e, function(e) {
		r(e), r(e.getTextContent()), r(e.getTextGuideLine());
	});
	function r(e) {
		if (!(!e || e.isGroup)) {
			var t = e.currentStates;
			if (t.length) for (var n = 0; n < t.length; n++) i(e.states[t[n]]);
			i(e);
		}
	}
	function i(e) {
		if (e) {
			var r = e.z2;
			r > t && (t = r), r < n && (n = r);
		}
	}
	return n > t && (n = t = 0), {
		min: n,
		max: t
	};
}
function of(e, t, n) {
	sf(e, t, n, -Infinity);
}
function sf(e, t, n, r) {
	if (e.ignoreModelZ) return r;
	var i = e.getTextContent(), a = e.getTextGuideLine();
	if (e.isGroup) for (var o = e.childrenRef(), s = 0; s < o.length; s++) r = Fo(sf(o[s], t, n, r), r);
	else e.z = t, e.zlevel = n, r = Fo(e.z2 || 0, r);
	if (i && (i.z = t, i.zlevel = n, isFinite(r) && (i.z2 = r + 2)), a) {
		var c = e.textGuideLineConfig;
		a.z = t, a.zlevel = n, isFinite(r) && (a.z2 = r + (c && c.showAbove ? 1 : -1));
	}
	return r;
}
function cf(e) {
	return e.animation = { duration: 0 }, e;
}
function lf(e, t) {
	return t ? ct(uf.transform, t) : st(uf.transform), uf.decomposeTransform(), Gn(e, uf), e;
}
var uf = new Hn();
uf.transform = ot();
function df(e) {
	var t = e.getZr().painter;
	return t.getType() === "canvas" ? t : null;
}
wd("circle", lu), wd("ellipse", du), wd("sector", Ou), wd("ring", Au), wd("polygon", Pu), wd("polyline", Iu), wd("rect", fo), wd("line", zu), wd("bezierCurve", Uu), wd("arc", Gu);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/label/labelStyle.js
var ff = {};
function pf(e, t) {
	for (var n = 0; n < Ic.length; n++) {
		var r = Ic[n], i = t[r], a = e.ensureState(r);
		a.style = a.style || {}, a.style.text = i;
	}
	var o = e.currentStates.slice();
	e.clearStates(!0), e.setStyle({ text: t.normal }), e.useStates(o, !0);
}
function mf(e, t, n) {
	var r = e.labelFetcher, i = e.labelDataIndex, a = e.labelDimIndex, o = t.normal, s;
	r && (s = r.getFormattedLabel(i, "normal", null, a, o && o.get("formatter"), n == null ? null : { interpolatedValue: n })), s ??= H(e.defaultText) ? e.defaultText(i, e, n) : e.defaultText;
	for (var c = { normal: s }, l = 0; l < Ic.length; l++) {
		var u = Ic[l], d = t[u];
		c[u] = G(r ? r.getFormattedLabel(i, u, null, a, d && d.get("formatter")) : null, s);
	}
	return c;
}
function hf(e, t, n, r) {
	n ||= ff;
	for (var i = e instanceof _o, a = !1, o = 0; o < Lc.length; o++) {
		var s = t[Lc[o]];
		if (s && s.getShallow("show")) {
			a = !0;
			break;
		}
	}
	var c = i ? e : e.getTextContent();
	if (a) {
		i || (c || (c = new _o(), e.setTextContent(c)), e.stateProxy && (c.stateProxy = e.stateProxy));
		var l = mf(n, t), u = t.normal, d = !!u.getShallow("show"), f = _f(u, r && r.normal, n, !1, !i);
		f.text = l.normal, i || e.setTextConfig(vf(u, n, !1));
		for (var o = 0; o < Ic.length; o++) {
			var p = Ic[o], s = t[p];
			if (s) {
				var m = c.ensureState(p), h = !!G(s.getShallow("show"), d);
				if (h !== d && (m.ignore = !h), m.style = _f(s, r && r[p], n, !0, !i), m.style.text = l[p], !i) {
					var g = e.ensureState(p);
					g.textConfig = vf(s, n, !0);
				}
			}
		}
		c.silent = !!u.getShallow("silent"), c.style.x != null && (f.x = c.style.x), c.style.y != null && (f.y = c.style.y), c.ignore = !d, c.useStyle(f), c.dirty(), n.enableTextSetter && (Ef(c).setLabelText = function(e) {
			var r = mf(n, t, e);
			pf(c, r);
		});
	} else c && (c.ignore = !0);
	e.dirty();
}
function gf(e, t) {
	t ||= "label";
	for (var n = { normal: e.getModel(t) }, r = 0; r < Ic.length; r++) {
		var i = Ic[r];
		n[i] = e.getModel([i, t]);
	}
	return n;
}
function _f(e, t, n, r, i) {
	var a = {};
	return yf(a, e, n, r, i), t && j(a, t), a;
}
function vf(e, t, n) {
	t ||= {};
	var r = {}, i, a = e.getShallow("rotate"), o = G(e.getShallow("distance"), n ? null : 5), s = e.getShallow("offset");
	return i = e.getShallow("position") || (n ? null : "inside"), i === "outside" && (i = t.defaultOutsidePosition || "top"), i != null && (r.position = i), s != null && (r.offset = s), a != null && (a *= Math.PI / 180, r.rotation = a), o != null && (r.distance = o), r.outsideFill = e.get("color") === "inherit" ? t.inheritColor || null : "auto", t.autoOverflowArea != null && (r.autoOverflowArea = t.autoOverflowArea), t.layoutRect != null && (r.layoutRect = t.layoutRect), r;
}
function yf(e, t, n, r, i) {
	n ||= ff;
	var a = t.ecModel, o = a && a.option.textStyle, s = bf(t), c;
	if (s) {
		c = {};
		var l = "richInheritPlainLabel", u = G(t.get(l), a ? a.get(l) : void 0);
		for (var d in s) if (s.hasOwnProperty(d)) {
			var f = t.getModel(["rich", d]);
			wf(c[d] = {}, f, o, t, u, n, r, i, !1, !0);
		}
	}
	c && (e.rich = c);
	var p = t.get("overflow");
	p && (e.overflow = p);
	var m = t.get("lineOverflow");
	m && (e.lineOverflow = m);
	var h = e, g = t.get("minMargin");
	if (g != null) g = se(g) ? g / 2 : 0, h.margin = [
		g,
		g,
		g,
		g
	], h.__marginType = kf.minMargin;
	else {
		var _ = t.get("textMargin");
		_ != null && (h.margin = _e(_), h.__marginType = kf.textMargin);
	}
	wf(e, t, o, null, null, n, r, i, !0, !1);
}
function bf(e) {
	for (var t; e && e !== e.ecModel;) {
		var n = (e.option || ff).rich;
		if (n) {
			t ||= {};
			for (var r = R(n), i = 0; i < r.length; i++) {
				var a = r[i];
				t[a] = 1;
			}
		}
		e = e.parentModel;
	}
	return t;
}
var xf = [
	"fontStyle",
	"fontWeight",
	"fontSize",
	"fontFamily",
	"textShadowColor",
	"textShadowBlur",
	"textShadowOffsetX",
	"textShadowOffsetY"
], Sf = [
	"align",
	"lineHeight",
	"width",
	"height",
	"tag",
	"verticalAlign",
	"ellipsis"
], Cf = [
	"padding",
	"borderWidth",
	"borderRadius",
	"borderDashOffset",
	"backgroundColor",
	"borderColor",
	"shadowColor",
	"shadowBlur",
	"shadowOffsetX",
	"shadowOffsetY"
];
function wf(e, t, n, r, i, a, o, s, c, l) {
	n = !o && n || ff;
	var u = a && a.inheritColor, d = t.getShallow("color"), f = t.getShallow("textBorderColor"), p = G(t.getShallow("opacity"), n.opacity);
	(d === "inherit" || d === "auto") && (d = u || null), (f === "inherit" || f === "auto") && (f = u || null), s || (d ||= n.color, f ||= n.textBorderColor), d != null && (e.fill = d), f != null && (e.stroke = f);
	var m = G(t.getShallow("textBorderWidth"), n.textBorderWidth);
	m != null && (e.lineWidth = m);
	var h = G(t.getShallow("textBorderType"), n.textBorderType);
	h != null && (e.lineDash = h);
	var g = G(t.getShallow("textBorderDashOffset"), n.textBorderDashOffset);
	g != null && (e.lineDashOffset = g), !o && p == null && !l && (p = a && a.defaultOpacity), p != null && (e.opacity = p), !o && !s && e.fill == null && a.inheritColor && (e.fill = a.inheritColor);
	for (var _ = 0; _ < xf.length; _++) {
		var v = xf[_], y = i !== !1 && r ? he(t.getShallow(v), r.getShallow(v), n[v]) : G(t.getShallow(v), n[v]);
		y != null && (e[v] = y);
	}
	for (var _ = 0; _ < Sf.length; _++) {
		var v = Sf[_], y = t.getShallow(v);
		y != null && (e[v] = y);
	}
	if (e.verticalAlign == null) {
		var b = t.getShallow("baseline");
		b != null && (e.verticalAlign = b);
	}
	if (!c || !a.disableBox) {
		for (var _ = 0; _ < Cf.length; _++) {
			var v = Cf[_], y = t.getShallow(v);
			y != null && (e[v] = y);
		}
		var x = t.getShallow("borderType");
		x != null && (e.borderDash = x), (e.backgroundColor === "auto" || e.backgroundColor === "inherit") && u && (e.backgroundColor = u), (e.borderColor === "auto" || e.borderColor === "inherit") && u && (e.borderColor = u);
	}
}
function Tf(e, t) {
	var n = t && t.getModel("textStyle");
	return ye([
		e.fontStyle || n && n.getShallow("fontStyle") || "",
		e.fontWeight || n && n.getShallow("fontWeight") || "",
		(e.fontSize || n && n.getShallow("fontSize") || 12) + "px",
		e.fontFamily || n && n.getShallow("fontFamily") || "sans-serif"
	].join(" "));
}
var Ef = Ws();
function Df(e, t, n, r) {
	if (e) {
		var i = Ef(e);
		i.prevValue = i.value, i.value = n;
		var a = t.normal;
		i.valueAnimation = a.get("valueAnimation"), i.valueAnimation && (i.precision = a.get("precision"), i.defaultInterpolatedText = r, i.statesModels = t);
	}
}
function Of(e, t, n, r, i) {
	var a = Ef(e);
	if (!a.valueAnimation || a.prevValue === a.value) return;
	var o = a.defaultInterpolatedText, s = G(a.interpolatedValue, a.prevValue), c = a.value;
	function l(r) {
		var l = ec(n, a.precision, s, c, r);
		a.interpolatedValue = r === 1 ? null : l, pf(e, mf({
			labelDataIndex: t,
			labelFetcher: i,
			defaultText: o ? o(l) : l + ""
		}, a.statesModels, l));
	}
	e.percent = 0, (a.prevValue == null ? dd : ud)(e, { percent: 1 }, r, t, null, l);
}
var kf = {
	minMargin: 1,
	textMargin: 2
}, Af = ["textStyle", "color"], jf = [
	"fontStyle",
	"fontWeight",
	"fontSize",
	"fontFamily",
	"padding",
	"lineHeight",
	"rich",
	"width",
	"height",
	"overflow"
], Mf = new _o(), Nf = function() {
	function e() {}
	return e.prototype.getTextColor = function(e) {
		var t = this.ecModel;
		return this.getShallow("color") || (!e && t ? t.get(Af) : null);
	}, e.prototype.getFont = function() {
		return Tf({
			fontStyle: this.getShallow("fontStyle"),
			fontWeight: this.getShallow("fontWeight"),
			fontSize: this.getShallow("fontSize"),
			fontFamily: this.getShallow("fontFamily")
		}, this.ecModel);
	}, e.prototype.getTextRect = function(e) {
		for (var t = {
			text: e,
			verticalAlign: this.getShallow("verticalAlign") || this.getShallow("baseline")
		}, n = 0; n < jf.length; n++) t[jf[n]] = this.getShallow(jf[n]);
		return Mf.useStyle(t), Mf.update(), Mf.getBoundingRect();
	}, e;
}(), Pf = [
	["lineWidth", "width"],
	["stroke", "color"],
	["opacity"],
	["shadowBlur"],
	["shadowOffsetX"],
	["shadowOffsetY"],
	["shadowColor"],
	["lineDash", "type"],
	["lineDashOffset", "dashOffset"],
	["lineCap", "cap"],
	["lineJoin", "join"],
	["miterLimit"]
], Ff = Ye(Pf), If = function() {
	function e() {}
	return e.prototype.getLineStyle = function(e) {
		return Ff(this, e);
	}, e;
}(), Lf = [
	["fill", "color"],
	["stroke", "borderColor"],
	["lineWidth", "borderWidth"],
	["opacity"],
	["shadowBlur"],
	["shadowOffsetX"],
	["shadowOffsetY"],
	["shadowColor"],
	["lineDash", "borderType"],
	["lineDashOffset", "borderDashOffset"],
	["lineCap", "borderCap"],
	["lineJoin", "borderJoin"],
	["miterLimit", "borderMiterLimit"]
], Rf = Ye(Lf), zf = function() {
	function e() {}
	return e.prototype.getItemStyle = function(e, t) {
		return Rf(this, e, t);
	}, e;
}(), Bf = function() {
	function e(e, t, n) {
		this.parentModel = t, this.ecModel = n, this.option = e;
	}
	return e.prototype.init = function(e, t, n) {}, e.prototype.mergeOption = function(e, t) {
		A(this.option, e, !0);
	}, e.prototype.get = function(e, t) {
		return e == null ? this.option : this._doGet(this.parsePath(e), !t && this.parentModel);
	}, e.prototype.getShallow = function(e, t) {
		var n = this.option, r = n == null ? n : n[e];
		if (r == null && !t) {
			var i = this.parentModel;
			i && (r = i.getShallow(e));
		}
		return r;
	}, e.prototype.getModel = function(t, n) {
		var r = t != null, i = r ? this.parsePath(t) : null, a = r ? this._doGet(i) : this.option;
		return n ||= this.parentModel && this.parentModel.getModel(this.resolveParentPath(i)), new e(a, n, this.ecModel);
	}, e.prototype.isEmpty = function() {
		return this.option == null;
	}, e.prototype.restoreData = function() {}, e.prototype.clone = function() {
		var e = this.constructor;
		return new e(k(this.option));
	}, e.prototype.parsePath = function(e) {
		return typeof e == "string" ? e.split(".") : e;
	}, e.prototype.resolveParentPath = function(e) {
		return e;
	}, e.prototype.isAnimationEnabled = function() {
		if (!q.node && this.option) {
			if (this.option.animation != null) return !!this.option.animation;
			if (this.parentModel) return this.parentModel.isAnimationEnabled();
		}
	}, e.prototype._doGet = function(e, t) {
		var n = this.option;
		if (!e) return n;
		for (var r = 0; r < e.length && !(e[r] && (n = n && typeof n == "object" ? n[e[r]] : null, n == null)); r++);
		return n == null && t && (n = t._doGet(this.resolveParentPath(e), t.parentModel)), n;
	}, e;
}();
Ve(Bf), Ge(Bf), P(Bf, If), P(Bf, zf), P(Bf, Ze), P(Bf, Nf);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/data/DataDiffer.js
function Vf(e) {
	return e == null ? 0 : e.length || 1;
}
function Hf(e) {
	return e;
}
var Uf = function() {
	function e(e, t, n, r, i, a) {
		this._old = e, this._new = t, this._oldKeyGetter = n || Hf, this._newKeyGetter = r || Hf, this.context = i, this._diffModeMultiple = a === "multiple";
	}
	return e.prototype.add = function(e) {
		return this._add = e, this;
	}, e.prototype.update = function(e) {
		return this._update = e, this;
	}, e.prototype.updateManyToOne = function(e) {
		return this._updateManyToOne = e, this;
	}, e.prototype.updateOneToMany = function(e) {
		return this._updateOneToMany = e, this;
	}, e.prototype.updateManyToMany = function(e) {
		return this._updateManyToMany = e, this;
	}, e.prototype.remove = function(e) {
		return this._remove = e, this;
	}, e.prototype.execute = function() {
		this[this._diffModeMultiple ? "_executeMultiple" : "_executeOneToOne"]();
	}, e.prototype._executeOneToOne = function() {
		var e = this._old, t = this._new, n = {}, r = Array(e.length), i = Array(t.length);
		this._initIndexMap(e, null, r, "_oldKeyGetter"), this._initIndexMap(t, n, i, "_newKeyGetter");
		for (var a = 0; a < e.length; a++) {
			var o = r[a], s = n[o], c = Vf(s);
			if (c > 1) {
				var l = s.shift();
				s.length === 1 && (n[o] = s[0]), this._update && this._update(l, a);
			} else c === 1 ? (n[o] = null, this._update && this._update(s, a)) : this._remove && this._remove(a);
		}
		this._performRestAdd(i, n);
	}, e.prototype._executeMultiple = function() {
		var e = this._old, t = this._new, n = {}, r = {}, i = [], a = [];
		this._initIndexMap(e, n, i, "_oldKeyGetter"), this._initIndexMap(t, r, a, "_newKeyGetter");
		for (var o = 0; o < i.length; o++) {
			var s = i[o], c = n[s], l = r[s], u = Vf(c), d = Vf(l);
			if (u > 1 && d === 1) this._updateManyToOne && this._updateManyToOne(l, c), r[s] = null;
			else if (u === 1 && d > 1) this._updateOneToMany && this._updateOneToMany(l, c), r[s] = null;
			else if (u === 1 && d === 1) this._update && this._update(l, c), r[s] = null;
			else if (u > 1 && d > 1) this._updateManyToMany && this._updateManyToMany(l, c), r[s] = null;
			else if (u > 1) for (var f = 0; f < u; f++) this._remove && this._remove(c[f]);
			else this._remove && this._remove(c);
		}
		this._performRestAdd(a, r);
	}, e.prototype._performRestAdd = function(e, t) {
		for (var n = 0; n < e.length; n++) {
			var r = e[n], i = t[r], a = Vf(i);
			if (a > 1) for (var o = 0; o < a; o++) this._add && this._add(i[o]);
			else a === 1 && this._add && this._add(i);
			t[r] = null;
		}
	}, e.prototype._initIndexMap = function(e, t, n, r) {
		for (var i = this._diffModeMultiple, a = 0; a < e.length; a++) {
			var o = "_ec_" + this[r](e[a], a);
			if (i || (n[a] = o), t) {
				var s = t[o], c = Vf(s);
				c === 0 ? (t[o] = a, i && n.push(o)) : c === 1 ? t[o] = [s, a] : s.push(a);
			}
		}
	}, e;
}(), Wf = {
	Must: 1,
	Might: 2,
	Not: 3
}, Gf = Ws();
function Kf(e) {
	Gf(e).datasetMap = K();
}
function qf(e, t, n) {
	var r = {}, i = Yf(t);
	if (!i || !e) return r;
	var a = [], o = [], s = t.ecModel, c = Gf(s).datasetMap, l = i.uid + "_" + n.seriesLayoutBy, u, d;
	e = e.slice(), I(e, function(t, n) {
		var i = W(t) ? t : e[n] = { name: t };
		i.type === "ordinal" && u == null && (u = n, d = m(i)), r[i.name] = [];
	});
	var f = c.get(l) || c.set(l, {
		categoryWayDim: d,
		valueWayDim: 0
	});
	I(e, function(e, t) {
		var n = e.name, i = m(e);
		if (u == null) {
			var s = f.valueWayDim;
			p(r[n], s, i), p(o, s, i), f.valueWayDim += i;
		} else if (u === t) p(r[n], 0, i), p(a, 0, i);
		else {
			var s = f.categoryWayDim;
			p(r[n], s, i), p(o, s, i), f.categoryWayDim += i;
		}
	});
	function p(e, t, n) {
		for (var r = 0; r < n; r++) e.push(t + r);
	}
	function m(e) {
		var t = e.dimsDef;
		return t ? t.length : 1;
	}
	return a.length && (r.itemName = a), o.length && (r.seriesName = o), r;
}
function Jf(e, t, n) {
	var r = {};
	if (!Yf(e)) return r;
	var i = t.sourceFormat, a = t.dimensionsDefine, o;
	(i === "objectRows" || i === "keyedColumns") && I(a, function(e, t) {
		(W(e) ? e.name : e) === "name" && (o = t);
	});
	var s = function() {
		for (var e = {}, r = {}, s = [], c = 0, l = Math.min(5, n); c < l; c++) {
			var u = Qf(t.data, i, t.seriesLayoutBy, a, t.startIndex, c);
			s.push(u);
			var d = u === Wf.Not;
			if (d && e.v == null && c !== o && (e.v = c), (e.n == null || e.n === e.v || !d && s[e.n] === Wf.Not) && (e.n = c), f(e) && s[e.n] !== Wf.Not) return e;
			d || (u === Wf.Might && r.v == null && c !== o && (r.v = c), (r.n == null || r.n === r.v) && (r.n = c));
		}
		function f(e) {
			return e.v != null && e.n != null;
		}
		return f(e) ? e : f(r) ? r : null;
	}();
	if (s) {
		r.value = [s.v];
		var c = o ?? s.n;
		r.itemName = [c], r.seriesName = [c];
	}
	return r;
}
function Yf(e) {
	if (!e.get("data", !0)) return Ys(e.ecModel, "dataset", {
		index: e.get("datasetIndex", !0),
		id: e.get("datasetId", !0)
	}, Js).models[0];
}
function Xf(e) {
	return !e.get("transform", !0) && !e.get("fromTransformResult", !0) ? [] : Ys(e.ecModel, "dataset", {
		index: e.get("fromDatasetIndex", !0),
		id: e.get("fromDatasetId", !0)
	}, Js).models;
}
function Zf(e, t) {
	return Qf(e.data, e.sourceFormat, e.seriesLayoutBy, e.dimensionsDefine, e.startIndex, t);
}
function Qf(e, t, n, r, i, a) {
	var o, s = 5;
	if (le(e)) return Wf.Not;
	var c, l;
	if (r) {
		var u = r[a];
		W(u) ? (c = u.name, l = u.type) : U(u) && (c = u);
	}
	if (l != null) return l === "ordinal" ? Wf.Must : Wf.Not;
	if (t === "arrayRows") {
		var d = e;
		if (n === "row") {
			for (var f = d[a], p = 0; p < (f || []).length && p < s; p++) if ((o = b(f[i + p])) != null) return o;
		} else for (var p = 0; p < d.length && p < s; p++) {
			var m = d[i + p];
			if (m && (o = b(m[a])) != null) return o;
		}
	} else if (t === "objectRows") {
		var h = e;
		if (!c) return Wf.Not;
		for (var p = 0; p < h.length && p < s; p++) {
			var g = h[p];
			if (g && (o = b(g[c])) != null) return o;
		}
	} else if (t === "keyedColumns") {
		var _ = e;
		if (!c) return Wf.Not;
		var f = _[c];
		if (!f || le(f)) return Wf.Not;
		for (var p = 0; p < f.length && p < s; p++) if ((o = b(f[p])) != null) return o;
	} else if (t === "original") for (var v = e, p = 0; p < v.length && p < s; p++) {
		var g = v[p], y = Ds(g);
		if (!V(y)) return Wf.Not;
		if ((o = b(y[a])) != null) return o;
	}
	function b(e) {
		var t = U(e);
		if (e != null && isFinite(Number(e)) && e !== "") return t ? Wf.Might : Wf.Not;
		if (t && e !== "-") return Wf.Must;
	}
	return Wf.Not;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/data/Source.js
var $f = function() {
	function e(e) {
		this.data = e.data || (e.sourceFormat === "keyedColumns" ? {} : []), this.sourceFormat = e.sourceFormat || "unknown", this.seriesLayoutBy = e.seriesLayoutBy || "column", this.startIndex = e.startIndex || 0, this.dimensionsDetectedCount = e.dimensionsDetectedCount, this.metaRawOption = e.metaRawOption;
		var t = this.dimensionsDefine = e.dimensionsDefine;
		if (t) for (var n = 0; n < t.length; n++) {
			var r = t[n];
			r.type == null && Zf(this, n) === Wf.Must && (r.type = "ordinal");
		}
	}
	return e;
}();
function ep(e) {
	return e instanceof $f;
}
function tp(e, t, n) {
	n ||= ip(e);
	var r = t.seriesLayoutBy, i = ap(e, n, r, t.sourceHeader, t.dimensions);
	return new $f({
		data: e,
		sourceFormat: n,
		seriesLayoutBy: r,
		dimensionsDefine: i.dimensionsDefine,
		startIndex: i.startIndex,
		dimensionsDetectedCount: i.dimensionsDetectedCount,
		metaRawOption: k(t)
	});
}
function np(e) {
	return new $f({
		data: e,
		sourceFormat: le(e) ? Ec : Sc
	});
}
function rp(e) {
	return new $f({
		data: e.data,
		sourceFormat: e.sourceFormat,
		seriesLayoutBy: e.seriesLayoutBy,
		dimensionsDefine: k(e.dimensionsDefine),
		startIndex: e.startIndex,
		dimensionsDetectedCount: e.dimensionsDetectedCount
	});
}
function ip(e) {
	var t = Dc;
	if (le(e)) t = Ec;
	else if (V(e)) {
		e.length === 0 && (t = Cc);
		for (var n = 0, r = e.length; n < r; n++) {
			var i = e[n];
			if (i != null) {
				if (V(i) || le(i)) {
					t = Cc;
					break;
				} else if (W(i)) {
					t = wc;
					break;
				}
			}
		}
	} else if (W(e)) {
		for (var a in e) if (Ae(e, a) && F(e[a])) {
			t = Tc;
			break;
		}
	}
	return t;
}
function ap(e, t, n, r, i) {
	var a, o;
	if (!e) return {
		dimensionsDefine: sp(i),
		startIndex: o,
		dimensionsDetectedCount: a
	};
	if (t === "arrayRows") {
		var s = e;
		r === "auto" || r == null ? cp(function(e) {
			e != null && e !== "-" && (U(e) ? o ??= 1 : o = 0);
		}, n, s, 10) : o = se(r) ? r : +!!r, !i && o === 1 && (i = [], cp(function(e, t) {
			i[t] = e == null ? "" : e + "";
		}, n, s, Infinity)), a = i ? i.length : n === "row" ? s.length : s[0] ? s[0].length : null;
	} else if (t === "objectRows") i ||= op(e);
	else if (t === "keyedColumns") i || (i = [], I(e, function(e, t) {
		i.push(t);
	}));
	else if (t === "original") {
		var c = Ds(e[0]);
		a = V(c) && c.length || 1;
	}
	return {
		startIndex: o,
		dimensionsDefine: sp(i),
		dimensionsDetectedCount: a
	};
}
function op(e) {
	for (var t = 0, n; t < e.length && !(n = e[t++]););
	if (n) return R(n);
}
function sp(e) {
	if (e) {
		var t = K();
		return L(e, function(e, n) {
			e = W(e) ? e : { name: e };
			var r = {
				name: e.name,
				displayName: e.displayName,
				type: e.type
			};
			if (r.name == null) return r;
			r.name += "", r.displayName ??= r.name;
			var i = t.get(r.name);
			return i ? r.name += "-" + i.count++ : t.set(r.name, { count: 1 }), r;
		});
	}
}
function cp(e, t, n, r) {
	if (t === "row") for (var i = 0; i < n.length && i < r; i++) e(n[i] ? n[i][0] : null, i);
	else for (var a = n[0] || [], i = 0; i < a.length && i < r; i++) e(a[i], i);
}
function lp(e) {
	var t = e.sourceFormat;
	return t === "objectRows" || t === "keyedColumns";
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/data/helper/dataProvider.js
var up, dp, fp, pp, mp, hp, gp = function() {
	function e(e, t) {
		var n = ep(e) ? e : np(e);
		this._source = n;
		var r = this._data = n.data, i = n.sourceFormat;
		n.seriesLayoutBy, i === "typedArray" && (this._offset = 0, this._dimSize = t, this._data = r), hp(this, r, n);
	}
	return e.prototype.getSource = function() {
		return this._source;
	}, e.prototype.count = function() {
		return 0;
	}, e.prototype.getItem = function(e, t) {}, e.prototype.appendData = function(e) {}, e.prototype.clean = function() {}, e.protoInitialize = function() {
		var t = e.prototype;
		t.pure = !1, t.persistent = !0;
	}(), e.internalField = function() {
		var e;
		hp = function(e, i, a) {
			var o = a.sourceFormat, s = a.seriesLayoutBy, c = a.startIndex, l = a.dimensionsDefine, u = mp[Dp(o, s)];
			j(e, u), o === "typedArray" ? (e.getItem = t, e.count = r, e.fillStorage = n) : (e.getItem = z(bp(o, s), null, i, c, l), e.count = z(Cp(o, s), null, i, c, l));
		};
		var t = function(e, t) {
			e -= this._offset, t ||= [];
			for (var n = this._data, r = this._dimSize, i = r * e, a = 0; a < r; a++) t[a] = n[i + a];
			return t;
		}, n = function(e, t, n, r) {
			for (var i = this._data, a = this._dimSize, o = 0; o < a; o++) {
				for (var s = r[o], c = s[0] == null ? Infinity : s[0], l = s[1] == null ? -Infinity : s[1], u = t - e, d = n[o], f = 0; f < u; f++) {
					var p = i[f * a + o];
					d[e + f] = p, p < c && (c = p), p > l && (l = p);
				}
				s[0] = c, s[1] = l;
			}
		}, r = function() {
			return this._data ? this._data.length / this._dimSize : 0;
		};
		mp = (e = {}, e[Cc + "_" + Oc] = {
			pure: !0,
			appendData: i
		}, e[Cc + "_row"] = {
			pure: !0,
			appendData: function() {
				throw Error("Do not support appendData when set seriesLayoutBy: \"row\".");
			}
		}, e[wc] = {
			pure: !0,
			appendData: i
		}, e[Tc] = {
			pure: !0,
			appendData: function(e) {
				var t = this._data;
				I(e, function(e, n) {
					for (var r = t[n] || (t[n] = []), i = 0; i < (e || []).length; i++) r.push(e[i]);
				});
			}
		}, e[Sc] = { appendData: i }, e[Ec] = {
			persistent: !1,
			pure: !0,
			appendData: function(e) {
				this._data = e;
			},
			clean: function() {
				this._offset += this.count(), this._data = null;
			}
		}, e);
		function i(e) {
			for (var t = 0; t < e.length; t++) this._data.push(e[t]);
		}
	}(), e;
}(), _p = function(e) {
	V(e) || ys("series.data or dataset.source must be an array.");
};
up = {}, up[Cc + "_" + Oc] = _p, up[Cc + "_row"] = _p, up[wc] = _p, up[Tc] = function(e, t) {
	for (var n = 0; n < t.length; n++) t[n].name ?? ys("dimension name must not be null/undefined.");
}, up[Sc] = _p;
var vp = function(e, t, n, r) {
	return e[r];
}, yp = (dp = {}, dp[Cc + "_" + Oc] = function(e, t, n, r) {
	return e[r + t];
}, dp[Cc + "_row"] = function(e, t, n, r, i) {
	r += t;
	for (var a = i || [], o = e, s = 0; s < o.length; s++) {
		var c = o[s];
		a[s] = c ? c[r] : null;
	}
	return a;
}, dp[wc] = vp, dp[Tc] = function(e, t, n, r, i) {
	for (var a = i || [], o = 0; o < n.length; o++) {
		var s = n[o].name, c = s == null ? null : e[s];
		a[o] = c ? c[r] : null;
	}
	return a;
}, dp[Sc] = vp, dp);
function bp(e, t) {
	return yp[Dp(e, t)];
}
var xp = function(e, t, n) {
	return e.length;
}, Sp = (fp = {}, fp[Cc + "_" + Oc] = function(e, t, n) {
	return Math.max(0, e.length - t);
}, fp[Cc + "_row"] = function(e, t, n) {
	var r = e[0];
	return r ? Math.max(0, r.length - t) : 0;
}, fp[wc] = xp, fp[Tc] = function(e, t, n) {
	var r = n[0].name, i = r == null ? null : e[r];
	return i ? i.length : 0;
}, fp[Sc] = xp, fp);
function Cp(e, t) {
	return Sp[Dp(e, t)];
}
var wp = function(e, t, n) {
	return e[t];
}, Tp = (pp = {}, pp[Cc] = wp, pp[wc] = function(e, t, n) {
	return e[n];
}, pp[Tc] = wp, pp[Sc] = function(e, t, n) {
	var r = Ds(e);
	return r instanceof Array ? r[t] : r;
}, pp[Ec] = wp, pp);
function Ep(e) {
	return Tp[e];
}
function Dp(e, t) {
	return e === "arrayRows" ? e + "_" + t : e;
}
function Op(e, t, n) {
	if (e) {
		var r = e.getRawDataItem(t);
		if (r != null) {
			var i = e.getStore(), a = i.getSource().sourceFormat;
			if (n != null) {
				var o = e.getDimensionIndex(n), s = i.getDimensionProperty(o);
				return Ep(a)(r, o, s);
			} else {
				var c = r;
				return a === "original" && (c = Ds(r)), c;
			}
		}
	}
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/data/helper/dimensionHelper.js
var kp = function() {
	function e(e, t) {
		this._encode = e, this._schema = t;
	}
	return e.prototype.get = function() {
		return {
			fullDimensions: this._getFullDimensionNames(),
			encode: this._encode
		};
	}, e.prototype._getFullDimensionNames = function() {
		return this._cachedDimNames ||= this._schema ? this._schema.makeOutputDimensionNames() : [], this._cachedDimNames;
	}, e;
}();
function Ap(e, t) {
	var n = {}, r = n.encode = {}, i = K(), a = [], o = [], s = {};
	I(e.dimensions, function(t) {
		var n = e.getDimensionInfo(t), c = n.coordDim;
		if (c) {
			var l = n.coordDimIndex;
			jp(r, c)[l] = t, n.isExtraCoord || (i.set(c, 1), Np(n.type) && (a[0] = t), jp(s, c)[l] = e.getDimensionIndex(n.name)), n.defaultTooltip && o.push(t);
		}
		xc.each(function(e, t) {
			var i = jp(r, t), a = n.otherDims[t];
			a != null && a !== !1 && (i[a] = n.name);
		});
	});
	var c = [], l = {};
	i.each(function(e, t) {
		var n = r[t];
		l[t] = n[0], c = c.concat(n);
	}), n.dataDimsOnCoord = c, n.dataDimIndicesOnCoord = L(c, function(t) {
		return e.getDimensionInfo(t).storeDimIndex;
	}), n.encodeFirstDimNotExtra = l;
	var u = r.label;
	u && u.length && (a = u.slice());
	var d = r.tooltip;
	return d && d.length ? o = d.slice() : o.length || (o = a.slice()), r.defaultedLabel = a, r.defaultedTooltip = o, n.userOutput = new kp(s, t), n;
}
function jp(e, t) {
	return e.hasOwnProperty(t) || (e[t] = []), e[t];
}
function Mp(e) {
	return e === "category" ? "ordinal" : e === "time" ? "time" : "float";
}
function Np(e) {
	return !(e === "ordinal" || e === "time");
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/data/SeriesDimensionDefine.js
var Pp = function() {
	function e(e) {
		this.otherDims = {}, e != null && j(this, e);
	}
	return e;
}();
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/data/helper/dataValueHelper.js
function Fp(e, t) {
	var n = t && t.type;
	return n === "ordinal" ? e : (n === "time" && !se(e) && e != null && e !== "-" && (e = +as(e)), e == null || e === "" ? NaN : Number(e));
}
K({
	number: function(e) {
		return parseFloat(e);
	},
	time: function(e) {
		return +as(e);
	},
	trim: function(e) {
		return U(e) ? ye(e) : e;
	}
});
var Ip = {
	lt: function(e, t) {
		return e < t;
	},
	lte: function(e, t) {
		return e <= t;
	},
	gt: function(e, t) {
		return e > t;
	},
	gte: function(e, t) {
		return e >= t;
	}
};
(function() {
	function e(e, t) {
		se(t) || bs(""), this._opFn = Ip[e], this._rvalFloat = ls(t);
	}
	return e.prototype.evaluate = function(e) {
		return se(e) ? this._opFn(e, this._rvalFloat) : this._opFn(ls(e), this._rvalFloat);
	}, e;
})();
var Lp = function() {
	function e(e, t) {
		var n = e === "desc";
		this._resultLT = n ? 1 : -1, t ??= n ? "min" : "max", this._incomparable = t === "min" ? -Infinity : Infinity;
	}
	return e.prototype.evaluate = function(e, t) {
		var n = se(e) ? e : ls(e), r = se(t) ? t : ls(t), i = isNaN(n), a = isNaN(r);
		if (i && (n = this._incomparable), a && (r = this._incomparable), i && a) {
			var o = U(e), s = U(t);
			o && (n = s ? e : 0), s && (r = o ? t : 0);
		}
		return n < r ? this._resultLT : n > r ? -this._resultLT : 0;
	}, e;
}();
(function() {
	function e(e, t) {
		this._rval = t, this._isEQ = e, this._rvalTypeof = typeof t, this._rvalFloat = ls(t);
	}
	return e.prototype.evaluate = function(e) {
		var t = e === this._rval;
		if (!t) {
			var n = typeof e;
			n !== this._rvalTypeof && (n === "number" || this._rvalTypeof === "number") && (t = ls(e) === this._rvalFloat);
		}
		return this._isEQ ? t : !t;
	}, e;
})();
function Rp(e) {
	var t = "", n = -Infinity, r = -Infinity, i = Infinity, a = Infinity;
	return e && (e.g != null && (t += "G" + e.g, n = e.g), e.ge != null && (t += "GE" + e.ge, r = e.ge), e.l != null && (t += "L" + e.l, i = e.l), e.le != null && (t += "LE" + e.le, a = e.le)), {
		key: t,
		g: n,
		ge: r,
		l: i,
		le: a
	};
}
function zp(e, t) {
	return t > e.g && t >= e.ge && t < e.l && t <= e.le;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/data/DataStore.js
var Bp = typeof Uint32Array > "u" ? Array : Uint32Array, Vp = typeof Uint16Array > "u" ? Array : Uint16Array, Hp = typeof Int32Array > "u" ? Array : Int32Array, Up = typeof Float64Array > "u" ? Array : Float64Array, Wp = {
	float: Up,
	int: Hp,
	ordinal: Array,
	number: Array,
	time: Up
}, Gp;
function Kp(e) {
	return e > 65535 ? Bp : Vp;
}
function qp(e) {
	var t = e.constructor;
	return t === Array ? e.slice() : new t(e);
}
function Jp(e, t, n, r, i) {
	var a = Wp[n || "float"];
	if (i) {
		var o = e[t], s = o && o.length;
		if (s !== r) {
			for (var c = new a(r), l = 0; l < s; l++) c[l] = o[l];
			e[t] = c;
		}
	} else e[t] = new a(r);
}
var Yp = function() {
	function e() {
		this._chunks = [], this._rawExtent = [], this._extent = [], this._count = 0, this._rawCount = 0, this._calcDimNameToIdx = K();
	}
	return e.prototype.initData = function(e, t, n) {
		this._provider = e, this._chunks = [], this._indices = null, this.getRawIndex = this._getRawIdxIdentity;
		var r = e.getSource(), i = this.defaultDimValueGetter = Gp[r.sourceFormat];
		this._dimValueGetter = n || i, this._rawExtent = [], lp(r), this._dimensions = L(t, function(e) {
			return {
				type: e.type,
				property: e.property
			};
		}), this._initDataFromProvider(0, e.count());
	}, e.prototype.getProvider = function() {
		return this._provider;
	}, e.prototype.getSource = function() {
		return this._provider.getSource();
	}, e.prototype.ensureCalculationDimension = function(e, t) {
		var n = this._calcDimNameToIdx, r = this._dimensions, i = n.get(e);
		if (i != null) {
			if (r[i].type === t) return i;
		} else i = r.length;
		return r[i] = { type: t }, n.set(e, i), this._chunks[i] = new Wp[t || "float"](this._rawCount), this._rawExtent[i] = tc(), i;
	}, e.prototype.collectOrdinalMeta = function(e, t) {
		var n = this._chunks[e], r = this._dimensions[e], i = this._rawExtent, a = r.ordinalOffset || 0, o = n.length;
		a === 0 && (i[e] = tc());
		for (var s = i[e], c = a; c < o; c++) {
			var l = n[c] = t.parseAndCollect(n[c]);
			isNaN(l) || (s[0] = Math.min(l, s[0]), s[1] = Math.max(l, s[1]));
		}
		r.ordinalMeta = t, r.ordinalOffset = o, r.type = "ordinal";
	}, e.prototype.getOrdinalMeta = function(e) {
		return this._dimensions[e].ordinalMeta;
	}, e.prototype.getDimensionProperty = function(e) {
		var t = this._dimensions[e];
		return t && t.property;
	}, e.prototype.appendData = function(e) {
		var t = this._provider, n = this.count();
		t.appendData(e);
		var r = t.count();
		return t.persistent || (r += n), n < r && this._initDataFromProvider(n, r, !0), [n, r];
	}, e.prototype.appendValues = function(e, t) {
		for (var n = this._chunks, r = this._dimensions, i = r.length, a = this._rawExtent, o = this.count(), s = o + Math.max(e.length, t || 0), c = 0; c < i; c++) {
			var l = r[c];
			Jp(n, c, l.type, s, !0);
		}
		for (var u = [], d = o; d < s; d++) for (var f = d - o, p = 0; p < i; p++) {
			var l = r[p], m = Gp.arrayRows.call(this, e[f] || u, l.property, f, p);
			n[p][d] = m;
			var h = a[p];
			m < h[0] && (h[0] = m), m > h[1] && (h[1] = m);
		}
		return this._rawCount = this._count = s, {
			start: o,
			end: s
		};
	}, e.prototype._initDataFromProvider = function(e, t, n) {
		for (var r = this._provider, i = this._chunks, a = this._dimensions, o = a.length, s = this._rawExtent, c = L(a, function(e) {
			return e.property;
		}), l = 0; l < o; l++) {
			var u = a[l];
			s[l] || (s[l] = tc()), Jp(i, l, u.type, t, n);
		}
		if (r.fillStorage) r.fillStorage(e, t, i, s);
		else for (var d = [], f = e; f < t; f++) {
			d = r.getItem(f, d);
			for (var p = 0; p < o; p++) {
				var m = i[p], h = this._dimValueGetter(d, c[p], f, p);
				m[f] = h;
				var g = s[p];
				h < g[0] && (g[0] = h), h > g[1] && (g[1] = h);
			}
		}
		!r.persistent && r.clean && r.clean(), this._rawCount = this._count = t, this._extent = [];
	}, e.prototype.count = function() {
		return this._count;
	}, e.prototype.get = function(e, t) {
		if (!(t >= 0 && t < this._count)) return NaN;
		var n = this._chunks[e];
		return n ? n[this.getRawIndex(t)] : NaN;
	}, e.prototype.getValues = function(e, t) {
		var n = [], r = [];
		if (t == null) {
			t = e, e = [];
			for (var i = 0; i < this._dimensions.length; i++) r.push(i);
		} else r = e;
		for (var i = 0, a = r.length; i < a; i++) n.push(this.get(r[i], t));
		return n;
	}, e.prototype.getByRawIndex = function(e, t) {
		if (!(t >= 0 && t < this._rawCount)) return NaN;
		var n = this._chunks[e];
		return n ? n[t] : NaN;
	}, e.prototype.getSum = function(e) {
		var t = this._chunks[e], n = 0;
		if (t) for (var r = 0, i = this.count(); r < i; r++) {
			var a = this.get(e, r);
			isNaN(a) || (n += a);
		}
		return n;
	}, e.prototype.getMedian = function(e) {
		var t = [];
		this.each([e], function(e) {
			isNaN(e) || t.push(e);
		}), Yo(t);
		var n = this.count();
		return n === 0 ? 0 : n % 2 == 1 ? t[(n - 1) / 2] : (t[n / 2] + t[n / 2 - 1]) / 2;
	}, e.prototype.indexOfRawIndex = function(e) {
		if (e >= this._rawCount || e < 0) return -1;
		if (!this._indices) return e;
		var t = this._indices, n = t[e];
		if (n != null && n < this._count && n === e) return e;
		for (var r = 0, i = this._count - 1; r <= i;) {
			var a = (r + i) / 2 | 0;
			if (t[a] < e) r = a + 1;
			else if (t[a] > e) i = a - 1;
			else return a;
		}
		return -1;
	}, e.prototype.getIndices = function() {
		var e, t = this._indices;
		if (t) {
			var n = t.constructor, r = this._count;
			if (n === Array) {
				e = new n(r);
				for (var i = 0; i < r; i++) e[i] = t[i];
			} else e = new n(t.buffer, 0, r);
		} else {
			var n = Kp(this._rawCount);
			e = new n(this.count());
			for (var i = 0; i < e.length; i++) e[i] = i;
		}
		return e;
	}, e.prototype.filter = function(e, t) {
		if (!this._count) return this;
		for (var n = this.clone(), r = n.count(), i = new (Kp(n._rawCount))(r), a = [], o = e.length, s = 0, c = e[0], l = n._chunks, u = 0; u < r; u++) {
			var d = void 0, f = n.getRawIndex(u);
			if (o === 0) d = t(u);
			else if (o === 1) {
				var p = l[c][f];
				d = t(p, u);
			} else {
				for (var m = 0; m < o; m++) a[m] = l[e[m]][f];
				a[m] = u, d = t.apply(null, a);
			}
			d && (i[s++] = f);
		}
		return s < r && (n._indices = i), n._count = s, n._extent = [], n._updateGetRawIdx(), n;
	}, e.prototype.selectRange = function(e) {
		var t = this.clone(), n = t._count;
		if (!n) return this;
		var r = R(e), i = r.length;
		if (!i) return this;
		var a = t.count(), o = new (Kp(t._rawCount))(a), s = 0, c = r[0], l = e[c][0], u = e[c][1], d = t._chunks, f = !1;
		if (!t._indices) {
			var p = 0;
			if (i === 1) {
				for (var m = d[r[0]], h = 0; h < n; h++) {
					var g = m[h];
					(g >= l && g <= u || isNaN(g)) && (o[s++] = p), p++;
				}
				f = !0;
			} else if (i === 2) {
				for (var m = d[r[0]], _ = d[r[1]], v = e[r[1]][0], y = e[r[1]][1], h = 0; h < n; h++) {
					var g = m[h], b = _[h];
					(g >= l && g <= u || isNaN(g)) && (b >= v && b <= y || isNaN(b)) && (o[s++] = p), p++;
				}
				f = !0;
			}
		}
		if (!f) if (i === 1) for (var h = 0; h < a; h++) {
			var x = t.getRawIndex(h), g = d[r[0]][x];
			(g >= l && g <= u || isNaN(g)) && (o[s++] = x);
		}
		else for (var h = 0; h < a; h++) {
			for (var S = !0, x = t.getRawIndex(h), C = 0; C < i; C++) {
				var w = r[C], g = d[w][x];
				(g < e[w][0] || g > e[w][1]) && (S = !1);
			}
			S && (o[s++] = t.getRawIndex(h));
		}
		return s < a && (t._indices = o), t._count = s, t._extent = [], t._updateGetRawIdx(), t;
	}, e.prototype.map = function(e, t) {
		var n = this.clone(e);
		return this._updateDims(n, e, t), n;
	}, e.prototype.modify = function(e, t) {
		this._updateDims(this, e, t);
	}, e.prototype._updateDims = function(e, t, n) {
		for (var r = e._chunks, i = [], a = t.length, o = e.count(), s = [], c = e._rawExtent, l = 0; l < t.length; l++) c[t[l]] = tc();
		for (var u = 0; u < o; u++) {
			for (var d = e.getRawIndex(u), f = 0; f < a; f++) s[f] = r[t[f]][d];
			s[a] = u;
			var p = n && n.apply(null, s);
			if (p != null) {
				typeof p != "object" && (i[0] = p, p = i);
				for (var l = 0; l < p.length; l++) {
					var m = t[l], h = p[l], g = c[m], _ = r[m];
					_ && (_[d] = h), h < g[0] && (g[0] = h), h > g[1] && (g[1] = h);
				}
			}
		}
	}, e.prototype.lttbDownSample = function(e, t) {
		var n = this.clone([e], !0), r = n._chunks[e], i = this.count(), a = 0, o = Math.floor(1 / t), s = this.getRawIndex(0), c, l, u, d = new (Kp(this._rawCount))(Math.min((Math.ceil(i / o) + 2) * 2, i));
		d[a++] = s;
		for (var f = 1; f < i - 1; f += o) {
			for (var p = Math.min(f + o, i - 1), m = Math.min(f + o * 2, i), h = (m + p) / 2, g = 0, _ = p; _ < m; _++) {
				var v = this.getRawIndex(_), y = r[v];
				isNaN(y) || (g += y);
			}
			g /= m - p;
			var b = f, x = Math.min(f + o, i), S = f - 1, C = r[s];
			c = -1, u = b;
			for (var w = -1, T = 0, _ = b; _ < x; _++) {
				var v = this.getRawIndex(_), y = r[v];
				if (isNaN(y)) {
					T++, w < 0 && (w = v);
					continue;
				}
				l = Math.abs((S - h) * (y - C) - (S - _) * (g - C)), l > c && (c = l, u = v);
			}
			T > 0 && T < x - b && (d[a++] = Math.min(w, u), u = Math.max(w, u)), d[a++] = u, s = u;
		}
		return d[a++] = this.getRawIndex(i - 1), n._count = a, n._indices = d, n.getRawIndex = this._getRawIdx, n;
	}, e.prototype.minmaxDownSample = function(e, t) {
		for (var n = this.clone([e], !0), r = n._chunks, i = Math.floor(1 / t), a = r[e], o = this.count(), s = new (Kp(this._rawCount))(Math.ceil(o / i) * 2), c = 0, l = 0; l < o; l += i) {
			var u = l, d = a[this.getRawIndex(u)], f = l, p = a[this.getRawIndex(f)], m = i;
			l + i > o && (m = o - l);
			for (var h = 0; h < m; h++) {
				var g = a[this.getRawIndex(l + h)];
				g < d && (d = g, u = l + h), g > p && (p = g, f = l + h);
			}
			var _ = this.getRawIndex(u), v = this.getRawIndex(f);
			u < f ? (s[c++] = _, s[c++] = v) : (s[c++] = v, s[c++] = _);
		}
		return n._count = c, n._indices = s, n._updateGetRawIdx(), n;
	}, e.prototype.downSample = function(e, t, n, r) {
		for (var i = this.clone([e], !0), a = i._chunks, o = [], s = Math.floor(1 / t), c = a[e], l = this.count(), u = i._rawExtent[e] = tc(), d = new (Kp(this._rawCount))(Math.ceil(l / s)), f = 0, p = 0; p < l; p += s) {
			s > l - p && (s = l - p, o.length = s);
			for (var m = 0; m < s; m++) o[m] = c[this.getRawIndex(p + m)];
			var h = n(o), g = this.getRawIndex(Math.min(p + r(o, h) || 0, l - 1));
			c[g] = h, h < u[0] && (u[0] = h), h > u[1] && (u[1] = h), d[f++] = g;
		}
		return i._count = f, i._indices = d, i._updateGetRawIdx(), i;
	}, e.prototype.each = function(e, t) {
		if (this._count) for (var n = e.length, r = this._chunks, i = 0, a = this.count(); i < a; i++) {
			var o = this.getRawIndex(i);
			switch (n) {
				case 0:
					t(i);
					break;
				case 1:
					t(r[e[0]][o], i);
					break;
				case 2:
					t(r[e[0]][o], r[e[1]][o], i);
					break;
				default:
					for (var s = 0, c = []; s < n; s++) c[s] = r[e[s]][o];
					c[s] = i, t.apply(null, c);
			}
		}
	}, e.prototype.getDataExtent = function(e, t) {
		var n = this._chunks[e], r = tc();
		if (!n) return r;
		var i = this.count();
		if (!this._indices && !t) return this._rawExtent[e].slice();
		var a = this._extent, o = a[e] || (a[e] = {}), s = Rp(t), c = s.key, l = o[c];
		if (l) return l.slice();
		for (var u = r[0], d = r[1], f = 0; f < i; f++) {
			var p = n[this.getRawIndex(f)];
			(!t || zp(s, p)) && (p < u && (u = p), p > d && (d = p));
		}
		return o[c] = [u, d];
	}, e.prototype.getRawDataItem = function(e) {
		var t = this.getRawIndex(e);
		if (this._provider.persistent) return this._provider.getItem(t);
		for (var n = [], r = this._chunks, i = 0; i < r.length; i++) n.push(r[i][t]);
		return n;
	}, e.prototype.clone = function(t, n) {
		var r = new e(), i = this._chunks, a = t && ne(t, function(e, t) {
			return e[t] = !0, e;
		}, {});
		if (a) for (var o = 0; o < i.length; o++) r._chunks[o] = a[o] ? qp(i[o]) : i[o];
		else r._chunks = i;
		return this._copyCommonProps(r), n || (r._indices = this._cloneIndices()), r._updateGetRawIdx(), r;
	}, e.prototype._copyCommonProps = function(e) {
		e._count = this._count, e._rawCount = this._rawCount, e._provider = this._provider, e._dimensions = this._dimensions, e._extent = k(this._extent), e._rawExtent = k(this._rawExtent);
	}, e.prototype._cloneIndices = function() {
		if (this._indices) {
			var e = this._indices.constructor, t = void 0;
			if (e === Array) {
				var n = this._indices.length;
				t = new e(n);
				for (var r = 0; r < n; r++) t[r] = this._indices[r];
			} else t = new e(this._indices);
			return t;
		}
		return null;
	}, e.prototype._getRawIdxIdentity = function(e) {
		return e;
	}, e.prototype._getRawIdx = function(e) {
		return e < this._count && e >= 0 ? this._indices[e] : -1;
	}, e.prototype._updateGetRawIdx = function() {
		this.getRawIndex = this._indices ? this._getRawIdx : this._getRawIdxIdentity;
	}, e.internalField = function() {
		function e(e, t, n, r) {
			return Fp(e[r], this._dimensions[r]);
		}
		Gp = {
			arrayRows: e,
			objectRows: function(e, t, n, r) {
				return Fp(e[t], this._dimensions[r]);
			},
			keyedColumns: e,
			original: function(e, t, n, r) {
				var i = e && (e.value == null ? e : e.value);
				return Fp(i instanceof Array ? i[r] : i, this._dimensions[r]);
			},
			typedArray: function(e, t, n, r) {
				return e[r];
			}
		};
	}(), e;
}(), Xp = Ws(), Zp = {
	float: "f",
	int: "i",
	ordinal: "o",
	number: "n",
	time: "t"
}, Qp = function() {
	function e(e) {
		this.dimensions = e.dimensions, this._dimOmitted = e.dimensionOmitted, this.source = e.source, this._fullDimCount = e.fullDimensionCount, this._updateDimOmitted(e.dimensionOmitted);
	}
	return e.prototype.isDimensionOmitted = function() {
		return this._dimOmitted;
	}, e.prototype._updateDimOmitted = function(e) {
		this._dimOmitted = e, e && (this._dimNameMap ||= tm(this.source));
	}, e.prototype.getSourceDimensionIndex = function(e) {
		return G(this._dimNameMap.get(e), -1);
	}, e.prototype.getSourceDimension = function(e) {
		var t = this.source.dimensionsDefine;
		if (t) return t[e];
	}, e.prototype.makeStoreSchema = function() {
		for (var e = this._fullDimCount, t = lp(this.source), n = !nm(e), r = "", i = [], a = 0, o = 0; a < e; a++) {
			var s = void 0, c = void 0, l = void 0, u = this.dimensions[o];
			if (u && u.storeDimIndex === a) s = t ? u.name : null, c = u.type, l = u.ordinalMeta, o++;
			else {
				var d = this.getSourceDimension(a);
				d && (s = t ? d.name : null, c = d.type);
			}
			i.push({
				property: s,
				type: c,
				ordinalMeta: l
			}), t && s != null && (!u || !u.isCalculationCoord) && (r += n ? s.replace(/\`/g, "`1").replace(/\$/g, "`2") : s), r += "$", r += Zp[c] || "f", l && (r += l.uid), r += "$";
		}
		var f = this.source;
		return {
			dimensions: i,
			hash: [
				f.seriesLayoutBy,
				f.startIndex,
				r
			].join("$$")
		};
	}, e.prototype.makeOutputDimensionNames = function() {
		for (var e = [], t = 0, n = 0; t < this._fullDimCount; t++) {
			var r = void 0, i = this.dimensions[n];
			if (i && i.storeDimIndex === t) i.isCalculationCoord || (r = i.name), n++;
			else {
				var a = this.getSourceDimension(t);
				a && (r = a.name);
			}
			e.push(r);
		}
		return e;
	}, e.prototype.appendCalculationDimension = function(e) {
		this.dimensions.push(e), e.isCalculationCoord = !0, this._fullDimCount++, this._updateDimOmitted(!0);
	}, e;
}();
function $p(e) {
	return e instanceof Qp;
}
function em(e) {
	for (var t = K(), n = 0; n < (e || []).length; n++) {
		var r = e[n], i = W(r) ? r.name : r;
		i != null && t.get(i) == null && t.set(i, n);
	}
	return t;
}
function tm(e) {
	var t = Xp(e);
	return t.dimNameMap ||= em(e.dimensionsDefine);
}
function nm(e) {
	return e > 30;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/data/SeriesData.js
var rm = W, im = L, am = typeof Int32Array > "u" ? Array : Int32Array, om = "e\0\0", sm = -1, cm = [
	"hasItemOption",
	"_nameList",
	"_idList",
	"_invertedIndicesMap",
	"_dimSummary",
	"userOutput",
	"_rawData",
	"_dimValueGetter",
	"_nameDimIdx",
	"_idDimIdx",
	"_nameRepeatCount"
], lm = ["_approximateExtent"], um, dm, fm, pm, mm, hm, gm, _m = function() {
	function e(e, t) {
		this.type = "list", this._dimOmitted = !1, this._nameList = [], this._idList = [], this._visual = {}, this._layout = {}, this._itemVisuals = [], this._itemLayouts = [], this._graphicEls = [], this._approximateExtent = {}, this._calculationInfo = {}, this.hasItemOption = !1, this.TRANSFERABLE_METHODS = [
			"cloneShallow",
			"downSample",
			"minmaxDownSample",
			"lttbDownSample",
			"map"
		], this.CHANGABLE_METHODS = ["filterSelf", "selectRange"], this.DOWNSAMPLE_METHODS = [
			"downSample",
			"minmaxDownSample",
			"lttbDownSample"
		];
		var n, r = !1;
		$p(e) ? (n = e.dimensions, this._dimOmitted = e.isDimensionOmitted(), this._schema = e) : (r = !0, n = e), n ||= ["x", "y"];
		for (var i = {}, a = [], o = {}, s = !1, c = {}, l = 0; l < n.length; l++) {
			var u = n[l], d = U(u) ? new Pp({ name: u }) : u instanceof Pp ? u : new Pp(u), f = d.name;
			d.type = d.type || "float", d.coordDim || (d.coordDim = f, d.coordDimIndex = 0);
			var p = d.otherDims = d.otherDims || {};
			a.push(f), i[f] = d, c[f] != null && (s = !0), d.createInvertedIndices && (o[f] = []), r && (d.storeDimIndex = l), p.itemName === 0 && (this._nameDimIdx = d.storeDimIndex), p.itemId === 0 && (this._idDimIdx = d.storeDimIndex);
		}
		if (this.dimensions = a, this._dimInfos = i, this._initGetDimensionInfo(s), this.hostModel = t, this._invertedIndicesMap = o, this._dimOmitted) {
			var m = this._dimIdxToName = K();
			I(a, function(e) {
				m.set(i[e].storeDimIndex, e);
			});
		}
	}
	return e.prototype.getDimension = function(e) {
		var t = this._recognizeDimIndex(e);
		if (t == null) return e;
		if (t = e, !this._dimOmitted) return this.dimensions[t];
		var n = this._dimIdxToName.get(t);
		if (n != null) return n;
		var r = this._schema.getSourceDimension(t);
		if (r) return r.name;
	}, e.prototype.getDimensionIndex = function(e) {
		var t = this._recognizeDimIndex(e);
		if (t != null) return t;
		if (e == null) return -1;
		var n = this._getDimInfo(e);
		return n ? n.storeDimIndex : this._dimOmitted ? this._schema.getSourceDimensionIndex(e) : -1;
	}, e.prototype._recognizeDimIndex = function(e) {
		if (se(e) || e != null && !isNaN(e) && !this._getDimInfo(e) && (!this._dimOmitted || this._schema.getSourceDimensionIndex(e) < 0)) return +e;
	}, e.prototype._getStoreDimIndex = function(e) {
		return this.getDimensionIndex(e);
	}, e.prototype.getDimensionInfo = function(e) {
		return this._getDimInfo(this.getDimension(e));
	}, e.prototype._initGetDimensionInfo = function(e) {
		var t = this._dimInfos;
		this._getDimInfo = e ? function(e) {
			return t.hasOwnProperty(e) ? t[e] : void 0;
		} : function(e) {
			return t[e];
		};
	}, e.prototype.getDimensionsOnCoord = function() {
		return this._dimSummary.dataDimsOnCoord.slice();
	}, e.prototype.mapDimension = function(e, t) {
		var n = this._dimSummary;
		if (t == null) return n.encodeFirstDimNotExtra[e];
		var r = n.encode[e];
		return r ? r[t] : null;
	}, e.prototype.mapDimensionsAll = function(e) {
		return (this._dimSummary.encode[e] || []).slice();
	}, e.prototype.getStore = function() {
		return this._store;
	}, e.prototype.initData = function(e, t, n) {
		var r = this, i;
		if (e instanceof Yp && (i = e), !i) {
			var a = this.dimensions, o = ep(e) || F(e) ? new gp(e, a.length) : e;
			i = new Yp();
			var s = im(a, function(e) {
				return {
					type: r._dimInfos[e].type,
					property: e
				};
			});
			i.initData(o, s, n);
		}
		this._store = i, this._nameList = (t || []).slice(), this._idList = [], this._nameRepeatCount = {}, this._doInit(0, i.count()), this._dimSummary = Ap(this, this._schema), this.userOutput = this._dimSummary.userOutput;
	}, e.prototype.appendData = function(e) {
		var t = this._store.appendData(e);
		this._doInit(t[0], t[1]);
	}, e.prototype.appendValues = function(e, t) {
		var n = this._store.appendValues(e, t && t.length), r = n.start, i = n.end, a = this._shouldMakeIdFromName();
		if (this._updateOrdinalMeta(), t) for (var o = r; o < i; o++) {
			var s = o - r;
			this._nameList[o] = t[s], a && gm(this, o);
		}
	}, e.prototype._updateOrdinalMeta = function() {
		for (var e = this._store, t = this.dimensions, n = 0; n < t.length; n++) {
			var r = this._dimInfos[t[n]];
			r.ordinalMeta && e.collectOrdinalMeta(r.storeDimIndex, r.ordinalMeta);
		}
	}, e.prototype._shouldMakeIdFromName = function() {
		var e = this._store.getProvider();
		return this._idDimIdx == null && e.getSource().sourceFormat !== "typedArray" && !e.fillStorage;
	}, e.prototype._doInit = function(e, t) {
		if (!(e >= t)) {
			var n = this._store.getProvider();
			this._updateOrdinalMeta();
			var r = this._nameList, i = this._idList;
			if (n.getSource().sourceFormat === "original" && !n.pure) for (var a = [], o = e; o < t; o++) {
				var s = n.getItem(o, a);
				if (!this.hasItemOption && Os(s) && (this.hasItemOption = !0), s) {
					var c = s.name;
					r[o] == null && c != null && (r[o] = Rs(c, null));
					var l = s.id;
					i[o] == null && l != null && (i[o] = Rs(l, null));
				}
			}
			if (this._shouldMakeIdFromName()) for (var o = e; o < t; o++) gm(this, o);
			um(this);
		}
	}, e.prototype.getApproximateExtent = function(e, t) {
		return this._approximateExtent[e] || this._store.getDataExtent(this._getStoreDimIndex(e), t);
	}, e.prototype.setApproximateExtent = function(e, t) {
		t = this.getDimension(t), this._approximateExtent[t] = e.slice();
	}, e.prototype.getCalculationInfo = function(e) {
		return this._calculationInfo[e];
	}, e.prototype.setCalculationInfo = function(e, t) {
		rm(e) ? j(this._calculationInfo, e) : this._calculationInfo[e] = t;
	}, e.prototype.getName = function(e) {
		var t = this.getRawIndex(e), n = this._nameList[t];
		return n == null && this._nameDimIdx != null && (n = fm(this, this._nameDimIdx, t)), n ??= "", n;
	}, e.prototype._getCategory = function(e, t) {
		var n = this._store.get(e, t), r = this._store.getOrdinalMeta(e);
		return r ? r.categories[n] : n;
	}, e.prototype.getId = function(e) {
		return dm(this, this.getRawIndex(e));
	}, e.prototype.count = function() {
		return this._store.count();
	}, e.prototype.get = function(e, t) {
		var n = this._store, r = this._dimInfos[e];
		if (r) return n.get(r.storeDimIndex, t);
	}, e.prototype.getByRawIndex = function(e, t) {
		var n = this._store, r = this._dimInfos[e];
		if (r) return n.getByRawIndex(r.storeDimIndex, t);
	}, e.prototype.getIndices = function() {
		return this._store.getIndices();
	}, e.prototype.getDataExtent = function(e) {
		return this._store.getDataExtent(this._getStoreDimIndex(e), null);
	}, e.prototype.getSum = function(e) {
		return this._store.getSum(this._getStoreDimIndex(e));
	}, e.prototype.getMedian = function(e) {
		return this._store.getMedian(this._getStoreDimIndex(e));
	}, e.prototype.getValues = function(e, t) {
		var n = this, r = this._store;
		return V(e) ? r.getValues(im(e, function(e) {
			return n._getStoreDimIndex(e);
		}), t) : r.getValues(e);
	}, e.prototype.hasValue = function(e) {
		for (var t = this._dimSummary.dataDimIndicesOnCoord, n = 0, r = t.length; n < r; n++) if (isNaN(this._store.get(t[n], e))) return !1;
		return !0;
	}, e.prototype.indexOfName = function(e) {
		for (var t = 0, n = this._store.count(); t < n; t++) if (this.getName(t) === e) return t;
		return -1;
	}, e.prototype.getRawIndex = function(e) {
		return this._store.getRawIndex(e);
	}, e.prototype.indexOfRawIndex = function(e) {
		return this._store.indexOfRawIndex(e);
	}, e.prototype.rawIndexOf = function(e, t) {
		var n = e && this._invertedIndicesMap[e], r = n && n[t];
		return r == null || isNaN(r) ? sm : r;
	}, e.prototype.each = function(e, t, n) {
		H(e) && (n = t, t = e, e = []);
		var r = n || this, i = im(pm(e), this._getStoreDimIndex, this);
		this._store.each(i, r ? z(t, r) : t);
	}, e.prototype.filterSelf = function(e, t, n) {
		H(e) && (n = t, t = e, e = []);
		var r = n || this, i = im(pm(e), this._getStoreDimIndex, this);
		return this._store = this._store.filter(i, r ? z(t, r) : t), this;
	}, e.prototype.selectRange = function(e) {
		var t = this, n = {}, r = R(e), i = [];
		return I(r, function(r) {
			var a = t._getStoreDimIndex(r);
			n[a] = e[r], i.push(a);
		}), this._store = this._store.selectRange(n), this;
	}, e.prototype.mapArray = function(e, t, n) {
		H(e) && (n = t, t = e, e = []), n ||= this;
		var r = [];
		return this.each(e, function() {
			r.push(t && t.apply(this, arguments));
		}, n), r;
	}, e.prototype.map = function(e, t, n, r) {
		var i = n || r || this, a = im(pm(e), this._getStoreDimIndex, this), o = hm(this);
		return o._store = this._store.map(a, i ? z(t, i) : t), o;
	}, e.prototype.modify = function(e, t, n, r) {
		var i = n || r || this, a = im(pm(e), this._getStoreDimIndex, this);
		this._store.modify(a, i ? z(t, i) : t);
	}, e.prototype.downSample = function(e, t, n, r) {
		var i = hm(this);
		return i._store = this._store.downSample(this._getStoreDimIndex(e), t, n, r), i;
	}, e.prototype.minmaxDownSample = function(e, t) {
		var n = hm(this);
		return n._store = this._store.minmaxDownSample(this._getStoreDimIndex(e), t), n;
	}, e.prototype.lttbDownSample = function(e, t) {
		var n = hm(this);
		return n._store = this._store.lttbDownSample(this._getStoreDimIndex(e), t), n;
	}, e.prototype.getRawDataItem = function(e) {
		return this._store.getRawDataItem(e);
	}, e.prototype.getItemModel = function(e) {
		var t = this.hostModel;
		return new Bf(this.getRawDataItem(e), t, t && t.ecModel);
	}, e.prototype.diff = function(e) {
		var t = this;
		return new Uf(e ? e.getStore().getIndices() : [], this.getStore().getIndices(), function(t) {
			return dm(e, t);
		}, function(e) {
			return dm(t, e);
		});
	}, e.prototype.getVisual = function(e) {
		var t = this._visual;
		return t && t[e];
	}, e.prototype.setVisual = function(e, t) {
		this._visual = this._visual || {}, rm(e) ? j(this._visual, e) : this._visual[e] = t;
	}, e.prototype.getItemVisual = function(e, t) {
		var n = this._itemVisuals[e];
		return (n && n[t]) ?? this.getVisual(t);
	}, e.prototype.hasItemVisual = function() {
		return this._itemVisuals.length > 0;
	}, e.prototype.ensureUniqueItemVisual = function(e, t) {
		var n = this._itemVisuals, r = n[e];
		r ||= n[e] = {};
		var i = r[t];
		return i ?? (i = this.getVisual(t), V(i) ? i = i.slice() : rm(i) && (i = j({}, i)), r[t] = i), i;
	}, e.prototype.setItemVisual = function(e, t, n) {
		var r = this._itemVisuals[e] || {};
		this._itemVisuals[e] = r, rm(t) ? j(r, t) : r[t] = n;
	}, e.prototype.clearAllVisual = function() {
		this._visual = {}, this._itemVisuals = [];
	}, e.prototype.setLayout = function(e, t) {
		rm(e) ? j(this._layout, e) : this._layout[e] = t;
	}, e.prototype.getLayout = function(e) {
		return this._layout[e];
	}, e.prototype.getItemLayout = function(e) {
		return this._itemLayouts[e];
	}, e.prototype.setItemLayout = function(e, t, n) {
		this._itemLayouts[e] = n ? j(this._itemLayouts[e] || {}, t) : t;
	}, e.prototype.clearItemLayouts = function() {
		this._itemLayouts.length = 0;
	}, e.prototype.setItemGraphicEl = function(e, t) {
		bc(this.hostModel && this.hostModel.seriesIndex, this.dataType, e, t), this._graphicEls[e] = t;
	}, e.prototype.getItemGraphicEl = function(e) {
		return this._graphicEls[e];
	}, e.prototype.eachItemGraphicEl = function(e, t) {
		I(this._graphicEls, function(n, r) {
			n && e && e.call(t, n, r);
		});
	}, e.prototype.cloneShallow = function(t) {
		return t ||= new e(this._schema ? this._schema : im(this.dimensions, this._getDimInfo, this), this.hostModel), mm(t, this), t._store = this._store, t;
	}, e.prototype.wrapMethod = function(e, t) {
		var n = this[e];
		H(n) && (this.__wrappedMethods = this.__wrappedMethods || [], this.__wrappedMethods.push(e), this[e] = function() {
			var e = n.apply(this, arguments);
			return t.apply(this, [e].concat(ge(arguments)));
		});
	}, e.internalField = function() {
		um = function(e) {
			var t = e._invertedIndicesMap;
			I(t, function(n, r) {
				var i = e._dimInfos[r], a = i.ordinalMeta, o = e._store;
				if (a) {
					n = t[r] = new am(a.categories.length);
					for (var s = 0; s < n.length; s++) n[s] = sm;
					for (var s = 0; s < o.count(); s++) n[o.get(i.storeDimIndex, s)] = s;
				}
			});
		}, fm = function(e, t, n) {
			return Rs(e._getCategory(t, n), null);
		}, dm = function(e, t) {
			var n = e._idList[t];
			return n == null && e._idDimIdx != null && (n = fm(e, e._idDimIdx, t)), n ??= om + t, n;
		}, pm = function(e) {
			return V(e) || (e = e == null ? [] : [e]), e;
		}, hm = function(t) {
			var n = new e(t._schema ? t._schema : im(t.dimensions, t._getDimInfo, t), t.hostModel);
			return mm(n, t), n;
		}, mm = function(e, t) {
			I(cm.concat(t.__wrappedMethods || []), function(n) {
				t.hasOwnProperty(n) && (e[n] = t[n]);
			}), e.__wrappedMethods = t.__wrappedMethods, I(lm, function(n) {
				e[n] = k(t[n]);
			}), e._calculationInfo = j({}, t._calculationInfo);
		}, gm = function(e, t) {
			var n = e._nameList, r = e._idList, i = e._nameDimIdx, a = e._idDimIdx, o = n[t], s = r[t];
			if (o == null && i != null && (n[t] = o = fm(e, i, t)), s == null && a != null && (r[t] = s = fm(e, a, t)), s == null && o != null) {
				var c = e._nameRepeatCount, l = c[o] = (c[o] || 0) + 1;
				s = o, l > 1 && (s += "__ec__" + l), r[t] = s;
			}
		};
	}(), e;
}();
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/data/helper/createDimensions.js
function vm(e, t) {
	ep(e) || (e = np(e)), t ||= {};
	var n = t.coordDimensions || [], r = t.dimensionsDefine || e.dimensionsDefine || [], i = K(), a = [], o = ym(e, n, r, t.dimensionsCount), s = t.canOmitUnusedDimensions && nm(o), c = r === e.dimensionsDefine, l = c ? tm(e) : em(r), u = t.encodeDefine;
	!u && t.encodeDefaulter && (u = t.encodeDefaulter(e, o));
	for (var d = K(u), f = new Hp(o), p = 0; p < f.length; p++) f[p] = -1;
	function m(e) {
		var t = f[e];
		if (t < 0) {
			var n = r[e], i = W(n) ? n : { name: n }, o = new Pp(), s = i.name;
			return s != null && l.get(s) != null && (o.name = o.displayName = s), i.type != null && (o.type = i.type), i.displayName != null && (o.displayName = i.displayName), f[e] = a.length, o.storeDimIndex = e, a.push(o), o;
		}
		return a[t];
	}
	if (!s) for (var p = 0; p < o; p++) m(p);
	d.each(function(e, t) {
		var n = ws(e).slice();
		if (n.length === 1 && !U(n[0]) && n[0] < 0) {
			d.set(t, !1);
			return;
		}
		var r = d.set(t, []);
		I(n, function(e, n) {
			var i = U(e) ? l.get(e) : e;
			i != null && i < o && (r[n] = i, g(m(i), t, n));
		});
	});
	var h = 0;
	I(n, function(e) {
		var t, n, r, i;
		if (U(e)) t = e, i = {};
		else {
			i = e, t = i.name;
			var a = i.ordinalMeta;
			i.ordinalMeta = null, i = j({}, i), i.ordinalMeta = a, n = i.dimsDef, r = i.otherDims, i.name = i.coordDim = i.coordDimIndex = i.dimsDef = i.otherDims = null;
		}
		var s = d.get(t);
		if (s !== !1) {
			if (s = ws(s), !s.length) for (var l = 0; l < (n && n.length || 1); l++) {
				for (; h < o && m(h).coordDim != null;) h++;
				h < o && s.push(h++);
			}
			I(s, function(e, a) {
				var o = m(e);
				if (c && i.type != null && (o.type = i.type), g(M(o, i), t, a), o.name == null && n) {
					var s = n[a];
					!W(s) && (s = { name: s }), o.name = o.displayName = s.name, o.defaultTooltip = s.defaultTooltip;
				}
				r && M(o.otherDims, r);
			});
		}
	});
	function g(e, t, n) {
		xc.get(t) == null ? (e.coordDim = t, e.coordDimIndex = n, i.set(t, !0)) : e.otherDims[t] = n;
	}
	var _ = t.generateCoord, v = t.generateCoordCount, y = v != null;
	v = _ ? v || 1 : 0;
	var b = _ || "value";
	function x(e) {
		e.name ??= e.coordDim;
	}
	if (s) I(a, function(e) {
		x(e);
	}), a.sort(function(e, t) {
		return e.storeDimIndex - t.storeDimIndex;
	});
	else for (var S = 0; S < o; S++) {
		var C = m(S);
		C.coordDim ?? (C.coordDim = bm(b, i, y), C.coordDimIndex = 0, (!_ || v <= 0) && (C.isExtraCoord = !0), v--), x(C), C.type == null && (Zf(e, S) === Wf.Must || C.isExtraCoord && (C.otherDims.itemName != null || C.otherDims.seriesName != null)) && (C.type = "ordinal");
	}
	return fc(a, function(e) {
		return e.name;
	}, function(e, t) {
		t > 0 && (e.name += t - 1);
	}), new Qp({
		source: e,
		dimensions: a,
		fullDimensionCount: o,
		dimensionOmitted: s
	});
}
function ym(e, t, n, r) {
	var i = Math.max(e.dimensionsDetectedCount || 1, t.length, n.length, r || 0);
	return I(t, function(e) {
		var t;
		W(e) && (t = e.dimsDef) && (i = Math.max(i, t.length));
	}), i;
}
function bm(e, t, n) {
	if (n || t.hasKey(e)) {
		for (var r = 0; t.hasKey(e + r);) r++;
		e += r;
	}
	return t.set(e, !0), e;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/core/CoordinateSystem.js
var xm = {}, Sm = {}, Cm = function() {
	function e() {
		this._normalMasterList = [], this._nonSeriesBoxMasterList = [];
	}
	return e.prototype.create = function(e, t) {
		this._nonSeriesBoxMasterList = n(xm, !0), this._normalMasterList = n(Sm, !1);
		function n(n, r) {
			var i = [];
			return I(n, function(n, r) {
				var a = n.create(e, t);
				i = i.concat(a || []);
			}), i;
		}
	}, e.prototype.update = function(e, t) {
		I(this._normalMasterList, function(n) {
			n.update && n.update(e, t);
		});
	}, e.prototype.getCoordinateSystems = function() {
		return this._normalMasterList.concat(this._nonSeriesBoxMasterList);
	}, e.register = function(e, t) {
		if (e === "matrix" || e === "calendar") {
			xm[e] = t;
			return;
		}
		Sm[e] = t;
	}, e.get = function(e) {
		return Sm[e] || xm[e];
	}, e;
}();
function wm(e) {
	return !!xm[e];
}
function Tm(e) {
	Em.set(e.fullType, { getCoord2: void 0 }).getCoord2 = e.getCoord2;
}
var Em = K();
function Dm(e) {
	var t = e.getShallow("coord", !0), n = 1;
	if (t == null) {
		var r = Em.get(e.type);
		r && r.getCoord2 && (n = 2, t = r.getCoord2(e));
	}
	return {
		coord: t,
		from: n
	};
}
function Om(e, t) {
	var n = e.getShallow("coordinateSystem"), r = e.getShallow("coordinateSystemUsage", !0), i = 0;
	if (n) {
		var a = e.mainType === "series";
		r ??= a ? "data" : "box", r === "data" ? (i = 1, a || (i = 0)) : r === "box" && (i = 2, !a && !wm(n) && (i = 0));
	}
	return {
		coordSysType: n,
		kind: i
	};
}
function km(e) {
	var t = e.targetModel, n = e.coordSysType, r = e.coordSysProvider, i = e.isDefaultDataCoordSys;
	e.allowNotFound;
	var a = Om(t, !0), o = a.kind, s = a.coordSysType;
	if (i && o !== 1 && (o = 1, s = n), o === 0 || s !== n) return 0;
	var c = r(n, t);
	return c ? (o === 1 ? t.coordinateSystem = c : t.boxCoordinateSystem = c, o) : 0;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/model/referHelper.js
var Am = function() {
	function e(e) {
		this.coordSysDims = [], this.axisMap = K(), this.categoryAxisMap = K(), this.coordSysName = e;
	}
	return e;
}();
function jm(e) {
	var t = e.get("coordinateSystem"), n = new Am(t), r = Mm[t];
	if (r) return r(e, n, n.axisMap, n.categoryAxisMap), n;
}
var Mm = {
	cartesian2d: function(e, t, n, r) {
		var i = e.getReferringComponents("xAxis", Js).models[0], a = e.getReferringComponents("yAxis", Js).models[0];
		t.coordSysDims = ["x", "y"], n.set("x", i), n.set("y", a), Nm(i) && (r.set("x", i), t.firstCategoryDimIndex = 0), Nm(a) && (r.set("y", a), t.firstCategoryDimIndex ??= 1);
	},
	singleAxis: function(e, t, n, r) {
		var i = e.getReferringComponents("singleAxis", Js).models[0];
		t.coordSysDims = ["single"], n.set("single", i), Nm(i) && (r.set("single", i), t.firstCategoryDimIndex = 0);
	},
	polar: function(e, t, n, r) {
		var i = e.getReferringComponents("polar", Js).models[0], a = i.findAxisModel("radiusAxis"), o = i.findAxisModel("angleAxis");
		t.coordSysDims = ["radius", "angle"], n.set("radius", a), n.set("angle", o), Nm(a) && (r.set("radius", a), t.firstCategoryDimIndex = 0), Nm(o) && (r.set("angle", o), t.firstCategoryDimIndex ??= 1);
	},
	geo: function(e, t, n, r) {
		t.coordSysDims = ["lng", "lat"];
	},
	parallel: function(e, t, n, r) {
		var i = e.ecModel, a = i.getComponent("parallel", e.get("parallelIndex")), o = t.coordSysDims = a.dimensions.slice();
		I(a.parallelAxisIndex, function(e, a) {
			var s = i.getComponent("parallelAxis", e), c = o[a];
			n.set(c, s), Nm(s) && (r.set(c, s), t.firstCategoryDimIndex ??= a);
		});
	},
	matrix: function(e, t, n, r) {
		var i = e.getReferringComponents("matrix", Js).models[0];
		t.coordSysDims = ["x", "y"];
		var a = i.getDimensionModel("x"), o = i.getDimensionModel("y");
		n.set("x", a), n.set("y", o), r.set("x", a), r.set("y", o);
	}
};
function Nm(e) {
	return e.get("type") === "category";
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/data/helper/dataStackHelper.js
function Pm(e, t, n) {
	n ||= {};
	var r = n.byIndex, i = n.stackedCoordDimension, a, o, s;
	Fm(t) ? a = t : (o = t.schema, a = o.dimensions, s = t.store);
	var c = !!(e && e.get("stack")), l, u, d, f, p = !0;
	function m(e) {
		return e.type !== "ordinal" && e.type !== "time";
	}
	if (I(a, function(e, t) {
		U(e) && (a[t] = e = { name: e }), m(e) || (p = !1);
	}), I(a, function(e, t) {
		c && !e.isExtraCoord && (!r && !l && e.ordinalMeta && (l = e), !u && m(e) && (!p || e.coordDim !== "x" && e.coordDim !== "angle") && (!i || i === e.coordDim) && (u = e));
	}), u && !r && !l && (r = !0), u) {
		d = "__\0ecstackresult_" + e.id, f = "__\0ecstackedover_" + e.id, l && (l.createInvertedIndices = !0);
		var h = u.coordDim, g = u.type, _ = 0;
		I(a, function(e) {
			e.coordDim === h && _++;
		});
		var v = {
			name: d,
			coordDim: h,
			coordDimIndex: _,
			type: g,
			isExtraCoord: !0,
			isCalculationCoord: !0,
			storeDimIndex: a.length
		}, y = {
			name: f,
			coordDim: f,
			coordDimIndex: _ + 1,
			type: g,
			isExtraCoord: !0,
			isCalculationCoord: !0,
			storeDimIndex: a.length + 1
		};
		o ? (s && (v.storeDimIndex = s.ensureCalculationDimension(f, g), y.storeDimIndex = s.ensureCalculationDimension(d, g)), o.appendCalculationDimension(v), o.appendCalculationDimension(y)) : (a.push(v), a.push(y));
	}
	return {
		stackedDimension: u && u.name,
		stackedByDimension: l && l.name,
		isStackedByIndex: r,
		stackedOverDimension: f,
		stackResultDimension: d
	};
}
function Fm(e) {
	return !$p(e.schema);
}
function Im(e, t) {
	return !!t && t === e.getCalculationInfo("stackedDimension");
}
function Lm(e, t) {
	return Im(e, t) ? e.getCalculationInfo("stackResultDimension") : t;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/helper/createSeriesData.js
function Rm(e, t) {
	var n = e.get("coordinateSystem"), r = Cm.get(n), i;
	return t && t.coordSysDims && (i = L(t.coordSysDims, function(e) {
		var n = { name: e }, r = t.axisMap.get(e);
		return r && (n.type = Mp(r.get("type"))), n;
	})), i ||= r && (r.getDimensionsInfo ? r.getDimensionsInfo() : r.dimensions.slice()) || ["x", "y"], i;
}
function zm(e, t, n) {
	var r, i;
	return n && I(e, function(e, a) {
		var o = e.coordDim, s = n.categoryAxisMap.get(o);
		s && (r ??= a, e.ordinalMeta = s.getOrdinalMeta(), t && (e.createInvertedIndices = !0)), e.otherDims.itemName != null && (i = !0);
	}), !i && r != null && (e[r].otherDims.itemName = 0), r;
}
function Bm(e, t, n) {
	n ||= {};
	var r = t.getSourceManager(), i, a = !1;
	e ? (a = !0, i = np(e)) : (i = r.getSource(), a = i.sourceFormat === Sc);
	var o = jm(t), s = Rm(t, o), c = n.useEncodeDefaulter, l = H(c) ? c : c ? B(qf, s, t) : null, u = {
		coordDimensions: s,
		generateCoord: n.generateCoord,
		encodeDefine: t.getEncode(),
		encodeDefaulter: l,
		canOmitUnusedDimensions: !a
	}, d = vm(i, u), f = zm(d.dimensions, n.createInvertedIndices, o), p = a ? null : r.getSharedDataStore(d), m = Pm(t, {
		schema: d,
		store: p
	}), h = new _m(d, t);
	h.setCalculationInfo(m);
	var g = f != null && Vm(i) ? function(e, t, n, r) {
		return r === f ? n : this.defaultDimValueGetter(e, t, n, r);
	} : null;
	return h.hasItemOption = !1, h.initData(a ? i : p, null, g), h;
}
function Vm(e) {
	if (e.sourceFormat === "original") return !V(Ds(Hm(e.data || [])));
}
function Hm(e) {
	for (var t = 0; t < e.length && e[t] == null;) t++;
	return e[t];
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/util/component.js
var Um = Math.round(Math.random() * 10);
function Wm(e) {
	return [e || "", Um++].join("_");
}
function Gm(e) {
	var t = {};
	e.registerSubTypeDefaulter = function(e, n) {
		var r = Re(e);
		t[r.main] = n;
	}, e.determineSubType = function(n, r) {
		var i = r.type;
		if (!i) {
			var a = Re(n).main;
			e.hasSubTypes(n) && t[a] && (i = t[a](r));
		}
		return i;
	};
}
function Km(e, t) {
	e.topologicalTravel = function(e, t, r, i) {
		if (!e.length) return;
		var a = n(t), o = a.graph, s = a.noEntryList, c = {};
		for (I(e, function(e) {
			c[e] = !0;
		}); s.length;) {
			var l = s.pop(), u = o[l], d = !!c[l];
			d && (r.call(i, l, u.originalDeps.slice()), delete c[l]), I(u.successor, d ? p : f);
		}
		I(c, function() {
			throw Error("");
		});
		function f(e) {
			o[e].entryCount--, o[e].entryCount === 0 && s.push(e);
		}
		function p(e) {
			c[e] = !0, f(e);
		}
	};
	function n(e) {
		var n = {}, a = [];
		return I(e, function(o) {
			var s = r(n, o), c = i(s.originalDeps = t(o), e);
			s.entryCount = c.length, s.entryCount === 0 && a.push(o), I(c, function(e) {
				N(s.predecessor, e) < 0 && s.predecessor.push(e);
				var t = r(n, e);
				N(t.successor, e) < 0 && t.successor.push(o);
			});
		}), {
			graph: n,
			noEntryList: a
		};
	}
	function r(e, t) {
		return e[t] || (e[t] = {
			predecessor: [],
			successor: []
		}), e[t];
	}
	function i(e, t) {
		var n = [];
		return I(e, function(e) {
			N(t, e) >= 0 && n.push(e);
		}), n;
	}
}
function qm(e, t) {
	return A(A({}, e, !0), t, !0);
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/core/fourPointsTransform.js
var Jm = Math.log(2);
function Ym(e, t, n, r, i, a) {
	var o = r + "-" + i, s = e.length;
	if (a.hasOwnProperty(o)) return a[o];
	if (t === 1) {
		var c = Math.round(Math.log((1 << s) - 1 & ~i) / Jm);
		return e[n][c];
	}
	for (var l = r | 1 << n, u = n + 1; r & 1 << u;) u++;
	for (var d = 0, f = 0, p = 0; f < s; f++) {
		var m = 1 << f;
		m & i || (d += (p % 2 ? -1 : 1) * e[n][f] * Ym(e, t - 1, u, l, i | m, a), p++);
	}
	return a[o] = d, d;
}
function Xm(e, t) {
	var n = [
		[
			e[0],
			e[1],
			1,
			0,
			0,
			0,
			-t[0] * e[0],
			-t[0] * e[1]
		],
		[
			0,
			0,
			0,
			e[0],
			e[1],
			1,
			-t[1] * e[0],
			-t[1] * e[1]
		],
		[
			e[2],
			e[3],
			1,
			0,
			0,
			0,
			-t[2] * e[2],
			-t[2] * e[3]
		],
		[
			0,
			0,
			0,
			e[2],
			e[3],
			1,
			-t[3] * e[2],
			-t[3] * e[3]
		],
		[
			e[4],
			e[5],
			1,
			0,
			0,
			0,
			-t[4] * e[4],
			-t[4] * e[5]
		],
		[
			0,
			0,
			0,
			e[4],
			e[5],
			1,
			-t[5] * e[4],
			-t[5] * e[5]
		],
		[
			e[6],
			e[7],
			1,
			0,
			0,
			0,
			-t[6] * e[6],
			-t[6] * e[7]
		],
		[
			0,
			0,
			0,
			e[6],
			e[7],
			1,
			-t[7] * e[6],
			-t[7] * e[7]
		]
	], r = {}, i = Ym(n, 8, 0, 0, 0, r);
	if (i !== 0) {
		for (var a = [], o = 0; o < 8; o++) for (var s = 0; s < 8; s++) a[s] ?? (a[s] = 0), a[s] += ((o + s) % 2 ? -1 : 1) * Ym(n, 7, +(o === 0), 1 << o, 1 << s, r) / i * t[o];
		return function(e, t, n) {
			var r = t * a[6] + n * a[7] + 1;
			e[0] = (t * a[0] + n * a[1] + a[2]) / r, e[1] = (t * a[3] + n * a[4] + a[5]) / r;
		};
	}
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/core/dom.js
var Zm = "___zrEVENTSAVED", Qm = [];
function $m(e, t, n, r, i) {
	return th(Qm, t, r, i, !0) && th(e, n, Qm[0], Qm[1]);
}
function eh(e, t) {
	e && n(e), t && n(t);
	function n(e) {
		var t = e[Zm];
		t && (t.clearMarkers && t.clearMarkers(), delete e[Zm]);
	}
}
function th(e, t, n, r, i) {
	if (t.getBoundingClientRect && q.domSupported && !ih(t)) {
		var a = t[Zm] || (t[Zm] = {}), o = rh(nh(t, a), a, i);
		if (o) return o(e, n, r), !0;
	}
	return !1;
}
function nh(e, t) {
	var n = t.markers;
	if (n) return n;
	n = t.markers = [];
	for (var r = ["left", "right"], i = ["top", "bottom"], a = 0; a < 4; a++) {
		var o = document.createElement("div"), s = o.style, c = a % 2, l = (a >> 1) % 2;
		s.cssText = [
			"position: absolute",
			"visibility: hidden",
			"padding: 0",
			"margin: 0",
			"border-width: 0",
			"user-select: none",
			"width:0",
			"height:0",
			r[c] + ":0",
			i[l] + ":0",
			r[1 - c] + ":auto",
			i[1 - l] + ":auto",
			""
		].join("!important;"), e.appendChild(o), n.push(o);
	}
	return t.clearMarkers = function() {
		I(n, function(e) {
			e.parentNode && e.parentNode.removeChild(e);
		});
	}, n;
}
function rh(e, t, n) {
	for (var r = n ? "invTrans" : "trans", i = t[r], a = t.srcCoords, o = [], s = [], c = !0, l = 0; l < 4; l++) {
		var u = e[l].getBoundingClientRect(), d = 2 * l, f = u.left, p = u.top;
		o.push(f, p), c = c && a && f === a[d] && p === a[d + 1], s.push(e[l].offsetLeft, e[l].offsetTop);
	}
	return c && i ? i : (t.srcCoords = o, t[r] = n ? Xm(s, o) : Xm(o, s));
}
function ih(e) {
	return e.nodeName.toUpperCase() === "CANVAS";
}
var ah = /([&<>"'])/g, oh = {
	"&": "&amp;",
	"<": "&lt;",
	">": "&gt;",
	"\"": "&quot;",
	"'": "&#39;"
};
function sh(e) {
	return e == null ? "" : (e + "").replace(ah, function(e, t) {
		return oh[t];
	});
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/i18n/langEN.js
var ch = {
	time: {
		month: [
			"January",
			"February",
			"March",
			"April",
			"May",
			"June",
			"July",
			"August",
			"September",
			"October",
			"November",
			"December"
		],
		monthAbbr: [
			"Jan",
			"Feb",
			"Mar",
			"Apr",
			"May",
			"Jun",
			"Jul",
			"Aug",
			"Sep",
			"Oct",
			"Nov",
			"Dec"
		],
		dayOfWeek: [
			"Sunday",
			"Monday",
			"Tuesday",
			"Wednesday",
			"Thursday",
			"Friday",
			"Saturday"
		],
		dayOfWeekAbbr: [
			"Sun",
			"Mon",
			"Tue",
			"Wed",
			"Thu",
			"Fri",
			"Sat"
		]
	},
	legend: { selector: {
		all: "All",
		inverse: "Inv"
	} },
	toolbox: {
		brush: { title: {
			rect: "Box Select",
			polygon: "Lasso Select",
			lineX: "Horizontally Select",
			lineY: "Vertically Select",
			keep: "Keep Selections",
			clear: "Clear Selections"
		} },
		dataView: {
			title: "Data View",
			lang: [
				"Data View",
				"Close",
				"Refresh"
			]
		},
		dataZoom: { title: {
			zoom: "Zoom",
			back: "Zoom Reset"
		} },
		magicType: { title: {
			line: "Switch to Line Chart",
			bar: "Switch to Bar Chart",
			stack: "Stack",
			tiled: "Tile"
		} },
		restore: { title: "Restore" },
		saveAsImage: {
			title: "Save as Image",
			lang: ["Right Click to Save Image"]
		}
	},
	series: { typeNames: {
		pie: "Pie chart",
		bar: "Bar chart",
		line: "Line chart",
		scatter: "Scatter plot",
		effectScatter: "Ripple scatter plot",
		radar: "Radar chart",
		tree: "Tree",
		treemap: "Treemap",
		boxplot: "Boxplot",
		candlestick: "Candlestick",
		k: "K line chart",
		heatmap: "Heat map",
		map: "Map",
		parallel: "Parallel coordinate map",
		lines: "Line graph",
		graph: "Relationship graph",
		sankey: "Sankey diagram",
		funnel: "Funnel chart",
		gauge: "Gauge",
		pictorialBar: "Pictorial bar",
		themeRiver: "Theme River Map",
		sunburst: "Sunburst",
		custom: "Custom chart",
		chart: "Chart"
	} },
	aria: {
		general: {
			withTitle: "This is a chart about \"{title}\"",
			withoutTitle: "This is a chart"
		},
		series: {
			single: {
				prefix: "",
				withName: " with type {seriesType} named {seriesName}.",
				withoutName: " with type {seriesType}."
			},
			multiple: {
				prefix: ". It consists of {seriesCount} series count.",
				withName: " The {seriesId} series is a {seriesType} representing {seriesName}.",
				withoutName: " The {seriesId} series is a {seriesType}.",
				separator: {
					middle: "",
					end: ""
				}
			}
		},
		data: {
			allData: "The data is as follows: ",
			partialData: "The first {displayCnt} items are: ",
			withName: "the data for {name} is {value}",
			withoutName: "{value}",
			separator: {
				middle: ", ",
				end: ". "
			}
		}
	}
}, lh = {
	time: {
		month: [
			"一月",
			"二月",
			"三月",
			"四月",
			"五月",
			"六月",
			"七月",
			"八月",
			"九月",
			"十月",
			"十一月",
			"十二月"
		],
		monthAbbr: [
			"1月",
			"2月",
			"3月",
			"4月",
			"5月",
			"6月",
			"7月",
			"8月",
			"9月",
			"10月",
			"11月",
			"12月"
		],
		dayOfWeek: [
			"星期日",
			"星期一",
			"星期二",
			"星期三",
			"星期四",
			"星期五",
			"星期六"
		],
		dayOfWeekAbbr: [
			"日",
			"一",
			"二",
			"三",
			"四",
			"五",
			"六"
		]
	},
	legend: { selector: {
		all: "全选",
		inverse: "反选"
	} },
	toolbox: {
		brush: { title: {
			rect: "矩形选择",
			polygon: "圈选",
			lineX: "横向选择",
			lineY: "纵向选择",
			keep: "保持选择",
			clear: "清除选择"
		} },
		dataView: {
			title: "数据视图",
			lang: [
				"数据视图",
				"关闭",
				"刷新"
			]
		},
		dataZoom: { title: {
			zoom: "区域缩放",
			back: "区域缩放还原"
		} },
		magicType: { title: {
			line: "切换为折线图",
			bar: "切换为柱状图",
			stack: "切换为堆叠",
			tiled: "切换为平铺"
		} },
		restore: { title: "还原" },
		saveAsImage: {
			title: "保存为图片",
			lang: ["右键另存为图片"]
		}
	},
	series: { typeNames: {
		pie: "饼图",
		bar: "柱状图",
		line: "折线图",
		scatter: "散点图",
		effectScatter: "涟漪散点图",
		radar: "雷达图",
		tree: "树图",
		treemap: "矩形树图",
		boxplot: "箱型图",
		candlestick: "K线图",
		k: "K线图",
		heatmap: "热力图",
		map: "地图",
		parallel: "平行坐标图",
		lines: "线图",
		graph: "关系图",
		sankey: "桑基图",
		funnel: "漏斗图",
		gauge: "仪表盘图",
		pictorialBar: "象形柱图",
		themeRiver: "主题河流图",
		sunburst: "旭日图",
		custom: "自定义图表",
		chart: "图表"
	} },
	aria: {
		general: {
			withTitle: "这是一个关于“{title}”的图表。",
			withoutTitle: "这是一个图表，"
		},
		series: {
			single: {
				prefix: "",
				withName: "图表类型是{seriesType}，表示{seriesName}。",
				withoutName: "图表类型是{seriesType}。"
			},
			multiple: {
				prefix: "它由{seriesCount}个图表系列组成。",
				withName: "第{seriesId}个系列是一个表示{seriesName}的{seriesType}，",
				withoutName: "第{seriesId}个系列是一个{seriesType}，",
				separator: {
					middle: "；",
					end: "。"
				}
			}
		},
		data: {
			allData: "其数据是——",
			partialData: "其中，前{displayCnt}项是——",
			withName: "{name}的数据是{value}",
			withoutName: "{value}",
			separator: {
				middle: "，",
				end: ""
			}
		}
	}
}, uh = "ZH", dh = "EN", fh = dh, ph = {}, mh = {}, hh = q.domSupported ? function() {
	return (document.documentElement.lang || navigator.language || navigator.browserLanguage || fh).toUpperCase().indexOf(uh) > -1 ? uh : fh;
}() : fh;
function gh(e, t) {
	e = e.toUpperCase(), mh[e] = new Bf(t), ph[e] = t;
}
function _h(e) {
	if (U(e)) {
		var t = ph[e.toUpperCase()] || {};
		return e === uh || e === dh ? k(t) : A(k(t), k(ph[fh]), !1);
	} else return A(k(e), k(ph[fh]), !1);
}
function vh(e) {
	return mh[e];
}
function yh() {
	return mh[fh];
}
gh(dh, ch), gh(uh, lh);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/scale/break.js
var bh = null;
function xh() {
	return bh;
}
function Sh(e, t) {
	var n = xh(), r = t.breakOption, i = t.breakParsed;
	return !i && n && (i = n.parseAxisBreakOption(r, e)), i;
}
function Ch(e) {
	var t = e.brk;
	return t ? t.breaks : [];
}
function wh(e) {
	var t = e.brk;
	return t ? t.hasBreaks() : !1;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/util/time.js
var Th = 1e3, Eh = Th * 60, Dh = Eh * 60, Oh = Dh * 24, kh = Oh * 365, Ah = {
	year: /({yyyy}|{yy})/,
	month: /({MMMM}|{MMM}|{MM}|{M})/,
	day: /({dd}|{d})/,
	hour: /({HH}|{H}|{hh}|{h})/,
	minute: /({mm}|{m})/,
	second: /({ss}|{s})/,
	millisecond: /({SSS}|{S})/
}, jh = {
	year: "{yyyy}",
	month: "{MMM}",
	day: "{d}",
	hour: "{HH}:{mm}",
	minute: "{HH}:{mm}",
	second: "{HH}:{mm}:{ss}",
	millisecond: "{HH}:{mm}:{ss} {SSS}"
}, Mh = "{yyyy}-{MM}-{dd} {HH}:{mm}:{ss} {SSS}", Nh = "{yyyy}-{MM}-{dd}", Ph = {
	year: "{yyyy}",
	month: "{yyyy}-{MM}",
	day: Nh,
	hour: Nh + " " + jh.hour,
	minute: Nh + " " + jh.minute,
	second: Nh + " " + jh.second,
	millisecond: Mh
}, Fh = [
	"year",
	"month",
	"day",
	"hour",
	"minute",
	"second",
	"millisecond"
], Ih = [
	"year",
	"half-year",
	"quarter",
	"month",
	"week",
	"half-week",
	"day",
	"half-day",
	"quarter-day",
	"hour",
	"minute",
	"second",
	"millisecond"
];
function Lh(e) {
	return !U(e) && !H(e) ? Rh(e) : e;
}
function Rh(e) {
	e ||= {};
	var t = {}, n = !0;
	return I(Fh, function(t) {
		n &&= e[t] == null;
	}), I(Fh, function(r, i) {
		var a = e[r];
		t[r] = {};
		for (var o = null, s = i; s >= 0; s--) {
			var c = Fh[s], l = W(a) && !V(a) ? a[c] : a, u = void 0;
			V(l) ? (u = l.slice(), o = u[0] || "") : U(l) ? (o = l, u = [o]) : (o == null ? o = jh[r] : Ah[c].test(o) || (o = t[c][c][0] + " " + o), u = [o], n && (u[1] = "{primary|" + o + "}")), t[r][c] = u;
		}
	}), t;
}
function zh(e, t) {
	return e += "", "0000".substr(0, t - e.length) + e;
}
function Bh(e) {
	switch (e) {
		case "half-year":
		case "quarter": return "month";
		case "week":
		case "half-week": return "day";
		case "half-day":
		case "quarter-day": return "hour";
		default: return e;
	}
}
function Vh(e) {
	return e === Bh(e);
}
function Hh(e) {
	switch (e) {
		case "year":
		case "month": return "day";
		case "millisecond": return "millisecond";
		default: return "second";
	}
}
function Uh(e, t, n, r) {
	var i = as(e), a = i[qh(n)](), o = i[Jh(n)]() + 1, s = Math.floor((o - 1) / 3) + 1, c = i[Yh(n)](), l = i["get" + (n ? "UTC" : "") + "Day"](), u = i[Xh(n)](), d = (u - 1) % 12 + 1, f = i[Zh(n)](), p = i[Qh(n)](), m = i[$h(n)](), h = u >= 12 ? "pm" : "am", g = h.toUpperCase(), _ = (r instanceof Bf ? r : vh(r || hh) || yh()).getModel("time"), v = _.get("month"), y = _.get("monthAbbr"), b = _.get("dayOfWeek"), x = _.get("dayOfWeekAbbr");
	return (t || "").replace(/{a}/g, h + "").replace(/{A}/g, g + "").replace(/{yyyy}/g, a + "").replace(/{yy}/g, zh(a % 100 + "", 2)).replace(/{Q}/g, s + "").replace(/{MMMM}/g, v[o - 1]).replace(/{MMM}/g, y[o - 1]).replace(/{MM}/g, zh(o, 2)).replace(/{M}/g, o + "").replace(/{dd}/g, zh(c, 2)).replace(/{d}/g, c + "").replace(/{eeee}/g, b[l]).replace(/{ee}/g, x[l]).replace(/{e}/g, l + "").replace(/{HH}/g, zh(u, 2)).replace(/{H}/g, u + "").replace(/{hh}/g, zh(d + "", 2)).replace(/{h}/g, d + "").replace(/{mm}/g, zh(f, 2)).replace(/{m}/g, f + "").replace(/{ss}/g, zh(p, 2)).replace(/{s}/g, p + "").replace(/{SSS}/g, zh(m, 3)).replace(/{S}/g, m + "");
}
function Wh(e, t, n, r, i) {
	var a = null;
	if (U(n)) a = n;
	else if (H(n)) {
		var o = {
			time: e.time,
			level: e.time ? e.time.level : 0
		}, s = xh();
		s && s.makeAxisLabelFormatterParamBreak(o, e.break), a = n(e.value, t, o);
	} else {
		var c = e.time;
		if (c) {
			var l = n[c.lowerTimeUnit][c.upperTimeUnit];
			a = l[Math.min(c.level, l.length - 1)] || "";
		} else {
			var u = Gh(e.value, i);
			a = n[u][u][0];
		}
	}
	return Uh(new Date(e.value), a, i, r);
}
function Gh(e, t) {
	var n = as(e), r = n[Jh(t)]() + 1, i = n[Yh(t)](), a = n[Xh(t)](), o = n[Zh(t)](), s = n[Qh(t)](), c = n[$h(t)]() === 0, l = c && s === 0, u = l && o === 0, d = u && a === 0, f = d && i === 1;
	return f && r === 1 ? "year" : f ? "month" : d ? "day" : u ? "hour" : l ? "minute" : c ? "second" : "millisecond";
}
function Kh(e, t, n) {
	switch (t) {
		case "year": e[tg(n)](0);
		case "month": e[ng(n)](1);
		case "day": e[rg(n)](0);
		case "hour": e[ig(n)](0);
		case "minute": e[ag(n)](0);
		case "second": e[og(n)](0);
	}
	return e;
}
function qh(e) {
	return e ? "getUTCFullYear" : "getFullYear";
}
function Jh(e) {
	return e ? "getUTCMonth" : "getMonth";
}
function Yh(e) {
	return e ? "getUTCDate" : "getDate";
}
function Xh(e) {
	return e ? "getUTCHours" : "getHours";
}
function Zh(e) {
	return e ? "getUTCMinutes" : "getMinutes";
}
function Qh(e) {
	return e ? "getUTCSeconds" : "getSeconds";
}
function $h(e) {
	return e ? "getUTCMilliseconds" : "getMilliseconds";
}
function eg(e) {
	return e ? "setUTCFullYear" : "setFullYear";
}
function tg(e) {
	return e ? "setUTCMonth" : "setMonth";
}
function ng(e) {
	return e ? "setUTCDate" : "setDate";
}
function rg(e) {
	return e ? "setUTCHours" : "setHours";
}
function ig(e) {
	return e ? "setUTCMinutes" : "setMinutes";
}
function ag(e) {
	return e ? "setUTCSeconds" : "setSeconds";
}
function og(e) {
	return e ? "setUTCMilliseconds" : "setMilliseconds";
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/util/format.js
function sg(e) {
	if (!us(e)) return U(e) ? e : "-";
	var t = (e + "").split(".");
	return t[0].replace(/(\d{1,3})(?=(?:\d{3})+(?!\d))/g, "$1,") + (t.length > 1 ? "." + t[1] : "");
}
function cg(e, t) {
	return e = (e || "").toLowerCase().replace(/-(.)/g, function(e, t) {
		return t.toUpperCase();
	}), t && e && (e = e.charAt(0).toUpperCase() + e.slice(1)), e;
}
var lg = _e;
function ug(e, t, n) {
	var r = "{yyyy}-{MM}-{dd} {HH}:{mm}:{ss}";
	function i(e) {
		return e && ye(e) ? e : "-";
	}
	function a(e) {
		return ms(e);
	}
	var o = t === "time", s = e instanceof Date;
	if (o || s) {
		var c = o ? as(e) : e;
		if (!isNaN(+c)) return Uh(c, r, n);
		if (s) return "-";
	}
	if (t === "ordinal") return oe(e) ? i(e) : se(e) && a(e) ? e + "" : "-";
	var l = ls(e);
	return a(l) ? sg(l) : oe(e) ? i(e) : typeof e == "boolean" ? e + "" : "-";
}
var dg = [
	"a",
	"b",
	"c",
	"d",
	"e",
	"f",
	"g"
], fg = function(e, t) {
	return "{" + e + (t ?? "") + "}";
};
function pg(e, t, n) {
	V(t) || (t = [t]);
	var r = t.length;
	if (!r) return "";
	for (var i = t[0].$vars || [], a = 0; a < i.length; a++) {
		var o = dg[a];
		e = e.replace(fg(o), fg(o, 0));
	}
	for (var s = 0; s < r; s++) for (var c = 0; c < i.length; c++) {
		var l = t[s][i[c]];
		e = e.replace(fg(dg[c], s), n ? sh(l) : l);
	}
	return e;
}
function mg(e, t) {
	var n = U(e) ? {
		color: e,
		extraCssText: t
	} : e || {}, r = n.color, i = n.type;
	t = n.extraCssText;
	var a = n.renderMode || "html";
	return r ? a === "html" ? i === "subItem" ? "<span style=\"display:inline-block;vertical-align:middle;margin-right:8px;margin-left:3px;border-radius:4px;width:4px;height:4px;background-color:" + sh(r) + ";" + (t || "") + "\"></span>" : "<span style=\"display:inline-block;margin-right:4px;border-radius:10px;width:10px;height:10px;background-color:" + sh(r) + ";" + (t || "") + "\"></span>" : {
		renderMode: a,
		content: "{" + (n.markerId || "markerX") + "|}  ",
		style: i === "subItem" ? {
			width: 4,
			height: 4,
			borderRadius: 2,
			backgroundColor: r
		} : {
			width: 10,
			height: 10,
			borderRadius: 5,
			backgroundColor: r
		}
	} : "";
}
function hg(e, t) {
	return t ||= "transparent", U(e) ? e : W(e) && e.colorStops && (e.colorStops[0] || {}).color || t;
}
function gg(e, t) {
	if (t === "_blank" || t === "blank") {
		var n = window.open();
		n.opener = null, n.location.href = e;
	} else window.open(e, t);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/util/layout.js
var _g = I, vg = [
	"left",
	"right",
	"top",
	"bottom",
	"width",
	"height"
], yg = [[
	"width",
	"left",
	"right"
], [
	"height",
	"top",
	"bottom"
]];
function bg(e, t, n, r, i) {
	var a = 0, o = 0;
	r ??= Infinity, i ??= Infinity;
	var s = 0;
	t.eachChild(function(c, l) {
		var u = c.getBoundingRect(), d = t.childAt(l + 1), f = d && d.getBoundingRect(), p, m;
		if (e === "horizontal") {
			var h = u.width + (f ? -f.x + u.x : 0);
			p = a + h, p > r || c.newline ? (a = 0, p = h, o += s + n, s = u.height) : s = Math.max(s, u.height);
		} else {
			var g = u.height + (f ? -f.y + u.y : 0);
			m = o + g, m > i || c.newline ? (a += s + n, o = 0, m = g, s = u.width) : s = Math.max(s, u.width);
		}
		c.newline || (c.x = a, c.y = o, c.markRedraw(), e === "horizontal" ? a = p + n : o = m + n);
	});
}
var xg = bg;
B(bg, "vertical"), B(bg, "horizontal");
function Sg(e, t) {
	return {
		left: e.getShallow("left", t),
		top: e.getShallow("top", t),
		right: e.getShallow("right", t),
		bottom: e.getShallow("bottom", t),
		width: e.getShallow("width", t),
		height: e.getShallow("height", t)
	};
}
function Cg(e, t) {
	var n = Dg(e, t, { enableLayoutOnlyByCenter: !0 }), r = e.getBoxLayoutParams(), i, a;
	if (n.type === Eg.point) a = n.refPoint, i = Tg(r, {
		width: t.getWidth(),
		height: t.getHeight()
	});
	else {
		var o = e.get("center"), s = V(o) ? o : [o, o];
		i = Tg(r, n.refContainer), a = n.boxCoordFrom === 2 ? n.refPoint : [X(s[0], i.width) + i.x, X(s[1], i.height) + i.y];
	}
	return {
		viewRect: i,
		center: a
	};
}
function wg(e, t) {
	var n = Cg(e, t), r = n.viewRect, i = n.center, a = e.get("radius");
	V(a) || (a = [0, a]);
	var o = X(r.width, t.getWidth()), s = X(r.height, t.getHeight()), c = Math.min(o, s), l = X(a[0], c / 2), u = X(a[1], c / 2);
	return {
		cx: i[0],
		cy: i[1],
		r0: l,
		r: u,
		viewRect: r
	};
}
function Tg(e, t, n) {
	n = lg(n || 0);
	var r = t.width, i = t.height, a = X(e.left, r), o = X(e.top, i), s = X(e.right, r), c = X(e.bottom, i), l = X(e.width, r), u = X(e.height, i), d = n[2] + n[0], f = n[1] + n[3], p = e.aspect;
	switch (isNaN(l) && (l = r - s - f - a), isNaN(u) && (u = i - c - d - o), p != null && (isNaN(l) && isNaN(u) && (p > r / i ? l = r * .8 : u = i * .8), isNaN(l) && (l = p * u), isNaN(u) && (u = l / p)), isNaN(a) && (a = r - s - l - f), isNaN(o) && (o = i - c - u - d), e.left || e.right) {
		case "center":
			a = r / 2 - l / 2 - n[3];
			break;
		case "right":
			a = r - l - f;
			break;
	}
	switch (e.top || e.bottom) {
		case "middle":
		case "center":
			o = i / 2 - u / 2 - n[0];
			break;
		case "bottom":
			o = i - u - d;
			break;
	}
	a ||= 0, o ||= 0, isNaN(l) && (l = r - f - a - (s || 0)), isNaN(u) && (u = i - d - o - (c || 0));
	var m = new Y((t.x || 0) + a + n[3], (t.y || 0) + o + n[0], l, u);
	return m.margin = n, m;
}
var Eg = {
	rect: 1,
	point: 2
};
function Dg(e, t, n) {
	var r, i, a, o = e.boxCoordinateSystem, s;
	if (o) {
		var c = Dm(e), l = c.coord, u = c.from;
		if (o.dataToLayout) {
			a = Eg.rect, s = u;
			var d = o.dataToLayout(l);
			r = d.contentRect || d.rect;
		} else n && n.enableLayoutOnlyByCenter && o.dataToPoint && (a = Eg.point, s = u, i = o.dataToPoint(l));
	}
	return a ??= Eg.rect, a === Eg.rect && (r ||= {
		x: 0,
		y: 0,
		width: t.getWidth(),
		height: t.getHeight()
	}, i = [r.x + r.width / 2, r.y + r.height / 2]), {
		type: a,
		refContainer: r,
		refPoint: i,
		boxCoordFrom: s
	};
}
function Og(e) {
	var t = e.layoutMode || e.constructor.layoutMode;
	return W(t) ? t : t ? { type: t } : null;
}
function kg(e, t, n) {
	var r = n && n.ignoreSize;
	!V(r) && (r = [r, r]);
	var i = o(yg[0], 0), a = o(yg[1], 1);
	c(yg[0], e, i), c(yg[1], e, a);
	function o(n, i) {
		var a = {}, o = 0, c = {}, l = 0, u = 2;
		if (_g(n, function(t) {
			c[t] = e[t];
		}), _g(n, function(e) {
			Ae(t, e) && (a[e] = c[e] = t[e]), s(a, e) && o++, s(c, e) && l++;
		}), r[i]) return s(t, n[1]) ? c[n[2]] = null : s(t, n[2]) && (c[n[1]] = null), c;
		if (l === u || !o) return c;
		if (o >= u) return a;
		for (var d = 0; d < n.length; d++) {
			var f = n[d];
			if (!Ae(a, f) && Ae(e, f)) {
				a[f] = e[f];
				break;
			}
		}
		return a;
	}
	function s(e, t) {
		return e[t] != null && e[t] !== "auto";
	}
	function c(e, t, n) {
		_g(e, function(e) {
			t[e] = n[e];
		});
	}
}
function Ag(e) {
	return jg({}, e);
}
function jg(e, t) {
	return t && e && _g(vg, function(n) {
		Ae(t, n) && (e[n] = t[n]);
	}), e;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/model/Component.js
var Mg = Ws(), Ng = function(e) {
	o(t, e);
	function t(t, n, r) {
		var i = e.call(this, t, n, r) || this;
		return i.uid = Wm("ec_cpt_model"), i;
	}
	return t.prototype.init = function(e, t, n) {
		this.mergeDefaultAndTheme(e, n);
	}, t.prototype.mergeDefaultAndTheme = function(e, t) {
		var n = Og(this), r = n ? Ag(e) : {};
		A(e, t.getTheme().get(this.mainType)), A(e, this.getDefaultOption()), n && kg(e, r, n);
	}, t.prototype.mergeOption = function(e, t) {
		A(this.option, e, !0);
		var n = Og(this);
		n && kg(this.option, e, n);
	}, t.prototype.optionUpdated = function(e, t) {}, t.prototype.getDefaultOption = function() {
		var e = this.constructor;
		if (!Be(e)) return e.defaultOption;
		var t = Mg(this);
		if (!t.defaultOption) {
			for (var n = [], r = e; r;) {
				var i = r.prototype.defaultOption;
				i && n.push(i), r = r.superClass;
			}
			for (var a = {}, o = n.length - 1; o >= 0; o--) a = A(a, n[o], !0);
			t.defaultOption = a;
		}
		return t.defaultOption;
	}, t.prototype.getReferringComponents = function(e, t) {
		var n = e + "Index", r = e + "Id";
		return Ys(this.ecModel, e, {
			index: this.get(n, !0),
			id: this.get(r, !0)
		}, t);
	}, t.prototype.getBoxLayoutParams = function() {
		return Sg(this, !1);
	}, t.prototype.getZLevelKey = function() {
		return "";
	}, t.prototype.setZLevel = function(e) {
		this.option.zlevel = e;
	}, t.protoInitialize = function() {
		var e = t.prototype;
		e.type = "component", e.id = "", e.name = "", e.mainType = "", e.subType = "", e.componentIndex = 0;
	}(), t;
}(Bf);
Ue(Ng, Bf), Je(Ng), Gm(Ng), Km(Ng, Pg);
function Pg(e) {
	var t = [];
	return I(Ng.getClassesByMainType(e), function(e) {
		t = t.concat(e.dependencies || e.prototype.dependencies || []);
	}), t = L(t, function(e) {
		return Re(e).main;
	}), e !== "dataset" && N(t, "dataset") <= 0 && t.unshift("dataset"), t;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/model/mixin/palette.js
var Fg = Ws();
Ws();
var Ig = function() {
	function e() {}
	return e.prototype.getColorFromPalette = function(e, t, n) {
		var r = ws(this.get("color", !0)), i = this.get("colorLayer", !0);
		return Rg(this, Fg, r, i, e, t, n);
	}, e.prototype.clearColorPalette = function() {
		zg(this, Fg);
	}, e;
}();
function Lg(e, t) {
	for (var n = e.length, r = 0; r < n; r++) if (e[r].length > t) return e[r];
	return e[n - 1];
}
function Rg(e, t, n, r, i, a, o) {
	a ||= e;
	var s = t(a), c = s.paletteIdx || 0, l = s.paletteNameMap = s.paletteNameMap || {};
	if (l.hasOwnProperty(i)) return l[i];
	var u = o == null || !r ? n : Lg(r, o);
	if (u ||= n, !(!u || !u.length)) {
		var d = u[c];
		return i && (l[i] = d), s.paletteIdx = (c + 1) % u.length, d;
	}
}
function zg(e, t) {
	t(e).paletteIdx = 0, t(e).paletteNameMap = {};
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/model/mixin/dataFormat.js
var Bg = /\{@(.+?)\}/g, Vg = function() {
	function e() {}
	return e.prototype.getDataParams = function(e, t) {
		var n = this.getData(t), r = this.getRawValue(e, t), i = n.getRawIndex(e), a = n.getName(e), o = n.getRawDataItem(e), s = n.getItemVisual(e, "style"), c = s && s[n.getItemVisual(e, "drawType") || "fill"], l = s && s.stroke, u = this.mainType, d = u === "series", f = n.userOutput && n.userOutput.get();
		return {
			componentType: u,
			componentSubType: this.subType,
			componentIndex: this.componentIndex,
			seriesType: d ? this.subType : null,
			seriesIndex: this.seriesIndex,
			seriesId: d ? this.id : null,
			seriesName: d ? this.name : null,
			name: a,
			dataIndex: i,
			data: o,
			dataType: t,
			value: r,
			color: c,
			borderColor: l,
			dimensionNames: f ? f.fullDimensions : null,
			encode: f ? f.encode : null,
			$vars: [
				"seriesName",
				"name",
				"value"
			]
		};
	}, e.prototype.getFormattedLabel = function(e, t, n, r, i, a) {
		t ||= "normal";
		var o = this.getData(n), s = this.getDataParams(e, n);
		if (a && (s.value = a.interpolatedValue), r != null && V(s.value) && (s.value = s.value[r]), i ||= o.getItemModel(e).get(t === "normal" ? ["label", "formatter"] : [
			t,
			"label",
			"formatter"
		]), H(i)) return s.status = t, s.dimensionIndex = r, i(s);
		if (U(i)) return pg(i, s).replace(Bg, function(t, n) {
			var r = n.length, i = n;
			i.charAt(0) === "[" && i.charAt(r - 1) === "]" && (i = +i.slice(1, r - 1));
			var s = Op(o, e, i);
			if (a && V(a.interpolatedValue)) {
				var c = o.getDimensionIndex(i);
				c >= 0 && (s = a.interpolatedValue[c]);
			}
			return s == null ? "" : s + "";
		});
	}, e.prototype.getRawValue = function(e, t) {
		return Op(this.getData(t), e);
	}, e.prototype.formatTooltip = function(e, t, n) {}, e;
}();
function Hg(e) {
	var t, n;
	return W(e) ? e.type && (n = e) : t = e, {
		text: t,
		frag: n
	};
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/core/task.js
function Ug(e) {
	return new Wg(e);
}
var Wg = function() {
	function e(e) {
		e ||= {}, this._reset = e.reset, this._plan = e.plan, this._count = e.count, this._onDirty = e.onDirty, this._dirty = !0;
	}
	return e.prototype.perform = function(e) {
		var t = this._upstream, n = e && e.skip;
		if (this._dirty && t) {
			var r = this.context;
			r.data = r.outputData = t.context.outputData;
		}
		this.__pipeline && (this.__pipeline.currentTask = this);
		var i;
		this._plan && !n && (i = this._plan(this.context));
		var a = l(this._modBy), o = this._modDataCount || 0, s = l(e && e.modBy), c = e && e.modDataCount || 0;
		(a !== s || o !== c) && (i = "reset");
		function l(e) {
			return !(e >= 1) && (e = 1), e;
		}
		var u;
		(this._dirty || i === "reset") && (this._dirty = !1, u = this._doReset(n)), this._modBy = s, this._modDataCount = c;
		var d = e && e.step;
		if (t ? this._dueEnd = t._outputDueEnd : this._dueEnd = this._count ? this._count(this.context) : Infinity, this._progress) {
			var f = this._dueIndex, p = Math.min(d == null ? Infinity : this._dueIndex + d, this._dueEnd);
			if (!n && (u || f < p)) {
				var m = this._progress;
				if (V(m)) for (var h = 0; h < m.length; h++) this._doProgress(m[h], f, p, s, c);
				else this._doProgress(m, f, p, s, c);
			}
			this._dueIndex = p;
			var g = this._settedOutputEnd == null ? p : this._settedOutputEnd;
			this._outputDueEnd = g;
		} else this._dueIndex = this._outputDueEnd = this._settedOutputEnd == null ? this._dueEnd : this._settedOutputEnd;
		return this.unfinished();
	}, e.prototype.dirty = function() {
		this._dirty = !0, this._onDirty && this._onDirty(this.context);
	}, e.prototype._doProgress = function(e, t, n, r, i) {
		Gg.reset(t, n, r, i), this._callingProgress = e, this._callingProgress({
			start: t,
			end: n,
			count: n - t,
			next: Gg.next
		}, this.context);
	}, e.prototype._doReset = function(e) {
		this._dueIndex = this._outputDueEnd = this._dueEnd = 0, this._settedOutputEnd = null;
		var t, n;
		!e && this._reset && (t = this._reset(this.context), t && t.progress && (n = t.forceFirstProgress, t = t.progress), V(t) && !t.length && (t = null)), this._progress = t, this._modBy = this._modDataCount = null;
		var r = this._downstream;
		return r && r.dirty(), n;
	}, e.prototype.unfinished = function() {
		return this._progress && this._dueIndex < this._dueEnd;
	}, e.prototype.pipe = function(e) {
		(this._downstream !== e || this._dirty) && (this._downstream = e, e._upstream = this, e.dirty());
	}, e.prototype.dispose = function() {
		this._disposed ||= (this._upstream && (this._upstream._downstream = null), this._downstream && (this._downstream._upstream = null), this._dirty = !1, !0);
	}, e.prototype.getUpstream = function() {
		return this._upstream;
	}, e.prototype.getDownstream = function() {
		return this._downstream;
	}, e.prototype.setOutputEnd = function(e) {
		this._outputDueEnd = this._settedOutputEnd = e;
	}, e;
}(), Gg = function() {
	var e, t, n, r, i, a = { reset: function(c, l, u, d) {
		t = c, e = l, n = u, r = d, i = Math.ceil(r / n), a.next = n > 1 && r > 0 ? s : o;
	} };
	return a;
	function o() {
		return t < e ? t++ : null;
	}
	function s() {
		var a = t % i * n + Math.ceil(t / i), o = t >= e ? null : a < r ? a : t;
		return t++, o;
	}
}(), Kg = function() {
	function e() {}
	return e.prototype.getRawData = function() {
		throw Error("not supported");
	}, e.prototype.getRawDataItem = function(e) {
		throw Error("not supported");
	}, e.prototype.cloneRawData = function() {}, e.prototype.getDimensionInfo = function(e) {}, e.prototype.cloneAllDimensionInfo = function() {}, e.prototype.count = function() {}, e.prototype.retrieveValue = function(e, t) {}, e.prototype.retrieveValueFromItem = function(e, t) {}, e.prototype.convertValue = function(e, t) {
		return Fp(e, t);
	}, e;
}();
function qg(e, t) {
	var n = new Kg(), r = e.data, i = n.sourceFormat = e.sourceFormat, a = e.startIndex;
	e.seriesLayoutBy !== "column" && bs("");
	var o = [], s = {}, c = e.dimensionsDefine;
	if (c) I(c, function(e, t) {
		var n = e.name, r = {
			index: t,
			name: n,
			displayName: e.displayName
		};
		o.push(r), n != null && (Ae(s, n) && bs(""), s[n] = r);
	});
	else for (var l = 0; l < e.dimensionsDetectedCount; l++) o.push({ index: l });
	var u = bp(i, Oc);
	t.__isBuiltIn && (n.getRawDataItem = function(e) {
		return u(r, a, o, e);
	}, n.getRawData = z(Jg, null, e)), n.cloneRawData = z(Yg, null, e), n.count = z(Cp(i, Oc), null, r, a, o);
	var d = Ep(i);
	n.retrieveValue = function(e, t) {
		return f(u(r, a, o, e), t);
	};
	var f = n.retrieveValueFromItem = function(e, t) {
		if (e != null) {
			var n = o[t];
			if (n) return d(e, t, n.name);
		}
	};
	return n.getDimensionInfo = z(Xg, null, o, s), n.cloneAllDimensionInfo = z(Zg, null, o), n;
}
function Jg(e) {
	var t = e.sourceFormat;
	return n_(t) || bs(""), e.data;
}
function Yg(e) {
	var t = e.sourceFormat, n = e.data;
	if (n_(t) || bs(""), t === "arrayRows") {
		for (var r = [], i = 0, a = n.length; i < a; i++) r.push(n[i].slice());
		return r;
	} else if (t === "objectRows") {
		for (var r = [], i = 0, a = n.length; i < a; i++) r.push(j({}, n[i]));
		return r;
	}
}
function Xg(e, t, n) {
	if (n != null) {
		if (se(n) || !isNaN(n) && !Ae(t, n)) return e[n];
		if (Ae(t, n)) return t[n];
	}
}
function Zg(e) {
	return k(e);
}
var Qg = K();
function $g(e) {
	e = k(e);
	var t = e.type, n = "";
	t || bs(n);
	var r = t.split(":");
	r.length !== 2 && bs(n);
	var i = !1;
	r[0] === "echarts" && (t = r[1], i = !0), e.__isBuiltIn = i, Qg.set(t, e);
}
function e_(e, t, n) {
	var r = ws(e), i = r.length;
	i || bs("");
	for (var a = 0, o = i; a < o; a++) {
		var s = r[a];
		t = t_(s, t, n, i === 1 ? null : a), a !== o - 1 && (t.length = Math.max(t.length, 1));
	}
	return t;
}
function t_(e, t, n, r) {
	var i = "";
	t.length || bs(i), W(e) || bs(i);
	var a = e.type, o = Qg.get(a);
	o || bs(i);
	var s = L(t, function(e) {
		return qg(e, o);
	});
	return L(ws(o.transform({
		upstream: s[0],
		upstreamList: s,
		config: k(e.config)
	})), function(e, n) {
		var r = "";
		W(e) || bs(r), e.data || bs(r), n_(ip(e.data)) || bs(r);
		var i, a = t[0];
		if (a && n === 0 && !e.dimensions) {
			var o = a.startIndex;
			o && (e.data = a.data.slice(0, o).concat(e.data)), i = {
				seriesLayoutBy: Oc,
				sourceHeader: o,
				dimensions: a.metaRawOption.dimensions
			};
		} else i = {
			seriesLayoutBy: Oc,
			sourceHeader: 0,
			dimensions: e.dimensions
		};
		return tp(e.data, i, null);
	});
}
function n_(e) {
	return e === "arrayRows" || e === "objectRows";
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/data/helper/sourceManager.js
var r_ = function() {
	function e(e) {
		this._sourceList = [], this._storeList = [], this._upstreamSignList = [], this._versionSignBase = 0, this._dirty = !0, this._sourceHost = e;
	}
	return e.prototype.dirty = function() {
		this._setLocalSource([], []), this._storeList = [], this._dirty = !0;
	}, e.prototype._setLocalSource = function(e, t) {
		this._sourceList = e, this._upstreamSignList = t, this._versionSignBase++, this._versionSignBase > 9e10 && (this._versionSignBase = 0);
	}, e.prototype._getVersionSign = function() {
		return this._sourceHost.uid + "_" + this._versionSignBase;
	}, e.prototype.prepareSource = function() {
		this._isDirty() && (this._createSource(), this._dirty = !1);
	}, e.prototype._createSource = function() {
		this._setLocalSource([], []);
		var e = this._sourceHost, t = this._getUpstreamSourceManagers(), n = !!t.length, r, i;
		if (i_(e)) {
			var a = e, o = void 0, s = void 0, c = void 0;
			if (n) {
				var l = t[0];
				l.prepareSource(), c = l.getSource(), o = c.data, s = c.sourceFormat, i = [l._getVersionSign()];
			} else o = a.get("data", !0), s = le(o) ? Ec : Sc, i = [];
			var u = this._getSourceMetaRawOption() || {}, d = c && c.metaRawOption || {}, f = G(u.seriesLayoutBy, d.seriesLayoutBy) || null, p = G(u.sourceHeader, d.sourceHeader), m = G(u.dimensions, d.dimensions);
			r = f !== d.seriesLayoutBy || !!p != !!d.sourceHeader || m ? [tp(o, {
				seriesLayoutBy: f,
				sourceHeader: p,
				dimensions: m
			}, s)] : [];
		} else {
			var h = e;
			if (n) {
				var g = this._applyTransform(t);
				r = g.sourceList, i = g.upstreamSignList;
			} else r = [tp(h.get("source", !0), this._getSourceMetaRawOption(), null)], i = [];
		}
		this._setLocalSource(r, i);
	}, e.prototype._applyTransform = function(e) {
		var t = this._sourceHost, n = t.get("transform", !0), r = t.get("fromTransformResult", !0);
		r != null && e.length !== 1 && a_("");
		var i, a = [], o = [];
		return I(e, function(e) {
			e.prepareSource();
			var t = e.getSource(r || 0);
			r != null && !t && a_(""), a.push(t), o.push(e._getVersionSign());
		}), n ? i = e_(n, a, { datasetIndex: t.componentIndex }) : r != null && (i = [rp(a[0])]), {
			sourceList: i,
			upstreamSignList: o
		};
	}, e.prototype._isDirty = function() {
		if (this._dirty) return !0;
		for (var e = this._getUpstreamSourceManagers(), t = 0; t < e.length; t++) {
			var n = e[t];
			if (n._isDirty() || this._upstreamSignList[t] !== n._getVersionSign()) return !0;
		}
	}, e.prototype.getSource = function(e) {
		e ||= 0;
		var t = this._sourceList[e];
		if (!t) {
			var n = this._getUpstreamSourceManagers();
			return n[0] && n[0].getSource(e);
		}
		return t;
	}, e.prototype.getSharedDataStore = function(e) {
		var t = e.makeStoreSchema();
		return this._innerGetDataStore(t.dimensions, e.source, t.hash);
	}, e.prototype._innerGetDataStore = function(e, t, n) {
		var r = 0, i = this._storeList, a = i[r];
		a ||= i[r] = {};
		var o = a[n];
		if (!o) {
			var s = this._getUpstreamSourceManagers()[0];
			i_(this._sourceHost) && s ? o = s._innerGetDataStore(e, t, n) : (o = new Yp(), o.initData(new gp(t, e.length), e)), a[n] = o;
		}
		return o;
	}, e.prototype._getUpstreamSourceManagers = function() {
		var e = this._sourceHost;
		if (i_(e)) {
			var t = Yf(e);
			return t ? [t.getSourceManager()] : [];
		} else return L(Xf(e), function(e) {
			return e.getSourceManager();
		});
	}, e.prototype._getSourceMetaRawOption = function() {
		var e = this._sourceHost, t, n, r;
		if (i_(e)) t = e.get("seriesLayoutBy", !0), n = e.get("sourceHeader", !0), r = e.get("dimensions", !0);
		else if (!this._getUpstreamSourceManagers().length) {
			var i = e;
			t = i.get("seriesLayoutBy", !0), n = i.get("sourceHeader", !0), r = i.get("dimensions", !0);
		}
		return {
			seriesLayoutBy: t,
			sourceHeader: n,
			dimensions: r
		};
	}, e;
}();
function i_(e) {
	return e.mainType === "series";
}
function a_(e) {
	throw Error(e);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/visual/tokens.js
var Q = {
	color: {},
	darkColor: {},
	size: {}
}, o_ = Q.color = {
	theme: [
		"#5070dd",
		"#b6d634",
		"#505372",
		"#ff994d",
		"#0ca8df",
		"#ffd10a",
		"#fb628b",
		"#785db0",
		"#3fbe95"
	],
	neutral00: "#fff",
	neutral05: "#f4f7fd",
	neutral10: "#e8ebf0",
	neutral15: "#dbdee4",
	neutral20: "#cfd2d7",
	neutral25: "#c3c5cb",
	neutral30: "#b7b9be",
	neutral35: "#aaacb2",
	neutral40: "#9ea0a5",
	neutral45: "#929399",
	neutral50: "#86878c",
	neutral55: "#797b7f",
	neutral60: "#6d6e73",
	neutral65: "#616266",
	neutral70: "#54555a",
	neutral75: "#48494d",
	neutral80: "#3c3c41",
	neutral85: "#303034",
	neutral90: "#232328",
	neutral95: "#17171b",
	neutral99: "#000",
	accent05: "#eff1f9",
	accent10: "#e0e4f2",
	accent15: "#d0d6ec",
	accent20: "#c0c9e6",
	accent25: "#b1bbdf",
	accent30: "#a1aed9",
	accent35: "#91a0d3",
	accent40: "#8292cc",
	accent45: "#7285c6",
	accent50: "#6578ba",
	accent55: "#5c6da9",
	accent60: "#536298",
	accent65: "#4a5787",
	accent70: "#404c76",
	accent75: "#374165",
	accent80: "#2e3654",
	accent85: "#252b43",
	accent90: "#1b2032",
	accent95: "#121521",
	transparent: "rgba(0,0,0,0)",
	highlight: "rgba(255,231,130,0.8)"
};
for (var s_ in j(o_, {
	primary: o_.neutral80,
	secondary: o_.neutral70,
	tertiary: o_.neutral60,
	quaternary: o_.neutral50,
	disabled: o_.neutral20,
	border: o_.neutral30,
	borderTint: o_.neutral20,
	borderShade: o_.neutral40,
	background: o_.neutral05,
	backgroundTint: "rgba(234,237,245,0.5)",
	backgroundTransparent: "rgba(255,255,255,0)",
	backgroundShade: o_.neutral10,
	shadow: "rgba(0,0,0,0.2)",
	shadowTint: "rgba(129,130,136,0.2)",
	axisLine: o_.neutral70,
	axisLineTint: o_.neutral40,
	axisTick: o_.neutral70,
	axisTickMinor: o_.neutral60,
	axisLabel: o_.neutral70,
	axisSplitLine: o_.neutral15,
	axisMinorSplitLine: o_.neutral05
}), o_) if (o_.hasOwnProperty(s_)) {
	var c_ = o_[s_];
	s_ === "theme" ? Q.darkColor.theme = o_.theme.slice() : s_ === "highlight" ? Q.darkColor.highlight = "rgba(255,231,130,0.4)" : s_.indexOf("accent") === 0 ? Q.darkColor[s_] = zr(c_, null, function(e) {
		return e * .5;
	}, function(e) {
		return Math.min(1, 1.3 - e);
	}) : Q.darkColor[s_] = zr(c_, null, function(e) {
		return e * .9;
	}, function(e) {
		return 1 - e ** 1.5;
	});
}
Q.size = {
	xxs: 2,
	xs: 5,
	s: 10,
	m: 15,
	l: 20,
	xl: 30,
	xxl: 40,
	xxxl: 50
};
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/tooltip/tooltipMarkup.js
var l_ = "line-height:1";
function u_(e) {
	var t = e.lineHeight;
	return t == null ? l_ : "line-height:" + sh(t + "") + "px";
}
function d_(e, t) {
	var n = e.color || Q.color.tertiary, r = e.fontSize || 12, i = e.fontWeight || "400", a = e.color || Q.color.secondary, o = e.fontSize || 14, s = e.fontWeight || "900";
	return t === "html" ? {
		nameStyle: "font-size:" + sh(r + "") + "px;color:" + sh(n) + ";font-weight:" + sh(i + ""),
		valueStyle: "font-size:" + sh(o + "") + "px;color:" + sh(a) + ";font-weight:" + sh(s + "")
	} : {
		nameStyle: {
			fontSize: r,
			fill: n,
			fontWeight: i
		},
		valueStyle: {
			fontSize: o,
			fill: a,
			fontWeight: s
		}
	};
}
var f_ = [
	0,
	10,
	20,
	30
], p_ = [
	"",
	"\n",
	"\n\n",
	"\n\n\n"
];
function m_(e, t) {
	return t.type = e, t;
}
function h_(e) {
	return e.type === "section";
}
function g_(e) {
	return h_(e) ? v_ : y_;
}
function __(e) {
	if (h_(e)) {
		var t = 0, n = e.blocks.length, r = n > 1 || n > 0 && !e.noHeader;
		return I(e.blocks, function(e) {
			var n = __(e);
			n >= t && (t = n + +(r && (!n || h_(e) && !e.noHeader)));
		}), t;
	}
	return 0;
}
function v_(e, t, n, r) {
	var i = t.noHeader, a = x_(__(t)), o = [], s = t.blocks || [];
	ve(!s || V(s)), s ||= [];
	var c = e.orderMode;
	if (t.sortBlocks && c) {
		s = s.slice();
		var l = {
			valueAsc: "asc",
			valueDesc: "desc"
		};
		if (Ae(l, c)) {
			var u = new Lp(l[c], null);
			s.sort(function(e, t) {
				return u.evaluate(e.sortParam, t.sortParam);
			});
		} else c === "seriesDesc" && s.reverse();
	}
	I(s, function(n, i) {
		var s = t.valueFormatter, c = g_(n)(s ? j(j({}, e), { valueFormatter: s }) : e, n, i > 0 ? a.html : 0, r);
		c != null && o.push(c);
	});
	var d = e.renderMode === "richText" ? o.join(a.richText) : S_(r, o.join(""), i ? n : a.html);
	if (i) return d;
	var f = ug(t.header, "ordinal", e.useUTC), p = d_(r, e.renderMode).nameStyle, m = u_(r);
	return e.renderMode === "richText" ? T_(e, f, p) + a.richText + d : S_(r, "<div style=\"" + p + ";" + m + ";\">" + sh(f) + "</div>" + d, n);
}
function y_(e, t, n, r) {
	var i = e.renderMode, a = t.noName, o = t.noValue, s = !t.markerType, c = t.name, l = e.useUTC, u = t.valueFormatter || e.valueFormatter || function(e) {
		return e = V(e) ? e : [e], L(e, function(e, t) {
			return ug(e, V(p) ? p[t] : p, l);
		});
	};
	if (!(a && o)) {
		var d = s ? "" : e.markupStyleCreator.makeTooltipMarker(t.markerType, t.markerColor || Q.color.secondary, i), f = a ? "" : ug(c, "ordinal", l), p = t.valueType, m = o ? [] : u(t.value, t.rawDataIndex), h = !s || !a, g = !s && a, _ = d_(r, i), v = _.nameStyle, y = _.valueStyle;
		return i === "richText" ? (s ? "" : d) + (a ? "" : T_(e, f, v)) + (o ? "" : E_(e, m, h, g, y)) : S_(r, (s ? "" : d) + (a ? "" : C_(f, !s, v)) + (o ? "" : w_(m, h, g, y)), n);
	}
}
function b_(e, t, n, r, i, a) {
	if (e) return g_(e)({
		useUTC: i,
		renderMode: n,
		orderMode: r,
		markupStyleCreator: t,
		valueFormatter: e.valueFormatter
	}, e, 0, a);
}
function x_(e) {
	return {
		html: f_[e],
		richText: p_[e]
	};
}
function S_(e, t, n) {
	var r = "<div style=\"clear:both\"></div>", i = "margin: " + n + "px 0 0", a = u_(e);
	return "<div style=\"" + i + ";" + a + ";\">" + t + r + "</div>";
}
function C_(e, t, n) {
	var r = t ? "margin-left:2px" : "";
	return "<span style=\"" + n + ";" + r + "\">" + sh(e) + "</span>";
}
function w_(e, t, n, r) {
	var i = t ? "float:right;margin-left:" + (n ? "10px" : "20px") : "";
	return e = V(e) ? e : [e], "<span style=\"" + i + ";" + r + "\">" + L(e, function(e) {
		return sh(e);
	}).join("&nbsp;&nbsp;") + "</span>";
}
function T_(e, t, n) {
	return e.markupStyleCreator.wrapRichTextStyle(t, n);
}
function E_(e, t, n, r, i) {
	var a = [i], o = r ? 10 : 20;
	return n && a.push({
		padding: [
			0,
			0,
			0,
			o
		],
		align: "right"
	}), e.markupStyleCreator.wrapRichTextStyle(V(t) ? t.join("  ") : t, a);
}
function D_(e, t) {
	var n = e.getData().getItemVisual(t, "style")[e.visualDrawType];
	return hg(n);
}
function O_(e, t) {
	return e.get("padding") ?? (t === "richText" ? [8, 10] : 10);
}
var k_ = function() {
	function e() {
		this.richTextStyles = {}, this._nextStyleNameId = ds();
	}
	return e.prototype._generateStyleName = function() {
		return "__EC_aUTo_" + this._nextStyleNameId++;
	}, e.prototype.makeTooltipMarker = function(e, t, n) {
		var r = n === "richText" ? this._generateStyleName() : null, i = mg({
			color: t,
			type: e,
			renderMode: n,
			markerId: r
		});
		return U(i) ? i : (this.richTextStyles[r] = i.style, i.content);
	}, e.prototype.wrapRichTextStyle = function(e, t) {
		var n = {};
		V(t) ? I(t, function(e) {
			return j(n, e);
		}) : j(n, t);
		var r = this._generateStyleName();
		return this.richTextStyles[r] = n, "{" + r + "|" + e + "}";
	}, e;
}();
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/tooltip/seriesFormatTooltip.js
function A_(e) {
	var t = e.series, n = e.dataIndex, r = e.multipleSeries, i = t.getData(), a = i.mapDimensionsAll("defaultedTooltip"), o = a.length, s = t.getRawValue(n), c = V(s), l = D_(t, n), u, d, f, p;
	if (o > 1 || c && !o) {
		var m = j_(s, t, n, a, l);
		u = m.inlineValues, d = m.inlineValueTypes, f = m.blocks, p = m.inlineValues[0];
	} else if (o) {
		var h = i.getDimensionInfo(a[0]);
		p = u = Op(i, n, a[0]), d = h.type;
	} else p = u = c ? s[0] : s;
	var g = zs(t), _ = g && t.name || "", v = i.getName(n), y = r ? _ : v;
	return m_("section", {
		header: _,
		noHeader: r || !g,
		sortParam: p,
		blocks: [m_("nameValue", {
			markerType: "item",
			markerColor: l,
			name: y,
			noName: !ye(y),
			value: u,
			valueType: d,
			rawDataIndex: i.getRawIndex(n)
		})].concat(f || [])
	});
}
function j_(e, t, n, r, i) {
	var a = t.getData(), o = ne(e, function(e, t, n) {
		var r = a.getDimensionInfo(n);
		return e ||= r && r.tooltip !== !1 && r.displayName != null;
	}, !1), s = [], c = [], l = [];
	r.length ? I(r, function(e) {
		u(Op(a, n, e), e);
	}) : I(e, u);
	function u(e, t) {
		var n = a.getDimensionInfo(t);
		!n || n.otherDims.tooltip === !1 || (o ? l.push(m_("nameValue", {
			markerType: "subItem",
			markerColor: i,
			name: n.displayName,
			value: e,
			valueType: n.type
		})) : (s.push(e), c.push(n.type)));
	}
	return {
		inlineValues: s,
		inlineValueTypes: c,
		blocks: l
	};
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/model/Series.js
var M_ = Ws();
function N_(e, t) {
	return e.getName(t) || e.getId(t);
}
var P_ = function(e) {
	o(t, e);
	function t() {
		var t = e !== null && e.apply(this, arguments) || this;
		return t._selectedDataIndicesMap = {}, t;
	}
	return t.prototype.init = function(e, t, n) {
		this.seriesIndex = this.componentIndex, this.dataTask = Ug({
			count: L_,
			reset: R_
		}), this.dataTask.context = { model: this }, this.mergeDefaultAndTheme(e, n), (M_(this).sourceManager = new r_(this)).prepareSource();
		var r = this.getInitialData(e, n);
		B_(r, this), this.dataTask.context.data = r, M_(this).dataBeforeProcessed = r, F_(this), this._initSelectedMapFromData(r);
	}, t.prototype.mergeDefaultAndTheme = function(e, t) {
		var n = Og(this), r = n ? Ag(e) : {}, i = this.subType;
		Ng.hasClass(i) && (i += "Series"), A(e, t.getTheme().get(this.subType)), A(e, this.getDefaultOption()), Ts(e, "label", ["show"]), this.fillDataTextStyle(e.data), n && kg(e, r, n);
	}, t.prototype.mergeOption = function(e, t) {
		e = A(this.option, e, !0), this.fillDataTextStyle(e.data);
		var n = Og(this);
		n && kg(this.option, e, n);
		var r = M_(this).sourceManager;
		r.dirty(), r.prepareSource();
		var i = this.getInitialData(e, t);
		B_(i, this), this.dataTask.dirty(), this.dataTask.context.data = i, M_(this).dataBeforeProcessed = i, F_(this), this._initSelectedMapFromData(i);
	}, t.prototype.fillDataTextStyle = function(e) {
		if (e && !le(e)) for (var t = ["show"], n = 0; n < e.length; n++) e[n] && e[n].label && Ts(e[n], "label", t);
	}, t.prototype.getInitialData = function(e, t) {}, t.prototype.appendData = function(e) {
		this.getRawData().appendData(e.data);
	}, t.prototype.getData = function(e) {
		var t = H_(this);
		if (t) {
			var n = t.context.data;
			return e == null || !n.getLinkedData ? n : n.getLinkedData(e);
		} else return M_(this).data;
	}, t.prototype.getAllData = function() {
		var e = this.getData();
		return e && e.getLinkedDataAll ? e.getLinkedDataAll() : [{ data: e }];
	}, t.prototype.setData = function(e) {
		var t = H_(this);
		if (t) {
			var n = t.context;
			n.outputData = e, t !== this.dataTask && (n.data = e);
		}
		M_(this).data = e;
	}, t.prototype.getEncode = function() {
		var e = this.get("encode", !0);
		if (e) return K(e);
	}, t.prototype.getSourceManager = function() {
		return M_(this).sourceManager;
	}, t.prototype.getSource = function() {
		return this.getSourceManager().getSource();
	}, t.prototype.getRawData = function() {
		return M_(this).dataBeforeProcessed;
	}, t.prototype.getColorBy = function() {
		return this.get("colorBy") || "series";
	}, t.prototype.isColorBySeries = function() {
		return this.getColorBy() === "series";
	}, t.prototype.getBaseAxis = function() {
		var e = this.coordinateSystem;
		return e && e.getBaseAxis && e.getBaseAxis();
	}, t.prototype.indicesOfNearest = function(e, t, n, r) {
		var i = this.getData(), a = this.coordinateSystem, o = a && a.getAxis(e);
		if (!a || !o) return [];
		var s = o.dataToCoord(n);
		r ??= Infinity;
		for (var c = [], l = Infinity, u = -1, d = 0, f = i.getDimensionIndex(t), p = i.getStore(), m = 0, h = p.count(); m < h; m++) {
			var g = p.get(f, m), _ = s - o.dataToCoord(g), v = Math.abs(_);
			v <= r && ((v < l || v === l && _ >= 0 && u < 0) && (l = v, u = _, d = 0), _ === u && (c[d++] = m));
		}
		return c.length = d, c;
	}, t.prototype.formatTooltip = function(e, t, n) {
		return A_({
			series: this,
			dataIndex: e,
			multipleSeries: t
		});
	}, t.prototype.isAnimationEnabled = function() {
		var e = this.ecModel;
		if (q.node && !(e && e.ssr)) return !1;
		var t = this.getShallow("animation");
		return t && this.getData().count() > this.getShallow("animationThreshold") && (t = !1), !!t;
	}, t.prototype.restoreData = function() {
		this.dataTask.dirty();
	}, t.prototype.getColorFromPalette = function(e, t, n) {
		var r = this.ecModel, i = Ig.prototype.getColorFromPalette.call(this, e, t, n);
		return i ||= r.getColorFromPalette(e, t, n), i;
	}, t.prototype.coordDimToDataDim = function(e) {
		return this.getRawData().mapDimensionsAll(e);
	}, t.prototype.getProgressive = function() {
		return this.get("progressive");
	}, t.prototype.getProgressiveThreshold = function() {
		return this.get("progressiveThreshold");
	}, t.prototype.select = function(e, t) {
		this._innerSelect(this.getData(t), e);
	}, t.prototype.unselect = function(e, t) {
		var n = this.option.selectedMap;
		if (n) {
			var r = this.option.selectedMode, i = this.getData(t);
			if (r === "series" || n === "all") {
				this.option.selectedMap = {}, this._selectedDataIndicesMap = {};
				return;
			}
			for (var a = 0; a < e.length; a++) {
				var o = e[a], s = N_(i, o);
				n[s] = !1, this._selectedDataIndicesMap[s] = -1;
			}
		}
	}, t.prototype.toggleSelect = function(e, t) {
		for (var n = [], r = 0; r < e.length; r++) n[0] = e[r], this.isSelected(e[r], t) ? this.unselect(n, t) : this.select(n, t);
	}, t.prototype.getSelectedDataIndices = function() {
		if (this.option.selectedMap === "all") return [].slice.call(this.getData().getIndices());
		for (var e = this._selectedDataIndicesMap, t = R(e), n = [], r = 0; r < t.length; r++) {
			var i = e[t[r]];
			i >= 0 && n.push(i);
		}
		return n;
	}, t.prototype.isSelected = function(e, t) {
		var n = this.option.selectedMap;
		if (!n) return !1;
		var r = this.getData(t);
		return (n === "all" || n[N_(r, e)]) && !r.getItemModel(e).get(["select", "disabled"]);
	}, t.prototype.isUniversalTransitionEnabled = function() {
		if (this.__universalTransitionEnabled) return !0;
		var e = this.option.universalTransition;
		return e ? e === !0 ? !0 : e && e.enabled : !1;
	}, t.prototype._innerSelect = function(e, t) {
		var n, r, i = this.option, a = i.selectedMode, o = t.length;
		if (!(!a || !o)) {
			if (a === "series") i.selectedMap = "all";
			else if (a === "multiple") {
				W(i.selectedMap) || (i.selectedMap = {});
				for (var s = i.selectedMap, c = 0; c < o; c++) {
					var l = t[c], u = N_(e, l);
					s[u] = !0, this._selectedDataIndicesMap[u] = e.getRawIndex(l);
				}
			} else if (a === "single" || a === !0) {
				var d = t[o - 1], u = N_(e, d);
				i.selectedMap = (n = {}, n[u] = !0, n), this._selectedDataIndicesMap = (r = {}, r[u] = e.getRawIndex(d), r);
			}
		}
	}, t.prototype._initSelectedMapFromData = function(e) {
		if (!this.option.selectedMap) {
			var t = [];
			e.hasItemOption && e.each(function(n) {
				var r = e.getRawDataItem(n);
				r && r.selected && t.push(n);
			}), t.length > 0 && this._innerSelect(e, t);
		}
	}, t.registerClass = function(e) {
		return Ng.registerClass(e);
	}, t.protoInitialize = function() {
		var e = t.prototype;
		e.type = "series.__base__", e.seriesIndex = 0, e.ignoreStyleOnData = !1, e.hasSymbolVisual = !1, e.defaultSymbol = "circle", e.visualStyleAccessPath = "itemStyle", e.visualDrawType = "fill";
	}(), t;
}(Ng);
P(P_, Vg), P(P_, Ig), Ue(P_, Ng);
function F_(e) {
	var t = e.name;
	zs(e) || (e.name = I_(e) || t);
}
function I_(e) {
	var t = e.getRawData(), n = t.mapDimensionsAll("seriesName"), r = [];
	return I(n, function(e) {
		var n = t.getDimensionInfo(e);
		n.displayName && r.push(n.displayName);
	}), r.join(" ");
}
function L_(e) {
	return e.model.getRawData().count();
}
function R_(e) {
	var t = e.model;
	return t.setData(t.getRawData().cloneShallow()), z_;
}
function z_(e, t) {
	t.outputData && e.end > t.outputData.count() && t.model.getRawData().cloneShallow(t.outputData);
}
function B_(e, t) {
	I(De(e.CHANGABLE_METHODS, e.DOWNSAMPLE_METHODS), function(n) {
		e.wrapMethod(n, B(V_, t));
	});
}
function V_(e, t) {
	var n = H_(e);
	return n && n.setOutputEnd((t || this).count()), t;
}
function H_(e) {
	var t = (e.ecModel || {}).scheduler, n = t && t.getPipeline(e.uid);
	if (n) {
		var r = n.currentTask;
		if (r) {
			var i = r.agentStubMap;
			i && (r = i.get(e.uid));
		}
		return r;
	}
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/util/symbol.js
var U_ = Za.extend({
	type: "triangle",
	shape: {
		cx: 0,
		cy: 0,
		width: 0,
		height: 0
	},
	buildPath: function(e, t) {
		var n = t.cx, r = t.cy, i = t.width / 2, a = t.height / 2;
		e.moveTo(n, r - a), e.lineTo(n + i, r + a), e.lineTo(n - i, r + a), e.closePath();
	}
}), W_ = {
	line: zu,
	rect: fo,
	roundRect: fo,
	square: fo,
	circle: lu,
	diamond: Za.extend({
		type: "diamond",
		shape: {
			cx: 0,
			cy: 0,
			width: 0,
			height: 0
		},
		buildPath: function(e, t) {
			var n = t.cx, r = t.cy, i = t.width / 2, a = t.height / 2;
			e.moveTo(n, r - a), e.lineTo(n + i, r), e.lineTo(n, r + a), e.lineTo(n - i, r), e.closePath();
		}
	}),
	pin: Za.extend({
		type: "pin",
		shape: {
			x: 0,
			y: 0,
			width: 0,
			height: 0
		},
		buildPath: function(e, t) {
			var n = t.x, r = t.y, i = t.width / 5 * 3, a = Math.max(i, t.height), o = i / 2, s = o * o / (a - o), c = r - a + o + s, l = Math.asin(s / o), u = Math.cos(l) * o, d = Math.sin(l), f = Math.cos(l), p = o * .6, m = o * .7;
			e.moveTo(n - u, c + s), e.arc(n, c, o, Math.PI - l, Math.PI * 2 + l), e.bezierCurveTo(n + u - d * p, c + s + f * p, n, r - m, n, r), e.bezierCurveTo(n, r - m, n - u + d * p, c + s + f * p, n - u, c + s), e.closePath();
		}
	}),
	arrow: Za.extend({
		type: "arrow",
		shape: {
			x: 0,
			y: 0,
			width: 0,
			height: 0
		},
		buildPath: function(e, t) {
			var n = t.height, r = t.width, i = t.x, a = t.y, o = r / 3 * 2;
			e.moveTo(i, a), e.lineTo(i + o, a + n), e.lineTo(i, a + n / 4 * 3), e.lineTo(i - o, a + n), e.lineTo(i, a), e.closePath();
		}
	}),
	triangle: U_
}, G_ = {
	line: function(e, t, n, r, i) {
		i.x1 = e, i.y1 = t + r / 2, i.x2 = e + n, i.y2 = t + r / 2;
	},
	rect: function(e, t, n, r, i) {
		i.x = e, i.y = t, i.width = n, i.height = r;
	},
	roundRect: function(e, t, n, r, i) {
		i.x = e, i.y = t, i.width = n, i.height = r, i.r = Math.min(n, r) / 4;
	},
	square: function(e, t, n, r, i) {
		var a = Math.min(n, r);
		i.x = e, i.y = t, i.width = a, i.height = a;
	},
	circle: function(e, t, n, r, i) {
		i.cx = e + n / 2, i.cy = t + r / 2, i.r = Math.min(n, r) / 2;
	},
	diamond: function(e, t, n, r, i) {
		i.cx = e + n / 2, i.cy = t + r / 2, i.width = n, i.height = r;
	},
	pin: function(e, t, n, r, i) {
		i.x = e + n / 2, i.y = t + r / 2, i.width = n, i.height = r;
	},
	arrow: function(e, t, n, r, i) {
		i.x = e + n / 2, i.y = t + r / 2, i.width = n, i.height = r;
	},
	triangle: function(e, t, n, r, i) {
		i.cx = e + n / 2, i.cy = t + r / 2, i.width = n, i.height = r;
	}
}, K_ = {};
I(W_, function(e, t) {
	K_[t] = new e();
});
var q_ = Za.extend({
	type: "symbol",
	shape: {
		symbolType: "",
		x: 0,
		y: 0,
		width: 0,
		height: 0
	},
	calculateTextPosition: function(e, t, n) {
		var r = fn(e, t, n), i = this.shape;
		return i && i.symbolType === "pin" && t.position === "inside" && (r.y = n.y + n.height * .4), r;
	},
	buildPath: function(e, t, n) {
		var r = t.symbolType;
		if (r !== "none") {
			var i = K_[r];
			i ||= (r = "rect", K_[r]), G_[r](t.x, t.y, t.width, t.height, i.shape), i.buildPath(e, i.shape, n);
		}
	}
});
function J_(e, t) {
	if (this.type !== "image") {
		var n = this.style;
		this.__isEmptyBrush ? (n.stroke = e, n.fill = t || Q.color.neutral00, n.lineWidth = 2) : this.shape.symbolType === "line" ? n.stroke = e : n.fill = e, this.markRedraw();
	}
}
function Y_(e, t, n, r, i, a, o) {
	var s = e.indexOf("empty") === 0;
	s && (e = e.substr(5, 1).toLowerCase() + e.substr(6));
	var c = e.indexOf("image://") === 0 ? Dd(e.slice(8), new Y(t, n, r, i), o ? "center" : "cover") : e.indexOf("path://") === 0 ? Ed(e.slice(7), {}, new Y(t, n, r, i), o ? "center" : "cover") : new q_({ shape: {
		symbolType: e,
		x: t,
		y: n,
		width: r,
		height: i
	} });
	return c.__isEmptyBrush = s, c.setColor = J_, a && c.setColor(a), c;
}
function X_(e) {
	return V(e) || (e = [+e, +e]), [e[0] || 0, e[1] || 0];
}
function Z_(e, t) {
	if (e != null) return V(e) || (e = [e, e]), [X(e[0], t[0]) || 0, X(G(e[1], e[0]), t[1]) || 0];
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/line/LineSeries.js
var Q_ = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n.hasSymbolVisual = !0, n;
	}
	return t.prototype.getInitialData = function(e) {
		return Bm(null, this, { useEncodeDefaulter: !0 });
	}, t.prototype.getLegendIcon = function(e) {
		var t = new su(), n = Y_("line", 0, e.itemHeight / 2, e.itemWidth, 0, e.lineStyle.stroke, !1);
		t.add(n), n.setStyle(e.lineStyle);
		var r = this.getData().getVisual("symbol"), i = this.getData().getVisual("symbolRotate"), a = r === "none" ? "circle" : r, o = e.itemHeight * .8, s = Y_(a, (e.itemWidth - o) / 2, (e.itemHeight - o) / 2, o, o, e.itemStyle.fill);
		return t.add(s), s.setStyle(e.itemStyle), s.rotation = (e.iconRotate === "inherit" ? i : e.iconRotate || 0) * Math.PI / 180, s.setOrigin([e.itemWidth / 2, e.itemHeight / 2]), a.indexOf("empty") > -1 && (s.style.stroke = s.style.fill, s.style.fill = Q.color.neutral00, s.style.lineWidth = 2), t;
	}, t.type = "series.line", t.dependencies = ["grid", "polar"], t.defaultOption = {
		z: 3,
		coordinateSystem: "cartesian2d",
		legendHoverLink: !0,
		clip: !0,
		label: { position: "top" },
		endLabel: {
			show: !1,
			valueAnimation: !0,
			distance: 8
		},
		lineStyle: {
			width: 2,
			type: "solid"
		},
		emphasis: { scale: !0 },
		step: !1,
		smooth: !1,
		smoothMonotone: null,
		symbol: "emptyCircle",
		symbolSize: 6,
		symbolRotate: null,
		showSymbol: !0,
		showAllSymbol: "auto",
		connectNulls: !1,
		sampling: "none",
		animationEasing: "linear",
		progressive: 0,
		hoverLayerThreshold: Infinity,
		universalTransition: { divideShape: "clone" },
		triggerLineEvent: !1,
		triggerEvent: !1
	}, t;
}(P_);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/helper/labelHelper.js
function $_(e, t) {
	var n = e.mapDimensionsAll("defaultedLabel"), r = n.length;
	if (r === 1) {
		var i = Op(e, t, n[0]);
		return i == null ? null : i + "";
	} else if (r) {
		for (var a = [], o = 0; o < n.length; o++) a.push(Op(e, t, n[o]));
		return a.join(" ");
	}
}
function ev(e, t) {
	var n = e.mapDimensionsAll("defaultedLabel");
	if (!V(t)) return t + "";
	for (var r = [], i = 0; i < n.length; i++) {
		var a = e.getDimensionIndex(n[i]);
		a >= 0 && r.push(t[a]);
	}
	return r.join(" ");
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/helper/Symbol.js
var tv = function(e) {
	o(t, e);
	function t(t, n, r, i) {
		var a = e.call(this) || this;
		return a.updateData(t, n, r, i), a;
	}
	return t.prototype._createSymbol = function(e, t, n, r, i, a) {
		this.removeAll();
		var o = Y_(e, -1, -1, 2, 2, null, a);
		o.attr({
			z2: G(i, 100),
			culling: !0,
			scaleX: r[0] / 2,
			scaleY: r[1] / 2
		}), o.drift = nv, this._symbolType = e, this.add(o);
	}, t.prototype.stopSymbolAnimation = function(e) {
		this.childAt(0).stopAnimation(null, e);
	}, t.prototype.getSymbolType = function() {
		return this._symbolType;
	}, t.prototype.getSymbolPath = function() {
		return this.childAt(0);
	}, t.prototype.highlight = function() {
		ll(this.childAt(0));
	}, t.prototype.downplay = function() {
		ul(this.childAt(0));
	}, t.prototype.setZ = function(e, t) {
		var n = this.childAt(0);
		n.zlevel = e, n.z = t;
	}, t.prototype.setDraggable = function(e, t) {
		var n = this.childAt(0);
		n.draggable = e, n.cursor = !t && e ? "move" : n.cursor;
	}, t.prototype.updateData = function(e, n, r, i) {
		this.silent = !1;
		var a = e.getItemVisual(n, "symbol") || "circle", o = e.hostModel, s = t.getSymbolSize(e, n), c = t.getSymbolZ2(e, n), l = a !== this._symbolType, u = i && i.disableAnimation;
		if (l) {
			var d = e.getItemVisual(n, "symbolKeepAspect");
			this._createSymbol(a, e, n, s, c, d);
		} else {
			var f = this.childAt(0);
			f.silent = !1;
			var p = {
				scaleX: s[0] / 2,
				scaleY: s[1] / 2
			};
			u ? f.attr(p) : ud(f, p, o, n), gd(f);
		}
		if (this._updateCommon(e, n, s, r, i), l) {
			var f = this.childAt(0);
			if (!u) {
				var p = {
					scaleX: this._sizeX,
					scaleY: this._sizeY,
					style: { opacity: f.style.opacity }
				};
				f.scaleX = f.scaleY = 0, f.style.opacity = 0, dd(f, p, o, n);
			}
		}
		u && this.childAt(0).stopAnimation("leave");
	}, t.prototype._updateCommon = function(e, t, n, r, i) {
		var a = this.childAt(0), o = e.hostModel, s, c, l, u, d, f, p, m, h;
		if (r && (s = r.emphasisItemStyle, c = r.blurItemStyle, l = r.selectItemStyle, u = r.focus, d = r.blurScope, p = r.labelStatesModels, m = r.hoverScale, h = r.cursorStyle, f = r.emphasisDisabled), !r || e.hasItemOption) {
			var g = r && r.itemModel ? r.itemModel : e.getItemModel(t), _ = g.getModel("emphasis");
			s = _.getModel("itemStyle").getItemStyle(), l = g.getModel(["select", "itemStyle"]).getItemStyle(), c = g.getModel(["blur", "itemStyle"]).getItemStyle(), u = _.get("focus"), d = _.get("blurScope"), f = _.get("disabled"), p = gf(g), m = _.getShallow("scale"), h = g.getShallow("cursor");
		}
		var v = e.getItemVisual(t, "symbolRotate");
		a.attr("rotation", (v || 0) * Math.PI / 180 || 0);
		var y = Z_(e.getItemVisual(t, "symbolOffset"), n);
		y && (a.x = y[0], a.y = y[1]), h && a.attr("cursor", h);
		var b = e.getItemVisual(t, "style"), x = b.fill;
		if (a instanceof ro) {
			var S = a.style;
			a.useStyle(j({
				image: S.image,
				x: S.x,
				y: S.y,
				width: S.width,
				height: S.height
			}, b));
		} else a.__isEmptyBrush ? a.useStyle(j({}, b)) : a.useStyle(b), a.style.decal = null, a.setColor(x, i && i.symbolInnerColor), a.style.strokeNoScale = !0;
		var C = e.getItemVisual(t, "liftZ"), w = this._z2;
		C == null ? w != null && (a.z2 = w, this._z2 = null) : w ?? (this._z2 = a.z2, a.z2 += C);
		var T = i && i.useNameLabel;
		hf(a, p, {
			labelFetcher: o,
			labelDataIndex: t,
			defaultText: E,
			inheritColor: x,
			defaultOpacity: b.opacity
		});
		function E(t) {
			return T ? e.getName(t) : $_(e, t);
		}
		this._sizeX = n[0] / 2, this._sizeY = n[1] / 2;
		var D = a.ensureState("emphasis");
		D.style = s, a.ensureState("select").style = l, a.ensureState("blur").style = c;
		var O = m == null || m === !0 ? Math.max(1.1, 3 / this._sizeY) : isFinite(m) && m > 0 ? +m : 1;
		D.scaleX = this._sizeX * O, D.scaleY = this._sizeY * O, this.setSymbolScale(1), Ol(this, u, d, f);
	}, t.prototype.setSymbolScale = function(e) {
		this.scaleX = this.scaleY = e;
	}, t.prototype.fadeOut = function(e, t, n) {
		var r = this.childAt(0), i = yc(this).dataIndex, a = n && n.animation;
		if (this.silent = r.silent = !0, n && n.fadeLabel) {
			var o = r.getTextContent();
			o && pd(o, { style: { opacity: 0 } }, t, {
				dataIndex: i,
				removeOpt: a,
				cb: function() {
					r.removeTextContent();
				}
			});
		} else r.removeTextContent();
		pd(r, {
			style: { opacity: 0 },
			scaleX: 0,
			scaleY: 0
		}, t, {
			dataIndex: i,
			cb: e,
			removeOpt: a
		});
	}, t.getSymbolSize = function(e, t) {
		return X_(e.getItemVisual(t, "symbolSize"));
	}, t.getSymbolZ2 = function(e, t) {
		return e.getItemVisual(t, "z2");
	}, t;
}(su);
function nv(e, t) {
	this.parent.drift(e, t);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/helper/SymbolDraw.js
function rv(e, t, n, r) {
	return t && !isNaN(t[0]) && !isNaN(t[1]) && !(r && r.isIgnore && r.isIgnore(n)) && !(r && r.clipShape && !r.clipShape.contain(t[0], t[1])) && e.getItemVisual(n, "symbol") !== "none";
}
function iv(e) {
	return e != null && !W(e) && (e = { isIgnore: e }), e || {};
}
function av(e) {
	var t = e.hostModel, n = t.getModel("emphasis");
	return {
		emphasisItemStyle: n.getModel("itemStyle").getItemStyle(),
		blurItemStyle: t.getModel(["blur", "itemStyle"]).getItemStyle(),
		selectItemStyle: t.getModel(["select", "itemStyle"]).getItemStyle(),
		focus: n.get("focus"),
		blurScope: n.get("blurScope"),
		emphasisDisabled: n.get("disabled"),
		hoverScale: n.get("scale"),
		labelStatesModels: gf(t),
		cursorStyle: t.get("cursor")
	};
}
function ov(e, t, n, r, i, a, o) {
	var s = new e(t, n, r, i);
	return s.setPosition(a), t.setItemGraphicEl(n, s), o.add(s), s;
}
var sv = function() {
	function e(e) {
		this.group = new su(), this._SymbolCtor = e || tv;
	}
	return e.prototype.updateData = function(e, t) {
		this._progressiveEls = null, t = iv(t);
		var n = this.group, r = e.hostModel, i = this._data, a = this._SymbolCtor, o = t.disableAnimation, s = this._seriesScope = av(e), c = { disableAnimation: o }, l = t.getSymbolPoint || function(t) {
			return e.getItemLayout(t);
		};
		i || n.removeAll(), e.diff(i).add(function(r) {
			var i = l(r);
			rv(e, i, r, t) && ov(a, e, r, s, c, i, n);
		}).update(function(u, d) {
			var f = i.getItemGraphicEl(d), p = l(u);
			if (!rv(e, p, u, t)) {
				n.remove(f);
				return;
			}
			var m = e.getItemVisual(u, "symbol") || "circle", h = f && f.getSymbolType && f.getSymbolType();
			if (!f || h && h !== m) n.remove(f), f = new a(e, u, s, c), f.setPosition(p);
			else {
				f.updateData(e, u, s, c);
				var g = {
					x: p[0],
					y: p[1]
				};
				o ? f.attr(g) : ud(f, g, r);
			}
			n.add(f), e.setItemGraphicEl(u, f);
		}).remove(function(e) {
			var t = i.getItemGraphicEl(e);
			t && t.fadeOut(function() {
				n.remove(t);
			}, r);
		}).execute(), this._getSymbolPoint = l, this._data = e;
	}, e.prototype.updateLayout = function(e) {
		var t = this._data;
		if (t) for (var n = this, r = t.getStore(), i = 0, a = r.count(); i < a; i++) {
			var o = t.getItemGraphicEl(i), s = n._getSymbolPoint(i);
			rv(t, s, i, e) ? (o ||= ov(n._SymbolCtor, t, i, n._seriesScope, { disableAnimation: !0 }, s, n.group), o.stopAnimation(), o.setPosition(s), o.markRedraw()) : o && (n.group.remove(o), t.setItemGraphicEl(i, null));
		}
	}, e.prototype.incrementalPrepareUpdate = function(e) {
		this._seriesScope = av(e), this._data = null, this.group.removeAll();
	}, e.prototype.incrementalUpdate = function(e, t, n, r) {
		this._progressiveEls = [], r = iv(r);
		function i(e) {
			e.isGroup || (e.incremental = n, e.ensureState("emphasis").hoverLayer = 2);
		}
		for (var a = e.start; a < e.end; a++) {
			var o = t.getItemLayout(a);
			if (rv(t, o, a, r)) {
				var s = new this._SymbolCtor(t, a, this._seriesScope);
				s.traverse(i), s.setPosition(o), this.group.add(s), t.setItemGraphicEl(a, s), this._progressiveEls.push(s);
			}
		}
	}, e.prototype.eachRendered = function(e) {
		Qd(this._progressiveEls || this.group, e);
	}, e.prototype.remove = function(e) {
		var t = this.group, n = this._data;
		n && e ? n.eachItemGraphicEl(function(e) {
			e.fadeOut(function() {
				t.remove(e);
			}, n.hostModel);
		}) : t.removeAll();
	}, e;
}();
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/line/helper.js
function cv(e, t, n) {
	var r = e.getBaseAxis(), i = e.getOtherAxis(r), a = lv(i, n), o = r.dim, s = i.dim, c = t.mapDimension(s), l = t.mapDimension(o), u = +(s === "x" || s === "radius"), d = L(e.dimensions, function(e) {
		return t.mapDimension(e);
	}), f = !1, p = t.getCalculationInfo("stackResultDimension");
	return Im(t, d[0]) && (f = !0, d[0] = p), Im(t, d[1]) && (f = !0, d[1] = p), {
		dataDimsForPoint: d,
		valueStart: a,
		valueAxisDim: s,
		baseAxisDim: o,
		stacked: !!f,
		valueDim: c,
		baseDim: l,
		baseDataOffset: u,
		stackedOverDimension: t.getCalculationInfo("stackedOverDimension")
	};
}
function lv(e, t) {
	var n = 0, r = e.scale.getExtent();
	return t === "start" ? n = r[0] : t === "end" ? n = r[1] : se(t) && !isNaN(t) ? n = t : r[0] > 0 ? n = r[0] : r[1] < 0 && (n = r[1]), n;
}
function uv(e, t, n, r) {
	var i = NaN;
	e.stacked && (i = n.get(n.getCalculationInfo("stackedOverDimension"), r)), isNaN(i) && (i = e.valueStart);
	var a = e.baseDataOffset, o = [];
	return o[a] = n.get(e.baseDim, r), o[1 - a] = i, t.dataToPoint(o);
}
function dv(e, t) {
	return !isFinite(e) || !isFinite(t);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/util/vendor.js
var fv = typeof Float32Array < "u" ? Float32Array : void 0, pv = typeof Float64Array < "u" ? Float64Array : void 0;
function mv(e) {
	return hv({ ctor: fv }, e).arr;
}
function hv(e, t) {
	var n = e.arr, r = e.ctor;
	if (t > ts && (t = ts), !n || e.typed && n.length < t) {
		var i = void 0;
		if (r) try {
			i = new r(t), e.typed = !0, n && i.set(n);
		} catch {}
		if (!i && (i = [], e.typed = !1, n)) for (var a = 0, o = n.length; a < o; a++) i[a] = n[a];
		e.arr = i;
	}
	return e;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/line/lineAnimationDiff.js
function gv(e, t) {
	var n = [];
	return t.diff(e).add(function(e) {
		n.push({
			cmd: "+",
			idx: e
		});
	}).update(function(e, t) {
		n.push({
			cmd: "=",
			idx: t,
			idx1: e
		});
	}).remove(function(e) {
		n.push({
			cmd: "-",
			idx: e
		});
	}).execute(), n;
}
function _v(e, t, n, r, i, a, o, s) {
	for (var c = gv(e, t), l = [], u = [], d = [], f = [], p = [], m = [], h = [], g = cv(i, t, o), _ = e.getLayout("points") || [], v = t.getLayout("points") || [], y = 0; y < c.length; y++) {
		var b = c[y], x = !0, S = void 0, C = void 0;
		switch (b.cmd) {
			case "=":
				S = b.idx * 2, C = b.idx1 * 2;
				var w = _[S], T = _[S + 1], E = v[C], D = v[C + 1];
				(isNaN(w) || isNaN(T)) && (w = E, T = D), l.push(w, T), u.push(E, D), d.push(n[S], n[S + 1]), f.push(r[C], r[C + 1]), h.push(t.getRawIndex(b.idx1));
				break;
			case "+":
				var O = b.idx, k = g.dataDimsForPoint, A = i.dataToPoint([t.get(k[0], O), t.get(k[1], O)]);
				C = O * 2, l.push(A[0], A[1]), u.push(v[C], v[C + 1]);
				var j = uv(g, i, t, O);
				d.push(j[0], j[1]), f.push(r[C], r[C + 1]), h.push(t.getRawIndex(O));
				break;
			case "-": x = !1;
		}
		x && (p.push(b), m.push(m.length));
	}
	m.sort(function(e, t) {
		return h[e] - h[t];
	});
	for (var ee = l.length, M = mv(ee), N = mv(ee), te = mv(ee), P = mv(ee), F = [], y = 0; y < m.length; y++) {
		var I = m[y], L = y * 2, ne = I * 2;
		M[L] = l[ne], M[L + 1] = l[ne + 1], N[L] = u[ne], N[L + 1] = u[ne + 1], te[L] = d[ne], te[L + 1] = d[ne + 1], P[L] = f[ne], P[L + 1] = f[ne + 1], F[y] = p[I];
	}
	return {
		current: M,
		next: N,
		stackedOnCurrent: te,
		stackedOnNext: P,
		status: F
	};
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/line/poly.js
var vv = Math.min, yv = Math.max;
function bv(e, t, n, r, i, a, o, s, c) {
	for (var l, u, d, f, p, m, h = n, g = 0; g < r; g++) {
		var _ = t[h * 2], v = t[h * 2 + 1];
		if (h >= i || h < 0) break;
		if (dv(_, v)) {
			if (c) {
				h += a;
				continue;
			}
			break;
		}
		if (h === n) e[a > 0 ? "moveTo" : "lineTo"](_, v), d = _, f = v;
		else {
			var y = _ - l, b = v - u;
			if (y * y + b * b < .5) {
				h += a;
				continue;
			}
			if (o > 0) {
				for (var x = h + a, S = t[x * 2], C = t[x * 2 + 1]; S === _ && C === v && g < r;) g++, x += a, h += a, S = t[x * 2], C = t[x * 2 + 1], _ = t[h * 2], v = t[h * 2 + 1], y = _ - l, b = v - u;
				var w = g + 1;
				if (c) for (; dv(S, C) && w < r;) w++, x += a, S = t[x * 2], C = t[x * 2 + 1];
				var T = .5, E = 0, D = 0, O = void 0, k = void 0;
				if (w >= r || dv(S, C)) p = _, m = v;
				else {
					E = S - l, D = C - u;
					var A = _ - l, j = S - _, ee = v - u, M = C - v, N = void 0, te = void 0;
					if (s === "x") {
						N = Math.abs(A), te = Math.abs(j);
						var P = E > 0 ? 1 : -1;
						p = _ - P * N * o, m = v, O = _ + P * te * o, k = v;
					} else if (s === "y") {
						N = Math.abs(ee), te = Math.abs(M);
						var F = D > 0 ? 1 : -1;
						p = _, m = v - F * N * o, O = _, k = v + F * te * o;
					} else N = Math.sqrt(A * A + ee * ee), te = Math.sqrt(j * j + M * M), T = te / (te + N), p = _ - E * o * (1 - T), m = v - D * o * (1 - T), O = _ + E * o * T, k = v + D * o * T, O = vv(O, yv(S, _)), k = vv(k, yv(C, v)), O = yv(O, vv(S, _)), k = yv(k, vv(C, v)), E = O - _, D = k - v, p = _ - E * N / te, m = v - D * N / te, p = vv(p, yv(l, _)), m = vv(m, yv(u, v)), p = yv(p, vv(l, _)), m = yv(m, vv(u, v)), E = _ - p, D = v - m, O = _ + E * te / N, k = v + D * te / N;
				}
				e.bezierCurveTo(d, f, p, m, _, v), d = O, f = k;
			} else e.lineTo(_, v);
		}
		l = _, u = v, h += a;
	}
	return g;
}
var xv = function() {
	function e() {
		this.smooth = 0, this.smoothConstraint = !0;
	}
	return e;
}(), Sv = function(e) {
	o(t, e);
	function t(t) {
		var n = e.call(this, t) || this;
		return n.type = "ec-polyline", n;
	}
	return t.prototype.getDefaultStyle = function() {
		return {
			stroke: Q.color.neutral99,
			fill: null
		};
	}, t.prototype.getDefaultShape = function() {
		return new xv();
	}, t.prototype.buildPath = function(e, t) {
		var n = t.points, r = 0, i = n.length / 2;
		if (t.connectNulls) {
			for (; i > 0 && dv(n[i * 2 - 2], n[i * 2 - 1]); i--);
			for (; r < i && dv(n[r * 2], n[r * 2 + 1]); r++);
		}
		for (; r < i;) r += bv(e, n, r, i, i, 1, t.smooth, t.smoothMonotone, t.connectNulls) + 1;
	}, t.prototype.getPointOn = function(e, t) {
		this.path || (this.createPathProxy(), this.buildPath(this.path, this.shape));
		for (var n = this.path.data, r = Ea.CMD, i, a, o = t === "x", s = [], c = 0; c < n.length;) {
			var l = n[c++], u = void 0, d = void 0, f = void 0, p = void 0, m = void 0, h = void 0, g = void 0;
			switch (l) {
				case r.M:
					i = n[c++], a = n[c++];
					break;
				case r.L:
					if (u = n[c++], d = n[c++], g = o ? (e - i) / (u - i) : (e - a) / (d - a), g <= 1 && g >= 0) {
						var _ = o ? (d - a) * g + a : (u - i) * g + i;
						return o ? [e, _] : [_, e];
					}
					i = u, a = d;
					break;
				case r.C:
					u = n[c++], d = n[c++], f = n[c++], p = n[c++], m = n[c++], h = n[c++];
					var v = o ? or(i, u, f, m, e, s) : or(a, d, p, h, e, s);
					if (v > 0) for (var y = 0; y < v; y++) {
						var b = s[y];
						if (b <= 1 && b >= 0) {
							var _ = o ? ir(a, d, p, h, b) : ir(i, u, f, m, b);
							return o ? [e, _] : [_, e];
						}
					}
					i = m, a = h;
					break;
			}
		}
	}, t;
}(Za), Cv = function(e) {
	o(t, e);
	function t() {
		return e !== null && e.apply(this, arguments) || this;
	}
	return t;
}(xv), wv = function(e) {
	o(t, e);
	function t(t) {
		var n = e.call(this, t) || this;
		return n.type = "ec-polygon", n;
	}
	return t.prototype.getDefaultShape = function() {
		return new Cv();
	}, t.prototype.buildPath = function(e, t) {
		var n = t.points, r = t.stackedOnPoints, i = 0, a = n.length / 2, o = t.smoothMonotone;
		if (t.connectNulls) {
			for (; a > 0 && dv(n[a * 2 - 2], n[a * 2 - 1]); a--);
			for (; i < a && dv(n[i * 2], n[i * 2 + 1]); i++);
		}
		for (; i < a;) {
			var s = bv(e, n, i, a, a, 1, t.smooth, o, t.connectNulls);
			bv(e, r, i + s - 1, s, a, -1, t.stackedOnSmooth, o, t.connectNulls), i += s + 1, e.closePath();
		}
	}, t;
}(Za);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/helper/createRenderPlanner.js
function Tv() {
	var e = Ws();
	return function(t) {
		var n = e(t), r = t.pipelineContext, i = !!n.large, a = !!n.progressiveRender, o = n.large = !!(r && r.large), s = n.progressiveRender = !!(r && r.progressiveRender);
		return (i !== o || a !== s) && "reset";
	};
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/view/Chart.js
var Ev = Ws(), Dv = Tv(), Ov = function() {
	function e() {
		this.group = new su(), this.uid = Wm("viewChart"), this.renderTask = Ug({
			plan: jv,
			reset: Mv
		}), this.renderTask.context = { view: this };
	}
	return e.prototype.init = function(e, t) {}, e.prototype.render = function(e, t, n, r) {}, e.prototype.highlight = function(e, t, n, r) {
		var i = e.getData(r && r.dataType);
		i && Av(i, r, "emphasis");
	}, e.prototype.downplay = function(e, t, n, r) {
		var i = e.getData(r && r.dataType);
		i && Av(i, r, "normal");
	}, e.prototype.remove = function(e, t) {
		this.group.removeAll();
	}, e.prototype.dispose = function(e, t) {}, e.prototype.updateView = function(e, t, n, r) {
		this.render(e, t, n, r);
	}, e.prototype.updateVisual = function(e, t, n, r) {
		this.render(e, t, n, r);
	}, e.prototype.eachRendered = function(e) {
		Qd(this.group, e);
	}, e.markUpdateMethod = function(e, t) {
		Ev(e).updateMethod = t;
	}, e.protoInitialize = function() {
		var t = e.prototype;
		t.type = "chart";
	}(), e;
}();
function kv(e, t, n) {
	e && Pl(e) && (t === "emphasis" ? ll : ul)(e, n);
}
function Av(e, t, n) {
	var r = Us(e, t), i = t && t.highlightKey != null ? Fl(t.highlightKey) : null;
	r == null ? e.eachItemGraphicEl(function(e) {
		kv(e, n, i);
	}) : I(ws(r), function(t) {
		kv(e.getItemGraphicEl(t), n, i);
	});
}
Ve(Ov, ["dispose"]), Je(Ov);
function jv(e) {
	return Dv(e.model);
}
function Mv(e) {
	var t = e.model, n = e.ecModel, r = e.api, i = e.payload, a = t.pipelineContext.progressiveRender, o = e.view, s = i && Ev(i).updateMethod, c = a ? "incrementalPrepareRender" : s && o[s] ? s : "render";
	return c !== "render" && o[c](t, n, r, i), Nv[c];
}
var Nv = {
	incrementalPrepareRender: { progress: function(e, t) {
		t.view.incrementalRender(e, t.model, t.ecModel, t.api, t.payload);
	} },
	render: {
		forceFirstProgress: !0,
		progress: function(e, t) {
			t.view.render(t.model, t.ecModel, t.api, t.payload);
		}
	}
};
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/helper/createClipPathFromCoordSys.js
function Pv(e, t, n, r, i) {
	var a = e.getArea(), o = a.x, s = a.y, c = a.width, l = a.height, u = n.get(["lineStyle", "width"]) || 0;
	o -= u / 2, s -= u / 2, c += u, l += u, c = Math.ceil(c), o !== Math.floor(o) && (o = Math.floor(o), c++);
	var d = new fo({ shape: {
		x: o,
		y: s,
		width: c,
		height: l
	} });
	if (t) {
		var f = e.getBaseAxis(), p = f.isHorizontal(), m = f.inverse;
		p ? (m && (d.shape.x += c), d.shape.width = 0) : (m || (d.shape.y += l), d.shape.height = 0);
		var h = H(i) ? function(e) {
			i(e, d);
		} : null;
		dd(d, { shape: {
			width: c,
			height: l,
			x: o,
			y: s
		} }, n, null, r, h);
	}
	return d;
}
function Fv(e, t, n) {
	var r = e.getArea(), i = Z(r.r0, 1), a = Z(r.r, 1), o = new Ou({ shape: {
		cx: Z(e.cx, 1),
		cy: Z(e.cy, 1),
		r0: i,
		r: a,
		startAngle: r.startAngle,
		endAngle: r.endAngle,
		clockwise: r.clockwise
	} });
	return t && (e.getBaseAxis().dim === "angle" ? o.shape.endAngle = r.startAngle : o.shape.r = i, dd(o, { shape: {
		endAngle: r.endAngle,
		r: a
	} }, n)), o;
}
function Iv(e, t, n, r, i) {
	return e ? e.type === "polar" ? Fv(e, t, n) : e.type === "cartesian2d" ? Pv(e, t, n, r, i) : null : null;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/coord/CoordinateSystem.js
function Lv(e, t) {
	return e.type === t;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/scale/Scale.js
var Rv = function() {
	function e() {}
	return e.prototype.isBlank = function() {
		return this._isBlank;
	}, e.prototype.setBlank = function(e) {
		this._isBlank = e;
	}, e;
}();
Je(Rv);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/data/OrdinalMeta.js
var zv = 0, Bv = function() {
	function e(e) {
		this.categories = e.categories || [], this._needCollect = e.needCollect, this._deduplication = e.deduplication, this.uid = ++zv, this._onCollect = e.onCollect;
	}
	return e.createByAxisModel = function(t) {
		var n = t.option, r = n.data, i = r && L(r, Vv);
		return new e({
			categories: i,
			needCollect: !i,
			deduplication: n.dedplication !== !1
		});
	}, e.prototype.getOrdinal = function(e) {
		return this._getOrCreateMap().get(e);
	}, e.prototype.parseAndCollect = function(e) {
		var t, n = this._needCollect;
		if (!U(e) && !n) return e;
		if (n && !this._deduplication) return t = this.categories.length, this.categories[t] = e, this._onCollect && this._onCollect(e, t), t;
		var r = this._getOrCreateMap();
		return t = r.get(e), t ?? (n ? (t = this.categories.length, this.categories[t] = e, r.set(e, t), this._onCollect && this._onCollect(e, t)) : t = NaN), t;
	}, e.prototype._getOrCreateMap = function() {
		return this._map ||= K(this.categories);
	}, e;
}();
function Vv(e) {
	return W(e) && e.value != null ? e.value : e + "";
}
var Hv = R({
	needTransform: 1,
	normalize: 1,
	scale: 1,
	transformIn: 1,
	transformOut: 1,
	contain: 1,
	getExtent: 1,
	getExtentUnsafe: 1,
	setExtent: 1,
	setExtent2: 1,
	getFilter: 1,
	sanitize: 1,
	getDefaultStartValue: 1,
	freeze: 1
});
function Uv(e, t, n) {
	var r;
	e ||= {};
	var i = xh();
	if (i) {
		var a = i.createBreakScaleMapper(t, n);
		a.hasBreaks() && (I(Hv, function(t) {
			a[t] && (e[t] = z(a[t], a));
		}), r = a);
	}
	return r ?? Xv(e, n), {
		brk: r,
		mapper: e
	};
}
function Wv(e, t) {
	I(Hv, function(n) {
		e[n] = t[n];
	});
}
function Gv(e, t) {
	e.freeze = je;
}
function Kv(e) {
	return e.getExtentUnsafe(0, 2);
}
function qv(e, t) {
	return e.getExtentUnsafe(1, t) || e.getExtentUnsafe(0, t);
}
function Jv(e) {
	var t = qv(e, 3);
	return t[1] - t[0];
}
function Yv(e) {
	var t = e.getExtentUnsafe(0, 3);
	return t[1] - t[0];
}
function Xv(e, t) {
	var n = e || {}, r = [];
	return n._extents = r, r[0] = t ? t.slice() : tc(), j(n, Zv), n;
}
var Zv = {
	needTransform: function() {
		return !1;
	},
	normalize: function(e) {
		var t = this._extents[1] || this._extents[0];
		return t[1] === t[0] ? .5 : (e - t[0]) / (t[1] - t[0]);
	},
	scale: function(e) {
		var t = this._extents[1] || this._extents[0];
		return e * (t[1] - t[0]) + t[0];
	},
	transformIn: function(e) {
		return e;
	},
	transformOut: function(e) {
		return e;
	},
	contain: function(e) {
		var t = qv(this, null);
		return e >= t[0] && e <= t[1];
	},
	getExtent: function() {
		return this._extents[0].slice();
	},
	getExtentUnsafe: function(e) {
		return this._extents[e];
	},
	setExtent: function(e, t) {
		Qv(this._extents, 0, e, t);
	},
	setExtent2: function(e, t, n) {
		var r = this._extents;
		r[e] || (r[e] = r[0].slice()), Qv(r, e, t, n);
	},
	freeze: function() {}
};
function Qv(e, t, n, r) {
	sc(n, r) && (e[t][0] = n, e[t][1] = r);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/scale/helper.js
function $v(e) {
	return ey(e) || ny(e);
}
function ey(e) {
	return e.type === "interval";
}
function ty(e) {
	return e.type === "time";
}
function ny(e) {
	return e.type === "log";
}
function ry(e) {
	return e.type === "ordinal";
}
function iy(e) {
	var t = ss(e), n = Bo(10, t), r = Lo(e / n);
	return r ? r === 2 ? r = 3 : r === 3 ? r = 5 : r *= 2 : r = 1, Z(r * n, -t);
}
function ay(e) {
	return Xo(e) + 2;
}
function oy(e, t) {
	return Vo(e) / Vo(t);
}
function sy(e, t, n) {
	var r = n && n.lookup;
	if (r) {
		for (var i = 0; i < r.from.length; i++) if (e === r.from[i]) return r.to[i];
	}
	return Bo(t, e);
}
function cy(e, t, n) {
	var r = e.slice();
	if (r[0] === r[1]) {
		var i = n && n.ctnShp;
		if (r[0] !== 0) {
			var a = Io(r[0]);
			t[1] || (r[1] += a / 2), r[0] -= a / 2;
		} else i && (r[0] = -1), r[1] = 1;
	}
	return (!oc(r[0]) || !oc(r[1])) && (r[0] = 0, r[1] = 1), r[1] < r[0] && r.reverse(), r;
}
function ly(e, t) {
	return [e[0] !== t[0], e[1] !== t[1]];
}
function uy(e, t) {
	return e ||= t, Lo(Fo(e, 1));
}
function dy(e, t, n) {
	var r = Kv(e), i = r[0], a = e.count(), o = Math.max((t || 0) + 1, 1);
	i !== 0 && o > 1 && a / o > 2 && (i = Math.round(Math.ceil(i / o) * o)), i !== r[0] && c(r[0], !0, !0);
	for (var s = i; s <= r[1]; s += o) c(s, !1, s === r[0] || s === r[1]);
	s - o !== r[1] && c(r[1], !0, !0);
	function c(e, t, r) {
		n({
			value: e,
			offInterval: t
		}, r);
	}
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/scale/Ordinal.js
var fy = function(e) {
	o(t, e);
	function t(n) {
		var r = e.call(this) || this;
		r.type = "ordinal", r.parse = t.parse, Wv(r, t.decoratedMethods);
		var i = n.ordinalMeta;
		i ||= new Bv({}), V(i) && (i = new Bv({ categories: L(i, function(e) {
			return W(e) ? e.value : e;
		}) })), r._ordinalMeta = i;
		var a = Uv(null, null, n.extent || [0, i.categories.length - 1]);
		return r._mapper = a.mapper, Gv(r, a.mapper), r;
	}
	return t.parse = function(e) {
		return e == null ? e = NaN : U(e) ? (e = this._ordinalMeta.getOrdinal(e), e ??= NaN) : e = Lo(e), e;
	}, t.prototype.getTicks = function() {
		var e = [];
		return dy(this, 0, function(t) {
			e.push(t);
		}), e;
	}, t.prototype.getMinorTicks = function(e) {}, t.prototype.setSortInfo = function(e) {
		if (e == null) {
			this._ordinalNumbersByTick = this._ticksByOrdinalNumber = null;
			return;
		}
		for (var t = e.ordinalNumbers, n = this._ordinalNumbersByTick = [], r = this._ticksByOrdinalNumber = [], i = 0, a = this._ordinalMeta.categories.length, o = Po(a, t.length); i < o; ++i) {
			var s = n[i] = t[i];
			r[s] = i;
		}
		for (var c = 0; i < a; ++i) {
			for (; r[c] != null;) c++;
			n[i] = c, r[c] = i;
		}
	}, t.prototype._getTickNumber = function(e) {
		var t = this._ticksByOrdinalNumber;
		return t && e >= 0 && e < t.length ? t[e] : e;
	}, t.prototype.getRawOrdinalNumber = function(e) {
		var t = this._ordinalNumbersByTick;
		return t && e >= 0 && e < t.length ? t[e] : e;
	}, t.prototype.getLabel = function(e) {
		if (!this.isBlank()) {
			var t = this.getRawOrdinalNumber(e.value), n = this._ordinalMeta.categories[t];
			return n == null ? "" : n + "";
		}
	}, t.prototype.count = function() {
		var e = Kv(this._mapper);
		return e[1] - e[0] + 1;
	}, t.prototype.getOrdinalMeta = function() {
		return this._ordinalMeta;
	}, t.type = "ordinal", t.decoratedMethods = {
		needTransform: function() {
			return this._mapper.needTransform();
		},
		contain: function(e) {
			return this._mapper.contain(this._getTickNumber(e)) && e >= 0 && e < this._ordinalMeta.categories.length;
		},
		normalize: function(e) {
			return this._mapper.normalize(this._getTickNumber(e));
		},
		scale: function(e) {
			return this.getRawOrdinalNumber(Lo(this._mapper.scale(e)));
		},
		transformIn: function(e, t) {
			return this._mapper.transformIn(this._getTickNumber(e), t);
		},
		transformOut: function(e, t) {
			return this.getRawOrdinalNumber(this._mapper.transformOut(e, t));
		},
		getExtent: function() {
			return this._mapper.getExtent();
		},
		getExtentUnsafe: function(e, t) {
			return this._mapper.getExtentUnsafe(e, t);
		},
		setExtent: function(e, t) {
			return this._mapper.setExtent(e, t);
		},
		setExtent2: function(e, t, n) {
			return this._mapper.setExtent2(e, t, n);
		}
	}, t;
}(Rv);
Rv.registerClass(fy);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/scale/minorTicks.js
function py(e, t, n, r) {
	for (var i = e.getTicks({ expandToNicedExtent: !0 }), a = [], o = e.getExtent(), s = 1; s < i.length; s++) {
		var c = i[s], l = i[s - 1];
		if (!(l.break || c.break)) {
			for (var u = 0, d = [], f = (c.value - l.value) / t, p = ay(f); u < t - 1;) {
				var m = Z(l.value + (u + 1) * f, p);
				m > o[0] && m < o[1] && d.push(m), u++;
			}
			var h = xh();
			h && h.pruneTicksByBreak("auto", d, n, function(e) {
				return e;
			}, r, o), a.push(d);
		}
	}
	return a;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/scale/Interval.js
var my = function(e) {
	o(t, e);
	function t(n) {
		var r = e.call(this) || this;
		return r.type = "interval", r.parse = t.parse, n ||= {}, r.brk = Uv(r, Sh(r, n), null).brk, r._cfg = {
			interval: 0,
			intervalPrecision: 2,
			intervalCount: void 0,
			niceExtent: void 0
		}, r;
	}
	return t.parse = function(e) {
		return e == null || e === "" ? NaN : Number(e);
	}, t.prototype.getConfig = function() {
		return k(this._cfg);
	}, t.prototype.setConfig = function(e) {
		var t = Kv(this);
		this._cfg = e = k(e), e.niceExtent ??= t.slice(), e.intervalPrecision ??= ay(e.interval);
	}, t.prototype.getTicks = function(e) {
		e ||= {};
		var t = this._cfg, n = t.interval, r = Kv(this), i = t.niceExtent, a = t.intervalPrecision, o = xh(), s = this.brk, c = o && s, l = [];
		if (!n) return l;
		if (e.breakTicks === "only_break" && c) return o.addBreaksToTicks(l, s.breaks, r), l;
		var u = 3e3;
		r[0] < i[0] && l.push({ value: e.expandToNicedExtent ? Z(i[0] - n, a) : r[0] });
		for (var d = function(e, t) {
			return Lo((t - e) / n);
		}, f = t.intervalCount, p = i[0], m = 0;; m++) {
			if (f == null) {
				if (p > i[1] || !isFinite(p) || !isFinite(i[1])) break;
			} else {
				if (m > f) break;
				p = Po(p, i[1]), m === f && (p = i[1]);
			}
			if (l.push({ value: p }), p = Z(p + n, a), s) {
				var h = s.calcNiceTickMultiple(p, d);
				h >= 0 && (p = Z(p + h * n, a));
			}
			if (l.length > 0 && p === l[l.length - 1].value) break;
			if (l.length > u) return [];
		}
		var g = l.length ? l[l.length - 1].value : i[1];
		return r[1] > g && l.push({ value: e.expandToNicedExtent ? Z(g + n, a) : r[1] }), c && o.pruneTicksByBreak(e.pruneByBreak, l, s.breaks, function(e) {
			return e.value;
		}, t.interval, r), c && e.breakTicks !== "none" && o.addBreaksToTicks(l, s.breaks, r), l;
	}, t.prototype.getMinorTicks = function(e) {
		return py(this, e, Ch(this), this._cfg.interval);
	}, t.prototype.getLabel = function(e, t) {
		if (e == null) return "";
		var n = t && t.precision;
		return n == null ? n = Xo(e.value) || 0 : n === "auto" && (n = this._cfg.intervalPrecision), sg(Z(e.value, n, !0));
	}, t.type = "interval", t;
}(Rv);
Rv.registerClass(my);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/scale/Time.js
var hy = function(e, t, n, r) {
	for (; n < r;) {
		var i = n + r >>> 1;
		e[i][1] < t ? n = i + 1 : r = i;
	}
	return n;
}, gy = function(e) {
	o(t, e);
	function t(n) {
		var r = e.call(this) || this;
		return r.type = "time", r.parse = t.parse, r._locale = n.locale, r._useUTC = n.useUTC, r._interval = 0, r.brk = Uv(r, Sh(r, n), null).brk, r;
	}
	return t.prototype.getLabel = function(e) {
		return Uh(e.value, Ph[Hh(Bh(this._minLevelUnit))] || Ph.second, this._useUTC, this._locale);
	}, t.prototype.getFormattedLabel = function(e, t, n) {
		return Wh(e, t, n, this._locale, this._useUTC);
	}, t.prototype.getTicks = function(e) {
		e ||= {};
		var t = this._interval, n = Kv(this), r = xh(), i = this.brk, a = r && i, o = [];
		if (!t) return o;
		var s = this._useUTC;
		if (a && e.breakTicks === "only_break") return xh().addBreaksToTicks(o, i.breaks, n), o;
		o = Ey(this._minLevelUnit, this._approxInterval, s, n, Yv(this), i);
		var c = Fh.length - 1, l = 0;
		return I(o, function(e) {
			e.time && (c = Math.min(c, N(Fh, e.time.upperTimeUnit)), l = Math.max(l, e.time.level));
		}), a && xh().pruneTicksByBreak(e.pruneByBreak, o, i.breaks, function(e) {
			return e.value;
		}, this._approxInterval, n), a && e.breakTicks !== "none" && xh().addBreaksToTicks(o, i.breaks, n, function(e) {
			for (var t = Math.max(N(Fh, Gh(e.vmin, s)), N(Fh, Gh(e.vmax, s))), n = 0, r = 0; r < Fh.length; r++) if (!vy(Fh[r], e.vmin, e.vmax, s)) {
				n = r;
				break;
			}
			var i = Math.min(n, c);
			return {
				level: l,
				lowerTimeUnit: Fh[Math.max(i, t)],
				upperTimeUnit: Fh[i]
			};
		}), o;
	}, t.prototype.getMinorTicks = function(e) {
		return py(this, e, Ch(this), this._interval);
	}, t.prototype.setTimeInterval = function(e) {
		this._interval = e.interval, this._approxInterval = e.approxInterval, this._minLevelUnit = e.minLevelUnit;
	}, t.parse = function(e) {
		return se(e) ? Math.round(e) : +as(e);
	}, t.type = "time", t;
}(Rv), _y = [
	["second", Th],
	["minute", Eh],
	["hour", Dh],
	["quarter-day", Dh * 6],
	["half-day", Dh * 12],
	["day", Oh * 1.2],
	["half-week", Oh * 3.5],
	["week", Oh * 7],
	["month", Oh * 31],
	["quarter", Oh * 95],
	["half-year", kh / 2],
	["year", kh]
];
function vy(e, t, n, r) {
	return Kh(new Date(t), e, r).getTime() === Kh(new Date(n), e, r).getTime();
}
function yy(e, t) {
	return e /= Oh, e > 16 ? 16 : e > 7.5 ? 7 : e > 3.5 ? 4 : e > 1.5 ? 2 : 1;
}
function by(e) {
	var t = 30 * Oh;
	return e /= t, e > 6 ? 6 : e > 3 ? 3 : e > 2 ? 2 : 1;
}
function xy(e) {
	return e /= Dh, e > 12 ? 12 : e > 6 ? 6 : e > 3.5 ? 4 : e > 2 ? 2 : 1;
}
function Sy(e, t) {
	return e /= t ? Eh : Th, e > 30 ? 30 : e > 20 ? 20 : e > 15 ? 15 : e > 10 ? 10 : e > 5 ? 5 : e > 2 ? 2 : 1;
}
function Cy(e) {
	return Fo(cs(e, !0), 1);
}
function wy(e, t, n) {
	var r = Math.max(0, N(Fh, t) - 1);
	return Kh(new Date(e), Fh[r], n).getTime();
}
function Ty(e, t) {
	var n = /* @__PURE__ */ new Date(0);
	n[e](1);
	var r = n.getTime();
	n[e](1 + t);
	var i = n.getTime() - r;
	return function(e, t) {
		return Math.max(0, Math.round((t - e) / i));
	};
}
function Ey(e, t, n, r, i, a) {
	var o = 3e3, s = Ih, c = 0;
	function l(e, t, n, i, s, l, u) {
		for (var d = Ty(s, e), f = t, p = new Date(f); f < n && f <= r[1] && (u.push({ value: f }), !(c++ > o));) if (p[s](p[i]() + e), f = p.getTime(), a) {
			var m = a.calcNiceTickMultiple(f, d);
			m > 0 && (p[s](p[i]() + m * e), f = p.getTime());
		}
		u.push({
			value: f,
			notAdd: f > r[1]
		});
	}
	function u(e, i, a) {
		var o = [], s = !i.length;
		if (!vy(Bh(e), r[0], r[1], n)) {
			s && (i = [{ value: wy(r[0], e, n) }, { value: r[1] }]);
			for (var c = 0; c < i.length - 1; c++) {
				var u = i[c].value, d = i[c + 1].value;
				if (u !== d) {
					var f = void 0, p = void 0, m = void 0, h = !1;
					switch (e) {
						case "year":
							f = Math.max(1, Math.round(t / Oh / 365)), p = qh(n), m = eg(n);
							break;
						case "half-year":
						case "quarter":
						case "month":
							f = by(t), p = Jh(n), m = tg(n);
							break;
						case "week":
						case "half-week":
						case "day":
							f = yy(t, 31), p = Yh(n), m = ng(n), h = !0;
							break;
						case "half-day":
						case "quarter-day":
						case "hour":
							f = xy(t), p = Xh(n), m = rg(n);
							break;
						case "minute":
							f = Sy(t, !0), p = Zh(n), m = ig(n);
							break;
						case "second":
							f = Sy(t, !1), p = Qh(n), m = ag(n);
							break;
						case "millisecond":
							f = Cy(t), p = $h(n), m = og(n);
							break;
					}
					d >= r[0] && u <= r[1] && l(f, u, d, p, m, h, o), e === "year" && a.length > 1 && c === 0 && a.unshift({ value: a[0].value - f });
				}
			}
			for (var c = 0; c < o.length; c++) a.push(o[c]);
		}
	}
	for (var d = [], f = [], p = 0, m = 0, h = 0; h < s.length; ++h) {
		var g = Bh(s[h]);
		if (Vh(s[h]) && (u(s[h], d[d.length - 1] || [], f), g !== (s[h + 1] ? Bh(s[h + 1]) : null))) {
			if (f.length) {
				m = p, f.sort(function(e, t) {
					return e.value - t.value;
				});
				for (var _ = [], v = 0; v < f.length; ++v) {
					var y = f[v].value;
					(v === 0 || f[v - 1].value !== y) && (_.push(f[v]), y >= r[0] && y <= r[1] && p++);
				}
				var b = i / t;
				if (p > b * 1.5 && m > b / 1.5 || (d.push(_), p > b || e === s[h])) break;
			}
			f = [];
		}
	}
	for (var x = re(L(d, function(e) {
		return re(e, function(e) {
			return e.value >= r[0] && e.value <= r[1] && !e.notAdd;
		});
	}), function(e) {
		return e.length > 0;
	}), S = x.length - 1, C = [], h = 0; h < x.length; ++h) for (var w = x[h], T = 0; T < w.length; ++T) {
		var E = Gh(w[T].value, n);
		C.push({
			value: w[T].value,
			time: {
				level: S - h,
				upperTimeUnit: E,
				lowerTimeUnit: E
			}
		});
	}
	fc(C, pc, null), C.sort(function(e, t) {
		return e.value - t.value;
	});
	var D = C[0], O = C[C.length - 1], k = Gh(r[0], n), A = Gh(r[1], n);
	return (!D || D.value > r[0]) && C.unshift({
		value: r[0],
		time: {
			level: 0,
			upperTimeUnit: k,
			lowerTimeUnit: k
		},
		notNice: !0
	}), (!O || O.value < r[1]) && C.push({
		value: r[1],
		time: {
			level: 0,
			upperTimeUnit: A,
			lowerTimeUnit: A
		},
		notNice: !0
	}), C;
}
var Dy = function(e, t) {
	var n = e.getExtent();
	if (n[0] === n[1] && (n[0] -= Oh, n[1] += Oh), n[1] === -Infinity && n[0] === Infinity) {
		var r = /* @__PURE__ */ new Date();
		n[1] = +new Date(r.getFullYear(), r.getMonth(), r.getDate()), n[0] = n[1] - Oh;
	}
	e.setExtent(n[0], n[1]);
	var i = uy(t.splitNumber, 10), a = Yv(e) / i, o = t.minInterval, s = t.maxInterval;
	o != null && a < o && (a = o), s != null && a > s && (a = s);
	var c = _y.length, l = Math.min(hy(_y, a, 0, c), c - 1), u = _y[l][1], d = _y[Math.max(l - 1, 0)][0];
	e.setTimeInterval({
		approxInterval: a,
		interval: u,
		minLevelUnit: d
	});
};
Rv.registerClass(gy);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/scale/Log.js
var Oy = 0, ky = 1, Ay = 2, jy = function(e) {
	o(t, e);
	function t(n) {
		var r = e.call(this) || this;
		r.type = "log", r.parse = my.parse, r.base = n.logBase || 10;
		var i = [], a = [], o = r._lookup = {
			from: i,
			to: a
		};
		i[Oy] = i[ky] = a[Oy] = a[ky] = NaN, Wv(r, t.mapperMethods);
		var s = xh(), c = n.breakOption, l = { lookup: o };
		return s && s.parseAxisBreakOptionInwardTransform(c, r, { noNegative: !0 }, Ay, l), r.powStub = new my({ breakParsed: l.original }), r.intervalStub = new my({ breakParsed: l.transformed }), Gv(r, r.intervalStub), r;
	}
	return t.prototype.getTicks = function(e) {
		var t = this.base, n = this.powStub, r = xh(), i = this.intervalStub, a = { lookup: {
			from: i.getExtent(),
			to: n.getExtent()
		} };
		return L(i.getTicks(e || {}), function(e) {
			var i = e.value, o = sy(i, t, a), s;
			if (r) {
				var c = r.getTicksBreakOutwardTransform(this, e, Ch(n), this._lookup);
				c && (s = c.vBreak, o = c.tickVal);
			}
			return {
				value: o,
				break: s
			};
		}, this);
	}, t.prototype.getMinorTicks = function(e) {
		return py(this, e, Ch(this.powStub), this.intervalStub.getConfig().interval);
	}, t.prototype.getLabel = function(e, t) {
		return this.intervalStub.getLabel(e, t);
	}, t.type = "log", t.mapperMethods = {
		needTransform: function() {
			return !0;
		},
		normalize: function(e) {
			return this.intervalStub.normalize(oy(e, this.base));
		},
		scale: function(e) {
			return sy(this.intervalStub.scale(e), this.base, null);
		},
		transformIn: function(e, t) {
			return e = oy(e, this.base), t && t.depth === 2 ? e : this.intervalStub.transformIn(e, t);
		},
		transformOut: function(e, t) {
			var n = t ? t.depth : null;
			return My.depth = n, Ny.lookup = this._lookup, sy(n === 2 ? e : this.intervalStub.transformOut(e, My), this.base, Ny);
		},
		contain: function(e) {
			return this.powStub.contain(e);
		},
		setExtent: function(e, t) {
			this.setExtent2(0, e, t);
		},
		setExtent2: function(e, t, n) {
			if (!(!sc(t, n) || t <= 0 || n <= 0)) {
				var r = Py, i = Py;
				if (e === 0) {
					var a = this._lookup;
					r = a.to, i = a.from;
				}
				this.powStub.setExtent2(e, r[Oy] = t, r[ky] = n);
				var o = this.base;
				this.intervalStub.setExtent2(e, i[Oy] = oy(t, o), i[ky] = oy(n, o));
			}
		},
		getFilter: function() {
			return { g: 0 };
		},
		sanitize: function(e, t) {
			return sc(t[0], t[1]) && ms(e) && e <= 0 && (e = t[0]), e;
		},
		getDefaultStartValue: function() {
			return 1;
		},
		getExtent: function() {
			return this.powStub.getExtent();
		},
		getExtentUnsafe: function(e, t) {
			return t === null ? this.powStub.getExtentUnsafe(e, null) : this.intervalStub.getExtentUnsafe(e, t);
		}
	}, t;
}(Rv);
Rv.registerClass(jy);
var My = {}, Ny = {}, Py = [], Fy = {
	value: 1,
	category: 1,
	time: 1,
	log: 1
}, Iy = Ws();
function Ly(e) {
	var t = e.get("type");
	return (t == null || !Ae(Fy, t) && !Rv.getClass(t)) && (t = "value"), t;
}
function Ry(e, t, n) {
	var r = xh(), i;
	switch (r && (i = Yy(e, t, n)), t) {
		case "category": return new fy({
			ordinalMeta: e.getOrdinalMeta ? e.getOrdinalMeta() : e.getCategories(),
			extent: tc()
		});
		case "time": return new gy({
			locale: e.ecModel.getLocaleModel(),
			useUTC: e.ecModel.get("useUTC"),
			breakOption: i
		});
		case "log": return new jy({
			logBase: e.get("logBase"),
			breakOption: i
		});
		case "value": return new my({ breakOption: i });
		default: return new ((Rv.getClass(t)) || my)({});
	}
}
function zy(e, t, n) {
	var r = n ? qv(e, null) : e.getExtentUnsafe(0, null), i = r[0], a = r[1];
	return sc(i, a) ? i === t || a === t ? 2 : i < t && a > t ? 1 : 3 : 3;
}
function By(e) {
	Iy(e).noOnMyZero = !0;
}
function Vy(e) {
	return Iy(e).noOnMyZero;
}
function Hy(e) {
	var t = e.getLabelModel().get("formatter");
	if (e.type === "time") {
		var n = Lh(t);
		return function(t, r) {
			return e.scale.getFormattedLabel(t, r, n);
		};
	} else if (U(t)) return function(n) {
		var r = e.scale.getLabel(n);
		return t.replace("{value}", r ?? "");
	};
	else if (H(t)) {
		if (e.type === "category") return function(n, r) {
			return t(Uy(e, n), n.value - e.scale.getExtent()[0], null);
		};
		var r = xh();
		return function(n, i) {
			var a = null;
			return r && (a = r.makeAxisLabelFormatterParamBreak(a, n.break)), t(Uy(e, n), i, a);
		};
	} else return function(t) {
		return e.scale.getLabel(t);
	};
}
function Uy(e, t) {
	var n = e.scale;
	return ry(n) ? n.getLabel(t) : t.value;
}
function Wy(e) {
	return e.get("interval") ?? "auto";
}
function Gy(e) {
	return e.type === "category" && Wy(e.getLabelModel()) === 0;
}
function Ky(e, t) {
	var n = {};
	return I(e.mapDimensionsAll(t), function(t) {
		n[Lm(e, t)] = !0;
	}), R(n);
}
function qy(e) {
	return e === "middle" || e === "center";
}
function Jy(e) {
	return e.getShallow("show");
}
function Yy(e, t, n) {
	var r = e.get("breaks", !0);
	if (r != null) return !xh() || !n || !Xy(t) ? void 0 : r;
}
function Xy(e) {
	return e !== "category";
}
function Zy(e, t, n, r, i, a) {
	var o = ny(e), s = o ? e.intervalStub : e;
	if (s.setExtent(r[0], r[1]), o) {
		var c = e.powStub, l = { depth: 2 }, u = e.transformOut(r[0], l), d = e.transformOut(r[1], l), f = ly(n, r);
		t[0] && !f[0] && (u = i[0]), t[1] && !f[1] && (d = i[1]), c.setExtent(u, d);
	}
	s.setConfig(a);
}
function Qy(e, t) {
	return ry(e) ? e.getRawOrdinalNumber(t.value) : t.value;
}
function $y(e, t) {
	return ry(e) && !!t.get("boundaryGap");
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/line/LineView.js
function eb(e, t) {
	if (e.length === t.length) {
		for (var n = 0; n < e.length; n++) if (e[n] !== t[n]) return;
		return !0;
	}
}
function tb(e) {
	for (var t = tc(), n = tc(), r = 0; r < e.length;) {
		var i = e[r++], a = e[r++];
		dv(i, a) || (nc(t, i), nc(n, a));
	}
	return [t, n];
}
function nb(e, t) {
	var n = tb(e), r = n[0], i = n[1], a = tb(t), o = a[0], s = a[1];
	return Math.max(Math.abs(r[0] - o[0]), Math.abs(i[0] - s[0]), Math.abs(r[1] - o[1]), Math.abs(i[1] - s[1]));
}
function rb(e) {
	return se(e) ? e : e ? .5 : 0;
}
function ib(e, t, n) {
	if (n.valueDim == null) return [];
	for (var r = t.count(), i = mv(r * 2), a = 0; a < r; a++) {
		var o = uv(n, e, t, a);
		i[a * 2] = o[0], i[a * 2 + 1] = o[1];
	}
	return i;
}
function ab(e, t, n, r, i) {
	var a = n.getBaseAxis(), o = a.dim === "x" || a.dim === "radius" ? 0 : 1, s = [], c = 0, l = [], u = [], d = [], f = [];
	if (i) {
		for (c = 0; c < e.length; c += 2) {
			var p = t || e;
			dv(p[c], p[c + 1]) || f.push(e[c], e[c + 1]);
		}
		e = f;
	}
	for (c = 0; c < e.length - 2; c += 2) switch (d[0] = e[c + 2], d[1] = e[c + 3], u[0] = e[c], u[1] = e[c + 1], s.push(u[0], u[1]), r) {
		case "end":
			l[o] = d[o], l[1 - o] = u[1 - o], s.push(l[0], l[1]);
			break;
		case "middle":
			var m = (u[o] + d[o]) / 2, h = [];
			l[o] = h[o] = m, l[1 - o] = u[1 - o], h[1 - o] = d[1 - o], s.push(l[0], l[1]), s.push(h[0], h[1]);
			break;
		default: l[o] = u[o], l[1 - o] = d[1 - o], s.push(l[0], l[1]);
	}
	return s.push(e[c++], e[c++]), s;
}
function ob(e, t) {
	var n = [], r = e.length, i, a;
	function o(e, t, n) {
		var r = e.coord;
		return {
			coord: n,
			color: Rr((n - r) / (t.coord - r), [e.color, t.color])
		};
	}
	for (var s = 0; s < r; s++) {
		var c = e[s], l = c.coord;
		if (l < 0) i = c;
		else if (l > t) {
			a ? n.push(o(a, c, t)) : i && n.push(o(i, c, 0), o(i, c, t));
			break;
		} else i &&= (n.push(o(i, c, 0)), null), n.push(c), a = c;
	}
	return n;
}
function sb(e, t, n) {
	var r = e.getVisual("visualMeta");
	if (!(!r || !r.length || !e.count()) && t.type === "cartesian2d") {
		for (var i, a, o = r.length - 1; o >= 0; o--) {
			var s = e.getDimensionInfo(r[o].dimension);
			if (i = s && s.coordDim, i === "x" || i === "y") {
				a = r[o];
				break;
			}
		}
		if (a) {
			var c = t.getAxis(i), l = L(a.stops, function(e) {
				return {
					coord: c.toGlobalCoord(c.dataToCoord(e.value)),
					color: e.color
				};
			}), u = l.length, d = a.outerColors.slice();
			u && l[0].coord > l[u - 1].coord && (l.reverse(), d.reverse());
			var f = ob(l, i === "x" ? n.getWidth() : n.getHeight()), p = f.length;
			if (!p && u) return l[0].coord < 0 ? d[1] ? d[1] : l[u - 1].color : d[0] ? d[0] : l[0].color;
			var m = 10, h = f[0].coord - m, g = f[p - 1].coord + m, _ = g - h;
			if (_ < .001) return "transparent";
			I(f, function(e) {
				e.offset = (e.coord - h) / _;
			}), f.push({
				offset: p ? f[p - 1].offset : .5,
				color: d[1] || "transparent"
			}), f.unshift({
				offset: p ? f[0].offset : .5,
				color: d[0] || "transparent"
			});
			var v = new Ju(0, 0, 0, 0, f, !0);
			return v[i] = h, v[i + "2"] = g, v;
		}
	}
}
function cb(e, t, n) {
	var r = e.get("showAllSymbol"), i = r === "auto";
	if (!(r && !i)) {
		var a = n.getAxesByScale("ordinal")[0];
		if (a && !(i && lb(a, t))) {
			var o = t.mapDimension(a.dim), s = {};
			return I(a.getViewLabels(), function(e) {
				e.tick.offInterval || (s[Qy(a.scale, e.tick)] = 1);
			}), function(e) {
				return !s.hasOwnProperty(t.get(o, e));
			};
		}
	}
}
function lb(e, t) {
	var n = e.getExtent(), r = Math.abs(n[1] - n[0]) / e.scale.count();
	isNaN(r) && (r = 0);
	for (var i = t.count(), a = Math.max(1, Math.round(i / 5)), o = 0; o < i; o += a) if (tv.getSymbolSize(t, o)[+!!e.isHorizontal()] * 1.5 > r) return !1;
	return !0;
}
function ub(e) {
	for (var t = e.length / 2; t > 0 && dv(e[t * 2 - 2], e[t * 2 - 1]); t--);
	return t - 1;
}
function db(e, t) {
	return [e[t * 2], e[t * 2 + 1]];
}
function fb(e, t, n) {
	for (var r = e.length / 2, i = n === "x" ? 0 : 1, a, o, s = 0, c = -1, l = 0; l < r; l++) if (o = e[l * 2 + i], !dv(o, e[l * 2 + 1 - i])) {
		if (l === 0) {
			a = o;
			continue;
		}
		if (a <= t && o >= t || a >= t && o <= t) {
			c = l;
			break;
		}
		s = l, a = o;
	}
	return {
		range: [s, c],
		t: (t - a) / (o - a)
	};
}
function pb(e) {
	if (e.get(["endLabel", "show"])) return !0;
	for (var t = 0; t < Ic.length; t++) if (e.get([
		Ic[t],
		"endLabel",
		"show"
	])) return !0;
	return !1;
}
function mb(e, t, n, r) {
	if (Lv(t, "cartesian2d")) {
		var i = r.getModel("endLabel"), a = i.get("valueAnimation"), o = r.getData(), s = { lastFrameIndex: 0 }, c = pb(r) ? function(n, r) {
			e._endLabelOnDuring(n, r, o, s, a, i, t);
		} : null, l = t.getBaseAxis().isHorizontal(), u = Pv(t, n, r, function() {
			var t = e._endLabel;
			t && n && s.originalX != null && t.attr({
				x: s.originalX,
				y: s.originalY
			});
		}, c);
		if (!r.get("clip", !0)) {
			var d = u.shape, f = Math.max(d.width, d.height);
			l ? (d.y -= f, d.height += f * 2) : (d.x -= f, d.width += f * 2);
		}
		return c && c(1, u), u;
	} else return Fv(t, n, r);
}
function hb(e, t) {
	var n = t.getBaseAxis(), r = n.isHorizontal(), i = n.inverse, a = r ? i ? "right" : "left" : "center", o = r ? "middle" : i ? "top" : "bottom";
	return { normal: {
		align: e.get("align") || a,
		verticalAlign: e.get("verticalAlign") || o
	} };
}
var gb = function(e) {
	o(t, e);
	function t() {
		return e !== null && e.apply(this, arguments) || this;
	}
	return t.prototype.init = function() {
		var e = new su(), t = new sv();
		this.group.add(t.group), this._symbolDraw = t, this._lineGroup = e, this._changePolyState = z(this._changePolyState, this);
	}, t.prototype.render = function(e, t, n) {
		var r = e.coordinateSystem, i = this.group, a = e.getData(), o = e.getModel("lineStyle"), s = e.getModel("areaStyle"), c = a.getLayout("points") || [], l = r.type === "polar", u = this._coordSys, d = this._symbolDraw, f = this._polyline, p = this._polygon, m = this._lineGroup, h = !t.ssr && e.get("animation"), g = !s.isEmpty(), _ = s.get("origin"), v = cv(r, a, _), y = g && ib(r, a, v), b = e.get("showSymbol"), x = e.get("connectNulls"), S = b && !l && cb(e, a, r), C = this._data;
		C && C.eachItemGraphicEl(function(e, t) {
			e.__temp && (i.remove(e), C.setItemGraphicEl(t, null));
		}), b || d.remove(), i.add(m);
		var w = l ? !1 : e.get("step"), T;
		r && r.getArea && e.get("clip", !0) && (T = r.getArea(), T.width == null ? T.r0 && (T.r0 -= .5, T.r += .5) : (T.x -= .1, T.y -= .1, T.width += .2, T.height += .2)), this._clipShapeForSymbol = T;
		var E = sb(a, r, n) || a.getVisual("style")[a.getVisual("drawType")];
		if (!(f && u.type === r.type && w === this._step)) b && d.updateData(a, {
			isIgnore: S,
			clipShape: T,
			disableAnimation: !0,
			getSymbolPoint: function(e) {
				return [c[e * 2], c[e * 2 + 1]];
			}
		}), h && this._initSymbolLabelAnimation(a, r, T), w && (y &&= ab(y, c, r, w, x), c = ab(c, null, r, w, x)), f = this._newPolyline(c), g ? p = this._newPolygon(c, y) : p &&= (m.remove(p), this._polygon = null), l || this._initOrUpdateEndLabel(e, r, hg(E)), m.setClipPath(mb(this, r, !0, e));
		else {
			g && !p ? p = this._newPolygon(c, y) : p && !g && (m.remove(p), p = this._polygon = null), l || this._initOrUpdateEndLabel(e, r, hg(E));
			var D = m.getClipPath();
			D ? dd(D, { shape: mb(this, r, !1, e).shape }, e) : m.setClipPath(mb(this, r, !0, e)), b && d.updateData(a, {
				isIgnore: S,
				clipShape: T,
				disableAnimation: !0,
				getSymbolPoint: function(e) {
					return [c[e * 2], c[e * 2 + 1]];
				}
			}), (!eb(this._stackedOnPoints, y) || !eb(this._points, c)) && (h ? this._doUpdateAnimation(a, y, r, n, w, _, x) : (w && (y &&= ab(y, c, r, w, x), c = ab(c, null, r, w, x)), f.setShape({ points: c }), p && p.setShape({
				points: c,
				stackedOnPoints: y
			})));
		}
		var O = e.getModel("emphasis"), k = O.get("focus"), A = O.get("blurScope"), j = O.get("disabled");
		if (f.useStyle(M(o.getLineStyle(), {
			fill: "none",
			stroke: E,
			lineJoin: "bevel"
		})), Ml(f, e, "lineStyle"), f.style.lineWidth > 0 && e.get([
			"emphasis",
			"lineStyle",
			"width"
		]) === "bolder") {
			var ee = f.getState("emphasis").style;
			ee.lineWidth = +f.style.lineWidth + 1;
		}
		yc(f).seriesIndex = e.seriesIndex, Ol(f, k, A, j);
		var N = rb(e.get("smooth")), te = e.get("smoothMonotone");
		if (f.setShape({
			smooth: N,
			smoothMonotone: te,
			connectNulls: x
		}), p) {
			var P = a.getCalculationInfo("stackedOnSeries"), F = 0;
			p.useStyle(M(s.getAreaStyle(), {
				fill: E,
				opacity: .7,
				lineJoin: "bevel",
				decal: a.getVisual("style").decal
			})), P && (F = rb(P.get("smooth"))), p.setShape({
				smooth: N,
				stackedOnSmooth: F,
				smoothMonotone: te,
				connectNulls: x
			}), Ml(p, e, "areaStyle"), yc(p).seriesIndex = e.seriesIndex, Ol(p, k, A, j);
		}
		var I = this._changePolyState;
		a.eachItemGraphicEl(function(e) {
			e && (e.onHoverStateChange = I);
		}), this._polyline.onHoverStateChange = I, this._data = a, this._coordSys = r, this._stackedOnPoints = y, this._points = c, this._step = w, this._valueOrigin = _;
		var L = e.get("triggerEvent"), ne = e.get("triggerLineEvent"), re = ne === !0 || L === !0 || L === "line", ie = ne === !0 || L === !0 || L === "area";
		this.packEventData(e, f, re), p && this.packEventData(e, p, ie);
	}, t.prototype.packEventData = function(e, t, n) {
		yc(t).eventData = n ? {
			componentType: "series",
			componentSubType: "line",
			componentIndex: e.componentIndex,
			seriesIndex: e.seriesIndex,
			seriesName: e.name,
			seriesType: "line",
			selfType: t === this._polygon ? "area" : "line"
		} : null;
	}, t.prototype.highlight = function(e, t, n, r) {
		var i = e.getData(), a = Us(i, r);
		if (this._changePolyState("emphasis"), !(a instanceof Array) && a != null && a >= 0) {
			var o = i.getLayout("points"), s = i.getItemGraphicEl(a);
			if (!s) {
				var c = o[a * 2], l = o[a * 2 + 1];
				if (dv(c, l) || this._clipShapeForSymbol && !this._clipShapeForSymbol.contain(c, l)) return;
				var u = e.get("zlevel") || 0, d = e.get("z") || 0;
				s = new tv(i, a), s.x = c, s.y = l, s.setZ(u, d);
				var f = s.getSymbolPath().getTextContent();
				f && (f.zlevel = u, f.z = d, f.z2 = this._polyline.z2 + 1), s.__temp = !0, i.setItemGraphicEl(a, s), s.stopSymbolAnimation(!0), this.group.add(s);
			}
			s.highlight();
		} else Ov.prototype.highlight.call(this, e, t, n, r);
	}, t.prototype.downplay = function(e, t, n, r) {
		var i = e.getData(), a = Us(i, r);
		if (this._changePolyState("normal"), a != null && a >= 0) {
			var o = i.getItemGraphicEl(a);
			o && (o.__temp ? (i.setItemGraphicEl(a, null), this.group.remove(o)) : o.downplay());
		} else Ov.prototype.downplay.call(this, e, t, n, r);
	}, t.prototype._changePolyState = function(e) {
		var t = this._polygon;
		el(this._polyline, e), t && el(t, e);
	}, t.prototype._newPolyline = function(e) {
		var t = this._polyline;
		return t && this._lineGroup.remove(t), t = new Sv({
			shape: { points: e },
			segmentIgnoreThreshold: 2,
			z2: 10
		}), this._lineGroup.add(t), this._polyline = t, t;
	}, t.prototype._newPolygon = function(e, t) {
		var n = this._polygon;
		return n && this._lineGroup.remove(n), n = new wv({
			shape: {
				points: e,
				stackedOnPoints: t
			},
			segmentIgnoreThreshold: 2
		}), this._lineGroup.add(n), this._polygon = n, n;
	}, t.prototype._initSymbolLabelAnimation = function(e, t, n) {
		var r, i, a = t.getBaseAxis(), o = a.inverse;
		t.type === "cartesian2d" ? (r = a.isHorizontal(), i = !1) : t.type === "polar" && (r = a.dim === "angle", i = !0);
		var s = e.hostModel, c = s.get("animationDuration");
		H(c) && (c = c(null));
		var l = s.get("animationDelay") || 0, u = H(l) ? l(null) : l;
		e.eachItemGraphicEl(function(e, a) {
			var s = e;
			if (s) {
				var d = [e.x, e.y], f = void 0, p = void 0, m = void 0;
				if (n) if (i) {
					var h = n, g = t.pointToCoord(d);
					r ? (f = h.startAngle, p = h.endAngle, m = -g[1] / 180 * Math.PI) : (f = h.r0, p = h.r, m = g[0]);
				} else {
					var _ = n;
					r ? (f = _.x, p = _.x + _.width, m = e.x) : (f = _.y + _.height, p = _.y, m = e.y);
				}
				var v = p === f ? 0 : (m - f) / (p - f);
				o && (v = 1 - v);
				var y = H(l) ? l(a) : c * v + u, b = s.getSymbolPath(), x = b.getTextContent();
				s.attr({
					scaleX: 0,
					scaleY: 0
				}), s.animateTo({
					scaleX: 1,
					scaleY: 1
				}, {
					duration: 200,
					setToFinal: !0,
					delay: y
				}), x && x.animateFrom({ style: { opacity: 0 } }, {
					duration: 300,
					delay: y
				}), b.disableLabelAnimation = !0;
			}
		});
	}, t.prototype._initOrUpdateEndLabel = function(e, t, n) {
		var r = e.getModel("endLabel");
		if (pb(e)) {
			var i = e.getData(), a = this._polyline, o = i.getLayout("points");
			if (!o) {
				a.removeTextContent(), this._endLabel = null;
				return;
			}
			var s = this._endLabel;
			s || (s = this._endLabel = new _o({ z2: 200 }), s.ignoreClip = !0, a.setTextContent(this._endLabel), a.disableLabelAnimation = !0);
			var c = ub(o);
			c >= 0 && (hf(a, gf(e, "endLabel"), {
				inheritColor: n,
				labelFetcher: e,
				labelDataIndex: c,
				defaultText: function(e, t, n) {
					return n == null ? $_(i, e) : ev(i, n);
				},
				enableTextSetter: !0
			}, hb(r, t)), a.textConfig.position = null);
		} else this._endLabel &&= (this._polyline.removeTextContent(), null);
	}, t.prototype._endLabelOnDuring = function(e, t, n, r, i, a, o) {
		var s = this._endLabel, c = this._polyline;
		if (s) {
			e < 1 && r.originalX == null && (r.originalX = s.x, r.originalY = s.y);
			var l = n.getLayout("points"), u = n.hostModel, d = u.get("connectNulls"), f = a.get("precision"), p = a.get("distance") || 0, m = o.getBaseAxis(), h = m.isHorizontal(), g = m.inverse, _ = t.shape, v = g ? h ? _.x : _.y + _.height : h ? _.x + _.width : _.y, y = (h ? p : 0) * (g ? -1 : 1), b = (h ? 0 : -p) * (g ? -1 : 1), x = h ? "x" : "y", S = fb(l, v, x), C = S.range, w = C[1] - C[0], T = void 0;
			if (w >= 1) {
				if (w > 1 && !d) {
					var E = db(l, C[0]);
					s.attr({
						x: E[0] + y,
						y: E[1] + b
					}), i && (T = u.getRawValue(C[0]));
				} else {
					var E = c.getPointOn(v, x);
					E && s.attr({
						x: E[0] + y,
						y: E[1] + b
					});
					var D = u.getRawValue(C[0]), O = u.getRawValue(C[1]);
					i && (T = ec(n, f, D, O, S.t));
				}
				r.lastFrameIndex = C[0];
			} else {
				var k = e === 1 || r.lastFrameIndex > 0 ? C[0] : 0, E = db(l, k);
				i && (T = u.getRawValue(k)), s.attr({
					x: E[0] + y,
					y: E[1] + b
				});
			}
			if (i) {
				var A = Ef(s);
				typeof A.setLabelText == "function" && A.setLabelText(T);
			}
		}
	}, t.prototype._doUpdateAnimation = function(e, t, n, r, i, a, o) {
		var s = this._polyline, c = this._polygon, l = e.hostModel, u = _v(this._data, e, this._stackedOnPoints, t, this._coordSys, n, this._valueOrigin, a), d = u.current, f = u.stackedOnCurrent, p = u.next, m = u.stackedOnNext;
		if (i && (f = ab(u.stackedOnCurrent, u.current, n, i, o), d = ab(u.current, null, n, i, o), m = ab(u.stackedOnNext, u.next, n, i, o), p = ab(u.next, null, n, i, o)), nb(d, p) > 3e3 || c && nb(f, m) > 3e3) {
			s.stopAnimation(), s.setShape({ points: p }), c && (c.stopAnimation(), c.setShape({
				points: p,
				stackedOnPoints: m
			}));
			return;
		}
		s.shape.__points = u.current, s.shape.points = d;
		var h = { shape: { points: p } };
		u.current !== d && (h.shape.__points = u.next), s.stopAnimation(), ud(s, h, l), c && (c.setShape({
			points: d,
			stackedOnPoints: f
		}), c.stopAnimation(), ud(c, { shape: { stackedOnPoints: m } }, l), s.shape.points !== c.shape.points && (c.shape.points = s.shape.points));
		for (var g = [], _ = u.status, v = 0; v < _.length; v++) if (_[v].cmd === "=") {
			var y = e.getItemGraphicEl(_[v].idx1);
			y && g.push({
				el: y,
				ptIdx: v
			});
		}
		s.animators && s.animators.length && s.animators[0].during(function() {
			c && c.dirtyShape();
			for (var e = s.shape.__points, t = 0; t < g.length; t++) {
				var n = g[t].el, r = g[t].ptIdx * 2;
				n.x = e[r], n.y = e[r + 1], n.markRedraw();
			}
		});
	}, t.prototype.remove = function(e) {
		var t = this.group, n = this._data;
		this._lineGroup.removeAll(), this._symbolDraw.remove(!0), n && n.eachItemGraphicEl(function(e, r) {
			e.__temp && (t.remove(e), n.setItemGraphicEl(r, null));
		}), this._polyline = this._polygon = this._coordSys = this._points = this._stackedOnPoints = this._endLabel = this._data = null;
	}, t.type = "line", t;
}(Ov);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/layout/points.js
function _b(e, t) {
	return {
		seriesType: e,
		plan: Tv(),
		reset: function(e) {
			var n = e.getData(), r = e.coordinateSystem, i = e.pipelineContext, a = t || i.large;
			if (r) {
				var o = L(r.dimensions, function(e) {
					return n.mapDimension(e);
				}).slice(0, 2), s = o.length, c = n.getCalculationInfo("stackResultDimension");
				Im(n, o[0]) && (o[0] = c), Im(n, o[1]) && (o[1] = c);
				var l = n.getStore(), u = n.getDimensionIndex(o[0]), d = n.getDimensionIndex(o[1]);
				return s && { progress: function(e, t) {
					for (var n = e.end - e.start, i = a && mv(n * s), o = [], c = [], f = e.start, p = 0; f < e.end; f++) {
						var m = void 0;
						if (s === 1) {
							var h = l.get(u, f);
							m = r.dataToPoint(h, null, c);
						} else o[0] = l.get(u, f), o[1] = l.get(d, f), m = r.dataToPoint(o, null, c);
						a ? (i[p++] = m[0], i[p++] = m[1]) : t.setItemLayout(f, m.slice());
					}
					a && (t.setLayout("points", i), t.setLayout("pointsRange", {
						start: e.start,
						end: e.end
					}));
				} };
			}
		}
	};
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/processor/dataSample.js
var vb = {
	average: function(e) {
		for (var t = 0, n = 0, r = 0; r < e.length; r++) isNaN(e[r]) || (t += e[r], n++);
		return n === 0 ? NaN : t / n;
	},
	sum: function(e) {
		for (var t = 0, n = 0; n < e.length; n++) t += e[n] || 0;
		return t;
	},
	max: function(e) {
		for (var t = -Infinity, n = 0; n < e.length; n++) e[n] > t && (t = e[n]);
		return isFinite(t) ? t : NaN;
	},
	min: function(e) {
		for (var t = Infinity, n = 0; n < e.length; n++) e[n] < t && (t = e[n]);
		return isFinite(t) ? t : NaN;
	},
	nearest: function(e) {
		return e[0];
	}
}, yb = function(e) {
	return Math.round(e.length / 2);
};
function bb(e) {
	return {
		seriesType: e,
		reset: function(e, t, n) {
			var r = e.getData(), i = e.get("sampling"), a = e.coordinateSystem, o = r.count();
			if (o > 10 && a.type === "cartesian2d" && i) {
				var s = a.getBaseAxis(), c = a.getOtherAxis(s), l = s.getExtent(), u = n.getDevicePixelRatio(), d = Math.abs(l[1] - l[0]) * (u || 1), f = Math.round(o / d);
				if (isFinite(f) && f > 1) {
					i === "lttb" ? e.setData(r.lttbDownSample(r.mapDimension(c.dim), 1 / f)) : i === "minmax" && e.setData(r.minmaxDownSample(r.mapDimension(c.dim), 1 / f));
					var p = void 0;
					U(i) ? p = vb[i] : H(i) && (p = i), p && e.setData(r.downSample(r.mapDimension(c.dim), 1 / f, p, yb));
				}
			}
		}
	};
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/line/install.js
function xb(e) {
	e.registerChartView(gb), e.registerSeriesModel(Q_), e.registerLayout(_b("line", !0)), e.registerVisual({
		seriesType: "line",
		reset: function(e) {
			var t = e.getData(), n = e.getModel("lineStyle").getLineStyle();
			n && !n.stroke && (n.stroke = t.getVisual("style").fill), t.setVisual("legendLineStyle", n);
		}
	}), e.registerProcessor(e.PRIORITY.PROCESSOR.STATISTIC, bb("line"));
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/coord/axisTickLabelBuilder.js
var Sb = Ws(), Cb = Ws(), wb = {
	estimate: 1,
	determine: 2
};
function Tb(e) {
	return {
		out: { noPxChangeTryDetermine: [] },
		kind: e
	};
}
function Eb(e, t) {
	var n = e.getLabelModel().get("customValues");
	if (n) {
		var r = e.scale;
		return { labels: L(Ob(n, r), function(t, n) {
			return {
				formattedLabel: Hy(e)(t, n),
				rawLabel: r.getLabel(t),
				tick: t
			};
		}) };
	}
	return e.type === "category" ? kb(e, t) : Mb(e);
}
function Db(e, t, n) {
	var r = e.scale, i = e.getTickModel().get("customValues");
	return i ? { ticks: Ob(i, r) } : e.type === "category" ? jb(e, t) : { ticks: r.getTicks(n) };
}
function Ob(e, t) {
	var n = t.getExtent(), r = [];
	return I(e, function(e) {
		e = t.parse(e), e >= n[0] && e <= n[1] && r.push(e);
	}), fc(r, mc, null), Yo(r), L(r, function(e) {
		return { value: e };
	});
}
function kb(e, t) {
	var n = e.getLabelModel(), r = Ab(e, n, t);
	return !n.get("show") || e.scale.isBlank() ? { labels: [] } : r;
}
function Ab(e, t, n) {
	var r = Pb(e), i = Wy(t), a = n.kind === wb.estimate;
	if (!a) {
		var o = Ib(r, i);
		if (o) return o;
	}
	var s, c;
	H(i) ? s = Ub(e, i, !1) : (c = i === "auto" ? Rb(e, n) : i, s = Ub(e, c, !1));
	var l = {
		labels: s,
		labelCategoryInterval: c
	};
	return a ? n.out.noPxChangeTryDetermine.push(function() {
		return Lb(r, i, l), !0;
	}) : Lb(r, i, l), l;
}
function jb(e, t) {
	var n = Nb(e), r = Wy(t), i = Ib(n, r);
	if (i) return i;
	var a, o;
	if ((!t.get("show") || e.scale.isBlank()) && (a = []), H(r)) a = Ub(e, r, !0);
	else if (r === "auto") {
		var s = Ab(e, e.getLabelModel(), Tb(wb.determine));
		o = s.labelCategoryInterval, a = L(s.labels, function(e) {
			return e.tick;
		});
	} else o = r, a = Ub(e, o, !0);
	return Lb(n, r, {
		ticks: a,
		tickCategoryInterval: o
	});
}
function Mb(e) {
	var t = e.scale.getTicks(), n = Hy(e);
	return { labels: L(t, function(t, r) {
		return {
			formattedLabel: n(t, r),
			rawLabel: e.scale.getLabel(t),
			tick: t
		};
	}) };
}
var Nb = Fb("axisTick"), Pb = Fb("axisLabel");
function Fb(e) {
	return function(t) {
		return Cb(t)[e] || (Cb(t)[e] = { list: [] });
	};
}
function Ib(e, t) {
	for (var n = 0; n < e.list.length; n++) if (e.list[n].key === t) return e.list[n].value;
}
function Lb(e, t, n) {
	return e.list.push({
		key: t,
		value: n
	}), n;
}
function Rb(e, t) {
	if (t.kind === wb.estimate) {
		var n = e.calculateCategoryInterval(t);
		return t.out.noPxChangeTryDetermine.push(function() {
			return Cb(e).autoInterval = n, !0;
		}), n;
	}
	return Cb(e).autoInterval ?? (Cb(e).autoInterval = e.calculateCategoryInterval(t));
}
function zb(e, t) {
	var n = t.kind, r = Hb(e), i = Hy(e), a = (r.axisRotate - r.labelRotate) / 180 * Math.PI, o = e.scale, s = o.getExtent(), c = o.count();
	if (s[1] - s[0] < 1) return 0;
	var l = 1, u = 40;
	c > u && (l = Math.max(1, Math.floor(c / u)));
	for (var d = s[0], f = e.dataToCoord(d + 1) - e.dataToCoord(d), p = Math.abs(f * Math.cos(a)), m = Math.abs(f * Math.sin(a)), h = 0, g = 0; d <= s[1]; d += l) {
		var _ = 0, v = 0, y = sn(i({ value: d }), r.font, "center", "top");
		_ = y.width * 1.3, v = y.height * 1.3, h = Math.max(h, _, 7), g = Math.max(g, v, 7);
	}
	var b = h / p, x = g / m;
	isNaN(b) && (b = Infinity), isNaN(x) && (x = Infinity);
	var S = Math.max(0, Math.floor(Math.min(b, x)));
	return n === wb.estimate ? (t.out.noPxChangeTryDetermine.push(z(Bb, null, e, S, c)), S) : Vb(e, S, c) ?? S;
}
function Bb(e, t, n) {
	return Vb(e, t, n) == null;
}
function Vb(e, t, n) {
	var r = Sb(e.model), i = e.getExtent(), a = r.lastAutoInterval, o = r.lastTickCount;
	if (a != null && o != null && Math.abs(a - t) <= 1 && Math.abs(o - n) <= 1 && a > t && r.axisExtent0 === i[0] && r.axisExtent1 === i[1]) return a;
	r.lastTickCount = n, r.lastAutoInterval = t, r.axisExtent0 = i[0], r.axisExtent1 = i[1];
}
function Hb(e) {
	var t = e.getLabelModel();
	return {
		axisRotate: e.getRotate ? e.getRotate() : e.isHorizontal && !e.isHorizontal() ? 90 : 0,
		labelRotate: t.get("rotate") || 0,
		font: t.getFont()
	};
}
function Ub(e, t, n) {
	var r = Hy(e), i = e.scale, a = [], o = H(t);
	return dy(i, o ? 0 : t, function(e, s) {
		var c = i.getLabel(e);
		if (o) {
			var l = !!t(e.value, c);
			if (e.offInterval = !l, !l && !s) return;
		}
		a.push(n ? e : {
			formattedLabel: r(e),
			rawLabel: c,
			tick: e
		});
	}), a;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/util/cycleCache.js
var Wb = Ws();
function Gb(e) {
	Wb(e).prepare = {};
}
function Kb(e) {
	Wb(e).fullUpdate = {};
}
function qb(e) {
	return Wb(e).prepare;
}
function Jb(e) {
	return Wb(e).fullUpdate;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/coord/axisStatistics.js
var Yb = uc(), Xb = Ws(), Zb = Ws();
function Qb(e, t) {
	var n = e.model, r = Xb(Jb(n.ecModel)).keyed, i = r && r.get(t);
	return i && i.get(n.uid);
}
function $b(e, t) {
	return nx(Qb(e, t));
}
function ex(e, t) {
	var n = [];
	return tx(e.model.ecModel, function(e) {
		for (var r = 0; r < t.length; r++) t[r] && e.serByIdx[t[r].seriesIndex] && n.push(nx(e));
	}), n;
}
function tx(e, t) {
	var n = Xb(Jb(e)).keyed;
	n && n.each(function(e, n) {
		e.each(function(e, r) {
			t(e, n, r);
		});
	});
}
function nx(e) {
	return { liPosMinGap: e ? e.liPosMinGap : void 0 };
}
function rx(e, t) {
	var n = e.model.ecModel, r = Xb(Jb(n)).axSer;
	r && ax(n, r.get(e.model.uid), t);
}
function ix(e, t, n) {
	var r = Qb(e, t);
	r && ax(e.model.ecModel, r.sers, n);
}
function ax(e, t, n) {
	if (t) for (var r = 0; r < t.length; r++) {
		var i = t[r];
		e.isSeriesFiltered(i) || n(i);
	}
}
function ox(e, t, n) {
	var r = Xb(Jb(e)).keyed, i = r && r.get(t);
	i && i.each(function(e) {
		n(e.axis);
	});
}
function sx(e, t) {
	var n = e.model, r = Xb(Jb(n.ecModel)).keys;
	r && I(r.get(n.uid), function(e) {
		t(e);
	});
}
function cx(e) {
	var t = Zb(qb(e)), n = t.keyed ||= K();
	tx(e, function(t, r, i) {
		var a = n.get(r) || n.set(r, K()), o = a.get(i) || a.set(i, {});
		t.metrics.liPosMinGap && ux.liPosMinGap(e, t, o);
	});
}
function lx(e, t) {
	ux[e] = t;
}
var ux = {};
function dx(e, t, n) {
	if (e) {
		var r = t.ecModel, i = Xb(Jb(r)), a = e.model.uid, o = i.axSer ||= K();
		(o.get(a) || o.set(a, [])).push(t);
		var s = t.subType, c = t.getBaseAxis() === e, l = mx.get(fx(s, c, n)) || mx.get(fx(s, c, null));
		if (l) {
			var u = i.keyed ||= K(), d = i.keys ||= K(), f = l.key, p = u.get(f) || u.set(f, K()), m = p.get(a);
			m || (m = p.set(a, {
				axis: e,
				sers: [],
				serByIdx: []
			}), m.metrics = l.getMetrics(e), (d.get(a) || d.set(a, [])).push(f)), m.sers.push(t), m.serByIdx[t.seriesIndex] = t;
		}
	}
}
function fx(e, t, n) {
	return e + "|&" + G(t, !0) + "|&" + (n || "");
}
function px(e, t) {
	var n = fx(t.seriesType, t.baseAxis, t.coordSysType);
	mx.set(n, t), Yb(e, function() {
		e.registerProcessor(e.PRIORITY.PROCESSOR.AXIS_STATISTICS, { overallReset: cx });
	});
}
var mx = K(), hx = .8;
function gx(e, t) {
	t ||= {};
	var n = {
		w: NaN,
		w2: NaN
	}, r = e.scale, i = t.fromStat, a = t.min, o = Jv(r);
	ms(o) || (o = NaN);
	var s = e.getExtent(), c = Io(s[1] - s[0]);
	return ry(r) ? _x(n, e, o, c) : i && vx(n, e, o, c, i), a != null && (n.w = ms(n.w) ? Fo(a, n.w) : a), n;
}
function _x(e, t, n, r) {
	var i = t.onBand, a = n + +!!i;
	a === 0 && (a = 1), e.w = r / a, !i && n && r && (e.w2 = e.w * n / r);
}
function vx(e, t, n, r, i) {
	var a = !1, o = -Infinity;
	I(i.key ? [$b(t, i.key)] : ex(t, i.sers || []), function(e) {
		var t = e.liPosMinGap;
		t != null && (t > 0 ? (t > o && (o = t), a = !1) : t === -2 && (a = !0));
	}), ms(n) && n > 0 && ms(o) ? (e.w = r / n * o, e.w2 = o) : a && (e.w = r * hx, e.w2 = e.w * n / r);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/coord/Axis.js
var yx = [0, 1], bx = function() {
	function e(e, t, n) {
		this.onBand = !1, this.inverse = !1, this.dim = e, this.scale = t, this._extent = n || [0, 0];
	}
	return e.prototype.contain = function(e) {
		var t = this._extent, n = Math.min(t[0], t[1]), r = Math.max(t[0], t[1]);
		return e >= n && e <= r;
	}, e.prototype.containData = function(e) {
		return this.scale.contain(this.scale.parse(e));
	}, e.prototype.getExtent = function() {
		return this._extent.slice();
	}, e.prototype.setExtent = function(e, t) {
		var n = this._extent;
		n[0] = e, n[1] = t;
	}, e.prototype.dataToCoord = function(e, t) {
		var n = this.scale;
		return e = n.normalize(n.parse(e)), Go(e, yx, xx(this), t);
	}, e.prototype.coordToData = function(e, t) {
		var n = Go(e, xx(this), yx, t);
		return this.scale.scale(n);
	}, e.prototype.pointToData = function(e, t) {}, e.prototype.getTicksCoords = function(e) {
		e ||= {};
		var t = e.tickModel || this.getTickModel(), n = L(Db(this, t, {
			breakTicks: e.breakTicks,
			pruneByBreak: e.pruneByBreak
		}).ticks, function(e) {
			return {
				coord: this.dataToCoord(Qy(this.scale, e)),
				tick: e
			};
		}, this), r = t.get("alignWithLabel"), i = Sx(this, n, r);
		return L(n, function(e) {
			return {
				coord: e.coord,
				tickValue: e.tick.value,
				onBand: i
			};
		});
	}, e.prototype.getMinorTicksCoords = function() {
		if (ry(this.scale)) return [];
		var e = this.model.getModel("minorTick").get("splitNumber");
		return e > 0 && e < 100 || (e = 5), L(this.scale.getMinorTicks(e), function(e) {
			return L(e, function(e) {
				return {
					coord: this.dataToCoord(e),
					tickValue: e
				};
			}, this);
		}, this);
	}, e.prototype.getViewLabels = function(e) {
		return e ||= Tb(wb.determine), Eb(this, e).labels;
	}, e.prototype.getLabelModel = function() {
		return this.model.getModel("axisLabel");
	}, e.prototype.getTickModel = function() {
		return this.model.getModel("axisTick");
	}, e.prototype.getBandWidth = function() {
		return gx(this, { min: 1 }).w;
	}, e.prototype.calculateCategoryInterval = function(e) {
		return e ||= Tb(wb.determine), zb(this, e);
	}, e;
}();
function xx(e) {
	var t = e.getExtent();
	if (e.onBand) {
		var n = (t[1] - t[0]) / e.scale.count() / 2;
		t[0] += n, t[1] -= n;
	}
	return t;
}
function Sx(e, t, n) {
	var r = t.length;
	if (!e.onBand || n || !r) return !1;
	var i = gx(e).w;
	if (!i) return !1;
	I(t, function(e) {
		e.coord -= i / 2;
	});
	var a = e.scale.getExtent(), o = t[r - 1];
	return o.tick.offInterval && t.pop(), t.push({
		coord: o.coord + i,
		tick: { value: a[1] + 1 }
	}), !0;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/coord/cartesian/Axis2D.js
var Cx = function(e) {
	o(t, e);
	function t(t, n, r, i, a) {
		var o = e.call(this, t, n, r) || this;
		return o.index = 0, o.type = i || "value", o.position = a || "bottom", o;
	}
	return t.prototype.isHorizontal = function() {
		var e = this.position;
		return e === "top" || e === "bottom";
	}, t.prototype.getGlobalExtent = function(e) {
		var t = this.getExtent();
		return t[0] = this.toGlobalCoord(t[0]), t[1] = this.toGlobalCoord(t[1]), e && t[0] > t[1] && t.reverse(), t;
	}, t.prototype.pointToData = function(e, t) {
		return this.coordToData(this.toLocalCoord(e[this.dim === "x" ? 0 : 1]), t);
	}, t.prototype.setCategorySortInfo = function(e) {
		if (this.type !== "category") return !1;
		this.model.option.categorySortInfo = e, this.scale.setSortInfo(e);
	}, t;
}(bx), Tx = [
	"label",
	"labelLine",
	"layoutOption",
	"priority",
	"defaultAttr",
	"marginForce",
	"minMarginForce",
	"marginDefault",
	"suggestIgnore"
], Ex = 1, Dx = 2, Ox = Ex | Dx;
function kx(e, t, n) {
	n ||= Ox, t ? e.dirty |= n : e.dirty &= ~n;
}
function Ax(e, t) {
	return t ||= Ox, e.dirty == null || !!(e.dirty & t);
}
function jx(e) {
	if (e) return Ax(e) && Mx(e, e.label, e), e;
}
function Mx(e, t, n) {
	var r = t.getComputedTransform();
	e.transform = nf(e.transform, r);
	var i = e.localRect = tf(e.localRect, t.getBoundingRect()), a = t.style, o = a.margin, s = n && n.marginForce, c = n && n.minMarginForce, l = n && n.marginDefault, u = a.__marginType;
	u == null && l && (o = l, u = kf.textMargin);
	for (var d = 0; d < 4; d++) Nx[d] = u === kf.minMargin && c && c[d] != null ? c[d] : s && s[d] != null ? s[d] : o ? o[d] : 0;
	u === kf.textMargin && qd(i, Nx, !1, !1);
	var f = e.rect = tf(e.rect, i);
	return r && f.applyTransform(r), u === kf.minMargin && qd(f, Nx, !1, !1), e.axisAligned = $d(r), (e.label = e.label || {}).ignore = t.ignore, kx(e, !1), kx(e, !0, Dx), e;
}
var Nx = [
	0,
	0,
	0,
	0
];
function Px(e, t, n) {
	return e.transform = nf(e.transform, n), e.localRect = tf(e.localRect, t), e.rect = tf(e.rect, t), n && e.rect.applyTransform(n), e.axisAligned = $d(n), e.obb = void 0, (e.label = e.label || {}).ignore = !1, e;
}
function Fx(e, t) {
	if (e) {
		e.label.x += t.x, e.label.y += t.y, e.label.markRedraw();
		var n = e.transform;
		n && (n[4] += t.x, n[5] += t.y);
		var r = e.rect;
		r && (r.x += t.x, r.y += t.y);
		var i = e.obb;
		i && i.fromBoundingRect(e.localRect, n);
	}
}
function Ix(e, t) {
	for (var n = 0; n < Tx.length; n++) {
		var r = Tx[n];
		e[r] ?? (e[r] = t[r]);
	}
	return jx(e);
}
function Lx(e) {
	var t = e.obb;
	return (!t || Ax(e, Dx)) && (e.obb = t ||= new id(), t.fromBoundingRect(e.localRect, e.transform), kx(e, !1, Dx)), t;
}
function Rx(e, t, n, r, i) {
	var a = e.length, o = yd[t], s = bd[t];
	if (a < 2) return !1;
	e.sort(function(e, t) {
		return e.rect[o] - t.rect[o];
	});
	for (var c = 0, l, u = !1, d = 0, f = 0; f < a; f++) {
		var p = e[f], m = p.rect;
		l = m[o] - c, l < 0 && (m[o] -= l, p.label[o] -= l, u = !0);
		var h = Math.max(-l, 0);
		d += h, c = m[o] + m[s];
	}
	d > 0 && i && S(-d / a, 0, a);
	var g = e[0], _ = e[a - 1], v, y;
	b(), v < 0 && C(-v, .8), y < 0 && C(y, .8), b(), x(v, y, 1), x(y, v, -1), b(), v < 0 && w(-v), y < 0 && w(y);
	function b() {
		v = g.rect[o] - n, y = r - _.rect[o] - _.rect[s];
	}
	function x(e, t, n) {
		if (e < 0) {
			var r = Math.min(t, -e);
			if (r > 0) {
				S(r * n, 0, a);
				var i = r + e;
				i < 0 && C(-i * n, 1);
			} else C(-e * n, 1);
		}
	}
	function S(t, n, r) {
		t !== 0 && (u = !0);
		for (var i = n; i < r; i++) {
			var a = e[i], s = a.rect;
			s[o] += t, a.label[o] += t;
		}
	}
	function C(t, n) {
		for (var r = [], i = 0, c = 1; c < a; c++) {
			var l = e[c - 1].rect, u = Math.max(e[c].rect[o] - l[o] - l[s], 0);
			r.push(u), i += u;
		}
		if (i) {
			var d = Math.min(Math.abs(t) / i, n);
			if (t > 0) for (var c = 0; c < a - 1; c++) {
				var f = r[c] * d;
				S(f, 0, c + 1);
			}
			else for (var c = a - 1; c > 0; c--) {
				var f = r[c - 1] * d;
				S(-f, c, a);
			}
		}
	}
	function w(e) {
		var t = e < 0 ? -1 : 1;
		e = Math.abs(e);
		for (var n = Math.ceil(e / (a - 1)), r = 0; r < a - 1; r++) if (t > 0 ? S(n, 0, r + 1) : S(-n, a - r - 1, a), e -= n, e <= 0) return;
	}
	return u;
}
function zx(e) {
	var t = [];
	e.sort(function(e, t) {
		return !!t.suggestIgnore - +!!e.suggestIgnore || t.priority - e.priority;
	});
	function n(e) {
		if (!e.ignore) {
			var t = e.ensureState("emphasis");
			t.ignore ??= !1;
		}
		e.ignore = !0;
	}
	for (var r = 0; r < e.length; r++) {
		var i = jx(e[r]);
		if (!i.label.ignore) {
			for (var a = i.label, o = i.labelLine, s = !1, c = 0; c < t.length; c++) if (Bx(i, t[c], null, { touchThreshold: .05 })) {
				s = !0;
				break;
			}
			s ? (n(a), o && n(o)) : t.push(i);
		}
	}
}
function Bx(e, t, n, r) {
	return !e || !t || e.label && e.label.ignore || t.label && t.label.ignore || !e.rect.intersect(t.rect, n, r) ? !1 : e.axisAligned && t.axisAligned ? !0 : Lx(e).intersect(Lx(t), n, r);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/axis/axisBreakHelper.js
var Vx = null;
function Hx() {
	return Vx;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/axis/axisAction.js
var Ux = "expandAxisBreak", Wx = Math.PI, Gx = [
	[
		1,
		2,
		1,
		2
	],
	[
		5,
		3,
		5,
		3
	],
	[
		8,
		3,
		8,
		3
	]
], Kx = [
	[
		0,
		1,
		0,
		1
	],
	[
		0,
		3,
		0,
		3
	],
	[
		0,
		3,
		0,
		3
	]
], qx = Ws(), Jx = Ws(), Yx = function() {
	function e(e) {
		this.recordMap = {}, this.resolveAxisNameOverlap = e;
	}
	return e.prototype.ensureRecord = function(e) {
		var t = e.axis.dim, n = e.componentIndex, r = this.recordMap, i = r[t] || (r[t] = []);
		return i[n] || (i[n] = { ready: {} });
	}, e;
}();
function Xx(e, t, n, r) {
	var i = n.axis, a = t.ensureRecord(n), o = [], s, c = bS(e.axisName) && qy(e.nameLocation);
	I(r, function(e) {
		var t = jx(e);
		if (!(!t || t.label.ignore)) {
			o.push(t);
			var n = a.transGroup;
			c && (n.transform ? pt(Zx, n.transform) : st(Zx), t.transform && lt(Zx, Zx, t.transform), Y.copy(Qx, t.localRect), Qx.applyTransform(Zx), s ? s.union(Qx) : Y.copy(s = new Y(0, 0, 0, 0), Qx));
		}
	});
	var l = Math.abs(a.dirVec.x) > .1 ? "x" : "y", u = a.transGroup[l];
	if (o.sort(function(e, t) {
		return Math.abs(e.label[l] - u) - Math.abs(t.label[l] - u);
	}), c && s) {
		var d = i.getExtent(), f = Math.min(d[0], d[1]), p = Math.max(d[0], d[1]) - f;
		s.union(new Y(f, 0, p, 1));
	}
	a.stOccupiedRect = s, a.labelInfoList = o;
}
var Zx = ot(), Qx = new Y(0, 0, 0, 0), $x = function(e, t, n, r, i, a) {
	if (qy(e.nameLocation)) {
		var o = a.stOccupiedRect;
		o && eS(Px({}, o, a.transGroup.transform), r, i);
	} else tS(a.labelInfoList, a.dirVec, r, i);
};
function eS(e, t, n) {
	var r = new J();
	Bx(e, t, r, {
		direction: Math.atan2(n.y, n.x),
		bidirectional: !1,
		touchThreshold: .05
	}) && Fx(t, r);
}
function tS(e, t, n, r) {
	for (var i = J.dot(r, t) >= 0, a = 0, o = e.length; a < o; a++) {
		var s = e[i ? a : o - 1 - a];
		s.label.ignore || eS(s, n, r);
	}
}
var nS = function() {
	function e(e, t, n, r) {
		this.group = new su(), this._axisModel = e, this._api = t, this._local = {}, this._shared = r || new Yx($x), this._resetCfgDetermined(n);
	}
	return e.prototype.updateCfg = function(e) {
		var t = this._cfg.raw;
		t.position = e.position, t.labelOffset = e.labelOffset, this._resetCfgDetermined(t);
	}, e.prototype.__getRawCfg = function() {
		return this._cfg.raw;
	}, e.prototype._resetCfgDetermined = function(e) {
		var t = this._axisModel, n = t.getDefaultOption ? t.getDefaultOption() : {}, r = G(e.axisName, t.get("name")), i = t.get("nameMoveOverlap");
		(i == null || i === "auto") && (i = G(e.defaultNameMoveOverlap, !0));
		var a = {
			raw: e,
			position: e.position,
			rotation: e.rotation,
			nameDirection: G(e.nameDirection, 1),
			tickDirection: G(e.tickDirection, 1),
			labelDirection: G(e.labelDirection, 1),
			labelOffset: G(e.labelOffset, 0),
			silent: G(e.silent, !0),
			axisName: r,
			nameLocation: he(t.get("nameLocation"), n.nameLocation, "end"),
			shouldNameMoveOverlap: bS(r) && i,
			optionHideOverlap: t.get(["axisLabel", "hideOverlap"]),
			showMinorTicks: t.get(["minorTick", "show"])
		};
		this._cfg = a;
		var o = new su({
			x: a.position[0],
			y: a.position[1],
			rotation: a.rotation
		});
		o.updateTransform(), this._transformGroup = o;
		var s = this._shared.ensureRecord(t);
		s.transGroup = this._transformGroup, s.dirVec = new J(Math.cos(-a.rotation), Math.sin(-a.rotation));
	}, e.prototype.build = function(e, t) {
		var n = this;
		return e ||= {
			axisLine: !0,
			axisTickLabelEstimate: !1,
			axisTickLabelDetermine: !0,
			axisName: !0
		}, I(rS, function(r) {
			e[r] && iS[r](n._cfg, n._local, n._shared, n._axisModel, n.group, n._transformGroup, n._api, t || {});
		}), this;
	}, e.innerTextLayout = function(e, t, n) {
		var r = ns(t - e), i, a;
		return rs(r) ? (a = n > 0 ? "top" : "bottom", i = "center") : rs(r - Wx) ? (a = n > 0 ? "bottom" : "top", i = "center") : (a = "middle", i = r > 0 && r < Wx ? n > 0 ? "right" : "left" : n > 0 ? "left" : "right"), {
			rotation: r,
			textAlign: i,
			textVerticalAlign: a
		};
	}, e.makeAxisEventDataBase = function(e) {
		var t = {
			componentType: e.mainType,
			componentIndex: e.componentIndex
		};
		return t[e.mainType + "Index"] = e.componentIndex, t;
	}, e.isLabelSilent = function(e) {
		var t = e.get("tooltip");
		return e.get("silent") || !(e.get("triggerEvent") || t && t.show);
	}, e;
}(), rS = [
	"axisLine",
	"axisTickLabelEstimate",
	"axisTickLabelDetermine",
	"axisName"
], iS = {
	axisLine: function(e, t, n, r, i, a, o) {
		var s = r.get(["axisLine", "show"]);
		if (s === "auto" && (s = !0, e.raw.axisLineAutoShow != null && (s = !!e.raw.axisLineAutoShow)), s) {
			var c = r.axis.getExtent(), l = a.transform, u = [c[0], 0], d = [c[1], 0], f = u[0] > d[0];
			l && (Ot(u, u, l), Ot(d, d, l));
			var p = j({ lineCap: "round" }, r.getModel(["axisLine", "lineStyle"]).getLineStyle()), m = {
				strokeContainThreshold: e.raw.strokeContainThreshold || 5,
				silent: !0,
				z2: 1,
				style: p
			};
			if (r.get(["axisLine", "breakLine"]) && wh(r.axis.scale)) Hx().buildAxisBreakLine(r, i, a, m);
			else {
				var h = new zu(j({ shape: {
					x1: u[0],
					y1: u[1],
					x2: d[0],
					y2: d[1]
				} }, m));
				jd(h.shape, h.style.lineWidth), h.anid = "line", i.add(h);
			}
			var g = r.get(["axisLine", "symbol"]);
			if (g != null) {
				var _ = r.get(["axisLine", "symbolSize"]);
				U(g) && (g = [g, g]), (U(_) || se(_)) && (_ = [_, _]);
				var v = Z_(r.get(["axisLine", "symbolOffset"]) || 0, _), y = _[0], b = _[1];
				I([{
					rotate: e.rotation + Math.PI / 2,
					offset: v[0],
					r: 0
				}, {
					rotate: e.rotation - Math.PI / 2,
					offset: v[1],
					r: Math.sqrt((u[0] - d[0]) * (u[0] - d[0]) + (u[1] - d[1]) * (u[1] - d[1]))
				}], function(t, n) {
					if (g[n] !== "none" && g[n] != null) {
						var r = Y_(g[n], -y / 2, -b / 2, y, b, p.stroke, !0), a = t.r + t.offset, o = f ? d : u;
						r.attr({
							rotation: t.rotate,
							x: o[0] + a * Math.cos(e.rotation),
							y: o[1] - a * Math.sin(e.rotation),
							silent: !0,
							z2: 11
						}), i.add(r);
					}
				});
			}
		}
	},
	axisTickLabelEstimate: function(e, t, n, r, i, a, o, s) {
		pS(t, i, s) && aS(e, t, n, r, i, a, o, wb.estimate);
	},
	axisTickLabelDetermine: function(e, t, n, r, i, a, o, s) {
		pS(t, i, s) && aS(e, t, n, r, i, a, o, wb.determine);
		var c = dS(e, i, a, r);
		cS(e, t.labelLayoutList, c), fS(e, i, a, r, e.tickDirection);
	},
	axisName: function(e, t, n, r, i, a, o, s) {
		var c = n.ensureRecord(r);
		t.nameEl &&= (i.remove(t.nameEl), c.nameLayout = c.nameLocation = null);
		var l = e.axisName;
		if (bS(l)) {
			var u = e.nameLocation, d = e.nameDirection, f = r.getModel("nameTextStyle"), p = r.get("nameGap") || 0, m = r.axis.getExtent(), h = r.axis.inverse ? -1 : 1, g = new J(0, 0), _ = new J(0, 0);
			u === "start" ? (g.x = m[0] - h * p, _.x = -h) : u === "end" ? (g.x = m[1] + h * p, _.x = h) : (g.x = (m[0] + m[1]) / 2, g.y = e.labelOffset + d * p, _.y = d);
			var v = ot();
			_.transform(dt(v, v, e.rotation));
			var y = r.get("nameRotate");
			y != null && (y = y * Wx / 180);
			var b, x;
			qy(u) ? b = nS.innerTextLayout(e.rotation, y ?? e.rotation, d) : (b = oS(e.rotation, u, y || 0, m), x = e.raw.axisNameAvailableWidth, x != null && (x = Math.abs(x / Math.sin(b.rotation)), !isFinite(x) && (x = null)));
			var S = f.getFont(), C = r.get("nameTruncate", !0) || {}, w = C.ellipsis, T = me(e.raw.nameTruncateMaxWidth, C.maxWidth, x), E = s.nameMarginLevel || 0, D = new _o({
				x: g.x,
				y: g.y,
				rotation: b.rotation,
				silent: nS.isLabelSilent(r),
				style: _f(f, {
					text: l,
					font: S,
					overflow: "truncate",
					width: T,
					ellipsis: w,
					fill: f.getTextColor() || r.get([
						"axisLine",
						"lineStyle",
						"color"
					]),
					align: f.get("align") || b.textAlign,
					verticalAlign: f.get("verticalAlign") || b.textVerticalAlign
				}),
				z2: 1
			});
			if (Xd({
				el: D,
				componentModel: r,
				itemName: l
			}), D.__fullText = l, D.anid = "name", r.get("triggerEvent")) {
				var O = nS.makeAxisEventDataBase(r);
				O.targetType = "axisName", O.name = l, yc(D).eventData = O;
			}
			a.add(D), D.updateTransform(), t.nameEl = D;
			var k = c.nameLayout = jx({
				label: D,
				priority: D.z2,
				defaultAttr: { ignore: D.ignore },
				marginDefault: qy(u) ? Gx[E] : Kx[E]
			});
			if (c.nameLocation = u, i.add(D), D.decomposeTransform(), e.shouldNameMoveOverlap && k) {
				var A = n.ensureRecord(r);
				n.resolveAxisNameOverlap(e, n, r, k, _, A);
			}
		}
	}
};
function aS(e, t, n, r, i, a, o, s) {
	hS(t) || mS(e, t, i, s, r, o);
	var c = t.labelLayoutList;
	_S(e, r, c, a), SS(r, e.rotation, c);
	var l = e.optionHideOverlap;
	sS(r, c, l), l && zx(re(c, function(e) {
		return e && !e.label.ignore;
	})), Xx(e, n, r, c);
}
function oS(e, t, n, r) {
	var i = ns(n - e), a, o, s = r[0] > r[1], c = t === "start" && !s || t !== "start" && s;
	return rs(i - Wx / 2) ? (o = c ? "bottom" : "top", a = "center") : rs(i - Wx * 1.5) ? (o = c ? "top" : "bottom", a = "center") : (o = "middle", a = i < Wx * 1.5 && i > Wx / 2 ? c ? "left" : "right" : c ? "right" : "left"), {
		rotation: i,
		textAlign: a,
		textVerticalAlign: o
	};
}
function sS(e, t, n) {
	var r = e.axis, i = e.get(["axisLabel", "customValues"]);
	if (Gy(r)) return;
	function a(e, a, o) {
		var s = jx(t[a]), c = jx(t[o]), l = r.scale;
		if (!(!s || !c)) {
			if (e == null) {
				if (!n && i) return;
				var u = qx(s.label).labelInfo.tick;
				if (ty(l) && u.notNice || ry(l) && u.offInterval) {
					lS(s.label);
					return;
				}
			}
			if (e === !1 || s.suggestIgnore) {
				lS(s.label);
				return;
			}
			if (c.suggestIgnore) {
				lS(c.label);
				return;
			}
			var d = .1;
			if (!n) {
				var f = [
					0,
					0,
					0,
					0
				];
				s = Ix({ marginForce: f }, s), c = Ix({ marginForce: f }, c);
			}
			Bx(s, c, null, { touchThreshold: d }) && lS(e ? c.label : s.label);
		}
	}
	var o = e.get(["axisLabel", "showMinLabel"]), s = e.get(["axisLabel", "showMaxLabel"]), c = t.length;
	a(o, 0, 1), a(s, c - 1, c - 2);
}
function cS(e, t, n) {
	e.showMinorTicks || I(t, function(e) {
		if (e && e.label.ignore) for (var t = 0; t < n.length; t++) {
			var r = n[t], i = Jx(r), a = qx(e.label);
			if (i.tickValue != null && !i.onBand && i.tickValue === a.labelInfo.tick.value) {
				lS(r);
				return;
			}
		}
	});
}
function lS(e) {
	e && (e.ignore = !0);
}
function uS(e, t, n, r, i) {
	for (var a = [], o = [], s = [], c = 0; c < e.length; c++) {
		var l = e[c].coord;
		o[0] = l, o[1] = 0, s[0] = l, s[1] = n, t && (Ot(o, o, t), Ot(s, s, t));
		var u = new zu({
			shape: {
				x1: o[0],
				y1: o[1],
				x2: s[0],
				y2: s[1]
			},
			style: r,
			z2: 2,
			autoBatch: !0,
			silent: !0
		});
		jd(u.shape, u.style.lineWidth), u.anid = i + "_" + e[c].tickValue, a.push(u);
		var d = Jx(u);
		d.onBand = !!e[c].onBand, d.tickValue = e[c].tickValue;
	}
	return a;
}
function dS(e, t, n, r) {
	var i = r.axis, a = r.getModel("axisTick"), o = a.get("show");
	if (o === "auto" && (o = !0, e.raw.axisTickAutoShow != null && (o = !!e.raw.axisTickAutoShow)), !o || i.scale.isBlank()) return [];
	for (var s = a.getModel("lineStyle"), c = e.tickDirection * a.get("length"), l = uS(i.getTicksCoords(), n.transform, c, M(s.getLineStyle(), { stroke: r.get([
		"axisLine",
		"lineStyle",
		"color"
	]) }), "ticks"), u = 0; u < l.length; u++) t.add(l[u]);
	return l;
}
function fS(e, t, n, r, i) {
	var a = r.axis, o = r.getModel("minorTick");
	if (!(!e.showMinorTicks || a.scale.isBlank())) {
		var s = a.getMinorTicksCoords();
		if (s.length) for (var c = o.getModel("lineStyle"), l = i * o.get("length"), u = M(c.getLineStyle(), M(r.getModel("axisTick").getLineStyle(), { stroke: r.get([
			"axisLine",
			"lineStyle",
			"color"
		]) })), d = 0; d < s.length; d++) for (var f = uS(s[d], n.transform, l, u, "minorticks_" + d), p = 0; p < f.length; p++) t.add(f[p]);
	}
}
function pS(e, t, n) {
	if (hS(e)) {
		var r = e.axisLabelsCreationContext.out.noPxChangeTryDetermine;
		if (n.noPxChange) {
			for (var i = !0, a = 0; a < r.length; a++) i &&= r[a]();
			if (i) return !1;
		}
		r.length && (t.remove(e.labelGroup), gS(e, null, null, null));
	}
	return !0;
}
function mS(e, t, n, r, i, a) {
	var o = i.axis, s = me(e.raw.axisLabelShow, i.get(["axisLabel", "show"])), c = new su();
	n.add(c);
	var l = Tb(r);
	if (!s || o.scale.isBlank()) {
		gS(t, [], c, l);
		return;
	}
	var u = i.getModel("axisLabel"), d = o.getViewLabels(l), f = (me(e.raw.labelRotate, u.get("rotate")) || 0) * Wx / 180, p = nS.innerTextLayout(e.rotation, f, e.labelDirection), m = i.getCategories && i.getCategories(!0), h = [], g = i.get("triggerEvent"), _ = Infinity, v = -Infinity;
	I(d, function(e, t) {
		var n = e.tick, r = e.formattedLabel, s = e.rawLabel, l = u, f = Qy(o.scale, n);
		if (m && m[f]) {
			var y = m[f];
			W(y) && y.textStyle && (l = new Bf(y.textStyle, u, i.ecModel));
		}
		var b = l.getTextColor() || i.get([
			"axisLine",
			"lineStyle",
			"color"
		]), x = l.getShallow("align", !0) || p.textAlign, S = G(l.getShallow("alignMinLabel", !0), x), C = G(l.getShallow("alignMaxLabel", !0), x), w = l.getShallow("verticalAlign", !0) || l.getShallow("baseline", !0) || p.textVerticalAlign, T = G(l.getShallow("verticalAlignMinLabel", !0), w), E = G(l.getShallow("verticalAlignMaxLabel", !0), w), D = 10 + (n.time?.level || 0);
		_ = Math.min(_, D), v = Math.max(v, D);
		var O = new _o({
			x: 0,
			y: 0,
			rotation: 0,
			silent: nS.isLabelSilent(i),
			z2: D,
			style: _f(l, {
				text: r,
				align: t === 0 ? S : t === d.length - 1 ? C : x,
				verticalAlign: t === 0 ? T : t === d.length - 1 ? E : w,
				fill: H(b) ? b(o.type === "category" ? s : o.type === "value" ? f + "" : f, t) : b
			})
		});
		O.anid = "label_" + f;
		var k = qx(O);
		if (k.labelInfo = e, k.layoutRotation = p.rotation, Xd({
			el: O,
			componentModel: i,
			itemName: r,
			formatterParamsExtra: {
				isTruncated: function() {
					return O.isTruncated;
				},
				value: s,
				tickIndex: t
			}
		}), g) {
			var A = nS.makeAxisEventDataBase(i);
			A.targetType = "axisLabel", A.value = s, A.tickIndex = t;
			var j = e.tick.break;
			if (j) {
				var ee = j.parsedBreak;
				A.break = {
					start: ee.vmin,
					end: ee.vmax
				};
			}
			o.type === "category" && (A.dataIndex = f), yc(O).eventData = A, j && xS(i, a, O, j);
		}
		h.push(O), c.add(O);
	}), gS(t, L(h, function(e) {
		return {
			label: e,
			priority: qx(e).labelInfo.tick.break ? e.z2 + (v - _ + 1) : e.z2,
			defaultAttr: { ignore: e.ignore }
		};
	}), c, l);
}
function hS(e) {
	return !!e.labelLayoutList;
}
function gS(e, t, n, r) {
	e.labelLayoutList = t, e.labelGroup = n, e.axisLabelsCreationContext = r;
}
function _S(e, t, n, r) {
	var i = t.get(["axisLabel", "margin"]);
	I(n, function(n, a) {
		var o = jx(n);
		if (o) {
			var s = o.label, c = qx(s);
			o.suggestIgnore = s.ignore, s.ignore = !1, Gn(vS, yS);
			var l = t.axis;
			vS.x = l.dataToCoord(Qy(l.scale, c.labelInfo.tick)), vS.y = e.labelOffset + e.labelDirection * i, vS.rotation = c.layoutRotation, r.add(vS), vS.updateTransform(), r.remove(vS), vS.decomposeTransform(), Gn(s, vS), s.markRedraw(), kx(o, !0), jx(o);
		}
	});
}
var vS = new fo(), yS = new fo();
function bS(e) {
	return !!e;
}
function xS(e, t, n, r) {
	n.on("click", function(n) {
		var i = {
			type: Ux,
			breaks: [{
				start: r.parsedBreak.breakOption.start,
				end: r.parsedBreak.breakOption.end
			}]
		};
		i[e.axis.dim + "AxisIndex"] = e.componentIndex, t.dispatchAction(i);
	});
}
function SS(e, t, n) {
	var r = xh();
	if (r) {
		var i = r.retrieveAxisBreakPairs(n, function(e) {
			return e && qx(e.label).labelInfo.tick.break;
		}, !0), a = e.get(["breakLabelLayout", "moveOverlap"], !0);
		(a === !0 || a === "auto") && I(i, function(r) {
			Hx().adjustBreakLabelPair(e.axis.inverse, t, [jx(n[r[0]]), jx(n[r[1]])]);
		});
	}
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/coord/cartesian/cartesianAxisHelper.js
function CS(e, t, n) {
	n ||= {};
	var r = t.axis, i = {}, a = r.getAxesOnZeroOf()[0], o = r.position, s = a ? "onZero" : o, c = r.dim, l = [
		e.x,
		e.x + e.width,
		e.y,
		e.y + e.height
	], u = {
		left: 0,
		right: 1,
		top: 0,
		bottom: 1,
		onZero: 2
	}, d = t.get("offset") || 0, f = c === "x" ? [l[2] - d, l[3] + d] : [l[0] - d, l[1] + d];
	if (a) {
		var p = a.toGlobalCoord(a.dataToCoord(0));
		f[u.onZero] = Math.max(Math.min(p, f[1]), f[0]);
	}
	i.position = [c === "y" ? f[u[s]] : l[0], c === "x" ? f[u[s]] : l[3]], i.rotation = Math.PI / 2 * (c === "x" ? 0 : 1), i.labelDirection = i.tickDirection = i.nameDirection = {
		top: -1,
		bottom: 1,
		left: -1,
		right: 1
	}[o], i.labelOffset = a ? f[u[o]] - f[u.onZero] : 0, t.get(["axisTick", "inside"]) && (i.tickDirection = -i.tickDirection), me(n.labelInside, t.get(["axisLabel", "inside"])) && (i.labelDirection = -i.labelDirection);
	var m = t.get(["axisLabel", "rotate"]);
	return i.labelRotate = s === "top" ? -m : m, i.z2 = 1, i;
}
function wS(e) {
	return e.coordinateSystem && e.coordinateSystem.type === "cartesian2d";
}
function TS(e) {
	var t = {
		xAxisModel: null,
		yAxisModel: null
	};
	return I(t, function(n, r) {
		var i = r.replace(/Model$/, "");
		t[r] = e.getReferringComponents(i, Js).models[0];
	}), t;
}
function ES(e, t, n, r, i, a) {
	for (var o = CS(e, n), s = !1, c = !1, l = 0; l < t.length; l++) $v(t[l].getOtherAxis(n.axis).scale) && (s = c = !0, n.axis.type === "category" && n.axis.onBand && (c = !1));
	return o.axisLineAutoShow = s, o.axisTickAutoShow = c, o.defaultNameMoveOverlap = a, new nS(n, r, o, i);
}
function DS(e, t, n) {
	var r = CS(t, n);
	e.updateCfg(r);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/coord/scaleRawExtentInfo.js
var OS = Ws(), kS = 3, AS = function() {
	function e(e, t, n, r, i) {
		var a = ry(e), o = a ? t.getCategories().length : null, s;
		if (a) {
			var c = t.getCategories(!0);
			s = c && !c.length;
		}
		var l = n.slice();
		(ey(e) || ny(e) || ty(e)) && (rc(l, MS(e, t.get("dataMin", !0))), ic(l, MS(e, t.get("dataMax", !0)))), cc(l) || (l[0] = l[1] = NaN);
		var u = [], d = [!1, !1], f = t.get("min", !0);
		f === "dataMin" ? (u[0] = l[0], d[0] = !0) : (u[0] = MS(e, H(f) ? f({
			min: l[0],
			max: l[1]
		}) : f), d[0] = u[0] != null);
		var p = t.get("max", !0);
		p === "dataMax" ? (u[1] = l[1], d[1] = !0) : (u[1] = MS(e, H(p) ? p({
			min: l[0],
			max: l[1]
		}) : p), d[1] = u[1] != null);
		var m = NS(e, t), h = a ? null : l[1] - l[0] || Math.abs(l[0]);
		u[0] ??= a ? s ? l[0] : o ? 0 : NaN : l[0] - m[0] * h, u[1] ??= a ? s ? l[1] : o ? o - 1 : NaN : l[1] + m[1] * h, !oc(u[0]) && (u[0] = NaN), !oc(u[1]) && (u[1] = NaN);
		var g = s || pe(u[0]) || pe(u[1]) || a && !o, _ = ey(e), v = _ && t.needIncludeZero && t.needIncludeZero();
		v && (u[0] > 0 && u[1] > 0 && !d[0] && (u[0] = 0), u[0] < 0 && u[1] < 0 && !d[1] && (u[1] = 0));
		var y = !1;
		u[0] > u[1] && (u.reverse(), y = !0);
		var b = MS(e, t.get("startValue", !0)), x = b != null;
		!ms(b) && r && (b = e.getDefaultStartValue ? e.getDefaultStartValue() : 0), ms(b) && (x || !_ || v) && (b < u[0] && !d[0] ? (u[0] = b, d[0] = !0) : b > u[1] && !d[1] && (u[1] = b, d[1] = !0)), jS(this._i = {
			scale: e,
			dataMM: l,
			noZoomEffMM: u,
			zoomMM: [],
			fixMM: d,
			zoomFixMM: [!1, !1],
			startValue: b,
			isBlank: g,
			incl0: v,
			tggAxInv: y,
			ctnShp: i
		}, u);
	}
	return e.prototype.makeNoZoom = function() {
		return this._i.noZoomEffMM.slice();
	}, e.prototype.makeFinal = function() {
		var e = this._i, t = e.zoomMM, n = e.noZoomEffMM, r = e.zoomFixMM, i = e.fixMM, a = {
			fixMM: i,
			zoomFixMM: r,
			isBlank: e.isBlank,
			incl0: e.incl0,
			tggAxInv: e.tggAxInv,
			ctnShp: e.ctnShp,
			effMM: n.slice()
		}, o = a.effMM;
		return t[0] != null && (o[0] = t[0], i[0] = r[0] = !0), t[1] != null && (o[1] = t[1], i[1] = r[1] = !0), jS(e, o), a;
	}, e.prototype.makeRenderInfo = function() {
		return { startValue: this._i.startValue };
	}, e.prototype.setZoomMM = function(e, t) {
		this._i.zoomMM[e] = t;
	}, e;
}();
function jS(e, t) {
	var n = e.scale, r = e.dataMM;
	n.sanitize && (t[0] = n.sanitize(t[0], r), t[1] = n.sanitize(t[1], r), lc(t));
}
function MS(e, t) {
	return t == null ? null : pe(t) ? NaN : e.parse(t);
}
function NS(e, t) {
	var n;
	if (ry(e)) n = [0, 0];
	else {
		var r = t.get("boundaryGap");
		typeof r == "boolean" && (r = null), n = V(r) ? r : [r, r];
	}
	return [PS(n[0]), PS(n[1])];
}
function PS(e) {
	return dn(typeof e == "boolean" ? 0 : e, 1) || 0;
}
function FS(e) {
	var t = OS(e.scale);
	return t.extent ||= tc(), t;
}
function IS(e, t) {
	FS(e).dimIdxInCoord = t.get(e.dim);
}
function LS(e, t) {
	var n = e.scale, r = e.model, i = e.dim;
	n.rawExtentInfo || RS(n, e, i, r, t);
}
function RS(e, t, n, r, i) {
	var a = FS(t), o = a.extent, s = !1;
	rx(t, function(r) {
		if (r.boxCoordinateSystem) {
			var i = Dm(r).coord, c = a.dimIdxInCoord;
			if (c >= 0 && V(i)) {
				var l = i[c];
				l != null && !V(l) && nc(o, e.parse(l));
			}
		} else if (r.coordinateSystem) {
			var u = r.getData();
			if (u) {
				var d = e.getFilter ? e.getFilter() : null;
				I(Ky(u, n), function(e) {
					ac(o, u.getApproximateExtent(e, d));
				});
			}
			r.__requireStartValue && r.__requireStartValue(t) && (s = !0);
		}
	});
	var c = WS(e, t, r);
	BS(e, new AS(e, r, o, s, c), i), a.extent = null;
}
function zS(e, t) {
	var n = e.scale;
	BS(n, new AS(n, e.model, t, !1, !1), kS);
}
function BS(e, t, n) {
	e.rawExtentInfo = t, t.from = n;
}
function VS(e, t) {
	HS.set(e, t);
}
var HS = K();
function US(e, t, n, r, i) {
	e.rawExtentInfo || zS({
		scale: e,
		model: t
	}, i || tc());
	var a = e.rawExtentInfo.makeFinal(), o = a.effMM;
	return e.setExtent(o[0], o[1]), e.setBlank(a.isBlank), r && a.tggAxInv && n && !n.get("legacyMinMaxDontInverseAxis") && (r.inverse = !r.inverse), a;
}
function WS(e, t, n) {
	var r = $y(e, n), i = n.get("containShape", !0);
	if (i == null && !r && (i = !0), !i) return !1;
	var a = !1;
	return sx(t, function(e) {
		a = !!HS.get(e) || a;
	}), a;
}
function GS(e, t, n, r) {
	if (n.ctnShp) {
		var i;
		if (sx(e, function(t) {
			var n = HS.get(t);
			if (n) {
				var a = n(e, r);
				a && (i ||= [0, 0], rc(i, a[0]), ic(i, a[1]), By(e));
			}
		}), i) {
			var a = t.getExtent();
			if (ry(t)) e.onBand || t.setExtent2(1, Po(a[0], a[0] + i[0]), Fo(a[1], a[1] + i[1]));
			else {
				var o = a.slice();
				n.zoomFixMM[0] || (o[0] = Po(o[0], t.transformOut(t.transformIn(o[0], null) + i[0], null))), n.zoomFixMM[1] || (o[1] = Fo(o[1], t.transformOut(t.transformIn(o[1], null) + i[1], null))), (o[0] < a[0] || o[1] > a[1]) && t.setExtent2(1, o[0], o[1]);
			}
		}
	}
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/coord/axisStatisticsMetricsImpl.js
function KS() {
	lx("liPosMinGap", qS);
}
function qS(e, t, n) {
	var r = K(), i = n.serUids, a = n.liPosMinGap, o, s = t.axis, c = s.scale, l = c.needTransform(), u = c.getFilter ? c.getFilter() : null, d = Rp(u);
	function f(n) {
		ax(e, t.sers, function(e) {
			var t = e.getRawData(), r = t.getDimensionIndex(t.mapDimension(s.dim));
			r >= 0 && n(r, e, t.getStore());
		});
	}
	var p = 0;
	if (f(function(e, t, n) {
		r.set(t.uid, 1), (!i || !i.hasKey(t.uid)) && (o = !0), p += n.count();
	}), (!i || i.keys().length !== r.keys().length) && (o = !0), !o && a != null) {
		t.liPosMinGap = a;
		return;
	}
	hv(JS, p);
	var m = 0;
	f(function(e, t, n) {
		for (var r = 0, i = n.count(); r < i; ++r) {
			var a = n.get(e, r);
			isFinite(a) && (!u || zp(d, a)) && (l && (a = c.transformIn(a, null)), JS.arr[m++] = a);
		}
	});
	var h = JS.typed ? JS.arr.subarray(0, m) : (JS.arr.length = m, JS.arr);
	JS.typed ? h.sort() : Yo(h);
	for (var g = Infinity, _ = 1; _ < m; ++_) {
		var v = h[_] - h[_ - 1];
		v > 0 && v < g && (g = v);
	}
	n.liPosMinGap = t.liPosMinGap = ms(g) ? g : m > 0 ? -2 : -1, n.serUids = r;
}
var JS = hv({ ctor: pv }, 50);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/helper/axisSnippets.js
function YS(e) {
	return function(t, n) {
		var r = gx(t, { fromStat: { key: e } });
		if (ms(r.w2)) return [-r.w2 / 2, r.w2 / 2];
	};
}
function XS(e, t) {
	return e + "|&" + t;
}
function ZS(e) {
	return KS(), { liPosMinGap: !ry(e.scale) };
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/layout/barCommon.js
function QS(e, t, n, r) {
	px(e, {
		key: t,
		seriesType: n,
		coordSysType: r,
		getMetrics: ZS
	});
}
function $S(e) {
	return e.scale.rawExtentInfo.makeRenderInfo().startValue;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/coord/cartesian/GridModel.js
var eC = {
	left: 0,
	right: 0,
	top: 0,
	bottom: 0
}, tC = ["25%", "25%"], nC = "cartesian2d", rC = function(e) {
	o(t, e);
	function t() {
		return e !== null && e.apply(this, arguments) || this;
	}
	return t.prototype.mergeDefaultAndTheme = function(t, n) {
		var r = Ag(t.outerBounds);
		e.prototype.mergeDefaultAndTheme.apply(this, arguments), r && t.outerBounds && kg(t.outerBounds, r);
	}, t.prototype.mergeOption = function(t, n) {
		e.prototype.mergeOption.apply(this, arguments), this.option.outerBounds && t.outerBounds && kg(this.option.outerBounds, t.outerBounds);
	}, t.type = "grid", t.dependencies = ["xAxis", "yAxis"], t.layoutMode = "box", t.defaultOption = {
		show: !1,
		z: 0,
		left: "15%",
		top: 65,
		right: "10%",
		bottom: 80,
		containLabel: !1,
		outerBoundsMode: "auto",
		outerBounds: eC,
		outerBoundsContain: "all",
		outerBoundsClampWidth: tC[0],
		outerBoundsClampHeight: tC[1],
		backgroundColor: Q.color.transparent,
		borderWidth: 1,
		borderColor: Q.color.neutral30
	}, t;
}(Ng), iC = uc(), aC = "__ec_stack_";
function oC(e) {
	return e.get("stack") || aC + e.seriesIndex;
}
function sC(e, t) {
	var n = cC(e, t);
	return n.columnMap = lC(n), n;
}
function cC(e, t) {
	var n = XS(t, nC), r = [], i = gx(e, {
		fromStat: { key: n },
		min: 1
	});
	return ix(e, n, function(e) {
		r.push({
			barWidth: X(e.get("barWidth"), i.w),
			barMaxWidth: X(e.get("barMaxWidth"), i.w),
			barMinWidth: X(e.get("barMinWidth") || (fC(e) ? .5 : 1), i.w),
			barGap: e.get("barGap"),
			barCategoryGap: e.get("barCategoryGap"),
			defaultBarGap: e.get("defaultBarGap"),
			stackId: oC(e)
		});
	}), {
		bandWidthResult: i,
		seriesInfo: r
	};
}
function lC(e) {
	var t = e.bandWidthResult.w, n = t, r = 0, i, a, o = [], s = {};
	I(e.seriesInfo, function(e, t) {
		t || (a = e.defaultBarGap || 0);
		var c = e.stackId;
		Ae(s, c) || r++;
		var l = s[c];
		l || (l = s[c] = {
			width: 0,
			maxWidth: 0
		}, o.push(c));
		var u = e.barWidth;
		u && !l.width && (l.width = u, u = Po(n, u), n -= u);
		var d = e.barMaxWidth;
		d && (l.maxWidth = d);
		var f = e.barMinWidth;
		f && (l.minWidth = f);
		var p = e.barGap;
		p != null && (a = p);
		var m = e.barCategoryGap;
		m != null && (i = m);
	}), i ??= Fo(35 - o.length * 4, 15) + "%";
	var c = X(i, t), l = X(a, 1), u = (n - c) / (r + (r - 1) * l);
	u = Fo(u, 0), I(o, function(e) {
		var t = s[e], i = t.maxWidth, a = t.minWidth;
		if (t.width) {
			var o = t.width;
			i && (o = Po(o, i)), a && (o = Fo(o, a)), t.width = o, n -= o + l * o, r--;
		} else {
			var o = u;
			i && i < o && (o = Po(i, n)), a && a > o && (o = a), o !== u && (t.width = o, n -= o + l * o, r--);
		}
	}), u = (n - c) / (r + (r - 1) * l), u = Fo(u, 0);
	var d = 0, f;
	I(o, function(e) {
		var t = s[e];
		t.width ||= u, f = t, d += t.width * (1 + l);
	}), f && (d -= f.width * l);
	var p = {}, m = -d / 2;
	return I(o, function(e) {
		var n = s[e];
		p[e] = p[e] || {
			bandWidth: t,
			offset: m,
			width: n.width
		}, m += n.width * (1 + l);
	}), p;
}
function uC(e) {
	return {
		seriesType: e,
		overallReset: function(t) {
			var n = XS(e, nC);
			ox(t, n, function(t) {
				var r = sC(t, e);
				ix(t, n, function(e) {
					var t = r.columnMap[oC(e)];
					e.getData().setLayout({
						bandWidth: t.bandWidth,
						offset: t.offset,
						size: t.width
					});
				});
			});
		}
	};
}
function dC(e) {
	return {
		seriesType: e,
		plan: Tv(),
		reset: function(e) {
			if (wS(e)) {
				var t = e.getData(), n = e.coordinateSystem, r = n.getBaseAxis(), i = n.getOtherAxis(r), a = t.getDimensionIndex(t.mapDimension(i.dim)), o = t.getDimensionIndex(t.mapDimension(r.dim)), s = e.get("showBackground", !0), c = t.mapDimension(i.dim), l = t.getCalculationInfo("stackResultDimension"), u = Im(t, c) && !!t.getCalculationInfo("stackedOnSeries"), d = i.isHorizontal(), f = i.toGlobalCoord(i.dataToCoord($S(i))), p = fC(e), m = e.get("barMinHeight") || 0, h = l && t.getDimensionIndex(l), g = t.getLayout("size"), _ = t.getLayout("offset");
				return { progress: function(e, t) {
					for (var r = e.count, i = p && mv(r * 3), c = p && s && mv(r * 3), l = p && mv(r), v = n.master.getRect(), y = d ? v.width : v.height, b, x = t.getStore(), S = 0; (b = e.next()) != null;) {
						var C = x.get(u ? h : a, b), w = x.get(o, b), T = f, E = void 0;
						u && (E = +C - x.get(a, b));
						var D = void 0, O = void 0, k = void 0, A = void 0;
						if (d) {
							var j = n.dataToPoint([C, w]);
							u && (T = n.dataToPoint([E, w])[0]), D = T, O = j[1] + _, k = j[0] - T, A = g, Io(k) < m && (k = (k < 0 ? -1 : 1) * m);
						} else {
							var j = n.dataToPoint([w, C]);
							u && (T = n.dataToPoint([w, E])[1]), D = j[0] + _, O = T, k = g, A = j[1] - T, Io(A) < m && (A = (A <= 0 ? -1 : 1) * m);
						}
						p ? (i[S] = D, i[S + 1] = O, i[S + 2] = d ? k : A, c && (c[S] = d ? v.x : D, c[S + 1] = d ? O : v.y, c[S + 2] = y), l[b] = b) : t.setItemLayout(b, {
							x: D,
							y: O,
							width: k,
							height: A
						}), S += 3;
					}
					p && t.setLayout({
						largePoints: i,
						largeDataIndices: l,
						largeBackgroundPoints: c,
						valueAxisHorizontal: d
					});
				} };
			}
		}
	};
}
function fC(e) {
	return e.pipelineContext && e.pipelineContext.large;
}
function pC(e) {
	return YS(XS(e, nC));
}
function mC(e) {
	iC(e, function() {
		function t(t) {
			var n = XS(t, nC);
			QS(e, n, t, nC), VS(n, pC(t));
		}
		t("bar"), t("pictorialBar");
	});
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/bar/BaseBarSeries.js
var hC = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n;
	}
	return t.prototype.getInitialData = function(e, t) {
		return Bm(null, this, { useEncodeDefaulter: !0 });
	}, t.prototype.getMarkerPosition = function(e, t, n) {
		var r = this.coordinateSystem;
		if (r && r.clampData) {
			var i = r.clampData(e), a = r.dataToPoint(i);
			if (n) I(r.getAxes(), function(e, n) {
				if (e.type === "category" && t != null) {
					var r = e.getTicksCoords(), o = e.getTickModel().get("alignWithLabel"), s = i[n], c = t[n] === "x1" || t[n] === "y1";
					if (c && !o && (s += 1), r.length < 2) return;
					if (r.length === 2) {
						a[n] = e.toGlobalCoord(e.getExtent()[+!!c]);
						return;
					}
					for (var l = void 0, u = void 0, d = 1, f = 0; f < r.length; f++) {
						var p = r[f].coord, m = f === r.length - 1 ? r[f - 1].tickValue + d : r[f].tickValue;
						if (m === s) {
							u = p;
							break;
						} else if (m < s) l = p;
						else if (l != null && m > s) {
							u = (p + l) / 2;
							break;
						}
						f === 1 && (d = m - r[0].tickValue);
					}
					u ?? (l ? l && (u = r[r.length - 1].coord) : u = r[0].coord), a[n] = e.toGlobalCoord(u);
				}
			});
			else {
				var o = this.getData(), s = o.getLayout("offset"), c = o.getLayout("size"), l = +!r.getBaseAxis().isHorizontal();
				a[l] += s + c / 2;
			}
			return a;
		}
		return [NaN, NaN];
	}, t.prototype.__requireStartValue = function(e) {
		return this.getBaseAxis() !== e;
	}, t.type = "series.__base_bar__", t.defaultOption = {
		z: 2,
		coordinateSystem: "cartesian2d",
		legendHoverLink: !0,
		barMinHeight: 0,
		barMinAngle: 0,
		large: !1,
		largeThreshold: 400,
		progressive: 3e3,
		progressiveChunkMode: "mod",
		defaultBarGap: "10%"
	}, t;
}(P_);
P_.registerClass(hC);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/bar/BarSeries.js
var gC = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n;
	}
	return t.prototype.getInitialData = function() {
		return Bm(null, this, {
			useEncodeDefaulter: !0,
			createInvertedIndices: !!this.get("realtimeSort", !0) || null
		});
	}, t.prototype.getProgressive = function() {
		return this.get("large") ? this.get("progressive") : !1;
	}, t.prototype.__preparePipelineContext = function(e, t) {
		var n = gc(this, e, t);
		return n.progressiveRender && (n.large = !0), n;
	}, t.prototype.brushSelector = function(e, t, n) {
		return n.rect(t.getItemLayout(e));
	}, t.type = "series.bar", t.dependencies = ["grid", "polar"], t.defaultOption = qm(hC.defaultOption, {
		clip: !0,
		roundCap: !1,
		showBackground: !1,
		backgroundStyle: {
			color: "rgba(180, 180, 180, 0.2)",
			borderColor: null,
			borderWidth: 0,
			borderType: "solid",
			borderRadius: 0,
			shadowBlur: 0,
			shadowColor: null,
			shadowOffsetX: 0,
			shadowOffsetY: 0,
			opacity: 1
		},
		select: { itemStyle: {
			borderColor: Q.color.primary,
			borderWidth: 2
		} },
		realtimeSort: !1
	}), t;
}(hC), _C = "\0__throttleOriginMethod", vC = "\0__throttleRate", yC = "\0__throttleType";
function bC(e, t, n) {
	var r, i = 0, a = 0, o = null, s, c, l, u;
	t ||= 0;
	function d() {
		a = (/* @__PURE__ */ new Date()).getTime(), o = null, e.apply(c, l || []);
	}
	var f = function() {
		var e = [...arguments];
		r = (/* @__PURE__ */ new Date()).getTime(), c = this, l = e;
		var f = u || t, p = u || n;
		u = null, s = r - (p ? i : a) - f, clearTimeout(o), p ? o = setTimeout(d, f) : s >= 0 ? d() : o = setTimeout(d, -s), i = r;
	};
	return f.clear = function() {
		o &&= (clearTimeout(o), null);
	}, f.debounceNextCall = function(e) {
		u = e;
	}, f;
}
function xC(e, t, n, r) {
	var i = e[t];
	if (i) {
		var a = i[_C] || i, o = i[yC];
		if (i[vC] !== n || o !== r) {
			if (n == null || !r) return e[t] = a;
			i = e[t] = bC(a, n, r === "debounce"), i[_C] = a, i[yC] = r, i[vC] = n;
		}
		return i;
	}
}
function SC(e, t) {
	var n = e[t];
	n && n[_C] && (n.clear && n.clear(), e[t] = n[_C]);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/util/shape/sausage.js
var CC = function() {
	function e() {
		this.cx = 0, this.cy = 0, this.r0 = 0, this.r = 0, this.startAngle = 0, this.endAngle = Math.PI * 2, this.clockwise = !0;
	}
	return e;
}(), wC = function(e) {
	o(t, e);
	function t(t) {
		var n = e.call(this, t) || this;
		return n.type = "sausage", n;
	}
	return t.prototype.getDefaultShape = function() {
		return new CC();
	}, t.prototype.buildPath = function(e, t) {
		var n = t.cx, r = t.cy, i = Math.max(t.r0 || 0, 0), a = Math.max(t.r, 0), o = (a - i) * .5, s = i + o, c = t.startAngle, l = t.endAngle, u = t.clockwise, d = Math.PI * 2, f = u ? l - c < d : c - l < d;
		f || (c = l - (u ? d : -d));
		var p = Math.cos(c), m = Math.sin(c), h = Math.cos(l), g = Math.sin(l);
		f ? (e.moveTo(p * i + n, m * i + r), e.arc(p * s + n, m * s + r, o, -Math.PI + c, c, !u)) : e.moveTo(p * a + n, m * a + r), e.arc(n, r, a, c, l, !u), e.arc(h * s + n, g * s + r, o, l - Math.PI * 2, l - Math.PI, !u), i !== 0 && e.arc(n, r, i, l, c, u);
	}, t;
}(Za);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/label/sectorLabel.js
function TC(e, t) {
	t ||= {};
	var n = t.isRoundCap;
	return function(t, r, i) {
		var a = r.position;
		if (!a || a instanceof Array) return fn(t, r, i);
		var o = e(a), s = r.distance == null ? 5 : r.distance, c = this.shape, l = c.cx, u = c.cy, d = c.r, f = c.r0, p = (d + f) / 2, m = c.startAngle, h = c.endAngle, g = (m + h) / 2, _ = n ? Math.abs(d - f) / 2 : 0, v = Math.cos, y = Math.sin, b = l + d * v(m), x = u + d * y(m), S = "left", C = "top";
		switch (o) {
			case "startArc":
				b = l + (f - s) * v(g), x = u + (f - s) * y(g), S = "center", C = "top";
				break;
			case "insideStartArc":
				b = l + (f + s) * v(g), x = u + (f + s) * y(g), S = "center", C = "bottom";
				break;
			case "startAngle":
				b = l + p * v(m) + DC(m, s + _, !1), x = u + p * y(m) + OC(m, s + _, !1), S = "right", C = "middle";
				break;
			case "insideStartAngle":
				b = l + p * v(m) + DC(m, -s + _, !1), x = u + p * y(m) + OC(m, -s + _, !1), S = "left", C = "middle";
				break;
			case "middle":
				b = l + p * v(g), x = u + p * y(g), S = "center", C = "middle";
				break;
			case "endArc":
				b = l + (d + s) * v(g), x = u + (d + s) * y(g), S = "center", C = "bottom";
				break;
			case "insideEndArc":
				b = l + (d - s) * v(g), x = u + (d - s) * y(g), S = "center", C = "top";
				break;
			case "endAngle":
				b = l + p * v(h) + DC(h, s + _, !0), x = u + p * y(h) + OC(h, s + _, !0), S = "left", C = "middle";
				break;
			case "insideEndAngle":
				b = l + p * v(h) + DC(h, -s + _, !0), x = u + p * y(h) + OC(h, -s + _, !0), S = "right", C = "middle";
				break;
			default: return fn(t, r, i);
		}
		return t ||= {}, t.x = b, t.y = x, t.align = S, t.verticalAlign = C, t;
	};
}
function EC(e, t, n, r) {
	if (se(r)) {
		e.setTextConfig({ rotation: r });
		return;
	} else if (V(t)) {
		e.setTextConfig({ rotation: 0 });
		return;
	}
	var i = e.shape, a = i.clockwise ? i.startAngle : i.endAngle, o = i.clockwise ? i.endAngle : i.startAngle, s = (a + o) / 2, c, l = n(t);
	switch (l) {
		case "startArc":
		case "insideStartArc":
		case "middle":
		case "insideEndArc":
		case "endArc":
			c = s;
			break;
		case "startAngle":
		case "insideStartAngle":
			c = a;
			break;
		case "endAngle":
		case "insideEndAngle":
			c = o;
			break;
		default:
			e.setTextConfig({ rotation: 0 });
			return;
	}
	var u = Math.PI * 1.5 - c;
	l === "middle" && u > Math.PI / 2 && u < Math.PI * 1.5 && (u -= Math.PI), e.setTextConfig({ rotation: u });
}
function DC(e, t, n) {
	return t * Math.sin(e) * (n ? -1 : 1);
}
function OC(e, t, n) {
	return t * Math.cos(e) * (n ? 1 : -1);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/helper/sectorHelper.js
function kC(e, t, n) {
	var r = e.get("borderRadius");
	if (r == null) return n ? { cornerRadius: 0 } : null;
	V(r) || (r = [
		r,
		r,
		r,
		r
	]);
	var i = Math.abs(t.r || 0 - t.r0 || 0);
	return { cornerRadius: L(r, function(e) {
		return dn(e, i);
	}) };
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/bar/BarView.js
var AC = Math.max, jC = Math.min, MC = function(e) {
	o(t, e);
	function t() {
		var t = e.call(this) || this;
		return t.type = "bar", t._isFirstFrame = !0, t;
	}
	return t.prototype.render = function(e, t, n, r) {
		this._model = e, this._removeOnRenderedListener(n), this._updateDrawMode(e);
		var i = e.get("coordinateSystem");
		(i === "cartesian2d" || i === "polar") && (this._progressiveEls = null, this._isLargeDraw ? this._renderLarge(e, t, n) : this._renderNormal(e, t, n, r));
	}, t.prototype.incrementalPrepareRender = function(e) {
		this._clear(), this._updateDrawMode(e), this._updateLargeClip(e);
	}, t.prototype.incrementalRender = function(e, t) {
		this._progressiveEls = [], this._incrementalRenderLarge(e, t);
	}, t.prototype.eachRendered = function(e) {
		Qd(this._progressiveEls || this.group, e);
	}, t.prototype._updateDrawMode = function(e) {
		var t = e.pipelineContext.large;
		(this._isLargeDraw == null || t !== this._isLargeDraw) && (this._isLargeDraw = t, this._clear());
	}, t.prototype._renderNormal = function(e, t, n, r) {
		var i = this.group, a = e.getData(), o = this._data, s = e.coordinateSystem, c = s.getBaseAxis(), l;
		s.type === "cartesian2d" ? l = c.isHorizontal() : s.type === "polar" && (l = c.dim === "angle");
		var u = e.isAnimationEnabled() ? e : null, d = FC(e, s);
		d && this._enableRealtimeSort(d, a, n);
		var f = e.get("clip", !0) || d, p = s.getArea();
		i.removeClipPath();
		var m = e.get("roundCap", !0), h = e.get("showBackground", !0), g = e.getModel("backgroundStyle"), _ = g.get("borderRadius") || 0, v = [], y = this._backgroundEls, b = r && r.isInitSort, x = r && r.type === "changeAxisOrder";
		function S(e) {
			var t = VC[s.type](a, e);
			if (!t) return null;
			var n = QC(s, l, t);
			return n.useStyle(g.getItemStyle()), s.type === "cartesian2d" ? n.setShape("r", _) : n.setShape("cornerRadius", _), v[e] = n, n;
		}
		a.diff(o).add(function(t) {
			var n = a.getItemModel(t), r = VC[s.type](a, t, n);
			if (r && (h && S(t), !(!a.hasValue(t) || !BC[s.type](r)))) {
				var o = !1;
				f && (o = NC[s.type](p, r));
				var g = PC[s.type](e, a, t, r, l, u, c.model, !1, m);
				d && (g.forceLabelAnimation = !0), WC(g, a, t, n, r, e, l, s.type === "polar"), b ? g.attr({ shape: r }) : d ? IC(d, u, g, r, t, l, !1, !1) : dd(g, { shape: r }, e, t), a.setItemGraphicEl(t, g), i.add(g), g.ignore = o;
			}
		}).update(function(t, n) {
			var r = a.getItemModel(t), C = VC[s.type](a, t, r);
			if (C) {
				if (h) {
					var w = void 0;
					y.length === 0 ? w = S(n) : (w = y[n], w.useStyle(g.getItemStyle()), s.type === "cartesian2d" ? w.setShape("r", _) : w.setShape("cornerRadius", _), v[t] = w);
					var T = VC[s.type](a, t), E = ZC(l, T, s);
					ud(w, { shape: E }, u, t);
				}
				var D = o.getItemGraphicEl(n);
				if (!a.hasValue(t) || !BC[s.type](C)) {
					i.remove(D);
					return;
				}
				var O = !1;
				if (f && (O = NC[s.type](p, C), O && i.remove(D)), D && (D.type === "sector" && m || D.type === "sausage" && !m) && (D && hd(D, e, n), D = null), D ? gd(D) : D = PC[s.type](e, a, t, C, l, u, c.model, !0, m), d && (D.forceLabelAnimation = !0), x) {
					var k = D.getTextContent();
					if (k) {
						var A = Ef(k);
						A.prevValue != null && (A.prevValue = A.value);
					}
				} else WC(D, a, t, r, C, e, l, s.type === "polar");
				b ? D.attr({ shape: C }) : d ? IC(d, u, D, C, t, l, !0, x) : ud(D, { shape: C }, e, t, null), a.setItemGraphicEl(t, D), D.ignore = O, i.add(D);
			}
		}).remove(function(t) {
			var n = o.getItemGraphicEl(t);
			n && hd(n, e, t);
		}).execute();
		var C = this._backgroundGroup ||= new su();
		C.removeAll();
		for (var w = 0; w < v.length; ++w) C.add(v[w]);
		i.add(C), this._backgroundEls = v, this._data = a;
	}, t.prototype._renderLarge = function(e, t, n) {
		this._clear(), JC(e, this.group), this._updateLargeClip(e);
	}, t.prototype._incrementalRenderLarge = function(e, t) {
		this._removeBackground(), JC(t, this.group, this._progressiveEls, !0);
	}, t.prototype._updateLargeClip = function(e) {
		var t = e.get("clip", !0) && Iv(e.coordinateSystem, !1, e), n = this.group;
		t ? n.setClipPath(t) : n.removeClipPath();
	}, t.prototype._enableRealtimeSort = function(e, t, n) {
		var r = this;
		if (t.count()) {
			var i = e.baseAxis;
			if (this._isFirstFrame) this._dispatchInitSort(t, e, n), this._isFirstFrame = !1;
			else {
				var a = function(e) {
					var n = t.getItemGraphicEl(e), r = n && n.shape;
					return r && Math.abs(i.isHorizontal() ? r.height : r.width) || 0;
				};
				this._onRendered = function() {
					r._updateSortWithinSameData(t, a, i, n);
				}, n.getZr().on("rendered", this._onRendered);
			}
		}
	}, t.prototype._dataSort = function(e, t, n) {
		var r = [];
		return e.each(e.mapDimension(t.dim), function(e, t) {
			var i = n(t);
			i ??= NaN, r.push({
				dataIndex: t,
				mappedValue: i,
				ordinalNumber: e
			});
		}), r.sort(function(e, t) {
			return t.mappedValue - e.mappedValue;
		}), { ordinalNumbers: L(r, function(e) {
			return e.ordinalNumber;
		}) };
	}, t.prototype._isOrderChangedWithinSameData = function(e, t, n) {
		for (var r = n.scale, i = e.mapDimension(n.dim), a = Number.MAX_VALUE, o = 0, s = r.getOrdinalMeta().categories.length; o < s; ++o) {
			var c = e.rawIndexOf(i, r.getRawOrdinalNumber(o)), l = c < 0 ? Number.MIN_VALUE : t(e.indexOfRawIndex(c));
			if (l > a) return !0;
			a = l;
		}
		return !1;
	}, t.prototype._isOrderDifferentInView = function(e, t) {
		for (var n = t.scale, r = n.getExtent(), i = Math.max(0, r[0]), a = Math.min(r[1], n.getOrdinalMeta().categories.length - 1); i <= a; ++i) if (e.ordinalNumbers[i] !== n.getRawOrdinalNumber(i)) return !0;
	}, t.prototype._updateSortWithinSameData = function(e, t, n, r) {
		if (this._isOrderChangedWithinSameData(e, t, n)) {
			var i = this._dataSort(e, n, t);
			this._isOrderDifferentInView(i, n) && (this._removeOnRenderedListener(r), r.dispatchAction({
				type: "changeAxisOrder",
				componentType: n.dim + "Axis",
				axisId: n.index,
				sortInfo: i
			}));
		}
	}, t.prototype._dispatchInitSort = function(e, t, n) {
		var r = t.baseAxis, i = this._dataSort(e, r, function(n) {
			return e.get(e.mapDimension(t.otherAxis.dim), n);
		});
		n.dispatchAction({
			type: "changeAxisOrder",
			componentType: r.dim + "Axis",
			isInitSort: !0,
			axisId: r.index,
			sortInfo: i
		});
	}, t.prototype.remove = function(e, t) {
		this._clear(this._model), this._removeOnRenderedListener(t);
	}, t.prototype.dispose = function(e, t) {
		this._removeOnRenderedListener(t);
	}, t.prototype._removeOnRenderedListener = function(e) {
		this._onRendered &&= (e.getZr().off("rendered", this._onRendered), null);
	}, t.prototype._clear = function(e) {
		var t = this.group, n = this._data;
		e && e.isAnimationEnabled() && n && !this._isLargeDraw ? (this._removeBackground(), this._backgroundEls = [], n.eachItemGraphicEl(function(t) {
			hd(t, e, yc(t).dataIndex);
		})) : t.removeAll(), this._data = null, this._isFirstFrame = !0;
	}, t.prototype._removeBackground = function() {
		this.group.remove(this._backgroundGroup), this._backgroundGroup = null;
	}, t.type = "bar", t;
}(Ov), NC = {
	cartesian2d: function(e, t) {
		var n = t.width < 0 ? -1 : 1, r = t.height < 0 ? -1 : 1;
		n < 0 && (t.x += t.width, t.width = -t.width), r < 0 && (t.y += t.height, t.height = -t.height);
		var i = e.x + e.width, a = e.y + e.height, o = AC(t.x, e.x), s = jC(t.x + t.width, i), c = AC(t.y, e.y), l = jC(t.y + t.height, a), u = s < o, d = l < c;
		return t.x = u && o > i ? s : o, t.y = d && c > a ? l : c, t.width = u ? 0 : s - o, t.height = d ? 0 : l - c, n < 0 && (t.x += t.width, t.width = -t.width), r < 0 && (t.y += t.height, t.height = -t.height), u || d;
	},
	polar: function(e, t) {
		var n = t.r0 <= t.r ? 1 : -1;
		if (n < 0) {
			var r = t.r;
			t.r = t.r0, t.r0 = r;
		}
		var i = jC(t.r, e.r), a = AC(t.r0, e.r0);
		t.r = i, t.r0 = a;
		var o = i - a < 0;
		if (n < 0) {
			var r = t.r;
			t.r = t.r0, t.r0 = r;
		}
		return o;
	}
}, PC = {
	cartesian2d: function(e, t, n, r, i, a, o, s, c) {
		var l = new fo({
			shape: j({}, r),
			z2: 1
		});
		if (l.__dataIndex = n, l.name = "item", a) {
			var u = l.shape, d = i ? "height" : "width";
			u[d] = 0;
		}
		return l;
	},
	polar: function(e, t, n, r, i, a, o, s, c) {
		var l = !i && c ? wC : Ou, u = new l({
			shape: r,
			z2: 1
		});
		if (u.name = "item", u.calculateTextPosition = TC(UC(i), { isRoundCap: l === wC }), a) {
			var d = u.shape, f = i ? "r" : "endAngle", p = {};
			d[f] = i ? r.r0 : r.startAngle, p[f] = r[f], (s ? ud : dd)(u, { shape: p }, a);
		}
		return u;
	}
};
function FC(e, t) {
	var n = e.get("realtimeSort", !0), r = t.getBaseAxis();
	if (n && r.type === "category" && t.type === "cartesian2d") return {
		baseAxis: r,
		otherAxis: t.getOtherAxis(r)
	};
}
function IC(e, t, n, r, i, a, o, s) {
	var c, l;
	a ? (l = {
		x: r.x,
		width: r.width
	}, c = {
		y: r.y,
		height: r.height
	}) : (l = {
		y: r.y,
		height: r.height
	}, c = {
		x: r.x,
		width: r.width
	}), s || (o ? ud : dd)(n, { shape: c }, t, i, null);
	var u = t ? e.baseAxis.model : null;
	(o ? ud : dd)(n, { shape: l }, u, i);
}
function LC(e, t) {
	for (var n = 0; n < t.length; n++) if (!isFinite(e[t[n]])) return !0;
	return !1;
}
var RC = [
	"x",
	"y",
	"width",
	"height"
], zC = [
	"cx",
	"cy",
	"r",
	"startAngle",
	"endAngle"
], BC = {
	cartesian2d: function(e) {
		return !LC(e, RC);
	},
	polar: function(e) {
		return !LC(e, zC);
	}
}, VC = {
	cartesian2d: function(e, t, n) {
		var r = e.getItemLayout(t);
		if (!r) return null;
		var i = n ? GC(n, r) : 0, a = r.width > 0 ? 1 : -1, o = r.height > 0 ? 1 : -1;
		return {
			x: r.x + a * i / 2,
			y: r.y + o * i / 2,
			width: r.width - a * i,
			height: r.height - o * i
		};
	},
	polar: function(e, t, n) {
		var r = e.getItemLayout(t);
		return {
			cx: r.cx,
			cy: r.cy,
			r0: r.r0,
			r: r.r,
			startAngle: r.startAngle,
			endAngle: r.endAngle,
			clockwise: r.clockwise
		};
	}
};
function HC(e) {
	return e.startAngle != null && e.endAngle != null && e.startAngle === e.endAngle;
}
function UC(e) {
	return function(e) {
		var t = e ? "Arc" : "Angle";
		return function(e) {
			switch (e) {
				case "start":
				case "insideStart":
				case "end":
				case "insideEnd": return e + t;
				default: return e;
			}
		};
	}(e);
}
function WC(e, t, n, r, i, a, o, s) {
	var c = t.getItemVisual(n, "style");
	if (!s) {
		var l = r.get(["itemStyle", "borderRadius"]) || 0;
		e.setShape("r", l);
	} else if (!a.get("roundCap")) {
		var u = e.shape;
		j(u, kC(r.getModel("itemStyle"), u, !0)), e.setShape(u);
	}
	e.useStyle(c);
	var d = r.getShallow("cursor");
	d && e.attr("cursor", d);
	var f = s ? o ? i.r >= i.r0 ? "endArc" : "startArc" : i.endAngle >= i.startAngle ? "endAngle" : "startAngle" : o ? $C(i, a.coordinateSystem) : ew(i, a.coordinateSystem), p = gf(r);
	hf(e, p, {
		labelFetcher: a,
		labelDataIndex: n,
		defaultText: $_(a.getData(), n),
		inheritColor: c.fill,
		defaultOpacity: c.opacity,
		defaultOutsidePosition: f
	});
	var m = e.getTextContent();
	if (s && m) {
		var h = r.get(["label", "position"]);
		e.textConfig.inside = h === "middle" ? !0 : null, EC(e, h === "outside" ? f : h, UC(o), r.get(["label", "rotate"]));
	}
	Df(m, p, a.getRawValue(n), function(e) {
		return ev(t, e);
	});
	var g = r.getModel(["emphasis"]);
	Ol(e, g.get("focus"), g.get("blurScope"), g.get("disabled")), Ml(e, r), HC(i) && (e.style.fill = "none", e.style.stroke = "none", I(e.states, function(e) {
		e.style && (e.style.fill = e.style.stroke = "none");
	}));
}
function GC(e, t) {
	var n = e.get(["itemStyle", "borderColor"]);
	if (!n || n === "none") return 0;
	var r = e.get(["itemStyle", "borderWidth"]) || 0, i = isNaN(t.width) ? Number.MAX_VALUE : Math.abs(t.width), a = isNaN(t.height) ? Number.MAX_VALUE : Math.abs(t.height);
	return Math.min(r, i, a);
}
var KC = function() {
	function e() {}
	return e;
}(), qC = function(e) {
	o(t, e);
	function t(t) {
		var n = e.call(this, t) || this;
		return n.type = "largeBar", n;
	}
	return t.prototype.getDefaultShape = function() {
		return new KC();
	}, t.prototype.buildPath = function(e, t) {
		for (var n = t.points, r = this.baseDimIdx, i = 1 - this.baseDimIdx, a = [], o = [], s = this.barWidth, c = 0; c < n.length; c += 3) o[r] = s, o[i] = n[c + 2], a[r] = n[c + r], a[i] = n[c + i], e.rect(a[0], a[1], o[0], o[1]);
	}, t;
}(Za);
function JC(e, t, n, r) {
	var i = e.getData(), a = +!!i.getLayout("valueAxisHorizontal"), o = i.getLayout("largeDataIndices"), s = i.getLayout("size"), c = e.getModel("backgroundStyle"), l = i.getLayout("largeBackgroundPoints"), u = r ? hc(e) : 0;
	if (l) {
		var d = new qC({
			shape: { points: l },
			incremental: u,
			silent: !0,
			z2: 0
		});
		d.baseDimIdx = a, d.largeDataIndices = o, d.barWidth = s, d.useStyle(c.getItemStyle()), t.add(d), n && n.push(d);
	}
	var f = new qC({
		shape: { points: i.getLayout("largePoints") },
		incremental: u,
		ignoreCoarsePointer: !0,
		z2: 1
	});
	f.baseDimIdx = a, f.largeDataIndices = o, f.barWidth = s, t.add(f), f.useStyle(i.getVisual("style")), f.style.stroke = null, yc(f).seriesIndex = e.seriesIndex, e.get("silent") || (f.on("mousedown", YC), f.on("mousemove", YC)), n && n.push(f);
}
var YC = bC(function(e) {
	var t = this, n = XC(t, e.offsetX, e.offsetY);
	yc(t).dataIndex = n >= 0 ? n : null;
}, 30, !1);
function XC(e, t, n) {
	for (var r = e.baseDimIdx, i = 1 - r, a = e.shape.points, o = e.largeDataIndices, s = [], c = [], l = e.barWidth, u = 0, d = a.length / 3; u < d; u++) {
		var f = u * 3;
		if (c[r] = l, c[i] = a[f + 2], s[r] = a[f + r], s[i] = a[f + i], c[i] < 0 && (s[i] += c[i], c[i] = -c[i]), t >= s[0] && t <= s[0] + c[0] && n >= s[1] && n <= s[1] + c[1]) return o[u];
	}
	return -1;
}
function ZC(e, t, n) {
	if (Lv(n, "cartesian2d")) {
		var r = t, i = n.getArea();
		return {
			x: e ? r.x : i.x,
			y: e ? i.y : r.y,
			width: e ? r.width : i.width,
			height: e ? i.height : r.height
		};
	} else {
		var i = n.getArea(), a = t;
		return {
			cx: i.cx,
			cy: i.cy,
			r0: e ? i.r0 : a.r0,
			r: e ? i.r : a.r,
			startAngle: e ? a.startAngle : 0,
			endAngle: e ? a.endAngle : Math.PI * 2
		};
	}
}
function QC(e, t, n) {
	return new (e.type === "polar" ? Ou : fo)({
		shape: ZC(t, n, e),
		silent: !0,
		z2: 0
	});
}
function $C(e, t) {
	return e.height === 0 ? t.getOtherAxis(t.getBaseAxis()).inverse ? "bottom" : "top" : e.height > 0 ? "bottom" : "top";
}
function ew(e, t) {
	return e.width === 0 ? t.getOtherAxis(t.getBaseAxis()).inverse ? "left" : "right" : e.width >= 0 ? "right" : "left";
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/bar/install.js
function tw(e) {
	e.registerChartView(MC), e.registerSeriesModel(gC), e.registerLayout(e.PRIORITY.VISUAL.LAYOUT, uC("bar")), e.registerLayout(e.PRIORITY.VISUAL.PROGRESSIVE_LAYOUT, dC("bar")), e.registerProcessor(e.PRIORITY.PROCESSOR.STATISTIC, bb("bar")), e.registerAction({
		type: "changeAxisOrder",
		event: "changeAxisOrder",
		update: "update"
	}, function(e, t) {
		var n = e.componentType || "series";
		t.eachComponent({
			mainType: n,
			query: e
		}, function(t) {
			e.sortInfo && t.axis.setCategorySortInfo(e.sortInfo);
		});
	}), mC(e);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/legacy/dataSelectAction.js
function nw(e, t) {
	function n(t, n) {
		var r = [];
		return t.eachComponent({
			mainType: "series",
			subType: e,
			query: n
		}, function(e) {
			r.push(e.seriesIndex);
		}), r;
	}
	I([
		[e + "ToggleSelect", "toggleSelect"],
		[e + "Select", "select"],
		[e + "UnSelect", "unselect"]
	], function(e) {
		t(e[0], function(t, r, i) {
			t = j({}, t), i.dispatchAction(j(t, {
				type: e[1],
				seriesIndex: n(r, t)
			}));
		});
	});
}
function rw(e, t, n, r, i) {
	var a = e + t;
	n.isSilent(a) || r.eachComponent({
		mainType: "series",
		subType: "pie"
	}, function(e) {
		for (var t = e.seriesIndex, r = e.option.selectedMap, o = i.selected, s = 0; s < o.length; s++) if (o[s].seriesIndex === t) {
			var c = e.getData(), l = Us(c, i.fromActionPayload);
			n.trigger(a, {
				type: a,
				seriesId: e.id,
				name: V(l) ? c.getName(l[0]) : c.getName(l),
				selected: U(r) ? r : j({}, r)
			});
		}
	});
}
function iw(e, t, n) {
	e.on("selectchanged", function(e) {
		var r = n.getModel();
		e.isFromClick ? (rw("map", "selectchanged", t, r, e), rw("pie", "selectchanged", t, r, e)) : e.fromAction === "select" ? (rw("map", "selected", t, r, e), rw("pie", "selected", t, r, e)) : e.fromAction === "unselect" && (rw("map", "unselected", t, r, e), rw("pie", "unselected", t, r, e));
	});
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/processor/dataFilter.js
function aw(e) {
	return {
		seriesType: e,
		reset: function(e, t) {
			var n = t.findComponents({ mainType: "legend" });
			if (!(!n || !n.length)) {
				var r = e.getData();
				r.filterSelf(function(e) {
					for (var t = r.getName(e), i = 0; i < n.length; i++) if (!n[i].isSelected(t)) return !1;
					return !0;
				});
			}
		}
	};
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/helper/createSeriesDataSimply.js
function ow(e, t, n) {
	t = V(t) && { coordDimensions: t } || j({ encodeDefine: e.getEncode() }, t);
	var r = e.getSource(), i = vm(r, t).dimensions, a = new _m(i, e);
	return a.initData(r, n), a;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/visual/LegendVisualProvider.js
var sw = function() {
	function e(e, t) {
		this._getDataWithEncodedVisual = e, this._getRawData = t;
	}
	return e.prototype.getAllNames = function() {
		var e = this._getRawData();
		return e.mapArray(e.getName);
	}, e.prototype.containName = function(e) {
		return this._getRawData().indexOfName(e) >= 0;
	}, e.prototype.indexOfName = function(e) {
		return this._getDataWithEncodedVisual().indexOfName(e);
	}, e.prototype.getItemVisual = function(e, t) {
		return this._getDataWithEncodedVisual().getItemVisual(e, t);
	}, e;
}(), cw = Ws(), lw = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n;
	}
	return t.prototype.init = function(t) {
		e.prototype.init.apply(this, arguments), this.legendVisualProvider = new sw(z(this.getData, this), z(this.getRawData, this)), this._defaultLabelLine(t);
	}, t.prototype.mergeOption = function() {
		e.prototype.mergeOption.apply(this, arguments);
	}, t.prototype.getInitialData = function() {
		return ow(this, {
			coordDimensions: ["value"],
			encodeDefaulter: B(Jf, this)
		});
	}, t.prototype.getDataParams = function(t) {
		var n = this.getData(), r = cw(n), i = r.seats;
		if (!i) {
			var a = [];
			n.each(n.mapDimension("value"), function(e) {
				a.push(e);
			}), i = r.seats = $o(a, n.hostModel.get("percentPrecision"));
		}
		var o = e.prototype.getDataParams.call(this, t);
		return o.percent = i[t] || 0, o.$vars.push("percent"), o;
	}, t.prototype._defaultLabelLine = function(e) {
		Ts(e, "labelLine", ["show"]);
		var t = e.labelLine, n = e.emphasis.labelLine;
		t.show = t.show && e.label.show, n.show = n.show && e.emphasis.label.show;
	}, t.type = "series.pie", t.defaultOption = {
		z: 2,
		legendHoverLink: !0,
		colorBy: "data",
		center: ["50%", "50%"],
		radius: [0, "50%"],
		clockwise: !0,
		startAngle: 90,
		endAngle: "auto",
		padAngle: 0,
		minAngle: 0,
		minShowLabelAngle: 0,
		selectedOffset: 10,
		percentPrecision: 2,
		stillShowZeroSum: !0,
		coordinateSystemUsage: "box",
		left: 0,
		top: 0,
		right: 0,
		bottom: 0,
		width: null,
		height: null,
		label: {
			rotate: 0,
			show: !0,
			overflow: "truncate",
			position: "outer",
			alignTo: "none",
			edgeDistance: "25%",
			distanceToLabelLine: 5
		},
		labelLine: {
			show: !0,
			length: 15,
			length2: 30,
			smooth: !1,
			minTurnAngle: 90,
			maxSurfaceAngle: 90,
			lineStyle: {
				width: 1,
				type: "solid"
			}
		},
		itemStyle: {
			borderWidth: 1,
			borderJoin: "round"
		},
		showEmptyCircle: !0,
		emptyCircleStyle: {
			color: "lightgray",
			opacity: 1
		},
		labelLayout: { hideOverlap: !0 },
		emphasis: {
			scale: !0,
			scaleSize: 5
		},
		avoidLabelOverlap: !0,
		animationType: "expansion",
		animationDuration: 1e3,
		animationTypeUpdate: "transition",
		animationEasingUpdate: "cubicInOut",
		animationDurationUpdate: 500,
		animationEasing: "cubicInOut"
	}, t;
}(P_);
Tm({
	fullType: lw.type,
	getCoord2: function(e) {
		return e.getShallow("center");
	}
}), Math.PI * 2, Ea.CMD;
function uw(e, t, n, r, i, a, o, s) {
	var c = i - e, l = a - t, u = n - e, d = r - t, f = Math.sqrt(u * u + d * d);
	u /= f, d /= f;
	var p = (c * u + l * d) / f;
	s && (p = Math.min(Math.max(p, 0), 1)), p *= f;
	var m = o[0] = e + p * u, h = o[1] = t + p * d;
	return Math.sqrt((m - i) * (m - i) + (h - a) * (h - a));
}
var dw = new J(), fw = new J(), pw = new J(), mw = new J(), hw = new J(), gw = [], _w = new J();
function vw(e, t) {
	if (t <= 180 && t > 0) {
		t = t / 180 * Math.PI, dw.fromArray(e[0]), fw.fromArray(e[1]), pw.fromArray(e[2]), J.sub(mw, dw, fw), J.sub(hw, pw, fw);
		var n = mw.len(), r = hw.len();
		if (!(n < .001 || r < .001)) {
			mw.scale(1 / n), hw.scale(1 / r);
			var i = mw.dot(hw);
			if (Math.cos(t) < i) {
				var a = uw(fw.x, fw.y, pw.x, pw.y, dw.x, dw.y, gw, !1);
				_w.fromArray(gw), _w.scaleAndAdd(hw, a / Math.tan(Math.PI - t));
				var o = pw.x === fw.x ? (_w.y - fw.y) / (pw.y - fw.y) : (_w.x - fw.x) / (pw.x - fw.x);
				if (isNaN(o)) return;
				o < 0 ? J.copy(_w, fw) : o > 1 && J.copy(_w, pw), _w.toArray(e[1]);
			}
		}
	}
}
function yw(e, t, n) {
	if (n <= 180 && n > 0) {
		n = n / 180 * Math.PI, dw.fromArray(e[0]), fw.fromArray(e[1]), pw.fromArray(e[2]), J.sub(mw, fw, dw), J.sub(hw, pw, fw);
		var r = mw.len(), i = hw.len();
		if (!(r < .001 || i < .001) && (mw.scale(1 / r), hw.scale(1 / i), mw.dot(t) < Math.cos(n))) {
			var a = uw(fw.x, fw.y, pw.x, pw.y, dw.x, dw.y, gw, !1);
			_w.fromArray(gw);
			var o = Math.PI / 2, s = o + Math.acos(hw.dot(t)) - n;
			if (s >= o) J.copy(_w, pw);
			else {
				_w.scaleAndAdd(hw, a / Math.tan(Math.PI / 2 - s));
				var c = pw.x === fw.x ? (_w.y - fw.y) / (pw.y - fw.y) : (_w.x - fw.x) / (pw.x - fw.x);
				if (isNaN(c)) return;
				c < 0 ? J.copy(_w, fw) : c > 1 && J.copy(_w, pw);
			}
			_w.toArray(e[1]);
		}
	}
}
function bw(e, t, n, r) {
	var i = n === "normal", a = i ? e : e.ensureState(n);
	a.ignore = t;
	var o = r.get("smooth");
	o = o === !0 ? .3 : Math.max(+o, 0) || 0, a.shape = a.shape || {}, a.shape.smooth = o;
	var s = r.getModel("lineStyle").getLineStyle();
	i ? e.useStyle(s) : a.style = s;
}
function xw(e, t) {
	var n = t.smooth, r = t.points;
	if (r) if (e.moveTo(r[0][0], r[0][1]), n > 0 && r.length >= 3) {
		var i = wt(r[0], r[1]), a = wt(r[1], r[2]);
		if (!i || !a) {
			e.lineTo(r[1][0], r[1][1]), e.lineTo(r[2][0], r[2][1]);
			return;
		}
		var o = Math.min(i, a) * n, s = Dt([], r[1], r[0], o / i), c = Dt([], r[1], r[2], o / a), l = Dt([], s, c, .5);
		e.bezierCurveTo(s[0], s[1], s[0], s[1], l[0], l[1]), e.bezierCurveTo(c[0], c[1], c[0], c[1], r[2][0], r[2][1]);
	} else for (var u = 1; u < r.length; u++) e.lineTo(r[u][0], r[u][1]);
}
function Sw(e, t, n) {
	var r = e.getTextGuideLine(), i = e.getTextContent();
	if (!i) {
		r && e.removeTextGuideLine();
		return;
	}
	for (var a = t.normal, o = a.get("show"), s = i.ignore, c = 0; c < Lc.length; c++) {
		var l = Lc[c], u = t[l], d = l === "normal";
		if (u) {
			var f = u.get("show");
			if ((d ? s : G(i.states[l] && i.states[l].ignore, s)) || !G(f, o)) {
				var p = d ? r : r && r.states[l];
				p && (p.ignore = !0), r && bw(r, !0, l, u);
				continue;
			}
			r || (r = new Iu(), e.setTextGuideLine(r), !d && (s || !o) && bw(r, !0, "normal", t.normal), e.stateProxy && (r.stateProxy = e.stateProxy)), bw(r, !1, l, u);
		}
	}
	if (r) {
		M(r.style, n), r.style.fill = null;
		var m = a.get("showAbove"), h = e.textGuideLineConfig = e.textGuideLineConfig || {};
		h.showAbove = m || !1, r.buildPath = xw;
	}
}
function Cw(e, t) {
	t ||= "labelLine";
	for (var n = { normal: e.getModel(t) }, r = 0; r < Ic.length; r++) {
		var i = Ic[r];
		n[i] = e.getModel([i, t]);
	}
	return n;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/pie/labelLayout.js
var ww = Math.PI / 180;
function Tw(e, t, n, r, i, a, o, s, c, l) {
	if (e.length < 2) return;
	function u(e) {
		for (var a = e.rB, o = a * a, s = 0; s < e.list.length; s++) {
			var c = e.list[s], l = Math.abs(c.label.y - n), u = r + c.len, d = u * u, f = t + (Math.sqrt(Math.abs((1 - l * l / o) * d)) + c.len2) * i, p = f - c.label.x;
			Dw(c, c.targetTextWidth - p * i, !0), c.label.x = f;
		}
	}
	function d(e) {
		for (var a = {
			list: [],
			maxY: 0
		}, o = {
			list: [],
			maxY: 0
		}, s = 0; s < e.length; s++) if (e[s].labelAlignTo === "none") {
			var c = e[s], l = c.label.y > n ? o : a, d = Math.abs(c.label.y - n);
			if (d >= l.maxY) {
				var f = c.label.x - t - c.len2 * i, p = r + c.len;
				l.rB = Math.abs(f) < p ? Math.sqrt(d * d / (1 - f * f / p / p)) : p, l.maxY = d;
			}
			l.list.push(c);
		}
		u(a), u(o);
	}
	for (var f = e.length, p = 0; p < f; p++) if (e[p].position === "outer" && e[p].labelAlignTo === "labelLine") {
		var m = e[p].label.x - l;
		e[p].linePoints[1][0] += m, e[p].label.x = l;
	}
	Rx(e, 1, c, c + o) && d(e);
}
function Ew(e, t, n, r, i, a, o, s) {
	for (var c = [], l = [], u = Number.MAX_VALUE, d = -Number.MAX_VALUE, f = 0; f < e.length; f++) {
		var p = e[f].label;
		jw(e[f]) || (p.x < t ? (u = Math.min(u, p.x), c.push(e[f])) : (d = Math.max(d, p.x), l.push(e[f])));
	}
	for (var f = 0; f < e.length; f++) {
		var m = e[f];
		if (!jw(m) && m.linePoints) {
			if (m.labelStyleWidth != null) continue;
			var p = m.label, h = m.linePoints, g = void 0;
			g = m.labelAlignTo === "edge" ? p.x < t ? h[2][0] - m.labelDistance - o - m.edgeDistance : o + i - m.edgeDistance - h[2][0] - m.labelDistance : m.labelAlignTo === "labelLine" ? p.x < t ? u - o - m.bleedMargin : o + i - d - m.bleedMargin : p.x < t ? p.x - o - m.bleedMargin : o + i - p.x - m.bleedMargin, m.targetTextWidth = g, Dw(m, g, !1);
		}
	}
	Tw(l, t, n, r, 1, i, a, o, s, d), Tw(c, t, n, r, -1, i, a, o, s, u);
	for (var f = 0; f < e.length; f++) {
		var m = e[f];
		if (!jw(m) && m.linePoints) {
			var p = m.label, h = m.linePoints, _ = m.labelAlignTo === "edge", v = p.style.padding, y = v ? v[1] + v[3] : 0, b = p.style.backgroundColor ? 0 : y, x = m.rect.width + b, S = h[1][0] - h[2][0];
			_ ? p.x < t ? h[2][0] = o + m.edgeDistance + x + m.labelDistance : h[2][0] = o + i - m.edgeDistance - x - m.labelDistance : (p.x < t ? h[2][0] = p.x + m.labelDistance : h[2][0] = p.x - m.labelDistance, h[1][0] = h[2][0] + S), h[1][1] = h[2][1] = p.y;
		}
	}
}
function Dw(e, t, n) {
	if (e.labelStyleWidth == null) {
		var r = e.label, i = r.style, a = e.rect, o = i.backgroundColor, s = i.padding, c = s ? s[1] + s[3] : 0, l = i.overflow, u = a.width + (o ? 0 : c);
		if (t < u || n) {
			if (l && l.match("break")) {
				r.setStyle("backgroundColor", null), r.setStyle("width", t - c);
				var d = r.getBoundingRect();
				r.setStyle("width", Math.ceil(d.width)), r.setStyle("backgroundColor", o);
			} else {
				var f = t - c, p = t < u ? f : n ? f > e.unconstrainedWidth ? null : f : null;
				r.setStyle("width", p);
			}
			Ow(a, r);
		}
	}
}
function Ow(e, t) {
	Aw.rect = e, Mx(Aw, t, kw);
}
var kw = {
	minMarginForce: [
		null,
		0,
		null,
		0
	],
	marginDefault: [
		1,
		0,
		1,
		0
	]
}, Aw = {};
function jw(e) {
	return e.position === "center";
}
function Mw(e) {
	var t = e.getData(), n = [], r, i, a = !1, o = (e.get("minShowLabelAngle") || 0) * ww, s = t.getLayout("viewRect"), c = t.getLayout("r"), l = s.width, u = s.x, d = s.y, f = s.height;
	function p(e) {
		e.ignore = !0;
	}
	function m(e) {
		if (!e.ignore) return !0;
		for (var t in e.states) if (e.states[t].ignore === !1) return !0;
		return !1;
	}
	t.each(function(e) {
		var s = t.getItemGraphicEl(e), d = s.shape, h = s.getTextContent(), g = s.getTextGuideLine(), _ = t.getItemModel(e), v = _.getModel("label"), y = v.get("position") || _.get([
			"emphasis",
			"label",
			"position"
		]), b = v.get("distanceToLabelLine"), x = v.get("alignTo"), S = X(v.get("edgeDistance"), l), C = v.get("bleedMargin");
		C ??= Math.min(l, f) > 200 ? 10 : 2;
		var w = _.getModel("labelLine"), T = w.get("length");
		T = X(T, l);
		var E = w.get("length2");
		if (E = X(E, l), Math.abs(d.endAngle - d.startAngle) < o) {
			I(h.states, p), h.ignore = !0, g && (I(g.states, p), g.ignore = !0);
			return;
		}
		if (m(h)) {
			var D = (d.startAngle + d.endAngle) / 2, O = Math.cos(D), k = Math.sin(D), A, j, ee, M;
			r = d.cx, i = d.cy;
			var N = y === "inside" || y === "inner";
			if (y === "center") A = d.cx, j = d.cy, M = "center";
			else {
				var te = (N ? (d.r + d.r0) / 2 * O : d.r * O) + r, P = (N ? (d.r + d.r0) / 2 * k : d.r * k) + i;
				if (A = te + O * 3, j = P + k * 3, !N) {
					var F = te + O * (T + c - d.r), L = P + k * (T + c - d.r), ne = F + (O < 0 ? -1 : 1) * E, re = L;
					A = x === "edge" ? O < 0 ? u + S : u + l - S : ne + (O < 0 ? -b : b), j = re, ee = [
						[te, P],
						[F, L],
						[ne, re]
					];
				}
				M = N ? "center" : x === "edge" ? O > 0 ? "right" : "left" : O > 0 ? "left" : "right";
			}
			var ie = Math.PI, R = 0, ae = v.get("rotate");
			if (se(ae)) R = ie / 180 * ae;
			else if (y === "center") R = 0;
			else if (ae === "radial" || ae === !0) R = O < 0 ? -D + ie : -D;
			else if (ae === "tangential" || ae === "tangential-noflip" && y !== "outside" && y !== "outer") {
				var z = Math.atan2(O, k);
				z < 0 && (z = ie * 2 + z), k > 0 && ae !== "tangential-noflip" && (z = ie + z), R = z - ie;
			}
			if (a = !!R, h.x = A, h.y = j, h.rotation = R, h.setStyle({ verticalAlign: "middle" }), N) {
				h.setStyle({ align: M });
				var B = h.states.select;
				B && (B.x += h.x, B.y += h.y);
			} else {
				var V = new Y(0, 0, 0, 0);
				Ow(V, h), n.push({
					label: h,
					labelLine: g,
					position: y,
					len: T,
					len2: E,
					minTurnAngle: w.get("minTurnAngle"),
					maxSurfaceAngle: w.get("maxSurfaceAngle"),
					surfaceNormal: new J(O, k),
					linePoints: ee,
					textAlign: M,
					labelDistance: b,
					labelAlignTo: x,
					edgeDistance: S,
					bleedMargin: C,
					rect: V,
					unconstrainedWidth: V.width,
					labelStyleWidth: h.style.width
				});
			}
			s.setTextConfig({ inside: N });
		}
	}), !a && e.get("avoidLabelOverlap") && Ew(n, r, i, c, l, f, u, d);
	for (var h = 0; h < n.length; h++) {
		var g = n[h], _ = g.label, v = g.labelLine, y = isNaN(_.x) || isNaN(_.y);
		if (_) {
			_.setStyle({ align: g.textAlign }), y && (I(_.states, p), _.ignore = !0);
			var b = _.states.select;
			b && (b.x += _.x, b.y += _.y);
		}
		if (v) {
			var x = g.linePoints;
			y || !x ? (I(v.states, p), v.ignore = !0) : (vw(x, g.minTurnAngle), yw(x, g.surfaceNormal, g.maxSurfaceAngle), v.setShape({ points: x }), _.__hostTarget.textGuideLineConfig = { anchor: new J(x[0][0], x[0][1]) });
		}
	}
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/pie/pieLayout.js
var Nw = Math.PI * 2, Pw = Math.PI / 180, Fw = _c("pie", Iw);
function Iw(e, t) {
	e.eachSeriesByType("pie", function(e) {
		var n = e.getData(), r = n.mapDimension("value"), i = wg(e, t), a = i.cx, o = i.cy, s = i.r, c = i.r0, l = i.viewRect, u = -e.get("startAngle") * Pw, d = e.get("endAngle"), f = e.get("padAngle") * Pw;
		d = d === "auto" ? u - Nw : -d * Pw;
		var p = e.get("minAngle") * Pw + f, m = 0;
		n.each(r, function(e) {
			!isNaN(e) && m++;
		});
		var h = n.getSum(r), g = Math.PI / (h || m) * 2, _ = e.get("clockwise"), v = e.get("roseType"), y = e.get("stillShowZeroSum"), b = n.getDataExtent(r);
		b[0] = 0;
		var x = _ ? 1 : -1, S = [u, d], C = x * f / 2;
		Ta(S, !_), u = S[0], d = S[1];
		var w = Lw(e);
		w.startAngle = u, w.endAngle = d, w.clockwise = _, w.cx = a, w.cy = o, w.r = s, w.r0 = c;
		var T = Math.abs(d - u), E = T, D = 0, O = u;
		if (n.setLayout({
			viewRect: l,
			r: s
		}), n.each(r, function(e, t) {
			var r;
			if (isNaN(e)) {
				n.setItemLayout(t, {
					angle: NaN,
					startAngle: NaN,
					endAngle: NaN,
					clockwise: _,
					cx: a,
					cy: o,
					r0: c,
					r: v ? NaN : s
				});
				return;
			}
			r = v === "area" ? T / m : h === 0 && y ? g : e * g, r < p ? (r = p, E -= p) : D += e;
			var i = O + x * r, l = 0, u = 0;
			f > r ? (l = O + x * r / 2, u = l) : (l = O + C, u = i - C), n.setItemLayout(t, {
				angle: r,
				startAngle: l,
				endAngle: u,
				clockwise: _,
				cx: a,
				cy: o,
				r0: c,
				r: v ? Go(e, b, [c, s]) : s
			}), O = i;
		}), E < Nw && m) if (E <= .001) {
			var k = T / m;
			n.each(r, function(e, t) {
				if (!isNaN(e)) {
					var r = n.getItemLayout(t);
					r.angle = k;
					var i = 0, a = 0;
					k < f ? (i = u + x * (t + 1 / 2) * k, a = i) : (i = u + x * t * k + C, a = u + x * (t + 1) * k - C), r.startAngle = i, r.endAngle = a;
				}
			});
		} else g = E / D, O = u, n.each(r, function(e, t) {
			if (!isNaN(e)) {
				var r = n.getItemLayout(t), i = r.angle === p ? p : e * g, a = 0, o = 0;
				i < f ? (a = O + x * i / 2, o = a) : (a = O + C, o = O + x * i - C), r.startAngle = a, r.endAngle = o, O += x * i;
			}
		});
	});
}
var Lw = Ws(), Rw = function(e) {
	o(t, e);
	function t(t, n, r) {
		var i = e.call(this) || this;
		i.z2 = 2;
		var a = new _o();
		return i.setTextContent(a), i.updateData(t, n, r, !0), i;
	}
	return t.prototype.updateData = function(e, t, n, r) {
		var i = this, a = e.hostModel, o = e.getItemModel(t), s = o.getModel("emphasis"), c = e.getItemLayout(t), l = j(kC(o.getModel("itemStyle"), c, !0), c);
		if (isNaN(l.startAngle)) {
			i.setShape(l);
			return;
		}
		if (r) {
			i.setShape(l);
			var u = a.getShallow("animationType");
			a.ecModel.ssr ? (dd(i, {
				scaleX: 0,
				scaleY: 0
			}, a, {
				dataIndex: t,
				isFrom: !0
			}), i.originX = l.cx, i.originY = l.cy) : u === "scale" ? (i.shape.r = c.r0, dd(i, { shape: { r: c.r } }, a, t)) : n == null ? (i.shape.endAngle = c.startAngle, ud(i, { shape: { endAngle: c.endAngle } }, a, t)) : (i.setShape({
				startAngle: n,
				endAngle: n
			}), dd(i, { shape: {
				startAngle: c.startAngle,
				endAngle: c.endAngle
			} }, a, t));
		} else gd(i), ud(i, { shape: l }, a, t);
		i.useStyle(e.getItemVisual(t, "style")), Ml(i, o);
		var d = (c.startAngle + c.endAngle) / 2, f = a.get("selectedOffset"), p = Math.cos(d) * f, m = Math.sin(d) * f, h = o.getShallow("cursor");
		h && i.attr("cursor", h), this._updateLabel(a, e, t), i.ensureState("emphasis").shape = j({ r: c.r + (s.get("scale") && s.get("scaleSize") || 0) }, kC(s.getModel("itemStyle"), c)), j(i.ensureState("select"), {
			x: p,
			y: m,
			shape: kC(o.getModel(["select", "itemStyle"]), c)
		}), j(i.ensureState("blur"), { shape: kC(o.getModel(["blur", "itemStyle"]), c) });
		var g = i.getTextGuideLine(), _ = i.getTextContent();
		g && j(g.ensureState("select"), {
			x: p,
			y: m
		}), j(_.ensureState("select"), {
			x: p,
			y: m
		}), Ol(this, s.get("focus"), s.get("blurScope"), s.get("disabled"));
	}, t.prototype._updateLabel = function(e, t, n) {
		var r = this, i = t.getItemModel(n), a = i.getModel("labelLine"), o = t.getItemVisual(n, "style"), s = o && o.fill, c = o && o.opacity;
		hf(r, gf(i), {
			labelFetcher: t.hostModel,
			labelDataIndex: n,
			inheritColor: s,
			defaultOpacity: c,
			defaultText: e.getFormattedLabel(n, "normal") || t.getName(n)
		});
		var l = r.getTextContent();
		r.setTextConfig({
			position: null,
			rotation: null
		}), l.attr({ z2: 10 });
		var u = i.get(["label", "position"]);
		if (u !== "outside" && u !== "outer") r.removeTextGuideLine();
		else {
			var d = this.getTextGuideLine();
			d || (d = new Iu(), this.setTextGuideLine(d)), Sw(this, Cw(i), {
				stroke: s,
				opacity: he(a.get(["lineStyle", "opacity"]), c, 1)
			});
		}
	}, t;
}(Ou), zw = function(e) {
	o(t, e);
	function t() {
		var t = e !== null && e.apply(this, arguments) || this;
		return t.type = "pie", t.ignoreLabelLineUpdate = !0, t;
	}
	return t.prototype.render = function(e, t, n, r) {
		var i = e.getData(), a = this._data, o = this.group, s;
		if (!a && i.count() > 0) {
			for (var c = i.getItemLayout(0), l = 1; isNaN(c && c.startAngle) && l < i.count(); ++l) c = i.getItemLayout(l);
			c && (s = c.startAngle);
		}
		if (this._emptyCircleSector && o.remove(this._emptyCircleSector), i.count() === 0 && e.get("showEmptyCircle")) {
			var u = new Ou({ shape: k(Lw(e)) });
			u.useStyle(e.getModel("emptyCircleStyle").getItemStyle()), this._emptyCircleSector = u, o.add(u);
		}
		i.diff(a).add(function(e) {
			var t = new Rw(i, e, s);
			i.setItemGraphicEl(e, t), o.add(t);
		}).update(function(e, t) {
			var n = a.getItemGraphicEl(t);
			n.updateData(i, e, s), n.off("click"), o.add(n), i.setItemGraphicEl(e, n);
		}).remove(function(t) {
			hd(a.getItemGraphicEl(t), e, t);
		}).execute(), Mw(e), e.get("animationTypeUpdate") !== "expansion" && (this._data = i);
	}, t.prototype.dispose = function() {}, t.prototype.containPoint = function(e, t) {
		var n = t.getData().getItemLayout(0);
		if (n) {
			var r = e[0] - n.cx, i = e[1] - n.cy, a = Math.sqrt(r * r + i * i);
			return a <= n.r && a >= n.r0;
		}
	}, t.type = "pie", t;
}(Ov);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/processor/negativeDataFilter.js
function Bw(e) {
	return {
		seriesType: e,
		reset: function(e, t) {
			var n = e.getData();
			n.filterSelf(function(e) {
				var t = n.mapDimension("value"), r = n.get(t, e);
				return !(se(r) && !isNaN(r) && r < 0);
			});
		}
	};
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/pie/install.js
function Vw(e) {
	e.registerChartView(zw), e.registerSeriesModel(lw), nw("pie", e.registerAction), e.registerLayout(Fw), e.registerProcessor(aw("pie")), e.registerProcessor(Bw("pie"));
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/mixin/Draggable.js
var Hw = function() {
	function e(e, t) {
		this.target = e, this.topTarget = t && t.topTarget;
	}
	return e;
}(), Uw = function() {
	function e(e) {
		this.handler = e, e.on("mousedown", this._dragStart, this), e.on("mousemove", this._drag, this), e.on("mouseup", this._dragEnd, this);
	}
	return e.prototype._dragStart = function(e) {
		for (var t = e.target; t && !t.draggable;) t = t.parent || t.__hostTarget;
		t && (this._draggingTarget = t, t.dragging = !0, this._x = e.offsetX, this._y = e.offsetY, this.handler.dispatchToElement(new Hw(t, e), "dragstart", e.event));
	}, e.prototype._drag = function(e) {
		var t = this._draggingTarget;
		if (t) {
			var n = e.offsetX, r = e.offsetY, i = n - this._x, a = r - this._y;
			this._x = n, this._y = r, t.drift(i, a, e), this.handler.dispatchToElement(new Hw(t, e), "drag", e.event);
			var o = this.handler.findHover(n, r, t).target, s = this._dropTarget;
			this._dropTarget = o, t !== o && (s && o !== s && this.handler.dispatchToElement(new Hw(s, e), "dragleave", e.event), o && o !== s && this.handler.dispatchToElement(new Hw(o, e), "dragenter", e.event));
		}
	}, e.prototype._dragEnd = function(e) {
		var t = this._draggingTarget;
		t && (t.dragging = !1), this.handler.dispatchToElement(new Hw(t, e), "dragend", e.event), this._dropTarget && this.handler.dispatchToElement(new Hw(this._dropTarget, e), "drop", e.event), this._draggingTarget = null, this._dropTarget = null;
	}, e;
}(), Ww = /^(?:mouse|pointer|contextmenu|drag|drop)|click/, Gw = [], Kw = q.browser.firefox && +q.browser.version.split(".")[0] < 39;
function qw(e, t, n, r) {
	return n ||= {}, r ? Jw(e, t, n) : Kw && t.layerX != null && t.layerX !== t.offsetX ? (n.zrX = t.layerX, n.zrY = t.layerY) : t.offsetX == null ? Jw(e, t, n) : (n.zrX = t.offsetX, n.zrY = t.offsetY), n;
}
function Jw(e, t, n) {
	if (q.domSupported && e.getBoundingClientRect) {
		var r = t.clientX, i = t.clientY;
		if (ih(e)) {
			var a = e.getBoundingClientRect();
			n.zrX = r - a.left, n.zrY = i - a.top;
			return;
		} else if (th(Gw, e, r, i)) {
			n.zrX = Gw[0], n.zrY = Gw[1];
			return;
		}
	}
	n.zrX = n.zrY = 0;
}
function Yw(e) {
	return e || window.event;
}
function Xw(e, t, n) {
	if (t = Yw(t), t.zrX != null) return t;
	var r = t.type;
	if (r && r.indexOf("touch") >= 0) {
		var i = r === "touchend" ? t.changedTouches[0] : t.targetTouches[0];
		i && qw(e, i, t, n);
	} else {
		qw(e, t, t, n);
		var a = Zw(t);
		t.zrDelta = a ? a / 120 : -(t.detail || 0) / 3;
	}
	var o = t.button;
	return t.which == null && o !== void 0 && Ww.test(t.type) && (t.which = o & 1 ? 1 : o & 2 ? 3 : o & 4 ? 2 : 0), t;
}
function Zw(e) {
	var t = e.wheelDelta;
	if (t) return t;
	var n = e.deltaX, r = e.deltaY;
	if (n == null || r == null) return t;
	var i = Math.abs(r === 0 ? n : r), a = r > 0 ? -1 : r < 0 ? 1 : n > 0 ? -1 : 1;
	return 3 * i * a;
}
function Qw(e, t, n, r) {
	e.addEventListener(t, n, r);
}
function $w(e, t, n, r) {
	e.removeEventListener(t, n, r);
}
var eT = function(e) {
	e.preventDefault(), e.stopPropagation(), e.cancelBubble = !0;
}, tT = function() {
	function e() {
		this._track = [];
	}
	return e.prototype.recognize = function(e, t, n) {
		return this._doTrack(e, t, n), this._recognize(e);
	}, e.prototype.clear = function() {
		return this._track.length = 0, this;
	}, e.prototype._doTrack = function(e, t, n) {
		var r = e.touches;
		if (r) {
			for (var i = {
				points: [],
				touches: [],
				target: t,
				event: e
			}, a = 0, o = r.length; a < o; a++) {
				var s = r[a], c = qw(n, s, {});
				i.points.push([c.zrX, c.zrY]), i.touches.push(s);
			}
			this._track.push(i);
		}
	}, e.prototype._recognize = function(e) {
		for (var t in iT) if (iT.hasOwnProperty(t)) {
			var n = iT[t](this._track, e);
			if (n) return n;
		}
	}, e;
}();
function nT(e) {
	var t = e[1][0] - e[0][0], n = e[1][1] - e[0][1];
	return Math.sqrt(t * t + n * n);
}
function rT(e) {
	return [(e[0][0] + e[1][0]) / 2, (e[0][1] + e[1][1]) / 2];
}
var iT = { pinch: function(e, t) {
	var n = e.length;
	if (n) {
		var r = (e[n - 1] || {}).points, i = (e[n - 2] || {}).points || r;
		if (i && i.length > 1 && r && r.length > 1) {
			var a = nT(r) / nT(i);
			!isFinite(a) && (a = 1), t.pinchScale = a;
			var o = rT(r);
			return t.pinchX = o[0], t.pinchY = o[1], {
				type: "pinch",
				target: e[0].target,
				event: t
			};
		}
	}
} }, aT = "silent";
function oT(e, t, n) {
	return {
		type: e,
		event: n,
		target: t.target,
		topTarget: t.topTarget,
		cancelBubble: !1,
		offsetX: n.zrX,
		offsetY: n.zrY,
		gestureEvent: n.gestureEvent,
		pinchX: n.pinchX,
		pinchY: n.pinchY,
		pinchScale: n.pinchScale,
		wheelDelta: n.zrDelta,
		zrByTouch: n.zrByTouch,
		which: n.which,
		stop: sT
	};
}
function sT() {
	eT(this.event);
}
var cT = function(e) {
	o(t, e);
	function t() {
		var t = e !== null && e.apply(this, arguments) || this;
		return t.handler = null, t;
	}
	return t.prototype.dispose = function() {}, t.prototype.setCursor = function() {}, t;
}(hi), lT = function() {
	function e(e, t) {
		this.x = e, this.y = t;
	}
	return e;
}(), uT = [
	"click",
	"dblclick",
	"mousewheel",
	"mouseout",
	"mouseup",
	"mousedown",
	"mousemove",
	"contextmenu"
], dT = new Y(0, 0, 0, 0), fT = function(e) {
	o(t, e);
	function t(t, n, r, i, a) {
		var o = e.call(this) || this;
		return o._hovered = new lT(0, 0), o.storage = t, o.painter = n, o.painterRoot = i, o._pointerSize = a, r ||= new cT(), o.proxy = null, o.setHandlerProxy(r), o._draggingMgr = new Uw(o), o;
	}
	return t.prototype.setHandlerProxy = function(e) {
		this.proxy && this.proxy.dispose(), e && (I(uT, function(t) {
			e.on && e.on(t, this[t], this);
		}, this), e.handler = this), this.proxy = e;
	}, t.prototype.mousemove = function(e) {
		var t = e.zrX, n = e.zrY, r = hT(this, t, n), i = this._hovered, a = i.target;
		a && !a.__zr && (i = this.findHover(i.x, i.y), a = i.target);
		var o = this._hovered = r ? new lT(t, n) : this.findHover(t, n), s = o.target, c = this.proxy;
		c.setCursor && c.setCursor(s ? s.cursor : "default"), a && s !== a && this.dispatchToElement(i, "mouseout", e), this.dispatchToElement(o, "mousemove", e), s && s !== a && this.dispatchToElement(o, "mouseover", e);
	}, t.prototype.mouseout = function(e) {
		var t = e.zrEventControl;
		t !== "only_globalout" && this.dispatchToElement(this._hovered, "mouseout", e), t !== "no_globalout" && this.trigger("globalout", {
			type: "globalout",
			event: e
		});
	}, t.prototype.resize = function() {
		this._hovered = new lT(0, 0);
	}, t.prototype.dispatch = function(e, t) {
		var n = this[e];
		n && n.call(this, t);
	}, t.prototype.dispose = function() {
		this.proxy.dispose(), this.storage = null, this.proxy = null, this.painter = null;
	}, t.prototype.setCursorStyle = function(e) {
		var t = this.proxy;
		t.setCursor && t.setCursor(e);
	}, t.prototype.dispatchToElement = function(e, t, n) {
		e ||= {};
		var r = e.target;
		if (!(r && r.silent)) {
			for (var i = "on" + t, a = oT(t, e, n); r && (r[i] && (a.cancelBubble = !!r[i].call(r, a)), r.trigger(t, a), r = r.__hostTarget ? r.__hostTarget : r.parent, !a.cancelBubble););
			a.cancelBubble || (this.trigger(t, a), this.painter && this.painter.eachOtherLayer && this.painter.eachOtherLayer(function(e) {
				typeof e[i] == "function" && e[i].call(e, a), e.trigger && e.trigger(t, a);
			}));
		}
	}, t.prototype.findHover = function(e, t, n) {
		var r = this.storage.getDisplayList(), i = new lT(e, t);
		if (mT(r, i, e, t, n), this._pointerSize && !i.target) {
			for (var a = [], o = this._pointerSize, s = o / 2, c = new Y(e - s, t - s, o, o), l = r.length - 1; l >= 0; l--) {
				var u = r[l];
				u !== n && !u.ignore && !u.ignoreCoarsePointer && (!u.parent || !u.parent.ignoreCoarsePointer) && (dT.copy(u.getBoundingRect()), u.transform && dT.applyTransform(u.transform), dT.intersect(c) && a.push(u));
			}
			if (a.length) {
				for (var d = 4, f = Math.PI / 12, p = Math.PI * 2, m = 0; m < s; m += d) for (var h = 0; h < p; h += f) if (mT(a, i, e + m * Math.cos(h), t + m * Math.sin(h), n), i.target) return i;
			}
		}
		return i;
	}, t.prototype.processGesture = function(e, t) {
		this._gestureMgr ||= new tT();
		var n = this._gestureMgr;
		t === "start" && n.clear();
		var r = n.recognize(e, this.findHover(e.zrX, e.zrY, null).target, this.proxy.dom);
		if (t === "end" && n.clear(), r) {
			var i = r.type;
			e.gestureEvent = i;
			var a = new lT();
			a.target = r.target, this.dispatchToElement(a, i, r.event);
		}
	}, t;
}(hi);
I([
	"click",
	"mousedown",
	"mouseup",
	"mousewheel",
	"dblclick",
	"contextmenu"
], function(e) {
	fT.prototype[e] = function(t) {
		var n = t.zrX, r = t.zrY, i = hT(this, n, r), a, o;
		if ((e !== "mouseup" || !i) && (a = this.findHover(n, r), o = a.target), e === "mousedown") this._downEl = o, this._downPoint = [t.zrX, t.zrY], this._upEl = o;
		else if (e === "mouseup") this._upEl = o;
		else if (e === "click") {
			if (this._downEl !== this._upEl || !this._downPoint || wt(this._downPoint, [t.zrX, t.zrY]) > 4) return;
			this._downPoint = null;
		}
		this.dispatchToElement(a, e, t);
	};
});
function pT(e, t, n) {
	if (e[e.rectHover ? "rectContain" : "contain"](t, n)) {
		for (var r = e, i = void 0, a = !1; r;) {
			if (r.ignoreClip && (a = !0), !a) {
				var o = r.getClipPath();
				if (o && !o.contain(t, n)) return !1;
			}
			r.silent && (i = !0);
			var s = r.__hostTarget;
			r = s ? r.ignoreHostSilent ? null : s : r.parent;
		}
		return i ? aT : !0;
	}
	return !1;
}
function mT(e, t, n, r, i) {
	for (var a = e.length - 1; a >= 0; a--) {
		var o = e[a], s = void 0;
		if (o !== i && !o.ignore && (s = pT(o, n, r)) && (!t.topTarget && (t.topTarget = o), s !== aT)) {
			t.target = o;
			break;
		}
	}
}
function hT(e, t, n) {
	var r = e.painter;
	return t < 0 || t > r.getWidth() || n < 0 || n > r.getHeight();
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/core/timsort.js
var gT = 32, _T = 7;
function vT(e) {
	for (var t = 0; e >= gT;) t |= e & 1, e >>= 1;
	return e + t;
}
function yT(e, t, n, r) {
	var i = t + 1;
	if (i === n) return 1;
	if (r(e[i++], e[t]) < 0) {
		for (; i < n && r(e[i], e[i - 1]) < 0;) i++;
		bT(e, t, i);
	} else for (; i < n && r(e[i], e[i - 1]) >= 0;) i++;
	return i - t;
}
function bT(e, t, n) {
	for (n--; t < n;) {
		var r = e[t];
		e[t++] = e[n], e[n--] = r;
	}
}
function xT(e, t, n, r, i) {
	for (r === t && r++; r < n; r++) {
		for (var a = e[r], o = t, s = r, c; o < s;) c = o + s >>> 1, i(a, e[c]) < 0 ? s = c : o = c + 1;
		var l = r - o;
		switch (l) {
			case 3: e[o + 3] = e[o + 2];
			case 2: e[o + 2] = e[o + 1];
			case 1:
				e[o + 1] = e[o];
				break;
			default: for (; l > 0;) e[o + l] = e[o + l - 1], l--;
		}
		e[o] = a;
	}
}
function ST(e, t, n, r, i, a) {
	var o = 0, s = 0, c = 1;
	if (a(e, t[n + i]) > 0) {
		for (s = r - i; c < s && a(e, t[n + i + c]) > 0;) o = c, c = (c << 1) + 1, c <= 0 && (c = s);
		c > s && (c = s), o += i, c += i;
	} else {
		for (s = i + 1; c < s && a(e, t[n + i - c]) <= 0;) o = c, c = (c << 1) + 1, c <= 0 && (c = s);
		c > s && (c = s);
		var l = o;
		o = i - c, c = i - l;
	}
	for (o++; o < c;) {
		var u = o + (c - o >>> 1);
		a(e, t[n + u]) > 0 ? o = u + 1 : c = u;
	}
	return c;
}
function CT(e, t, n, r, i, a) {
	var o = 0, s = 0, c = 1;
	if (a(e, t[n + i]) < 0) {
		for (s = i + 1; c < s && a(e, t[n + i - c]) < 0;) o = c, c = (c << 1) + 1, c <= 0 && (c = s);
		c > s && (c = s);
		var l = o;
		o = i - c, c = i - l;
	} else {
		for (s = r - i; c < s && a(e, t[n + i + c]) >= 0;) o = c, c = (c << 1) + 1, c <= 0 && (c = s);
		c > s && (c = s), o += i, c += i;
	}
	for (o++; o < c;) {
		var u = o + (c - o >>> 1);
		a(e, t[n + u]) < 0 ? c = u : o = u + 1;
	}
	return c;
}
function wT(e, t) {
	var n = _T, r, i, a = 0, o = [];
	r = [], i = [];
	function s(e, t) {
		r[a] = e, i[a] = t, a += 1;
	}
	function c() {
		for (; a > 1;) {
			var e = a - 2;
			if (e >= 1 && i[e - 1] <= i[e] + i[e + 1] || e >= 2 && i[e - 2] <= i[e] + i[e - 1]) i[e - 1] < i[e + 1] && e--;
			else if (i[e] > i[e + 1]) break;
			u(e);
		}
	}
	function l() {
		for (; a > 1;) {
			var e = a - 2;
			e > 0 && i[e - 1] < i[e + 1] && e--, u(e);
		}
	}
	function u(n) {
		var o = r[n], s = i[n], c = r[n + 1], l = i[n + 1];
		i[n] = s + l, n === a - 3 && (r[n + 1] = r[n + 2], i[n + 1] = i[n + 2]), a--;
		var u = CT(e[c], e, o, s, 0, t);
		o += u, s -= u, s !== 0 && (l = ST(e[o + s - 1], e, c, l, l - 1, t), l !== 0 && (s <= l ? d(o, s, c, l) : f(o, s, c, l)));
	}
	function d(r, i, a, s) {
		var c = 0;
		for (c = 0; c < i; c++) o[c] = e[r + c];
		var l = 0, u = a, d = r;
		if (e[d++] = e[u++], --s === 0) {
			for (c = 0; c < i; c++) e[d + c] = o[l + c];
			return;
		}
		if (i === 1) {
			for (c = 0; c < s; c++) e[d + c] = e[u + c];
			e[d + s] = o[l];
			return;
		}
		for (var f = n, p, m, h;;) {
			p = 0, m = 0, h = !1;
			do
				if (t(e[u], o[l]) < 0) {
					if (e[d++] = e[u++], m++, p = 0, --s === 0) {
						h = !0;
						break;
					}
				} else if (e[d++] = o[l++], p++, m = 0, --i === 1) {
					h = !0;
					break;
				}
			while ((p | m) < f);
			if (h) break;
			do {
				if (p = CT(e[u], o, l, i, 0, t), p !== 0) {
					for (c = 0; c < p; c++) e[d + c] = o[l + c];
					if (d += p, l += p, i -= p, i <= 1) {
						h = !0;
						break;
					}
				}
				if (e[d++] = e[u++], --s === 0) {
					h = !0;
					break;
				}
				if (m = ST(o[l], e, u, s, 0, t), m !== 0) {
					for (c = 0; c < m; c++) e[d + c] = e[u + c];
					if (d += m, u += m, s -= m, s === 0) {
						h = !0;
						break;
					}
				}
				if (e[d++] = o[l++], --i === 1) {
					h = !0;
					break;
				}
				f--;
			} while (p >= _T || m >= _T);
			if (h) break;
			f < 0 && (f = 0), f += 2;
		}
		if (n = f, n < 1 && (n = 1), i === 1) {
			for (c = 0; c < s; c++) e[d + c] = e[u + c];
			e[d + s] = o[l];
		} else if (i === 0) throw Error();
		else for (c = 0; c < i; c++) e[d + c] = o[l + c];
	}
	function f(r, i, a, s) {
		var c = 0;
		for (c = 0; c < s; c++) o[c] = e[a + c];
		var l = r + i - 1, u = s - 1, d = a + s - 1, f = 0, p = 0;
		if (e[d--] = e[l--], --i === 0) {
			for (f = d - (s - 1), c = 0; c < s; c++) e[f + c] = o[c];
			return;
		}
		if (s === 1) {
			for (d -= i, l -= i, p = d + 1, f = l + 1, c = i - 1; c >= 0; c--) e[p + c] = e[f + c];
			e[d] = o[u];
			return;
		}
		for (var m = n;;) {
			var h = 0, g = 0, _ = !1;
			do
				if (t(o[u], e[l]) < 0) {
					if (e[d--] = e[l--], h++, g = 0, --i === 0) {
						_ = !0;
						break;
					}
				} else if (e[d--] = o[u--], g++, h = 0, --s === 1) {
					_ = !0;
					break;
				}
			while ((h | g) < m);
			if (_) break;
			do {
				if (h = i - CT(o[u], e, r, i, i - 1, t), h !== 0) {
					for (d -= h, l -= h, i -= h, p = d + 1, f = l + 1, c = h - 1; c >= 0; c--) e[p + c] = e[f + c];
					if (i === 0) {
						_ = !0;
						break;
					}
				}
				if (e[d--] = o[u--], --s === 1) {
					_ = !0;
					break;
				}
				if (g = s - ST(e[l], o, 0, s, s - 1, t), g !== 0) {
					for (d -= g, u -= g, s -= g, p = d + 1, f = u + 1, c = 0; c < g; c++) e[p + c] = o[f + c];
					if (s <= 1) {
						_ = !0;
						break;
					}
				}
				if (e[d--] = e[l--], --i === 0) {
					_ = !0;
					break;
				}
				m--;
			} while (h >= _T || g >= _T);
			if (_) break;
			m < 0 && (m = 0), m += 2;
		}
		if (n = m, n < 1 && (n = 1), s === 1) {
			for (d -= i, l -= i, p = d + 1, f = l + 1, c = i - 1; c >= 0; c--) e[p + c] = e[f + c];
			e[d] = o[u];
		} else if (s === 0) throw Error();
		else for (f = d - (s - 1), c = 0; c < s; c++) e[f + c] = o[c];
	}
	return {
		mergeRuns: c,
		forceMergeRuns: l,
		pushRun: s
	};
}
function TT(e, t, n, r) {
	n ||= 0, r ||= e.length;
	var i = r - n;
	if (!(i < 2)) {
		var a = 0;
		if (i < gT) {
			a = yT(e, n, r, t), xT(e, n, r, n + a, t);
			return;
		}
		var o = wT(e, t), s = vT(i);
		do {
			if (a = yT(e, n, r, t), a < s) {
				var c = i;
				c > s && (c = s), xT(e, n, n + c, n + a, t), a = c;
			}
			o.pushRun(n, a), o.mergeRuns(), i -= a, n += a;
		} while (i !== 0);
		o.forceMergeRuns();
	}
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/Storage.js
var ET = !1;
function DT() {
	ET || (ET = !0, console.warn("z / z2 / zlevel of displayable is invalid, which may cause unexpected errors"));
}
function OT(e, t) {
	return e.zlevel === t.zlevel ? e.z === t.z ? e.z2 - t.z2 : e.z - t.z : e.zlevel - t.zlevel;
}
var kT = function() {
	function e() {
		this._roots = [], this._displayList = [], this._displayListLen = 0, this.displayableSortFunc = OT;
	}
	return e.prototype.traverse = function(e, t) {
		for (var n = 0; n < this._roots.length; n++) this._roots[n].traverse(e, t);
	}, e.prototype.getDisplayList = function(e, t) {
		t ||= !1;
		var n = this._displayList;
		return (e || !n.length) && this.updateDisplayList(t), n;
	}, e.prototype.updateDisplayList = function(e) {
		this._displayListLen = 0;
		for (var t = this._roots, n = this._displayList, r = 0, i = t.length; r < i; r++) this._updateAndAddDisplayable(t[r], null, e);
		n.length = this._displayListLen, TT(n, OT);
	}, e.prototype._updateAndAddDisplayable = function(e, t, n) {
		if (!(e.ignore && !n)) {
			e.beforeUpdate(), e.update(), e.afterUpdate();
			var r = e.getClipPath(), i = t && t.length, a = 0, o = e.__clipPaths;
			if (!e.ignoreClip && (i || r)) {
				if (o ||= e.__clipPaths = [], i) for (var s = 0; s < t.length; s++) o[a++] = t[s];
				for (var c = r, l = e; c;) c.parent = l, c.updateTransform(), o[a++] = c, l = c, c = c.getClipPath();
			}
			if (o && (o.length = a), e.childrenRef) {
				for (var u = e.childrenRef(), d = 0; d < u.length; d++) {
					var f = u[d];
					e.__dirty && (f.__dirty |= 1), this._updateAndAddDisplayable(f, o, n);
				}
				e.__dirty = 0;
			} else {
				var p = e;
				isNaN(p.z) && (DT(), p.z = 0), isNaN(p.z2) && (DT(), p.z2 = 0), isNaN(p.zlevel) && (DT(), p.zlevel = 0), this._displayList[this._displayListLen++] = p;
			}
			var m = e.getDecalElement && e.getDecalElement();
			m && this._updateAndAddDisplayable(m, o, n);
			var h = e.getTextGuideLine();
			h && this._updateAndAddDisplayable(h, o, n);
			var g = e.getTextContent();
			g && this._updateAndAddDisplayable(g, o, n);
		}
	}, e.prototype.addRoot = function(e) {
		e.__zr && e.__zr.storage === this || this._roots.push(e);
	}, e.prototype.delRoot = function(e) {
		if (e instanceof Array) {
			for (var t = 0, n = e.length; t < n; t++) this.delRoot(e[t]);
			return;
		}
		var r = N(this._roots, e);
		r >= 0 && this._roots.splice(r, 1);
	}, e.prototype.delAllRoots = function() {
		this._roots = [], this._displayList = [], this._displayListLen = 0;
	}, e.prototype.getRoots = function() {
		return this._roots;
	}, e.prototype.dispose = function() {
		this._displayList = null, this._roots = null;
	}, e;
}(), AT = q.hasGlobalWindow && (window.requestAnimationFrame && window.requestAnimationFrame.bind(window) || window.msRequestAnimationFrame && window.msRequestAnimationFrame.bind(window) || window.mozRequestAnimationFrame || window.webkitRequestAnimationFrame) || function(e) {
	return setTimeout(e, 16);
};
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/animation/Animation.js
function jT() {
	return (/* @__PURE__ */ new Date()).getTime();
}
var MT = function(e) {
	o(t, e);
	function t(t) {
		var n = e.call(this) || this;
		return n._running = !1, n._time = 0, n._pausedTime = 0, n._pauseStart = 0, n._paused = !1, t ||= {}, n.stage = t.stage || {}, n;
	}
	return t.prototype.addClip = function(e) {
		e.animation && this.removeClip(e), this._head ? (this._tail.next = e, e.prev = this._tail, e.next = null, this._tail = e) : this._head = this._tail = e, e.animation = this;
	}, t.prototype.addAnimator = function(e) {
		e.animation = this;
		var t = e.getClip();
		t && this.addClip(t);
	}, t.prototype.removeClip = function(e) {
		if (e.animation) {
			var t = e.prev, n = e.next;
			t ? t.next = n : this._head = n, n ? n.prev = t : this._tail = t, e.next = e.prev = e.animation = null;
		}
	}, t.prototype.removeAnimator = function(e) {
		var t = e.getClip();
		t && this.removeClip(t), e.animation = null;
	}, t.prototype.update = function(e) {
		for (var t = jT() - this._pausedTime, n = t - this._time, r = this._head; r;) {
			var i = r.next;
			r.step(t, n) ? (r.ondestroy(), this.removeClip(r), r = i) : r = i;
		}
		this._time = t, e || (this.trigger("frame", n), this.stage.update && this.stage.update());
	}, t.prototype._startLoop = function() {
		var e = this;
		this._running = !0;
		function t() {
			e._running && (AT(t), !e._paused && e.update());
		}
		AT(t);
	}, t.prototype.start = function() {
		this._running || (this._time = jT(), this._pausedTime = 0, this._startLoop());
	}, t.prototype.stop = function() {
		this._running = !1;
	}, t.prototype.pause = function() {
		this._paused ||= (this._pauseStart = jT(), !0);
	}, t.prototype.resume = function() {
		this._paused &&= (this._pausedTime += jT() - this._pauseStart, !1);
	}, t.prototype.clear = function() {
		for (var e = this._head; e;) {
			var t = e.next;
			e.prev = e.next = e.animation = null, e = t;
		}
		this._head = this._tail = null;
	}, t.prototype.isFinished = function() {
		return this._head == null;
	}, t.prototype.animate = function(e, t) {
		t ||= {}, this.start();
		var n = new mi(e, t.loop);
		return this.addAnimator(n), n;
	}, t;
}(hi), NT = 300, PT = q.domSupported, FT = (function() {
	var e = [
		"click",
		"dblclick",
		"mousewheel",
		"wheel",
		"mouseout",
		"mouseup",
		"mousedown",
		"mousemove",
		"contextmenu"
	], t = [
		"touchstart",
		"touchend",
		"touchmove"
	], n = {
		pointerdown: 1,
		pointerup: 1,
		pointermove: 1,
		pointerout: 1
	};
	return {
		mouse: e,
		touch: t,
		pointer: L(e, function(e) {
			var t = e.replace("mouse", "pointer");
			return n.hasOwnProperty(t) ? t : e;
		})
	};
})(), IT = {
	mouse: ["mousemove", "mouseup"],
	pointer: ["pointermove", "pointerup"]
}, LT = !1;
function RT(e) {
	var t = e.pointerType;
	return t === "pen" || t === "touch";
}
function zT(e) {
	e.touching = !0, e.touchTimer != null && (clearTimeout(e.touchTimer), e.touchTimer = null), e.touchTimer = setTimeout(function() {
		e.touching = !1, e.touchTimer = null;
	}, 700);
}
function BT(e) {
	e && (e.zrByTouch = !0);
}
function VT(e, t) {
	return Xw(e.dom, new UT(e, t), !0);
}
function HT(e, t) {
	for (var n = t, r = !1; n && n.nodeType !== 9 && !(r = n.domBelongToZr || n !== t && n === e.painterRoot);) n = n.parentNode;
	return r;
}
var UT = function() {
	function e(e, t) {
		this.stopPropagation = je, this.stopImmediatePropagation = je, this.preventDefault = je, this.type = t.type, this.target = this.currentTarget = e.dom, this.pointerType = t.pointerType, this.clientX = t.clientX, this.clientY = t.clientY;
	}
	return e;
}(), WT = {
	mousedown: function(e) {
		e = Xw(this.dom, e), this.__mayPointerCapture = [e.zrX, e.zrY], this.trigger("mousedown", e);
	},
	mousemove: function(e) {
		e = Xw(this.dom, e);
		var t = this.__mayPointerCapture;
		t && (e.zrX !== t[0] || e.zrY !== t[1]) && this.__togglePointerCapture(!0), this.trigger("mousemove", e);
	},
	mouseup: function(e) {
		e = Xw(this.dom, e), this.__togglePointerCapture(!1), this.trigger("mouseup", e);
	},
	mouseout: function(e) {
		e = Xw(this.dom, e);
		var t = e.toElement || e.relatedTarget;
		HT(this, t) || (this.__pointerCapturing && (e.zrEventControl = "no_globalout"), this.trigger("mouseout", e));
	},
	wheel: function(e) {
		LT = !0, e = Xw(this.dom, e), this.trigger("mousewheel", e);
	},
	mousewheel: function(e) {
		LT || (e = Xw(this.dom, e), this.trigger("mousewheel", e));
	},
	touchstart: function(e) {
		e = Xw(this.dom, e), BT(e), this.__lastTouchMoment = /* @__PURE__ */ new Date(), this.handler.processGesture(e, "start"), WT.mousemove.call(this, e), WT.mousedown.call(this, e);
	},
	touchmove: function(e) {
		e = Xw(this.dom, e), BT(e), this.handler.processGesture(e, "change"), WT.mousemove.call(this, e);
	},
	touchend: function(e) {
		e = Xw(this.dom, e), BT(e), this.handler.processGesture(e, "end"), WT.mouseup.call(this, e), /* @__PURE__ */ new Date() - +this.__lastTouchMoment < NT && WT.click.call(this, e);
	},
	pointerdown: function(e) {
		WT.mousedown.call(this, e);
	},
	pointermove: function(e) {
		RT(e) || WT.mousemove.call(this, e);
	},
	pointerup: function(e) {
		WT.mouseup.call(this, e);
	},
	pointerout: function(e) {
		RT(e) || WT.mouseout.call(this, e);
	}
};
I([
	"click",
	"dblclick",
	"contextmenu"
], function(e) {
	WT[e] = function(t) {
		t = Xw(this.dom, t), this.trigger(e, t);
	};
});
var GT = {
	pointermove: function(e) {
		RT(e) || GT.mousemove.call(this, e);
	},
	pointerup: function(e) {
		GT.mouseup.call(this, e);
	},
	mousemove: function(e) {
		this.trigger("mousemove", e);
	},
	mouseup: function(e) {
		var t = this.__pointerCapturing;
		this.__togglePointerCapture(!1), this.trigger("mouseup", e), t && (e.zrEventControl = "only_globalout", this.trigger("mouseout", e));
	}
};
function KT(e, t) {
	var n = t.domHandlers;
	q.pointerEventsSupported ? I(FT.pointer, function(r) {
		JT(t, r, function(t) {
			n[r].call(e, t);
		});
	}) : (q.touchEventsSupported && I(FT.touch, function(r) {
		JT(t, r, function(i) {
			n[r].call(e, i), zT(t);
		});
	}), I(FT.mouse, function(r) {
		JT(t, r, function(i) {
			i = Yw(i), t.touching || n[r].call(e, i);
		});
	}));
}
function qT(e, t) {
	q.pointerEventsSupported ? I(IT.pointer, n) : q.touchEventsSupported || I(IT.mouse, n);
	function n(n) {
		function r(r) {
			r = Yw(r), HT(e, r.target) || (r = VT(e, r), t.domHandlers[n].call(e, r));
		}
		JT(t, n, r, { capture: !0 });
	}
}
function JT(e, t, n, r) {
	e.mounted[t] = n, e.listenerOpts[t] = r, Qw(e.domTarget, t, n, r);
}
function YT(e) {
	var t = e.mounted;
	for (var n in t) t.hasOwnProperty(n) && $w(e.domTarget, n, t[n], e.listenerOpts[n]);
	e.mounted = {};
}
var XT = function() {
	function e(e, t) {
		this.mounted = {}, this.listenerOpts = {}, this.touching = !1, this.domTarget = e, this.domHandlers = t;
	}
	return e;
}(), ZT = function(e) {
	o(t, e);
	function t(t, n) {
		var r = e.call(this) || this;
		return r.__pointerCapturing = !1, r.dom = t, r.painterRoot = n, r._localHandlerScope = new XT(t, WT), PT && (r._globalHandlerScope = new XT(document, GT)), KT(r, r._localHandlerScope), r;
	}
	return t.prototype.dispose = function() {
		YT(this._localHandlerScope), PT && YT(this._globalHandlerScope);
	}, t.prototype.setCursor = function(e) {
		this.dom.style && (this.dom.style.cursor = e || "default");
	}, t.prototype.__togglePointerCapture = function(e) {
		if (this.__mayPointerCapture = null, PT && this.__pointerCapturing ^ +e) {
			this.__pointerCapturing = e;
			var t = this._globalHandlerScope;
			e ? qT(this, t) : YT(t);
		}
	}, t;
}(hi), QT = {}, $T = {};
function eE(e) {
	delete $T[e];
}
function tE(e) {
	if (!e) return !1;
	if (typeof e == "string") return Vr(e, 1) < vi;
	if (e.colorStops) {
		for (var t = e.colorStops, n = 0, r = t.length, i = 0; i < r; i++) n += Vr(t[i].color, 1);
		return n /= r, n < vi;
	}
	return !1;
}
var nE = function() {
	function e(e, t, n) {
		var r = this;
		this._sleepAfterStill = 10, this._stillFrameAccum = 0, this._needsRefresh = !0, this._needsRefreshHover = !1, this._darkMode = !1, n ||= {}, this.dom = t, this.id = e;
		var i = new kT(), a = n.renderer || "canvas";
		QT[a] || (a = R(QT)[0]), n.useDirtyRect = n.useDirtyRect == null ? !1 : n.useDirtyRect;
		var o = new QT[a](t, i, n, e), s = n.ssr || o.ssrOnly;
		this.storage = i, this.painter = o;
		var c = !q.node && !q.worker && !s ? new ZT(o.getViewportRoot(), o.root) : null, l = n.useCoarsePointer, u = l == null || l === "auto" ? q.touchEventsSupported : !!l, d = 44, f;
		u && (f = G(n.pointerSize, d)), this.handler = new fT(i, o, c, o.root, f), this.animation = new MT({ stage: { update: s ? null : function() {
			return r._flush(!1);
		} } }), s || this.animation.start();
	}
	return e.prototype.add = function(e) {
		this._disposed || !e || (this.storage.addRoot(e), e.addSelfToZr(this), this.refresh());
	}, e.prototype.remove = function(e) {
		this._disposed || !e || (this.storage.delRoot(e), e.removeSelfFromZr(this), this.refresh());
	}, e.prototype.configLayer = function(e, t) {
		this._disposed || (this.painter.configLayer && this.painter.configLayer(e, t), this.refresh());
	}, e.prototype.setBackgroundColor = function(e) {
		this._disposed || (this.painter.setBackgroundColor && this.painter.setBackgroundColor(e), this.refresh(), this._backgroundColor = e, this._darkMode = tE(e));
	}, e.prototype.getBackgroundColor = function() {
		return this._backgroundColor;
	}, e.prototype.setDarkMode = function(e) {
		this._darkMode = e;
	}, e.prototype.isDarkMode = function() {
		return this._darkMode;
	}, e.prototype.refreshImmediately = function(e) {
		this._disposed || this._refresh({
			animUpdate: !e,
			refresh: !0,
			refreshHover: !1
		});
	}, e.prototype._refresh = function(e) {
		e.animUpdate && this.animation.update(!0), this._needsRefresh = this._needsRefreshHover = !1, this.painter.refresh({
			refresh: e.refresh,
			refreshHover: e.refreshHover
		}), this._needsRefresh = this._needsRefreshHover = !1;
	}, e.prototype.refresh = function() {
		this._disposed || (this._needsRefresh = !0, this.animation.start());
	}, e.prototype.flush = function() {
		this._disposed || this._flush(!0);
	}, e.prototype._flush = function(e) {
		var t, n = jT(), r = this._needsRefresh, i = this._needsRefreshHover;
		(r || i) && (t = !0, this._refresh({
			animUpdate: e,
			refresh: r,
			refreshHover: i
		}));
		var a = jT();
		t ? (this._stillFrameAccum = 0, this.trigger("rendered", { elapsedTime: a - n })) : this._sleepAfterStill > 0 && (this._stillFrameAccum++, this._stillFrameAccum > this._sleepAfterStill && this.animation.stop());
	}, e.prototype.setSleepAfterStill = function(e) {
		this._sleepAfterStill = e;
	}, e.prototype.wakeUp = function() {
		this._disposed || (this.animation.start(), this._stillFrameAccum = 0);
	}, e.prototype.refreshHover = function() {
		this._needsRefreshHover = !0;
	}, e.prototype.refreshHoverImmediately = function() {
		this._disposed || this._refresh({
			animUpdate: !1,
			refresh: !1,
			refreshHover: !0
		});
	}, e.prototype.resize = function(e) {
		this._disposed || (e ||= {}, this.painter.resize(e.width, e.height), this.handler.resize());
	}, e.prototype.clearAnimation = function() {
		this._disposed || this.animation.clear();
	}, e.prototype.getWidth = function() {
		if (!this._disposed) return this.painter.getWidth();
	}, e.prototype.getHeight = function() {
		if (!this._disposed) return this.painter.getHeight();
	}, e.prototype.setCursorStyle = function(e) {
		this._disposed || this.handler.setCursorStyle(e);
	}, e.prototype.findHover = function(e, t) {
		if (!this._disposed) return this.handler.findHover(e, t);
	}, e.prototype.on = function(e, t, n) {
		return this._disposed || this.handler.on(e, t, n), this;
	}, e.prototype.off = function(e, t) {
		this._disposed || this.handler.off(e, t);
	}, e.prototype.trigger = function(e, t) {
		this._disposed || this.handler.trigger(e, t);
	}, e.prototype.clear = function() {
		if (!this._disposed) {
			for (var e = this.storage.getRoots(), t = 0; t < e.length; t++) e[t] instanceof su && e[t].removeSelfFromZr(this);
			this.storage.delAllRoots(), this.painter.clear();
		}
	}, e.prototype.dispose = function() {
		this._disposed || (this.animation.stop(), this.clear(), this.storage.dispose(), this.painter.dispose(), this.handler.dispose(), this.animation = this.storage = this.painter = this.handler = null, this._disposed = !0, eE(this.id));
	}, e;
}();
function rE(e, t) {
	var n = new nE(D(), e, t);
	return $T[n.id] = n, n;
}
function iE(e, t) {
	QT[e] = t;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/model/globalDefault.js
var aE = "";
typeof navigator < "u" && (aE = navigator.platform || "");
var oE = "rgba(0, 0, 0, 0.2)", sE = Q.color.theme[0], cE = zr(sE, null, null, .9), lE = {
	darkMode: "auto",
	colorBy: "series",
	color: Q.color.theme,
	gradientColor: [cE, sE],
	aria: { decal: { decals: [
		{
			color: oE,
			dashArrayX: [1, 0],
			dashArrayY: [2, 5],
			symbolSize: 1,
			rotation: Math.PI / 6
		},
		{
			color: oE,
			symbol: "circle",
			dashArrayX: [[8, 8], [
				0,
				8,
				8,
				0
			]],
			dashArrayY: [6, 0],
			symbolSize: .8
		},
		{
			color: oE,
			dashArrayX: [1, 0],
			dashArrayY: [4, 3],
			rotation: -Math.PI / 4
		},
		{
			color: oE,
			dashArrayX: [[6, 6], [
				0,
				6,
				6,
				0
			]],
			dashArrayY: [6, 0]
		},
		{
			color: oE,
			dashArrayX: [[1, 0], [1, 6]],
			dashArrayY: [
				1,
				0,
				6,
				0
			],
			rotation: Math.PI / 4
		},
		{
			color: oE,
			symbol: "triangle",
			dashArrayX: [[9, 9], [
				0,
				9,
				9,
				0
			]],
			dashArrayY: [7, 2],
			symbolSize: .75
		}
	] } },
	textStyle: {
		fontFamily: aE.match(/^Win/) ? "Microsoft YaHei" : "sans-serif",
		fontSize: 12,
		fontStyle: "normal",
		fontWeight: "normal"
	},
	blendMode: null,
	stateAnimation: {
		duration: 300,
		easing: "cubicOut"
	},
	animation: "auto",
	animationDuration: 1e3,
	animationDurationUpdate: 500,
	animationEasing: "cubicInOut",
	animationEasingUpdate: "cubicInOut",
	animationThreshold: 2e3,
	progressiveThreshold: 3e3,
	progressive: 400,
	hoverLayerThreshold: 3e3,
	useUTC: !1
}, uE = K();
function dE(e, t, n) {
	var r = uE.get(t);
	if (!r) return n;
	var i = r(e);
	return i ? n.concat(i) : n;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/model/Global.js
var fE, pE, mE, hE = "\0_ec_inner", gE = 1, _E = function(e) {
	o(t, e);
	function t() {
		return e !== null && e.apply(this, arguments) || this;
	}
	return t.prototype.init = function(e, t, n, r, i, a) {
		r ||= {}, this.option = null, this._theme = new Bf(r), this._locale = new Bf(i), this._optionManager = a;
	}, t.prototype.setOption = function(e, t, n) {
		var r = SE(t);
		this._optionManager.setOption(e, n, r), this._resetOption(null, r);
	}, t.prototype.resetOption = function(e, t) {
		return this._resetOption(e, SE(t));
	}, t.prototype._resetOption = function(e, t) {
		var n = !1, r = this._optionManager;
		if (!e || e === "recreate") {
			var i = r.mountOption(e === "recreate");
			!this.option || e === "recreate" ? mE(this, i) : (this.restoreData(), this._mergeOption(i, t)), n = !0;
		}
		if ((e === "timeline" || e === "media") && this.restoreData(), !e || e === "recreate" || e === "timeline") {
			var a = r.getTimelineOption(this);
			a && (n = !0, this._mergeOption(a, t));
		}
		if (!e || e === "recreate" || e === "media") {
			var o = r.getMediaOption(this);
			o.length && I(o, function(e) {
				n = !0, this._mergeOption(e, t);
			}, this);
		}
		return n;
	}, t.prototype.mergeOption = function(e) {
		this._mergeOption(e, null);
	}, t.prototype._mergeOption = function(e, t) {
		var n = this.option, r = this._componentsMap, i = this._componentsCount, a = [], o = K(), s = t && t.replaceMergeMainTypeMap;
		Kf(this), I(e, function(e, t) {
			e != null && (Ng.hasClass(t) ? t && (a.push(t), o.set(t, !0)) : n[t] = n[t] == null ? k(e) : A(n[t], e, !0));
		}), s && s.each(function(e, t) {
			Ng.hasClass(t) && !o.get(t) && (a.push(t), o.set(t, !0));
		}), Ng.topologicalTravel(a, Ng.getAllClassMainTypes(), c, this);
		function c(t) {
			var a = dE(this, t, ws(e[t])), o = r.get(t), c = ks(o, a, o ? s && s.get(t) ? "replaceMerge" : "normalMerge" : "replaceAll");
			Vs(c, t, Ng), n[t] = null, r.set(t, null), i.set(t, 0);
			var l = [], u = [], d = 0, f;
			I(c, function(e, n) {
				var r = e.existing, i = e.newOption;
				if (!i) r && (r.mergeOption({}, this), r.optionUpdated({}, !1));
				else {
					var a = t === "series", o = Ng.getClass(t, e.keyInfo.subType, !a);
					if (!o) return;
					if (t === "tooltip") {
						if (f) return;
						f = !0;
					}
					if (r && r.constructor === o) r.name = e.keyInfo.name, r.mergeOption(i, this), r.optionUpdated(i, !1);
					else {
						var s = j({ componentIndex: n }, e.keyInfo);
						r = new o(i, this, this, s), j(r, s), e.brandNew && (r.__requireNewView = !0), r.init(i, this, this), r.optionUpdated(null, !0);
					}
				}
				r ? (l.push(r.option), u.push(r), d++) : (l.push(void 0), u.push(void 0));
			}, this), n[t] = l, r.set(t, u), i.set(t, d), t === "series" && fE(this);
		}
		this._seriesIndices || fE(this);
	}, t.prototype.getOption = function() {
		var e = k(this.option);
		return I(e, function(t, n) {
			if (Ng.hasClass(n)) {
				for (var r = ws(t), i = r.length, a = !1, o = i - 1; o >= 0; o--) r[o] && !Bs(r[o]) ? a = !0 : (r[o] = null, !a && i--);
				r.length = i, e[n] = r;
			}
		}), delete e[hE], e;
	}, t.prototype.setTheme = function(e) {
		this._theme = new Bf(e), this._resetOption("recreate", null);
	}, t.prototype.getTheme = function() {
		return this._theme;
	}, t.prototype.getLocaleModel = function() {
		return this._locale;
	}, t.prototype.setUpdatePayload = function(e) {
		this._payload = e;
	}, t.prototype.getUpdatePayload = function() {
		return this._payload;
	}, t.prototype.getComponent = function(e, t) {
		var n = this._componentsMap.get(e);
		if (n) {
			var r = n[t || 0];
			if (r) return r;
			if (t == null) {
				for (var i = 0; i < n.length; i++) if (n[i]) return n[i];
			}
		}
	}, t.prototype.queryComponents = function(e) {
		var t = e.mainType;
		if (!t) return [];
		var n = e.index, r = e.id, i = e.name, a = this._componentsMap.get(t);
		if (!a || !a.length) return [];
		var o;
		return n == null ? o = r == null ? i == null ? re(a, function(e) {
			return !!e;
		}) : bE("name", i, a) : bE("id", r, a) : (o = [], I(ws(n), function(e) {
			a[e] && o.push(a[e]);
		})), xE(o, e);
	}, t.prototype.findComponents = function(e) {
		var t = e.query, n = e.mainType, r = i(t);
		return a(xE(r ? this.queryComponents(r) : re(this._componentsMap.get(n), function(e) {
			return !!e;
		}), e));
		function i(e) {
			var t = n + "Index", r = n + "Id", i = n + "Name";
			return e && (e[t] != null || e[r] != null || e[i] != null) ? {
				mainType: n,
				index: e[t],
				id: e[r],
				name: e[i]
			} : null;
		}
		function a(t) {
			return e.filter ? re(t, e.filter) : t;
		}
	}, t.prototype.eachComponent = function(e, t, n) {
		var r = this._componentsMap;
		if (H(e)) {
			var i = t, a = e;
			r.each(function(e, t) {
				for (var n = 0; e && n < e.length; n++) {
					var r = e[n];
					r && a.call(i, t, r, r.componentIndex);
				}
			});
		} else for (var o = U(e) ? r.get(e) : W(e) ? this.findComponents(e) : null, s = 0; o && s < o.length; s++) {
			var c = o[s];
			c && t.call(n, c, c.componentIndex);
		}
	}, t.prototype.getSeriesByName = function(e) {
		var t = Rs(e, null);
		return re(this._componentsMap.get("series"), function(e) {
			return !!e && t != null && e.name === t;
		});
	}, t.prototype.getSeriesByIndex = function(e) {
		return this._componentsMap.get("series")[e];
	}, t.prototype.getSeriesByType = function(e) {
		return re(this._componentsMap.get("series"), function(t) {
			return !!t && t.subType === e;
		});
	}, t.prototype.getSeries = function() {
		return re(this._componentsMap.get("series"), function(e) {
			return !!e;
		});
	}, t.prototype.getSeriesCount = function() {
		return this._componentsCount.get("series");
	}, t.prototype.eachSeries = function(e, t) {
		pE(this), I(this._seriesIndices, function(n) {
			var r = this._componentsMap.get("series")[n];
			e.call(t, r, n);
		}, this);
	}, t.prototype.eachRawSeries = function(e, t) {
		I(this._componentsMap.get("series"), function(n) {
			n && e.call(t, n, n.componentIndex);
		});
	}, t.prototype.eachSeriesByType = function(e, t, n) {
		pE(this), I(this._seriesIndices, function(r) {
			var i = this._componentsMap.get("series")[r];
			i.subType === e && t.call(n, i, r);
		}, this);
	}, t.prototype.eachRawSeriesByType = function(e, t, n) {
		return I(this.getSeriesByType(e), t, n);
	}, t.prototype.isSeriesFiltered = function(e) {
		return pE(this), this._seriesIndicesMap.get(e.componentIndex) == null;
	}, t.prototype.getCurrentSeriesIndices = function() {
		return (this._seriesIndices || []).slice();
	}, t.prototype.filterSeries = function(e, t) {
		pE(this);
		var n = [];
		I(this._seriesIndices, function(r) {
			var i = this._componentsMap.get("series")[r];
			e.call(t, i, r) && n.push(r);
		}, this), this._seriesIndices = n, this._seriesIndicesMap = K(n);
	}, t.prototype.restoreData = function(e) {
		fE(this);
		var t = this._componentsMap, n = [];
		t.each(function(e, t) {
			Ng.hasClass(t) && n.push(t);
		}), Ng.topologicalTravel(n, Ng.getAllClassMainTypes(), function(n) {
			I(t.get(n), function(t) {
				t && (n !== "series" || !vE(t, e)) && t.restoreData();
			});
		});
	}, t.internalField = function() {
		fE = function(e) {
			var t = e._seriesIndices = [];
			I(e._componentsMap.get("series"), function(e) {
				e && t.push(e.componentIndex);
			}), e._seriesIndicesMap = K(t);
		}, pE = function(e) {}, mE = function(e, t) {
			e.option = {}, e.option[hE] = gE, e._componentsMap = K({ series: [] }), e._componentsCount = K();
			var n = t.aria;
			W(n) && n.enabled == null && (n.enabled = !0), yE(t, e._theme.option), A(t, lE, !1), e._mergeOption(t, null);
		};
	}(), t;
}(Bf);
function vE(e, t) {
	if (t) {
		var n = t.seriesIndex, r = t.seriesId, i = t.seriesName;
		return n != null && e.componentIndex !== n || r != null && e.id !== r || i != null && e.name !== i;
	}
}
function yE(e, t) {
	var n = e.color && !e.colorLayer;
	I(t, function(t, r) {
		r === "colorLayer" && n || r === "color" && e.color || Ng.hasClass(r) || (typeof t == "object" ? e[r] = e[r] ? A(e[r], t, !1) : k(t) : e[r] ?? (e[r] = t));
	});
}
function bE(e, t, n) {
	if (V(t)) {
		var r = K();
		return I(t, function(e) {
			e != null && Rs(e, null) != null && r.set(e, !0);
		}), re(n, function(t) {
			return t && r.get(t[e]);
		});
	} else {
		var i = Rs(t, null);
		return re(n, function(t) {
			return t && i != null && t[e] === i;
		});
	}
}
function xE(e, t) {
	return t.hasOwnProperty("subType") ? re(e, function(e) {
		return e && e.subType === t.subType;
	}) : e;
}
function SE(e) {
	var t = K();
	return e && I(ws(e.replaceMerge), function(e) {
		t.set(e, !0);
	}), { replaceMergeMainTypeMap: t };
}
P(_E, Ig);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/model/OptionManager.js
var CE = /^(min|max)?(.+)$/, wE = function() {
	function e(e) {
		this._timelineOptions = [], this._mediaList = [], this._currentMediaIndices = [], this._api = e;
	}
	return e.prototype.setOption = function(e, t, n) {
		e && (I(ws(e.series), function(e) {
			e && e.data && le(e.data) && xe(e.data);
		}), I(ws(e.dataset), function(e) {
			e && e.source && le(e.source) && xe(e.source);
		})), e = k(e);
		var r = this._optionBackup, i = TE(e, t, !r);
		this._newBaseOption = i.baseOption, r ? (i.timelineOptions.length && (r.timelineOptions = i.timelineOptions), i.mediaList.length && (r.mediaList = i.mediaList), i.mediaDefault && (r.mediaDefault = i.mediaDefault)) : this._optionBackup = i;
	}, e.prototype.mountOption = function(e) {
		var t = this._optionBackup;
		return this._timelineOptions = t.timelineOptions, this._mediaList = t.mediaList, this._mediaDefault = t.mediaDefault, this._currentMediaIndices = [], k(e ? t.baseOption : this._newBaseOption);
	}, e.prototype.getTimelineOption = function(e) {
		var t, n = this._timelineOptions;
		if (n.length) {
			var r = e.getComponent("timeline");
			r && (t = k(n[r.getCurrentIndex()]));
		}
		return t;
	}, e.prototype.getMediaOption = function(e) {
		var t = this._api.getWidth(), n = this._api.getHeight(), r = this._mediaList, i = this._mediaDefault, a = [], o = [];
		if (!r.length && !i) return o;
		for (var s = 0, c = r.length; s < c; s++) EE(r[s].query, t, n) && a.push(s);
		return !a.length && i && (a = [-1]), a.length && !OE(a, this._currentMediaIndices) && (o = L(a, function(e) {
			return k(e === -1 ? i.option : r[e].option);
		})), this._currentMediaIndices = a, o;
	}, e;
}();
function TE(e, t, n) {
	var r = [], i, a, o = e.baseOption, s = e.timeline, c = e.options, l = e.media, u = !!e.media, d = !!(c || s || o && o.timeline);
	o ? (a = o, a.timeline ||= s) : ((d || u) && (e.options = e.media = null), a = e), u && V(l) && I(l, function(e) {
		e && e.option && (e.query ? r.push(e) : i ||= e);
	}), f(a), I(c, function(e) {
		return f(e);
	}), I(r, function(e) {
		return f(e.option);
	});
	function f(e) {
		I(t, function(t) {
			t(e, n);
		});
	}
	return {
		baseOption: a,
		timelineOptions: c || [],
		mediaDefault: i,
		mediaList: r
	};
}
function EE(e, t, n) {
	var r = {
		width: t,
		height: n,
		aspectratio: t / n
	}, i = !0;
	return I(e, function(e, t) {
		var n = t.match(CE);
		if (!(!n || !n[1] || !n[2])) {
			var a = n[1];
			DE(r[n[2].toLowerCase()], e, a) || (i = !1);
		}
	}), i;
}
function DE(e, t, n) {
	return n === "min" ? e >= t : n === "max" ? e <= t : e === t;
}
function OE(e, t) {
	return e.join(",") === t.join(",");
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/preprocessor/helper/compatStyle.js
var kE = I, AE = W, jE = [
	"areaStyle",
	"lineStyle",
	"nodeStyle",
	"linkStyle",
	"chordStyle",
	"label",
	"labelLine"
];
function ME(e) {
	var t = e && e.itemStyle;
	if (t) for (var n = 0, r = jE.length; n < r; n++) {
		var i = jE[n], a = t.normal, o = t.emphasis;
		a && a[i] && (e[i] = e[i] || {}, e[i].normal ? A(e[i].normal, a[i]) : e[i].normal = a[i], a[i] = null), o && o[i] && (e[i] = e[i] || {}, e[i].emphasis ? A(e[i].emphasis, o[i]) : e[i].emphasis = o[i], o[i] = null);
	}
}
function NE(e, t, n) {
	if (e && e[t] && (e[t].normal || e[t].emphasis)) {
		var r = e[t].normal, i = e[t].emphasis;
		r && (n ? (e[t].normal = e[t].emphasis = null, M(e[t], r)) : e[t] = r), i && (e.emphasis = e.emphasis || {}, e.emphasis[t] = i, i.focus && (e.emphasis.focus = i.focus), i.blurScope && (e.emphasis.blurScope = i.blurScope));
	}
}
function PE(e) {
	NE(e, "itemStyle"), NE(e, "lineStyle"), NE(e, "areaStyle"), NE(e, "label"), NE(e, "labelLine"), NE(e, "upperLabel"), NE(e, "edgeLabel");
}
function FE(e, t) {
	var n = AE(e) && e[t], r = AE(n) && n.textStyle;
	if (r) for (var i = 0, a = Es.length; i < a; i++) {
		var o = Es[i];
		r.hasOwnProperty(o) && (n[o] = r[o]);
	}
}
function IE(e) {
	e && (PE(e), FE(e, "label"), e.emphasis && FE(e.emphasis, "label"));
}
function LE(e) {
	if (AE(e)) {
		ME(e), PE(e), FE(e, "label"), FE(e, "upperLabel"), FE(e, "edgeLabel"), e.emphasis && (FE(e.emphasis, "label"), FE(e.emphasis, "upperLabel"), FE(e.emphasis, "edgeLabel"));
		var t = e.markPoint;
		t && (ME(t), IE(t));
		var n = e.markLine;
		n && (ME(n), IE(n));
		var r = e.markArea;
		r && IE(r);
		var i = e.data;
		if (e.type === "graph") {
			i ||= e.nodes;
			var a = e.links || e.edges;
			if (a && !le(a)) for (var o = 0; o < a.length; o++) IE(a[o]);
			I(e.categories, function(e) {
				PE(e);
			});
		}
		if (i && !le(i)) for (var o = 0; o < i.length; o++) IE(i[o]);
		if (t = e.markPoint, t && t.data) for (var s = t.data, o = 0; o < s.length; o++) IE(s[o]);
		if (n = e.markLine, n && n.data) for (var c = n.data, o = 0; o < c.length; o++) V(c[o]) ? (IE(c[o][0]), IE(c[o][1])) : IE(c[o]);
		e.type === "gauge" ? (FE(e, "axisLabel"), FE(e, "title"), FE(e, "detail")) : e.type === "treemap" ? (NE(e.breadcrumb, "itemStyle"), I(e.levels, function(e) {
			PE(e);
		})) : e.type === "tree" && PE(e.leaves);
	}
}
function RE(e) {
	return V(e) ? e : e ? [e] : [];
}
function zE(e) {
	return (V(e) ? e[0] : e) || {};
}
function BE(e, t) {
	kE(RE(e.series), function(e) {
		AE(e) && LE(e);
	});
	var n = [
		"xAxis",
		"yAxis",
		"radiusAxis",
		"angleAxis",
		"singleAxis",
		"parallelAxis",
		"radar"
	];
	t && n.push("valueAxis", "categoryAxis", "logAxis", "timeAxis"), kE(n, function(t) {
		kE(RE(e[t]), function(e) {
			e && (FE(e, "axisLabel"), FE(e.axisPointer, "label"));
		});
	}), kE(RE(e.parallel), function(e) {
		var t = e && e.parallelAxisDefault;
		FE(t, "axisLabel"), FE(t && t.axisPointer, "label");
	}), kE(RE(e.calendar), function(e) {
		NE(e, "itemStyle"), FE(e, "dayLabel"), FE(e, "monthLabel"), FE(e, "yearLabel");
	}), kE(RE(e.radar), function(e) {
		FE(e, "name"), e.name && e.axisName == null && (e.axisName = e.name, delete e.name), e.nameGap != null && e.axisNameGap == null && (e.axisNameGap = e.nameGap, delete e.nameGap);
	}), kE(RE(e.geo), function(e) {
		AE(e) && (IE(e), kE(RE(e.regions), function(e) {
			IE(e);
		}));
	}), kE(RE(e.timeline), function(e) {
		IE(e), NE(e, "label"), NE(e, "itemStyle"), NE(e, "controlStyle", !0);
		var t = e.data;
		V(t) && I(t, function(e) {
			W(e) && (NE(e, "label"), NE(e, "itemStyle"));
		});
	}), kE(RE(e.toolbox), function(e) {
		NE(e, "iconStyle"), kE(e.feature, function(e) {
			NE(e, "iconStyle");
		});
	}), FE(zE(e.axisPointer), "label"), FE(zE(e.tooltip).axisPointer, "label");
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/preprocessor/backwardCompat.js
function VE(e, t) {
	for (var n = t.split(","), r = e, i = 0; i < n.length && (r &&= r[n[i]], r != null); i++);
	return r;
}
function HE(e, t, n, r) {
	for (var i = t.split(","), a = e, o, s = 0; s < i.length - 1; s++) o = i[s], a[o] ?? (a[o] = {}), a = a[o];
	(r || a[i[s]] == null) && (a[i[s]] = n);
}
function UE(e) {
	e && I(WE, function(t) {
		t[0] in e && !(t[1] in e) && (e[t[1]] = e[t[0]]);
	});
}
var WE = [
	["x", "left"],
	["y", "top"],
	["x2", "right"],
	["y2", "bottom"]
], GE = [
	"grid",
	"geo",
	"parallel",
	"legend",
	"toolbox",
	"title",
	"visualMap",
	"dataZoom",
	"timeline"
], KE = [
	["borderRadius", "barBorderRadius"],
	["borderColor", "barBorderColor"],
	["borderWidth", "barBorderWidth"]
];
function qE(e) {
	var t = e && e.itemStyle;
	if (t) for (var n = 0; n < KE.length; n++) {
		var r = KE[n][1], i = KE[n][0];
		t[r] != null && (t[i] = t[r]);
	}
}
function JE(e) {
	e && e.alignTo === "edge" && e.margin != null && e.edgeDistance == null && (e.edgeDistance = e.margin);
}
function YE(e) {
	e && e.downplay && !e.blur && (e.blur = e.downplay);
}
function XE(e) {
	e && e.focusNodeAdjacency != null && (e.emphasis = e.emphasis || {}, e.emphasis.focus ?? (e.emphasis.focus = "adjacency"));
}
function ZE(e, t) {
	if (e) for (var n = 0; n < e.length; n++) t(e[n]), e[n] && ZE(e[n].children, t);
}
function QE(e, t) {
	BE(e, t), e.series = ws(e.series), I(e.series, function(e) {
		if (W(e)) {
			var t = e.type;
			if (t === "line") e.clipOverflow != null && (e.clip = e.clipOverflow);
			else if (t === "pie" || t === "gauge") {
				e.clockWise != null && (e.clockwise = e.clockWise), JE(e.label);
				var n = e.data;
				if (n && !le(n)) for (var r = 0; r < n.length; r++) JE(n[r]);
				e.hoverOffset != null && (e.emphasis = e.emphasis || {}, (e.emphasis.scaleSize = null) && (e.emphasis.scaleSize = e.hoverOffset));
			} else if (t === "gauge") {
				var i = VE(e, "pointer.color");
				i != null && HE(e, "itemStyle.color", i);
			} else if (t === "bar") {
				qE(e), qE(e.backgroundStyle), qE(e.emphasis);
				var n = e.data;
				if (n && !le(n)) for (var r = 0; r < n.length; r++) typeof n[r] == "object" && (qE(n[r]), qE(n[r] && n[r].emphasis));
			} else if (t === "sunburst") {
				var a = e.highlightPolicy;
				a && (e.emphasis = e.emphasis || {}, e.emphasis.focus || (e.emphasis.focus = a)), YE(e), ZE(e.data, YE);
			} else t === "graph" || t === "sankey" ? XE(e) : t === "map" && (e.mapType && !e.map && (e.map = e.mapType), e.mapLocation && M(e, e.mapLocation));
			e.hoverAnimation != null && (e.emphasis = e.emphasis || {}, e.emphasis && e.emphasis.scale == null && (e.emphasis.scale = e.hoverAnimation)), UE(e);
		}
	}), e.dataRange && (e.visualMap = e.dataRange), I(GE, function(t) {
		var n = e[t];
		n && (V(n) || (n = [n]), I(n, function(e) {
			UE(e);
		}));
	});
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/processor/dataStack.js
var $E = vc(eD);
function eD(e) {
	var t = K();
	e.eachSeries(function(e) {
		var n = e.get("stack");
		if (n) {
			var r = t.get(n) || t.set(n, []), i = e.getData(), a = {
				stackResultDimension: i.getCalculationInfo("stackResultDimension"),
				stackedOverDimension: i.getCalculationInfo("stackedOverDimension"),
				stackedDimension: i.getCalculationInfo("stackedDimension"),
				stackedByDimension: i.getCalculationInfo("stackedByDimension"),
				isStackedByIndex: i.getCalculationInfo("isStackedByIndex"),
				data: i,
				seriesModel: e
			};
			if (!a.stackedDimension || !(a.isStackedByIndex || a.stackedByDimension)) return;
			r.push(a);
		}
	}), t.each(function(e) {
		e.length !== 0 && ((e[0].seriesModel.get("stackOrder") || "seriesAsc") === "seriesDesc" && e.reverse(), I(e, function(t, n) {
			t.data.setCalculationInfo("stackedOnSeries", n > 0 ? e[n - 1].seriesModel : null);
		}), tD(e));
	});
}
function tD(e) {
	I(e, function(t, n) {
		var r = [], i = [NaN, NaN], a = [t.stackResultDimension, t.stackedOverDimension], o = t.data, s = t.isStackedByIndex, c = t.seriesModel.get("stackStrategy") || "samesign";
		o.modify(a, function(a, l, u) {
			var d = o.get(t.stackedDimension, u);
			if (isNaN(d)) return i;
			var f, p;
			s ? p = o.getRawIndex(u) : f = o.get(t.stackedByDimension, u);
			for (var m = NaN, h = n - 1; h >= 0; h--) {
				var g = e[h];
				if (s || (p = g.data.rawIndexOf(g.stackedByDimension, f)), p >= 0) {
					var _ = g.data.getByRawIndex(g.stackResultDimension, p);
					if (c === "all" || c === "positive" && _ > 0 || c === "negative" && _ < 0 || c === "samesign" && d >= 0 && _ > 0 || c === "samesign" && d <= 0 && _ < 0) {
						d = es(d, _), m = _;
						break;
					}
				}
			}
			return r[0] = d, r[1] = m, r;
		});
	});
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/view/Component.js
var nD = function() {
	function e() {
		this.group = new su(), this.uid = Wm("viewComponent");
	}
	return e.prototype.init = function(e, t) {}, e.prototype.render = function(e, t, n, r) {}, e.prototype.dispose = function(e, t) {}, e.prototype.updateView = function(e, t, n, r) {}, e.prototype.updateLayout = function(e, t, n, r) {}, e.prototype.updateVisual = function(e, t, n, r) {}, e.prototype.toggleBlurSeries = function(e, t, n) {}, e.prototype.eachRendered = function(e) {
		var t = this.group;
		t && t.traverse(e);
	}, e;
}();
Ve(nD), Je(nD);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/visual/style.js
var rD = Ws(), iD = {
	itemStyle: Ye(Lf, !0),
	lineStyle: Ye(Pf, !0)
}, aD = {
	lineStyle: "stroke",
	itemStyle: "fill"
};
function oD(e, t) {
	return e.visualStyleMapper || iD[t] || (console.warn("Unknown style type '" + t + "'."), iD.itemStyle);
}
function sD(e, t) {
	return e.visualDrawType || aD[t] || (console.warn("Unknown style type '" + t + "'."), "fill");
}
var cD = {
	createOnAllSeries: !0,
	performRawSeries: !0,
	reset: function(e, t) {
		var n = e.getData(), r = e.visualStyleAccessPath || "itemStyle", i = e.getModel(r), a = oD(e, r)(i), o = i.getShallow("decal");
		o && (n.setVisual("decal", o), o.dirty = !0);
		var s = sD(e, r), c = a[s], l = H(c) ? c : null, u = a.fill === "auto" || a.stroke === "auto";
		if (!a[s] || l || u) {
			var d = e.getColorFromPalette(e.name, null, t.getSeriesCount());
			a[s] || (a[s] = d, n.setVisual("colorFromPalette", !0)), a.fill = a.fill === "auto" || H(a.fill) ? d : a.fill, a.stroke = a.stroke === "auto" || H(a.stroke) ? d : a.stroke;
		}
		if (n.setVisual("style", a), n.setVisual("drawType", s), !t.isSeriesFiltered(e) && l) return n.setVisual("colorFromPalette", !1), { dataEach: function(t, n) {
			var r = e.getDataParams(n), i = j({}, a);
			i[s] = l(r), t.setItemVisual(n, "style", i);
		} };
	}
}, lD = new Bf(), uD = {
	createOnAllSeries: !0,
	reset: function(e, t) {
		if (!e.ignoreStyleOnData) {
			var n = e.getData(), r = e.visualStyleAccessPath || "itemStyle", i = oD(e, r), a = n.getVisual("drawType");
			return { dataEach: n.hasItemOption ? function(e, t) {
				var n = e.getRawDataItem(t);
				if (n && n[r]) {
					lD.option = n[r];
					var o = i(lD);
					j(e.ensureUniqueItemVisual(t, "style"), o), lD.option.decal && (e.setItemVisual(t, "decal", lD.option.decal), lD.option.decal.dirty = !0), a in o && e.setItemVisual(t, "colorFromPalette", !1);
				}
			} : null };
		}
	}
}, dD = {
	performRawSeries: !0,
	overallReset: function(e) {
		var t = K();
		e.eachSeries(function(e) {
			if (!e.isColorBySeries()) {
				var n = e.type + "-" + e.getColorBy();
				rD(e).scope = t.get(n) || t.set(n, {});
			}
		}), e.eachSeries(function(e) {
			if (!e.isColorBySeries()) {
				var t = e.getRawData(), n = {}, r = e.getData(), i = rD(e).scope, a = sD(e, e.visualStyleAccessPath || "itemStyle");
				r.each(function(e) {
					var t = r.getRawIndex(e);
					n[t] = e;
				}), t.each(function(o) {
					var s = n[o];
					if (r.getItemVisual(s, "colorFromPalette")) {
						var c = r.ensureUniqueItemVisual(s, "style"), l = t.getName(o) || o + "", u = t.count();
						c[a] = e.getColorFromPalette(l, i, u);
					}
				});
			}
		});
	}
}, fD = Math.PI;
function pD(e, t) {
	t ||= {}, M(t, {
		text: "loading",
		textColor: Q.color.primary,
		fontSize: 12,
		fontWeight: "normal",
		fontStyle: "normal",
		fontFamily: "sans-serif",
		maskColor: "rgba(255,255,255,0.8)",
		showSpinner: !0,
		color: Q.color.theme[0],
		spinnerRadius: 10,
		lineWidth: 5,
		zlevel: 0
	});
	var n = new su(), r = new fo({
		style: { fill: t.maskColor },
		zlevel: t.zlevel,
		z: 1e4
	});
	n.add(r);
	var i = new _o({
		style: {
			text: t.text,
			fill: t.textColor,
			fontSize: t.fontSize,
			fontWeight: t.fontWeight,
			fontStyle: t.fontStyle,
			fontFamily: t.fontFamily
		},
		zlevel: t.zlevel,
		z: 10001
	}), a = new fo({
		style: { fill: "none" },
		textContent: i,
		textConfig: {
			position: "right",
			distance: 10
		},
		zlevel: t.zlevel,
		z: 10001
	});
	n.add(a);
	var o;
	return t.showSpinner && (o = new Gu({
		shape: {
			startAngle: -fD / 2,
			endAngle: -fD / 2 + .1,
			r: t.spinnerRadius
		},
		style: {
			stroke: t.color,
			lineCap: "round",
			lineWidth: t.lineWidth
		},
		zlevel: t.zlevel,
		z: 10001
	}), o.animateShape(!0).when(1e3, { endAngle: fD * 3 / 2 }).start("circularInOut"), o.animateShape(!0).when(1e3, { startAngle: fD * 3 / 2 }).delay(300).start("circularInOut"), n.add(o)), n.resize = function() {
		var n = i.getBoundingRect().width, s = t.showSpinner ? t.spinnerRadius : 0, c = (e.getWidth() - s * 2 - (t.showSpinner && n ? 10 : 0) - n) / 2 - (t.showSpinner && n ? 0 : 5 + n / 2) + (t.showSpinner ? 0 : n / 2) + (n ? 0 : s), l = e.getHeight() / 2;
		t.showSpinner && o.setShape({
			cx: c,
			cy: l
		}), a.setShape({
			x: c - s,
			y: l - s,
			width: s * 2,
			height: s * 2
		}), r.setShape({
			x: 0,
			y: 0,
			width: e.getWidth(),
			height: e.getHeight()
		});
	}, n.resize(), n;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/core/Scheduler.js
var mD = function() {
	function e(e, t, n, r) {
		this._stageTaskMap = K(), this.ecInstance = e, this.api = t, n = this._dataProcessorHandlers = n.slice(), r = this._visualHandlers = r.slice(), this._allHandlers = n.concat(r);
	}
	return e.prototype.restoreData = function(e, t) {
		e.restoreData(t), this._stageTaskMap.each(function(e) {
			var t = e.overallTask;
			t && t.dirty();
		});
	}, e.prototype.getPerformArgs = function(e, t) {
		if (e.__pipeline) {
			var n = this._pipelineMap.get(e.__pipeline.id), r = n.context, i = !t && n.progressiveEnabled && (!r || r.progressiveRender) && e.__idxInPipeline > n.blockIndex ? n.step : null, a = r && r.modDataCount;
			return {
				step: i,
				modBy: a == null ? null : Math.ceil(a / i),
				modDataCount: a
			};
		}
	}, e.prototype.getPipeline = function(e) {
		return this._pipelineMap.get(e);
	}, e.prototype.updateStreamModes = function(e, t) {
		var n = this._pipelineMap.get(e.uid);
		e.pipelineContext = n.context = e.__preparePipelineContext ? e.__preparePipelineContext(t, n) : gc(e, t, n);
	}, e.prototype.restorePipelines = function(e, t) {
		var n = this, r = n._pipelineMap = K();
		t.eachSeries(function(t) {
			var i = e.painter.type === "canvas" && t.getProgressive(), a = t.uid;
			r.set(a, {
				id: a,
				head: null,
				tail: null,
				threshold: t.getProgressiveThreshold(),
				progressiveEnabled: i && !(t.preventIncremental && t.preventIncremental()),
				blockIndex: -1,
				step: Math.round(i || 700),
				count: 0
			}), n._pipe(t, t.dataTask);
		});
	}, e.prototype.prepareStageTasks = function() {
		var e = this._stageTaskMap, t = this.api.getModel(), n = this.api;
		I(this._allHandlers, function(r) {
			var i = e.get(r.uid) || e.set(r.uid, {});
			ve(!(r.reset && r.overallReset), ""), r.reset && this._createSeriesStageTask(r, i, t, n), r.overallReset && this._createOverallStageTask(r, i, t, n);
		}, this);
	}, e.prototype.prepareView = function(e, t, n, r) {
		var i = e.renderTask, a = i.context;
		a.model = t, a.ecModel = n, a.api = r, i.__block = !e.incrementalPrepareRender, this._pipe(t, i);
	}, e.prototype.performDataProcessorTasks = function(e, t) {
		this._performStageTasks(this._dataProcessorHandlers, e, t, { block: !0 });
	}, e.prototype.performVisualTasks = function(e, t, n) {
		this._performStageTasks(this._visualHandlers, e, t, n);
	}, e.prototype._performStageTasks = function(e, t, n, r) {
		r ||= {};
		var i = !1, a = this;
		I(e, function(e, s) {
			if (!(r.visualType && r.visualType !== e.visualType)) {
				var c = a._stageTaskMap.get(e.uid), l = c.seriesTaskMap, u = c.overallTask;
				if (u) {
					var d, f = u.agentStubMap;
					f.each(function(e) {
						o(r, e) && (e.dirty(), d = !0);
					}), d && u.dirty(), a.updatePayload(u, n);
					var p = a.getPerformArgs(u, r.block);
					f.each(function(e) {
						e.perform(p);
					}), u.perform(p) && (i = !0);
				} else l && l.each(function(s, c) {
					o(r, s) && s.dirty();
					var l = a.getPerformArgs(s, r.block);
					l.skip = !e.performRawSeries && t.isSeriesFiltered(s.context.model), a.updatePayload(s, n), s.perform(l) && (i = !0);
				});
			}
		});
		function o(e, t) {
			return e.setDirty && (!e.dirtyMap || e.dirtyMap.get(t.__pipeline.id));
		}
		this.unfinished = i || this.unfinished;
	}, e.prototype.performSeriesTasks = function(e) {
		var t;
		e.eachSeries(function(e) {
			t = e.dataTask.perform() || t;
		}), this.unfinished = t || this.unfinished;
	}, e.prototype.plan = function() {
		this._pipelineMap.each(function(e) {
			var t = e.tail;
			do {
				if (t.__block) {
					e.blockIndex = t.__idxInPipeline;
					break;
				}
				t = t.getUpstream();
			} while (t);
		});
	}, e.prototype.updatePayload = function(e, t) {
		t !== "remain" && (e.context.payload = t);
	}, e.prototype._createSeriesStageTask = function(e, t, n, r) {
		var i = this, a = t.seriesTaskMap, o = t.seriesTaskMap = K(), s = e.seriesType, c = e.getTargetSeries;
		e.createOnAllSeries ? n.eachRawSeries(l) : s ? n.eachRawSeriesByType(s, l) : c && c(n, r).each(l);
		function l(t) {
			var s = t.uid, c = o.set(s, a && a.get(s) || Ug({
				plan: yD,
				reset: bD,
				count: CD
			}));
			c.context = {
				model: t,
				ecModel: n,
				api: r,
				useClearVisual: e.isVisual && !e.isLayout,
				plan: e.plan,
				reset: e.reset,
				scheduler: i
			}, i._pipe(t, c);
		}
	}, e.prototype._createOverallStageTask = function(e, t, n, r) {
		var i = this, a = t.overallTask = t.overallTask || Ug({ reset: hD });
		a.context = {
			ecModel: n,
			api: r,
			overallReset: e.overallReset,
			scheduler: i
		};
		var o = a.agentStubMap, s = a.agentStubMap = K(), c = e.seriesType, l = e.getTargetSeries, u = e.dirtyOnOverallProgress, d = !1;
		ve(!e.createOnAllSeries, ""), c ? n.eachRawSeriesByType(c, f) : l ? l(n, r).each(f) : I(n.getSeries(), f);
		function f(e) {
			var t = e.uid, n = s.set(t, o && o.get(t) || (d = !0, Ug({
				reset: gD,
				onDirty: vD
			})));
			n.context = {
				model: e,
				dirtyOnOverallProgress: u
			}, n.agent = a, n.__block = u, i._pipe(e, n);
		}
		d && a.dirty();
	}, e.prototype._pipe = function(e, t) {
		var n = e.uid, r = this._pipelineMap.get(n);
		!r.head && (r.head = t), r.tail && r.tail.pipe(t), r.tail = t, t.__idxInPipeline = r.count++, t.__pipeline = r;
	}, e.wrapStageHandler = function(e, t) {
		return H(e) && (e = {
			overallReset: e,
			seriesType: wD(e)
		}), e.uid = Wm("stageHandler"), t && (e.visualType = t), e;
	}, e;
}();
function hD(e) {
	e.overallReset(e.ecModel, e.api, e.payload);
}
function gD(e) {
	return e.dirtyOnOverallProgress && _D;
}
function _D() {
	this.agent.dirty(), this.getDownstream().dirty();
}
function vD() {
	this.agent && this.agent.dirty();
}
function yD(e) {
	return e.plan ? e.plan(e.model, e.ecModel, e.api, e.payload) : null;
}
function bD(e) {
	e.useClearVisual && e.data.clearAllVisual();
	var t = e.resetDefines = ws(e.reset(e.model, e.ecModel, e.api, e.payload));
	return t.length > 1 ? L(t, function(e, t) {
		return SD(t);
	}) : xD;
}
var xD = SD(0);
function SD(e) {
	return function(t, n) {
		var r = n.data, i = n.resetDefines[e];
		if (i && i.dataEach) for (var a = t.start; a < t.end; a++) i.dataEach(r, a);
		else i && i.progress && i.progress(t, r);
	};
}
function CD(e) {
	return e.data.count();
}
function wD(e) {
	DD = null;
	try {
		e(TD, ED);
	} catch {}
	return DD;
}
var TD = {}, ED = {}, DD;
OD(TD, _E), OD(ED, Ac), TD.eachSeriesByType = TD.eachRawSeriesByType = function(e) {
	DD = e;
}, TD.eachComponent = function(e) {
	e.mainType === "series" && e.subType && (DD = e.subType);
};
function OD(e, t) {
	for (var n in t.prototype) e[n] = je;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/theme/dark.js
var $ = Q.darkColor, kD = $.background, AD = function() {
	return {
		axisLine: { lineStyle: { color: $.axisLine } },
		splitLine: { lineStyle: { color: $.axisSplitLine } },
		splitArea: { areaStyle: { color: [$.backgroundTint, $.backgroundTransparent] } },
		minorSplitLine: { lineStyle: { color: $.axisMinorSplitLine } },
		axisLabel: { color: $.axisLabel },
		axisName: {}
	};
}, jD = {
	label: { color: $.secondary },
	itemStyle: { borderColor: $.borderTint },
	dividerLineStyle: { color: $.border }
}, MD = {
	darkMode: !0,
	color: $.theme,
	backgroundColor: kD,
	axisPointer: {
		lineStyle: { color: $.border },
		crossStyle: { color: $.borderShade },
		label: { color: $.tertiary }
	},
	legend: {
		textStyle: { color: $.secondary },
		pageTextStyle: { color: $.tertiary }
	},
	textStyle: { color: $.secondary },
	title: {
		textStyle: { color: $.primary },
		subtextStyle: { color: $.quaternary }
	},
	toolbox: {
		iconStyle: { borderColor: $.accent50 },
		feature: { dataView: {
			backgroundColor: kD,
			textColor: $.primary,
			textareaColor: $.background,
			textareaBorderColor: $.border,
			buttonColor: $.accent50,
			buttonTextColor: $.neutral00
		} }
	},
	tooltip: {
		backgroundColor: $.neutral20,
		defaultBorderColor: $.border,
		textStyle: { color: $.tertiary }
	},
	dataZoom: {
		borderColor: $.accent10,
		textStyle: { color: $.tertiary },
		brushStyle: { color: $.backgroundTint },
		handleStyle: {
			color: $.neutral00,
			borderColor: $.accent20
		},
		moveHandleStyle: { color: $.accent40 },
		emphasis: { handleStyle: { borderColor: $.accent50 } },
		dataBackground: {
			lineStyle: { color: $.accent30 },
			areaStyle: { color: $.accent20 }
		},
		selectedDataBackground: {
			lineStyle: { color: $.accent50 },
			areaStyle: { color: $.accent30 }
		}
	},
	visualMap: {
		textStyle: { color: $.secondary },
		handleStyle: { borderColor: $.neutral30 }
	},
	timeline: {
		lineStyle: { color: $.accent10 },
		label: { color: $.tertiary },
		controlStyle: {
			color: $.accent30,
			borderColor: $.accent30
		}
	},
	calendar: {
		itemStyle: {
			color: $.neutral00,
			borderColor: $.neutral20
		},
		dayLabel: { color: $.tertiary },
		monthLabel: { color: $.secondary },
		yearLabel: { color: $.secondary }
	},
	matrix: {
		x: jD,
		y: jD,
		backgroundColor: { borderColor: $.axisLine },
		body: { itemStyle: { borderColor: $.borderTint } }
	},
	timeAxis: AD(),
	logAxis: AD(),
	valueAxis: AD(),
	categoryAxis: AD(),
	line: { symbol: "circle" },
	graph: { color: $.theme },
	gauge: {
		title: { color: $.secondary },
		axisLine: { lineStyle: { color: [[1, $.neutral05]] } },
		axisLabel: { color: $.axisLabel },
		detail: { color: $.primary }
	},
	candlestick: { itemStyle: {
		color: "#f64e56",
		color0: "#54ea92",
		borderColor: "#f64e56",
		borderColor0: "#54ea92"
	} },
	funnel: { itemStyle: { borderColor: $.background } },
	radar: function() {
		var e = AD();
		return e.axisName = { color: $.axisLabel }, e.axisLine.lineStyle.color = $.neutral20, e;
	}(),
	treemap: { breadcrumb: {
		itemStyle: {
			color: $.neutral20,
			textStyle: { color: $.secondary }
		},
		emphasis: { itemStyle: { color: $.neutral30 } }
	} },
	sunburst: { itemStyle: { borderColor: $.background } },
	map: {
		itemStyle: {
			borderColor: $.border,
			areaColor: $.neutral10
		},
		label: { color: $.tertiary },
		emphasis: {
			label: { color: $.primary },
			itemStyle: { areaColor: $.highlight }
		},
		select: {
			label: { color: $.primary },
			itemStyle: { areaColor: $.highlight }
		}
	},
	geo: {
		itemStyle: {
			borderColor: $.border,
			areaColor: $.neutral10
		},
		emphasis: {
			label: { color: $.primary },
			itemStyle: { areaColor: $.highlight }
		},
		select: {
			label: { color: $.primary },
			itemStyle: { color: $.highlight }
		}
	}
};
MD.categoryAxis.splitLine.show = !1;
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/util/ECEventProcessor.js
var ND = function() {
	function e() {}
	return e.prototype.normalizeQuery = function(e) {
		var t = {}, n = {}, r = {};
		if (U(e)) {
			var i = Re(e);
			t.mainType = i.main || null, t.subType = i.sub || null;
		} else {
			var a = [
				"Index",
				"Name",
				"Id"
			], o = {
				name: 1,
				dataIndex: 1,
				dataType: 1
			};
			I(e, function(e, i) {
				for (var s = !1, c = 0; c < a.length; c++) {
					var l = a[c], u = i.lastIndexOf(l);
					if (u > 0 && u === i.length - l.length) {
						var d = i.slice(0, u);
						d !== "data" && (t.mainType = d, t[l.toLowerCase()] = e, s = !0);
					}
				}
				o.hasOwnProperty(i) && (n[i] = e, s = !0), s || (r[i] = e);
			});
		}
		return {
			cptQuery: t,
			dataQuery: n,
			otherQuery: r
		};
	}, e.prototype.filter = function(e, t) {
		var n = this.eventInfo;
		if (!n) return !0;
		var r = n.targetEl, i = n.packedEvent, a = n.model, o = n.view;
		if (!a || !o) return !0;
		var s = t.cptQuery, c = t.dataQuery;
		return l(s, a, "mainType") && l(s, a, "subType") && l(s, a, "index", "componentIndex") && l(s, a, "name") && l(s, a, "id") && l(c, i, "name") && l(c, i, "dataIndex") && l(c, i, "dataType") && (!o.filterForExposedEvent || o.filterForExposedEvent(e, t.otherQuery, r, i));
		function l(e, t, n, r) {
			return e[n] == null || t[r || n] === e[n];
		}
	}, e.prototype.afterTrigger = function() {
		this.eventInfo = null;
	}, e;
}(), PD = [
	"symbol",
	"symbolSize",
	"symbolRotate",
	"symbolOffset"
], FD = PD.concat(["symbolKeepAspect"]), ID = {
	createOnAllSeries: !0,
	performRawSeries: !0,
	reset: function(e, t) {
		var n = e.getData();
		if (e.legendIcon && n.setVisual("legendIcon", e.legendIcon), !e.hasSymbolVisual) return;
		for (var r = {}, i = {}, a = !1, o = 0; o < PD.length; o++) {
			var s = PD[o], c = e.get(s);
			H(c) ? (a = !0, i[s] = c) : r[s] = c;
		}
		if (r.symbol = r.symbol || e.defaultSymbol, n.setVisual(j({
			legendIcon: e.legendIcon || r.symbol,
			symbolKeepAspect: e.get("symbolKeepAspect")
		}, r)), t.isSeriesFiltered(e)) return;
		var l = R(i);
		function u(t, n) {
			for (var r = e.getRawValue(n), a = e.getDataParams(n), o = 0; o < l.length; o++) {
				var s = l[o];
				t.setItemVisual(n, s, i[s](r, a));
			}
		}
		return { dataEach: a ? u : null };
	}
}, LD = {
	createOnAllSeries: !0,
	performRawSeries: !0,
	reset: function(e, t) {
		if (!e.hasSymbolVisual || t.isSeriesFiltered(e)) return;
		var n = e.getData();
		function r(e, t) {
			for (var n = e.getItemModel(t), r = 0; r < FD.length; r++) {
				var i = FD[r], a = n.getShallow(i, !0);
				a != null && e.setItemVisual(t, i, a);
			}
		}
		return { dataEach: n.hasItemOption ? r : null };
	}
};
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/visual/helper.js
function RD(e, t, n) {
	switch (n) {
		case "color": return e.getItemVisual(t, "style")[e.getVisual("drawType")];
		case "opacity": return e.getItemVisual(t, "style").opacity;
		case "symbol":
		case "symbolSize":
		case "liftZ": return e.getItemVisual(t, n);
		default:
	}
}
function zD(e, t) {
	switch (t) {
		case "color": return e.getVisual("style")[e.getVisual("drawType")];
		case "opacity": return e.getVisual("style").opacity;
		case "symbol":
		case "symbolSize":
		case "liftZ": return e.getVisual(t);
		default:
	}
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/util/event.js
function BD(e, t, n) {
	for (var r; e && !(t(e) && (r = e, n));) e = e.__hostTarget || e.parent;
	return r;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/core/lifecycle.js
var VD = new hi(), HD = {};
function UD(e, t) {
	HD[e] = t;
}
function WD(e) {
	return HD[e];
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/custom/customSeriesRegister.js
var GD = {};
function KD(e, t) {
	GD[e] = t;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/core/WeakMap.js
var qD = Math.round(Math.random() * 9), JD = typeof Object.defineProperty == "function", YD = function() {
	function e() {
		this._id = "__ec_inner_" + qD++;
	}
	return e.prototype.get = function(e) {
		return this._guard(e)[this._id];
	}, e.prototype.set = function(e, t) {
		var n = this._guard(e);
		return JD ? Object.defineProperty(n, this._id, {
			value: t,
			enumerable: !1,
			configurable: !0
		}) : n[this._id] = t, this;
	}, e.prototype.delete = function(e) {
		return this.has(e) ? (delete this._guard(e)[this._id], !0) : !1;
	}, e.prototype.has = function(e) {
		return !!this._guard(e)[this._id];
	}, e.prototype._guard = function(e) {
		if (e !== Object(e)) throw TypeError("Value of WeakMap is not a non-null object.");
		return e;
	}, e;
}();
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/canvas/helper.js
function XD(e) {
	return isFinite(e);
}
function ZD(e, t, n) {
	var r = t.x == null ? 0 : t.x, i = t.x2 == null ? 1 : t.x2, a = t.y == null ? 0 : t.y, o = t.y2 == null ? 0 : t.y2;
	return t.global || (r = r * n.width + n.x, i = i * n.width + n.x, a = a * n.height + n.y, o = o * n.height + n.y), r = XD(r) ? r : 0, i = XD(i) ? i : 1, a = XD(a) ? a : 0, o = XD(o) ? o : 0, e.createLinearGradient(r, a, i, o);
}
function QD(e, t, n) {
	var r = n.width, i = n.height, a = Math.min(r, i), o = t.x == null ? .5 : t.x, s = t.y == null ? .5 : t.y, c = t.r == null ? .5 : t.r;
	return t.global || (o = o * r + n.x, s = s * i + n.y, c *= a), o = XD(o) ? o : .5, s = XD(s) ? s : .5, c = c >= 0 && XD(c) ? c : .5, e.createRadialGradient(o, s, 0, o, s, c);
}
function $D(e, t, n) {
	for (var r = t.type === "radial" ? QD(e, t, n) : ZD(e, t, n), i = t.colorStops, a = 0; a < i.length; a++) r.addColorStop(i[a].offset, i[a].color);
	return r;
}
function eO(e, t) {
	if (e === t || !e && !t) return !1;
	if (!e || !t || e.length !== t.length) return !0;
	for (var n = 0; n < e.length; n++) if (e[n] !== t[n]) return !0;
	return !1;
}
function tO(e) {
	return parseInt(e, 10);
}
function nO(e, t, n) {
	var r = ["width", "height"][t], i = ["clientWidth", "clientHeight"][t], a = ["paddingLeft", "paddingTop"][t], o = ["paddingRight", "paddingBottom"][t];
	if (n[r] != null && n[r] !== "auto") return parseFloat(n[r]);
	var s = document.defaultView.getComputedStyle(e);
	return (e[i] || tO(s[r]) || tO(e.style[r])) - (tO(s[a]) || 0) - (tO(s[o]) || 0) || 0;
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/canvas/dashStyle.js
function rO(e, t) {
	return !e || e === "solid" || !(t > 0) ? null : e === "dashed" ? [4 * t, 2 * t] : e === "dotted" ? [t] : se(e) ? [e] : V(e) ? e : null;
}
function iO(e) {
	var t = e.style, n = t.lineDash && t.lineWidth > 0 && rO(t.lineDash, t.lineWidth), r = t.lineDashOffset;
	if (n) {
		var i = t.strokeNoScale && e.getLineScale ? e.getLineScale() : 1;
		i && i !== 1 && (n = L(n, function(e) {
			return e / i;
		}), r /= i);
	}
	return [n, r];
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/canvas/graphic.js
var aO = new Ea(!0);
function oO(e) {
	var t = e.stroke;
	return !(t == null || t === "none" || !(e.lineWidth > 0));
}
function sO(e) {
	return typeof e == "string" && e !== "none";
}
function cO(e) {
	var t = e.fill;
	return t != null && t !== "none";
}
function lO(e, t) {
	if (t.fillOpacity != null && t.fillOpacity !== 1) {
		var n = e.globalAlpha;
		e.globalAlpha = t.fillOpacity * t.opacity, e.fill(), e.globalAlpha = n;
	} else e.fill();
}
function uO(e, t) {
	if (t.strokeOpacity != null && t.strokeOpacity !== 1) {
		var n = e.globalAlpha;
		e.globalAlpha = t.strokeOpacity * t.opacity, e.stroke(), e.globalAlpha = n;
	} else e.stroke();
}
function dO(e, t, n) {
	var r = rt(t.image, t.__image, n);
	if (at(r)) {
		var i = e.createPattern(r, t.repeat || "repeat");
		if (typeof DOMMatrix == "function" && i && i.setTransform) {
			var a = new DOMMatrix();
			a.translateSelf(t.x || 0, t.y || 0), a.rotateSelf(0, 0, (t.rotation || 0) * Me), a.scaleSelf(t.scaleX || 1, t.scaleY || 1), i.setTransform(a);
		}
		return i;
	}
}
function fO(e, t, n, r, i) {
	var a, o = oO(n), s = cO(n), c = n.strokePercent, l = c < 1, u = !t.path;
	(!t.silent || l) && u && t.createPathProxy();
	var d = t.path || aO, f = t.__dirty;
	if (!r) {
		var p = n.fill, m = n.stroke, h = s && !!p.colorStops, g = o && !!m.colorStops, _ = s && !!p.image, v = o && !!m.image, y = void 0, b = void 0, x = void 0, S = void 0, C = void 0;
		(h || g) && (C = t.getBoundingRect()), h && (y = f ? $D(e, p, C) : t.__canvasFillGradient, t.__canvasFillGradient = y), g && (b = f ? $D(e, m, C) : t.__canvasStrokeGradient, t.__canvasStrokeGradient = b), _ && (x = f || !t.__canvasFillPattern ? dO(e, p, t) : t.__canvasFillPattern, t.__canvasFillPattern = x), v && (S = f || !t.__canvasStrokePattern ? dO(e, m, t) : t.__canvasStrokePattern, t.__canvasStrokePattern = S), h ? e.fillStyle = y : _ && (x ? e.fillStyle = x : s = !1), g ? e.strokeStyle = b : v && (S ? e.strokeStyle = S : o = !1);
	}
	var w = t.getGlobalScale();
	d.setScale(w[0], w[1], t.segmentIgnoreThreshold);
	var T, E;
	e.setLineDash && n.lineDash && (a = iO(t), T = a[0], E = a[1]);
	var D = !0;
	(u || f & 4) && (d.setDPR(e.dpr), l ? d.setContext(null) : (d.setContext(e), D = !1), d.reset(), t.buildPath(d, t.shape, r), d.toStatic(), t.pathUpdated()), D && d.rebuildPath(e, l ? c : 1), T && (e.setLineDash(T), e.lineDashOffset = E), r ? (i.batchFill = s, i.batchStroke = o) : n.strokeFirst ? (o && uO(e, n), s && lO(e, n)) : (s && lO(e, n), o && uO(e, n)), T && e.setLineDash([]);
}
function pO(e, t, n) {
	var r = t.__image = rt(n.image, t.__image, t, t.onload);
	if (!(!r || !at(r))) {
		var i = n.x || 0, a = n.y || 0, o = t.getWidth(), s = t.getHeight(), c = r.width / r.height;
		if (o == null && s != null ? o = s * c : s == null && o != null ? s = o / c : o == null && s == null && (o = r.width, s = r.height), n.sWidth && n.sHeight) {
			var l = n.sx || 0, u = n.sy || 0;
			e.drawImage(r, l, u, n.sWidth, n.sHeight, i, a, o, s);
		} else if (n.sx && n.sy) {
			var l = n.sx, u = n.sy, d = o - l, f = s - u;
			e.drawImage(r, l, u, d, f, i, a, o, s);
		} else e.drawImage(r, i, a, o, s);
	}
}
function mO(e, t, n) {
	var r, i = n.text;
	if (i != null && (i += ""), i) {
		e.font = n.font || "12px sans-serif", e.textAlign = n.textAlign, e.textBaseline = n.textBaseline;
		var a = void 0, o = void 0;
		e.setLineDash && n.lineDash && (r = iO(t), a = r[0], o = r[1]), a && (e.setLineDash(a), e.lineDashOffset = o), n.strokeFirst ? (oO(n) && e.strokeText(i, n.x, n.y), cO(n) && e.fillText(i, n.x, n.y)) : (cO(n) && e.fillText(i, n.x, n.y), oO(n) && e.strokeText(i, n.x, n.y)), a && e.setLineDash([]);
	}
}
var hO = [
	"shadowBlur",
	"shadowOffsetX",
	"shadowOffsetY"
], gO = [
	["lineCap", "butt"],
	["lineJoin", "miter"],
	["miterLimit", 10]
];
function _O(e, t, n, r, i) {
	var a = !1;
	if (!r && (n ||= {}, t === n)) return !1;
	if (r || t.opacity !== n.opacity) {
		OO(e, i), a = !0;
		var o = Math.max(Math.min(t.opacity, 1), 0);
		e.globalAlpha = isNaN(o) ? Bi.opacity : o;
	}
	(r || t.blend !== n.blend) && (a ||= (OO(e, i), !0), e.globalCompositeOperation = t.blend || Bi.blend);
	for (var s = 0; s < hO.length; s++) {
		var c = hO[s];
		(r || t[c] !== n[c]) && (a ||= (OO(e, i), !0), e[c] = e.dpr * (t[c] || 0));
	}
	return (r || t.shadowColor !== n.shadowColor) && (a ||= (OO(e, i), !0), e.shadowColor = t.shadowColor || Bi.shadowColor), a;
}
function vO(e, t, n, r, i) {
	var a = t.style, o = r ? null : n && n.style || {};
	if (a === o) return !1;
	var s = _O(e, a, o, r, i);
	if ((r || a.fill !== o.fill) && (s ||= (OO(e, i), !0), sO(a.fill) && (e.fillStyle = a.fill)), (r || a.stroke !== o.stroke) && (s ||= (OO(e, i), !0), sO(a.stroke) && (e.strokeStyle = a.stroke)), (r || a.opacity !== o.opacity) && (s ||= (OO(e, i), !0), e.globalAlpha = a.opacity == null ? 1 : a.opacity), t.hasStroke()) {
		var c = a.lineWidth / (a.strokeNoScale && t.getLineScale ? t.getLineScale() : 1);
		e.lineWidth !== c && (s ||= (OO(e, i), !0), e.lineWidth = c);
	}
	for (var l = 0; l < gO.length; l++) {
		var u = gO[l], d = u[0];
		(r || a[d] !== o[d]) && (s ||= (OO(e, i), !0), e[d] = a[d] || u[1]);
	}
	return s;
}
function yO(e, t, n, r, i) {
	return _O(e, t.style, n && n.style, r, i);
}
function bO(e, t) {
	var n = t.transform, r = e.dpr || 1;
	n ? e.setTransform(r * n[0], r * n[1], r * n[2], r * n[3], r * n[4], r * n[5]) : e.setTransform(r, 0, 0, r, 0, 0);
}
function xO(e, t, n) {
	for (var r = !1, i = 0; i < e.length; i++) {
		var a = e[i];
		r ||= a.isZeroArea(), bO(t, a), t.beginPath(), a.buildPath(t, a.shape), t.clip();
	}
	n.allClipped = r;
}
function SO(e, t) {
	return e && t ? e[0] !== t[0] || e[1] !== t[1] || e[2] !== t[2] || e[3] !== t[3] || e[4] !== t[4] || e[5] !== t[5] : !(!e && !t);
}
var CO = 1, wO = 2, TO = 3, EO = 4;
function DO(e) {
	var t = cO(e), n = oO(e);
	return !(e.lineDash || !(t ^ +n) || t && typeof e.fill != "string" || n && typeof e.stroke != "string" || e.strokePercent < 1 || e.strokeOpacity < 1 || e.fillOpacity < 1);
}
function OO(e, t) {
	t.batchFill && (t.batchFill = !1, e.fill()), t.batchStroke && (t.batchStroke = !1, e.stroke());
}
function kO(e, t) {
	var n = {
		inHover: !1,
		viewWidth: 0,
		viewHeight: 0,
		beforeBrushParam: {}
	};
	AO(e, t, n), jO(e, n);
}
function AO(e, t, n) {
	var r = t.transform;
	if (!t.shouldBePainted(n.viewWidth, n.viewHeight, !1, !1)) {
		t.__dirty &= -2, t.__isRendered = !1;
		return;
	}
	var i = t.__clipPaths, a = n.prevElClipPaths, o = t.style, s = !1, c = !1;
	if ((!a || eO(i, a)) && (a && (OO(e, n), e.restore(), c = s = !0, n.prevElClipPaths = null, n.allClipped = !1, n.prevEl = null), i && i.length && (OO(e, n), e.save(), xO(i, e, n), s = !0, n.prevElClipPaths = i)), n.allClipped) {
		t.__dirty &= -2, t.__isRendered = !1;
		return;
	}
	t.beforeBrush && t.beforeBrush(n.beforeBrushParam), t.innerBeforeBrush();
	var l = n.prevEl;
	l || (c = s = !0);
	var u = t instanceof Za && t.autoBatch && DO(o);
	s || SO(r, l.transform) ? (OO(e, n), bO(e, t)) : u || OO(e, n), t instanceof Za ? (n.lastDrawType !== CO && (c = !0, n.lastDrawType = CO), vO(e, t, l, c, n), (!u || !n.batchFill && !n.batchStroke) && e.beginPath(), fO(e, t, o, u, n)) : t instanceof $a ? (n.lastDrawType !== TO && (c = !0, n.lastDrawType = TO), vO(e, t, l, c, n), mO(e, t, o)) : t instanceof ro ? (n.lastDrawType !== wO && (c = !0, n.lastDrawType = wO), yO(e, t, l, c, n), pO(e, t, o)) : t.getTemporalDisplayables && (n.lastDrawType !== EO && (c = !0, n.lastDrawType = EO), MO(e, t, n)), t.innerAfterBrush(), t.afterBrush && (u && OO(e, n), t.afterBrush()), n.prevEl = t, t.__dirty = 0, t.__isRendered = !0;
}
function jO(e, t) {
	OO(e, t), t.prevElClipPaths && e.restore();
}
function MO(e, t, n) {
	var r = t.getDisplayables(), i = t.getTemporalDisplayables();
	e.save();
	var a = {
		prevElClipPaths: null,
		prevEl: null,
		allClipped: !1,
		viewWidth: n.viewWidth,
		viewHeight: n.viewHeight,
		inHover: n.inHover,
		beforeBrushParam: {}
	}, o, s;
	for (o = t.getCursor(), s = r.length; o < s; o++) {
		var c = r[o];
		c.beforeBrush && c.beforeBrush(n.beforeBrushParam), c.innerBeforeBrush(), AO(e, c, a), c.innerAfterBrush(), c.afterBrush && c.afterBrush(), a.prevEl = c;
	}
	jO(e, a);
	for (var l = 0, u = i.length; l < u; l++) {
		var c = i[l];
		c.beforeBrush && c.beforeBrush(n.beforeBrushParam), c.innerBeforeBrush(), AO(e, c, a), c.innerAfterBrush(), c.afterBrush && c.afterBrush(), a.prevEl = c;
	}
	jO(e, a), t.clearTemporalDisplayables(), t.notClear = !0, e.restore();
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/util/decal.js
var NO = new YD(), PO = new et(100), FO = [
	"symbol",
	"symbolSize",
	"symbolKeepAspect",
	"color",
	"backgroundColor",
	"dashArrayX",
	"dashArrayY",
	"maxTileWidth",
	"maxTileHeight"
];
function IO(e, t) {
	if (e === "none") return null;
	var n = t.getDevicePixelRatio(), r = t.getZr(), i = r.painter.type === "svg";
	e.dirty && NO.delete(e);
	var a = NO.get(e);
	if (a) return a;
	var o = M(e, {
		symbol: "rect",
		symbolSize: 1,
		symbolKeepAspect: !0,
		color: "rgba(0, 0, 0, 0.2)",
		backgroundColor: null,
		dashArrayX: 5,
		dashArrayY: 5,
		rotation: 0,
		maxTileWidth: 512,
		maxTileHeight: 512
	});
	o.backgroundColor === "none" && (o.backgroundColor = null);
	var s = { repeat: "repeat" };
	return c(s), s.rotation = o.rotation, s.scaleX = s.scaleY = i ? 1 : 1 / n, NO.set(e, s), e.dirty = !1, s;
	function c(e) {
		for (var t = [n], a = !0, s = 0; s < FO.length; ++s) {
			var c = o[FO[s]];
			if (c != null && !V(c) && !U(c) && !se(c) && typeof c != "boolean") {
				a = !1;
				break;
			}
			t.push(c);
		}
		var l;
		if (a) {
			l = t.join(",") + (i ? "-svg" : "");
			var u = PO.get(l);
			u && (i ? e.svgElement = u : e.image = u);
		}
		var d = RO(o.dashArrayX), f = zO(o.dashArrayY), m = LO(o.symbol), h = BO(d), g = VO(f), _ = !i && p.createCanvas(), v = i && {
			tag: "g",
			attrs: {},
			key: "dcl",
			children: []
		}, y = x(), b;
		_ && (_.width = y.width * n, _.height = y.height * n, b = _.getContext("2d")), S(), a && PO.put(l, _ || v), e.image = _, e.svgElement = v, e.svgWidth = y.width, e.svgHeight = y.height;
		function x() {
			for (var e = 1, t = 0, n = h.length; t < n; ++t) e = ps(e, h[t]);
			for (var r = 1, t = 0, n = m.length; t < n; ++t) r = ps(r, m[t].length);
			e *= r;
			var i = g * h.length * m.length;
			return {
				width: Math.max(1, Math.min(e, o.maxTileWidth)),
				height: Math.max(1, Math.min(i, o.maxTileHeight))
			};
		}
		function S() {
			b && (b.clearRect(0, 0, _.width, _.height), o.backgroundColor && (b.fillStyle = o.backgroundColor, b.fillRect(0, 0, _.width, _.height)));
			for (var e = 0, t = 0; t < f.length; ++t) e += f[t];
			if (e <= 0) return;
			for (var a = -g, s = 0, c = 0, l = 0; a < y.height;) {
				if (s % 2 == 0) {
					for (var u = c / 2 % m.length, p = 0, h = 0, x = 0; p < y.width * 2;) {
						for (var S = 0, t = 0; t < d[l].length; ++t) S += d[l][t];
						if (S <= 0) break;
						if (h % 2 == 0) {
							var C = (1 - o.symbolSize) * .5, w = p + d[l][h] * C, T = a + f[s] * C, E = d[l][h] * o.symbolSize, D = f[s] * o.symbolSize, O = x / 2 % m[u].length;
							k(w, T, E, D, m[u][O]);
						}
						p += d[l][h], ++x, ++h, h === d[l].length && (h = 0);
					}
					++l, l === d.length && (l = 0);
				}
				a += f[s], ++c, ++s, s === f.length && (s = 0);
			}
			function k(e, t, a, s, c) {
				var l = i ? 1 : n, u = Y_(c, e * l, t * l, a * l, s * l, o.color, o.symbolKeepAspect);
				if (i) {
					var d = r.painter.renderOneToVNode(u);
					d && v.children.push(d);
				} else kO(b, u);
			}
		}
	}
}
function LO(e) {
	if (!e || e.length === 0) return [["rect"]];
	if (U(e)) return [[e]];
	for (var t = !0, n = 0; n < e.length; ++n) if (!U(e[n])) {
		t = !1;
		break;
	}
	if (t) return LO([e]);
	for (var r = [], n = 0; n < e.length; ++n) U(e[n]) ? r.push([e[n]]) : r.push(e[n]);
	return r;
}
function RO(e) {
	if (!e || e.length === 0) return [[0, 0]];
	if (se(e)) {
		var t = Math.ceil(e);
		return [[t, t]];
	}
	for (var n = !0, r = 0; r < e.length; ++r) if (!se(e[r])) {
		n = !1;
		break;
	}
	if (n) return RO([e]);
	for (var i = [], r = 0; r < e.length; ++r) if (se(e[r])) {
		var t = Math.ceil(e[r]);
		i.push([t, t]);
	} else {
		var t = L(e[r], function(e) {
			return Math.ceil(e);
		});
		t.length % 2 == 1 ? i.push(t.concat(t)) : i.push(t);
	}
	return i;
}
function zO(e) {
	if (!e || typeof e == "object" && e.length === 0) return [0, 0];
	if (se(e)) {
		var t = Math.ceil(e);
		return [t, t];
	}
	var n = L(e, function(e) {
		return Math.ceil(e);
	});
	return e.length % 2 ? n.concat(n) : n;
}
function BO(e) {
	return L(e, function(e) {
		return VO(e);
	});
}
function VO(e) {
	for (var t = 0, n = 0; n < e.length; ++n) t += e[n];
	return e.length % 2 == 1 ? t * 2 : t;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/visual/decal.js
var HO = vc(UO);
function UO(e, t) {
	e.eachRawSeries(function(n) {
		if (!e.isSeriesFiltered(n)) {
			var r = n.getData();
			r.hasItemVisual() && r.each(function(e) {
				var n = r.getItemVisual(e, "decal");
				if (n) {
					var i = r.ensureUniqueItemVisual(e, "style");
					i.decal = IO(n, t);
				}
			});
			var i = r.getVisual("decal");
			if (i) {
				var a = r.getVisual("style");
				a.decal = IO(i, t);
			}
		}
	});
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/core/echarts.js
var WO = 1, GO = 800, KO = 900, qO = 920, JO = 1e3, YO = 2e3, XO = 5e3, ZO = 1e3, QO = 1100, $O = 2e3, ek = 3e3, tk = 4e3, nk = 4500, rk = 4600, ik = 5e3, ak = 6e3, ok = 7e3, sk = {
	PROCESSOR: {
		SERIES_FILTER: GO,
		AXIS_STATISTICS: qO,
		FILTER: JO,
		STATISTIC: XO,
		STATISTICS: XO
	},
	VISUAL: {
		LAYOUT: ZO,
		PROGRESSIVE_LAYOUT: QO,
		GLOBAL: $O,
		CHART: ek,
		POST_CHART_LAYOUT: rk,
		COMPONENT: tk,
		BRUSH: ik,
		CHART_ITEM: nk,
		ARIA: ak,
		DECAL: ok
	}
}, ck = "__flagInMainProcess", lk = "__mainProcessVersion", uk = "__pendingUpdate", dk = "__needsUpdateStatus", fk = /^[a-zA-Z0-9_]+$/, pk = "__connectUpdateStatus", mk = 0, hk = 1, gk = 2;
function _k(e) {
	return function() {
		var t = [...arguments];
		if (this.isDisposed()) {
			this.id;
			return;
		}
		return yk(this, e, t);
	};
}
function vk(e) {
	return function() {
		var t = [...arguments];
		return yk(this, e, t);
	};
}
function yk(e, t, n) {
	return n[0] = n[0] && n[0].toLowerCase(), hi.prototype[t].apply(e, n);
}
var bk = function(e) {
	o(t, e);
	function t() {
		return e !== null && e.apply(this, arguments) || this;
	}
	return t;
}(hi), xk = bk.prototype;
xk.on = vk("on"), xk.off = vk("off");
var Sk, Ck, wk, Tk, Ek, Dk, Ok, kk, Ak, jk, Mk, Nk, Pk, Fk, Ik, Lk, Rk, zk, Bk, Vk = function(e) {
	o(t, e);
	function t(t, n, r) {
		var i = e.call(this, new ND()) || this;
		i._chartsViews = [], i._chartsMap = {}, i._componentsViews = [], i._componentsMap = {}, i._pendingActions = [], r ||= {}, i.__v_skip = !0, i._dom = t;
		var a = "canvas", o = "auto", s = !1;
		i[lk] = 1, r.ssr;
		var c = i._zr = rE(t, {
			renderer: r.renderer || a,
			devicePixelRatio: r.devicePixelRatio,
			width: r.width,
			height: r.height,
			ssr: r.ssr,
			useDirtyRect: G(r.useDirtyRect, s),
			useCoarsePointer: G(r.useCoarsePointer, o),
			pointerSize: r.pointerSize
		});
		i._ssr = r.ssr, i._throttledZrFlush = bC(z(c.flush, c), 17), i._updateTheme(n), i._locale = _h(r.locale || hh), i._coordSysMgr = new Cm();
		var l = i._api = Ik(i);
		function u(e, t) {
			return e.__prio - t.__prio;
		}
		return TT(Yk, u), TT(qk, u), i._scheduler = new mD(i, l, qk, Yk), i._messageCenter = new bk(), i._initEvents(), i.resize = z(i.resize, i), c.animation.on("frame", i._onframe, i), jk(c, i), Mk(c, i), xe(i), i;
	}
	return t.prototype._onframe = function() {
		if (!this._disposed) {
			var e = this._scheduler, t = this._model, n = this._api;
			if (zk(this), this[uk]) {
				var r = this[uk].silent;
				this[ck] = !0, Bk(this);
				try {
					Sk(this), Tk.update.call(this, null, this[uk].updateParams);
				} catch (e) {
					throw this[ck] = !1, this[uk] = null, e;
				}
				this._zr.flush(), this[ck] = !1, this[uk] = null, kk.call(this, r), Ak.call(this, r);
			} else if (e.unfinished) {
				var i = WO;
				do {
					e.unfinished = !1;
					var a = p.getTime();
					e.performSeriesTasks(t), e.performDataProcessorTasks(t), Dk(this, t), e.performVisualTasks(t), Fk(this, this._model, n, "remain", {}), i -= p.getTime() - a;
				} while (i > 0 && e.unfinished);
				e.unfinished || this._zr.flush();
			}
		}
	}, t.prototype.getDom = function() {
		return this._dom;
	}, t.prototype.getId = function() {
		return this.id;
	}, t.prototype.getZr = function() {
		return this._zr;
	}, t.prototype.isSSR = function() {
		return this._ssr;
	}, t.prototype.setOption = function(e, t, n) {
		if (!this[ck]) {
			if (this._disposed) {
				this.id;
				return;
			}
			var r, i, a;
			if (W(t) && (n = t.lazyUpdate, r = t.silent, i = t.replaceMerge, a = t.transition, t = t.notMerge), this[ck] = !0, Bk(this), !this._model || t) {
				var o = new wE(this._api), s = this._theme, c = this._model = new _E();
				c.scheduler = this._scheduler, c.ssr = this._ssr, c.init(null, null, null, s, this._locale, o);
			}
			this._model.setOption(e, { replaceMerge: i }, Jk);
			var l = {
				seriesTransition: a,
				optionChanged: !0
			};
			if (n) this[uk] = {
				silent: r,
				updateParams: l
			}, this[ck] = !1, this.getZr().wakeUp();
			else {
				try {
					Sk(this), Tk.update.call(this, null, l);
				} catch (e) {
					throw this[uk] = null, this[ck] = !1, e;
				}
				this._ssr || this._zr.flush(), this[uk] = null, this[ck] = !1, kk.call(this, r), Ak.call(this, r);
			}
		}
	}, t.prototype.setTheme = function(e, t) {
		if (!this[ck]) {
			if (this._disposed) {
				this.id;
				return;
			}
			var n = this._model;
			if (n) {
				var r = t && t.silent, i = null;
				this[uk] && (r ??= this[uk].silent, i = this[uk].updateParams, this[uk] = null), this[ck] = !0, Bk(this);
				try {
					this._updateTheme(e), n.setTheme(this._theme), Sk(this), Tk.update.call(this, { type: "setTheme" }, i);
				} catch (e) {
					throw this[ck] = !1, e;
				}
				this[ck] = !1, kk.call(this, r), Ak.call(this, r);
			}
		}
	}, t.prototype._updateTheme = function(e) {
		U(e) && (e = Xk[e]), e && (e = k(e), e && QE(e, !0), this._theme = e);
	}, t.prototype.getModel = function() {
		return this._model;
	}, t.prototype.getOption = function() {
		return this._model && this._model.getOption();
	}, t.prototype.getWidth = function() {
		return this._zr.getWidth();
	}, t.prototype.getHeight = function() {
		return this._zr.getHeight();
	}, t.prototype.getDevicePixelRatio = function() {
		return this._zr.painter.dpr || q.hasGlobalWindow && window.devicePixelRatio || 1;
	}, t.prototype.getRenderedCanvas = function(e) {
		return this.renderToCanvas(e);
	}, t.prototype.renderToCanvas = function(e) {
		return e ||= {}, this._zr.painter.getRenderedCanvas({
			backgroundColor: e.backgroundColor || this._model.get("backgroundColor"),
			pixelRatio: e.pixelRatio || this.getDevicePixelRatio()
		});
	}, t.prototype.renderToSVGString = function(e) {
		return e ||= {}, this._zr.painter.renderToString({ useViewBox: e.useViewBox });
	}, t.prototype.getSvgDataURL = function() {
		var e = this._zr;
		return I(e.storage.getDisplayList(), function(e) {
			e.stopAnimation(null, !0);
		}), e.painter.toDataURL();
	}, t.prototype.getDataURL = function(e) {
		if (this._disposed) {
			this.id;
			return;
		}
		e ||= {};
		var t = e.excludeComponents, n = this._model, r = [], i = this;
		I(t, function(e) {
			n.eachComponent({ mainType: e }, function(e) {
				var t = i._componentsMap[e.__viewId];
				t.group.ignore || (r.push(t), t.group.ignore = !0);
			});
		});
		var a = this._zr.painter.getType() === "svg" ? this.getSvgDataURL() : this.renderToCanvas(e).toDataURL("image/" + (e && e.type || "png"));
		return I(r, function(e) {
			e.group.ignore = !1;
		}), a;
	}, t.prototype.getConnectedDataURL = function(e) {
		if (this._disposed) {
			this.id;
			return;
		}
		var t = e.type === "svg", n = this.group, r = Math.min, i = Math.max, a = Infinity;
		if ($k[n]) {
			var o = a, s = a, c = -a, l = -a, u = [], d = e && e.pixelRatio || this.getDevicePixelRatio();
			I(Qk, function(a, d) {
				if (a.group === n) {
					var f = t ? a.getZr().painter.getSvgDom().innerHTML : a.renderToCanvas(k(e)), p = a.getDom().getBoundingClientRect();
					o = r(p.left, o), s = r(p.top, s), c = i(p.right, c), l = i(p.bottom, l), u.push({
						dom: f,
						left: p.left,
						top: p.top
					});
				}
			}), o *= d, s *= d, c *= d, l *= d;
			var f = c - o, m = l - s, h = p.createCanvas(), g = rE(h, { renderer: t ? "svg" : "canvas" });
			if (g.resize({
				width: f,
				height: m
			}), t) {
				var _ = "";
				return I(u, function(e) {
					var t = e.left - o, n = e.top - s;
					_ += "<g transform=\"translate(" + t + "," + n + ")\">" + e.dom + "</g>";
				}), g.painter.getSvgRoot().innerHTML = _, e.connectedBackgroundColor && g.painter.setBackgroundColor(e.connectedBackgroundColor), g.refreshImmediately(), g.painter.toDataURL();
			} else return e.connectedBackgroundColor && g.add(new fo({
				shape: {
					x: 0,
					y: 0,
					width: f,
					height: m
				},
				style: { fill: e.connectedBackgroundColor }
			})), I(u, function(e) {
				var t = new ro({ style: {
					x: e.left * d - o,
					y: e.top * d - s,
					image: e.dom
				} });
				g.add(t);
			}), g.refreshImmediately(), h.toDataURL("image/" + (e && e.type || "png"));
		} else return this.getDataURL(e);
	}, t.prototype.convertToPixel = function(e, t, n) {
		return Ek(this, "convertToPixel", e, t, n);
	}, t.prototype.convertToLayout = function(e, t, n) {
		return Ek(this, "convertToLayout", e, t, n);
	}, t.prototype.convertFromPixel = function(e, t, n) {
		return Ek(this, "convertFromPixel", e, t, n);
	}, t.prototype.containPixel = function(e, t) {
		if (this._disposed) {
			this.id;
			return;
		}
		var n = this._model, r;
		return I(Ks(n, e), function(e, n) {
			n.indexOf("Models") >= 0 && I(e, function(e) {
				var i = e.coordinateSystem;
				if (i && i.containPoint) r ||= !!i.containPoint(t);
				else if (n === "seriesModels") {
					var a = this._chartsMap[e.__viewId];
					a && a.containPoint && (r ||= a.containPoint(t, e));
				}
			}, this);
		}, this), !!r;
	}, t.prototype.getVisual = function(e, t) {
		var n = this._model, r = Ks(n, e, { defaultMainType: "series" }), i = r.seriesModel.getData(), a = r.hasOwnProperty("dataIndexInside") ? r.dataIndexInside : r.hasOwnProperty("dataIndex") ? i.indexOfRawIndex(r.dataIndex) : null;
		return a == null ? zD(i, t) : RD(i, a, t);
	}, t.prototype.getViewOfComponentModel = function(e) {
		return this._componentsMap[e.__viewId];
	}, t.prototype.getViewOfSeriesModel = function(e) {
		return this._chartsMap[e.__viewId];
	}, t.prototype._initEvents = function() {
		var e = this;
		I(Uk, function(t) {
			var n = function(n) {
				var r = e.getModel(), i = n.target, a;
				if (t === "globalout" ? a = {} : i && BD(i, function(e) {
					var t = yc(e);
					if (t && t.dataIndex != null) {
						var n = t.dataModel || r.getSeriesByIndex(t.seriesIndex);
						return a = n && n.getDataParams(t.dataIndex, t.dataType, i) || {}, !0;
					} else if (t.eventData) return a = j({}, t.eventData), !0;
				}, !0), a) {
					var o = a.componentType, s = a.componentIndex;
					(o === "markLine" || o === "markPoint" || o === "markArea") && (o = "series", s = a.seriesIndex);
					var c = o && s != null && r.getComponent(o, s), l = c && e[c.mainType === "series" ? "_chartsMap" : "_componentsMap"][c.__viewId];
					a.event = n, a.type = t, e._$eventProcessor.eventInfo = {
						targetEl: i,
						packedEvent: a,
						model: c,
						view: l
					}, e.trigger(t, a);
				}
			};
			n.zrEventfulCallAtLast = !0, e._zr.on(t, n, e);
		});
		var t = this._messageCenter;
		I(Kk, function(n, r) {
			t.on(r, function(t) {
				e.trigger(r, t);
			});
		}), iw(t, this, this._api);
	}, t.prototype.isDisposed = function() {
		return this._disposed;
	}, t.prototype.clear = function() {
		if (this._disposed) {
			this.id;
			return;
		}
		this.setOption({ series: [] }, !0);
	}, t.prototype.dispose = function() {
		if (this._disposed) {
			this.id;
			return;
		}
		this._disposed = !0, this.getDom() && Zs(this.getDom(), tA, "");
		var e = this, t = e._api, n = e._model;
		I(e._componentsViews, function(e) {
			e.dispose(n, t);
		}), I(e._chartsViews, function(e) {
			e.dispose(n, t);
		}), e._zr.dispose(), e._dom = e._model = e._chartsMap = e._componentsMap = e._chartsViews = e._componentsViews = e._scheduler = e._api = e._zr = e._throttledZrFlush = e._theme = e._coordSysMgr = e._messageCenter = null, delete Qk[e.id];
	}, t.prototype.resize = function(e) {
		if (!this[ck]) {
			if (this._disposed) {
				this.id;
				return;
			}
			this._zr.resize(e);
			var t = this._model;
			if (this._loadingFX && this._loadingFX.resize(), t) {
				var n = t.resetOption("media"), r = e && e.silent;
				this[uk] && (r ??= this[uk].silent, n = !0, this[uk] = null), this[ck] = !0, Bk(this);
				try {
					n && Sk(this), Tk.update.call(this, {
						type: "resize",
						animation: j({ duration: 0 }, e && e.animation)
					});
				} catch (e) {
					throw this[ck] = !1, e;
				}
				this[ck] = !1, kk.call(this, r), Ak.call(this, r);
			}
		}
	}, t.prototype.showLoading = function(e, t) {
		if (this._disposed) {
			this.id;
			return;
		}
		if (W(e) && (t = e, e = ""), e ||= "default", this.hideLoading(), Zk[e]) {
			var n = Zk[e](this._api, t), r = this._zr;
			this._loadingFX = n, r.add(n);
		}
	}, t.prototype.hideLoading = function() {
		if (this._disposed) {
			this.id;
			return;
		}
		this._loadingFX && this._zr.remove(this._loadingFX), this._loadingFX = null;
	}, t.prototype.makeActionFromEvent = function(e) {
		var t = j({}, e);
		return t.type = Gk[e.type], t;
	}, t.prototype.dispatchAction = function(e, t) {
		if (this._disposed) {
			this.id;
			return;
		}
		if (W(t) || (t = { silent: !!t }), Wk[e.type] && this._model) {
			if (this[ck]) {
				this._pendingActions.push(e);
				return;
			}
			var n = t.silent;
			Ok.call(this, e, n);
			var r = t.flush;
			r ? this._zr.flush() : r !== !1 && q.browser.weChat && this._throttledZrFlush(), kk.call(this, n), Ak.call(this, n);
		}
	}, t.prototype.updateLabelLayout = function() {
		VD.trigger("series:layoutlabels", this._model, this._api, { updatedSeries: [] });
	}, t.prototype.appendData = function(e) {
		if (this._disposed) {
			this.id;
			return;
		}
		var t = e.seriesIndex;
		this.getModel().getSeriesByIndex(t).appendData(e), this._scheduler.unfinished = !0, this.getZr().wakeUp();
	}, t.internalField = function() {
		Sk = function(e) {
			Gb(e._model);
			var t = e._scheduler;
			t.restorePipelines(e._zr, e._model), t.prepareStageTasks(), Ck(e, !0), Ck(e, !1), t.plan();
		}, Ck = function(e, t) {
			for (var n = e._model, r = e._scheduler, i = t ? e._componentsViews : e._chartsViews, a = t ? e._componentsMap : e._chartsMap, o = e._zr, s = e._api, c = 0; c < i.length; c++) i[c].__alive = !1;
			t ? n.eachComponent(function(e, t) {
				e !== "series" && l(t);
			}) : n.eachSeries(l);
			function l(e) {
				var c = e.__requireNewView;
				e.__requireNewView = !1;
				var l = "_ec_" + e.id + "_" + e.type, u = !c && a[l];
				if (!u) {
					var d = Re(e.type);
					u = new (t ? nD.getClass(d.main, d.sub) : Ov.getClass(d.sub))(), u.init(n, s), a[l] = u, i.push(u), o.add(u.group);
				}
				e.__viewId = u.__id = l, u.__alive = !0, u.__model = e, u.group.__ecComponentInfo = {
					mainType: e.mainType,
					index: e.componentIndex
				}, !t && r.prepareView(u, e, n, s);
			}
			for (var c = 0; c < i.length;) {
				var u = i[c];
				u.__alive ? c++ : (!t && u.renderTask.dispose(), o.remove(u.group), u.dispose(n, s), i.splice(c, 1), a[u.__id] === u && delete a[u.__id], u.__id = u.group.__ecComponentInfo = null);
			}
		}, wk = function(e, t, n, r, i) {
			var a = e._model;
			if (a.setUpdatePayload(n), !r) {
				I([].concat(e._componentsViews, e._chartsViews), l);
				return;
			}
			var o = Xs(n, r, i), s = n.excludeSeriesId, c;
			s != null && (c = K(), I(ws(s), function(e) {
				var t = Rs(e, null);
				t != null && c.set(t, !0);
			})), a && a.eachComponent(o, function(t) {
				if (!(c && c.get(t.id) != null)) if (Ll(n)) if (t instanceof P_) n.type === "highlight" && !n.notBlur && !t.get(["emphasis", "disabled"]) && yl(t, n, e._api);
				else {
					var r = bl(t.mainType, t.componentIndex, n.name, e._api), i = r.focusSelf, a = r.dispatchers;
					n.type === "highlight" && i && !n.notBlur && vl(t.mainType, t.componentIndex, e._api), a && I(a, function(e) {
						n.type === "highlight" ? ll(e) : ul(e);
					});
				}
				else Il(n) && t instanceof P_ && (Cl(t, n, e._api), wl(t), Rk(e));
			}, e), a && a.eachComponent(o, function(t) {
				c && c.get(t.id) != null || l(e[r === "series" ? "_chartsMap" : "_componentsMap"][t.__viewId]);
			}, e);
			function l(r) {
				r && r.__alive && r[t] && r[t](r.__model, a, e._api, n);
			}
		}, Tk = {
			prepareAndUpdate: function(e) {
				Sk(this), Tk.update.call(this, e, e && { optionChanged: e.newOption != null });
			},
			update: function(e, n) {
				var r = this._model, i = this._api, a = this._zr, o = this._coordSysMgr, s = this._scheduler;
				if (r) {
					Kb(r), r.setUpdatePayload(e), s.restoreData(r, e), s.performSeriesTasks(r), o.create(r, i), VD.trigger("coordsys:aftercreate", r, i), s.performDataProcessorTasks(r, e), Dk(this, r), o.update(r, i), t(r), s.performVisualTasks(r, e);
					var c = r.get("backgroundColor") || "transparent";
					a.setBackgroundColor(c);
					var l = r.get("darkMode");
					l != null && l !== "auto" && a.setDarkMode(l), Nk(this, r, i, e, n), VD.trigger("afterupdate", r, i);
				}
			},
			updateTransform: function(e) {
				var t = this, n = t._model, r = t._api;
				if (n) {
					n.setUpdatePayload(e);
					var i = [];
					n.eachComponent(function(a, o) {
						if (a !== "series") {
							var s = t.getViewOfComponentModel(o);
							if (s && s.__alive) if (s.updateTransform) {
								var c = s.updateTransform(o, n, r, e);
								c && c.update && i.push(s);
							} else i.push(s);
						}
					});
					var a = K();
					n.eachSeries(function(i) {
						var o = t._chartsMap[i.__viewId], s = i.pipelineContext;
						if (o.updateTransform && !s.progressiveRender) {
							var c = o.updateTransform(i, n, r, e);
							c && c.update && a.set(i.uid, 1);
						} else a.set(i.uid, 1);
					}), t._scheduler.performVisualTasks(n, e, {
						setDirty: !0,
						dirtyMap: a
					}), Fk(t, n, r, e, {}, a), VD.trigger("afterupdate", n, r);
				}
			},
			updateView: function(e) {
				var n = this._model;
				n && (n.setUpdatePayload(e), Ov.markUpdateMethod(e, "updateView"), t(n), this._scheduler.performVisualTasks(n, e, { setDirty: !0 }), Nk(this, n, this._api, e, {}), VD.trigger("afterupdate", n, this._api));
			},
			updateVisual: function(e) {
				var n = this, r = this._model;
				r && (r.setUpdatePayload(e), r.eachSeries(function(e) {
					e.getData().clearAllVisual();
				}), Ov.markUpdateMethod(e, "updateVisual"), t(r), this._scheduler.performVisualTasks(r, e, {
					visualType: "visual",
					setDirty: !0
				}), r.eachComponent(function(t, i) {
					if (t !== "series") {
						var a = n.getViewOfComponentModel(i);
						a && a.__alive && a.updateVisual(i, r, n._api, e);
					}
				}), r.eachSeries(function(t) {
					n._chartsMap[t.__viewId].updateVisual(t, r, n._api, e);
				}), VD.trigger("afterupdate", r, this._api));
			},
			updateLayout: function(e) {
				Tk.update.call(this, e);
			}
		};
		function e(e, t, n, r, i) {
			if (e._disposed) {
				e.id;
				return;
			}
			for (var a = e._model, o = e._coordSysMgr.getCoordinateSystems(), s, c = Ks(a, n), l = 0; l < o.length; l++) {
				var u = o[l];
				if (u[t] && (s = u[t](a, c, r, i)) != null) return s;
			}
		}
		Ek = e, Dk = function(e, t) {
			var n = e._chartsMap, r = e._scheduler;
			t.eachSeries(function(e) {
				r.updateStreamModes(e, n[e.__viewId]);
			});
		}, Ok = function(e, t) {
			var n = this, r = this.getModel(), i = e.type, a = e.escapeConnect, o = Wk[i], s = (o.update || "update").split(":"), c = s.pop(), l = s[0] != null && Re(s[0]);
			this[ck] = !0, Bk(this);
			var u = [e], d = !1;
			e.batch && (d = !0, u = L(e.batch, function(t) {
				return t = M(j({}, t), e), t.batch = null, t;
			}));
			var f = [], p, m = [], h = o.nonRefinedEventType, g = Il(e), _ = Ll(e);
			if (_ && gl(this._api), I(u, function(t) {
				var i = o.action(t, r, n._api);
				if (o.refineEvent ? m.push(i) : p = i, p ||= j({}, t), p.type = h, f.push(p), _) {
					var a = qs(e), s = a.queryOptionMap, u = a.mainTypeSpecified ? s.keys()[0] : "series";
					wk(n, c, t, u), Rk(n);
				} else g ? (wk(n, c, t, "series"), Rk(n)) : l && wk(n, c, t, l.main, l.sub);
			}), c !== "none" && !_ && !g && !l) try {
				this[uk] ? (Sk(this), Tk.update.call(this, e), this[uk] = null) : Tk[c].call(this, e);
			} catch (e) {
				throw this[ck] = !1, e;
			}
			if (p = d ? {
				type: h,
				escapeConnect: a,
				batch: f
			} : f[0], this[ck] = !1, !t) {
				var v = void 0;
				if (o.refineEvent) {
					var y = o.refineEvent(m, e, r, this._api).eventContent;
					ve(W(y)), v = M({ type: o.refinedEventType }, y), v.fromAction = e.type, v.fromActionPayload = e, v.escapeConnect = !0;
				}
				var b = this._messageCenter;
				b.trigger(p.type, p), v && b.trigger(v.type, v);
			}
		}, kk = function(e) {
			for (var t = this._pendingActions; t.length;) {
				var n = t.shift();
				Ok.call(this, n, e);
			}
		}, Ak = function(e) {
			!e && this.trigger("updated");
		}, jk = function(e, t) {
			e.on("rendered", function(n) {
				t.trigger("rendered", n), e.animation.isFinished() && !t[uk] && !t._scheduler.unfinished && !t._pendingActions.length ? t.trigger("finished") : e.refresh();
			});
		}, Mk = function(e, t) {
			e.on("mouseover", function(e) {
				var n = e.target, r = BD(n, Pl);
				r && (xl(r, e, t._api), Rk(t));
			}).on("mouseout", function(e) {
				var n = e.target, r = BD(n, Pl);
				r && (Sl(r, e, t._api), Rk(t));
			}).on("click", function(e) {
				var n = e.target, r = BD(n, function(e) {
					return yc(e).dataIndex != null;
				}, !0);
				if (r) {
					var i = r.selected ? "unselect" : "select", a = yc(r);
					t._api.dispatchAction({
						type: i,
						dataType: a.dataType,
						dataIndexInside: a.dataIndex,
						seriesIndex: a.seriesIndex,
						isFromClick: !0
					});
				}
			});
		};
		function t(e) {
			e.clearColorPalette(), e.eachSeries(function(e) {
				e.clearColorPalette();
			});
		}
		function n(e) {
			var t = [], n = [], r = !1;
			if (e.eachComponent(function(e, i) {
				var a = i.get("zlevel") || 0, o = i.get("z") || 0, s = i.getZLevelKey();
				r ||= !!s, (e === "series" ? n : t).push({
					zlevel: a,
					z: o,
					idx: i.componentIndex,
					type: e,
					key: s
				});
			}), r) {
				var i = t.concat(n), a, o;
				TT(i, function(e, t) {
					return e.zlevel === t.zlevel ? e.z - t.z : e.zlevel - t.zlevel;
				}), I(i, function(t) {
					var n = e.getComponent(t.type, t.idx), r = t.zlevel, i = t.key;
					a != null && (r = Math.max(a, r)), i ? (r === a && i !== o && r++, o = i) : o &&= (r === a && r++, ""), a = r, n.setZLevel(r);
				});
			}
		}
		Nk = function(e, t, r, i, a) {
			n(t), Pk(e, t, r, i, a), I(e._chartsViews, function(e) {
				e.__alive = !1;
			}), Fk(e, t, r, i, a), I(e._chartsViews, function(e) {
				e.__alive || e.remove(t, r);
			});
		}, Pk = function(e, t, n, r, i, a) {
			I(a || e._componentsViews, function(e) {
				var i = e.__model;
				c(i, e), e.render(i, t, n, r), s(i, e), l(i, e);
			});
		}, Fk = function(e, t, n, r, o, u) {
			var d = e._scheduler;
			o = j(o || {}, { updatedSeries: t.getSeries() }), VD.trigger("series:beforeupdate", t, n, o);
			var f = !1;
			t.eachSeries(function(t) {
				var n = e._chartsMap[t.__viewId];
				n.__alive = !0;
				var i = n.renderTask;
				d.updatePayload(i, r), c(t, n), u && u.get(t.uid) && i.dirty(), i.perform(d.getPerformArgs(i)) && (f = !0), n.group.silent = !!t.get("silent"), a(t, n), wl(t);
			}), d.unfinished = f || d.unfinished, VD.trigger("series:layoutlabels", t, n, o), VD.trigger("series:transition", t, n, o), t.eachSeries(function(t) {
				var n = e._chartsMap[t.__viewId];
				s(t, n), l(t, n);
			}), i(e, t), VD.trigger("series:afterupdate", t, n, o);
		}, Rk = function(e) {
			e[dk] = !0, e.getZr().wakeUp();
		}, Bk = function(e) {
			e[lk] = (e[lk] + 1) % 1e6;
		}, zk = function(e) {
			e[dk] && (e.getZr().storage.traverse(function(e) {
				fd(e) || r(e);
			}), e[dk] = !1);
		};
		function r(e) {
			for (var t = [], n = e.currentStates, r = 0; r < n.length; r++) {
				var i = n[r];
				i === "emphasis" || i === "blur" || i === "select" || t.push(i);
			}
			e.selected && e.states.select && t.push("select"), e.hoverState === 2 && e.states.emphasis ? t.push("emphasis") : e.hoverState === 1 && e.states.blur && t.push("blur"), e.useStates(t);
		}
		function i(e, t) {
			var n = e._zr;
			if (n.painter.type === "canvas") {
				var r = n.storage, i = 0;
				r.traverse(function(e) {
					e.isGroup || i++;
				});
				var a = i > G(t.get("hoverLayerThreshold"), lE.hoverLayerThreshold) && !q.node && !q.worker;
				(e._usingTHL || a) && (t.eachSeries(function(t) {
					if (!t.preventUsingHoverLayer) {
						var n = e._chartsMap[t.__viewId];
						n.__alive && n.eachRendered(function(e) {
							var t = e.states.emphasis;
							t && t.hoverLayer !== 2 && (t.hoverLayer = +!!a);
						});
					}
				}), e._usingTHL = a);
			}
		}
		function a(e, t) {
			var n = e.get("blendMode") || null;
			t.eachRendered(function(e) {
				e.isGroup || (e.style.blend = n);
			});
		}
		function s(e, t) {
			if (!e.preventAutoZ) {
				var n = rf(e);
				t.eachRendered(function(e) {
					return of(e, n.z, n.zlevel), !0;
				});
			}
		}
		function c(e, t) {
			t.eachRendered(function(e) {
				if (!fd(e)) {
					var t = e.getTextContent(), n = e.getTextGuideLine();
					e.stateTransition &&= null, t && t.stateTransition && (t.stateTransition = null), n && n.stateTransition && (n.stateTransition = null), e.hasState() ? (e.prevStates = e.currentStates, e.clearStates()) : e.prevStates &&= null;
				}
			});
		}
		function l(e, t) {
			var n = e.getModel("stateAnimation"), i = e.isAnimationEnabled(), a = n.get("duration"), o = a > 0 ? {
				duration: a,
				delay: n.get("delay"),
				easing: n.get("easing")
			} : null;
			t.eachRendered(function(e) {
				if (e.states && e.states.emphasis) {
					if (fd(e)) return;
					if (e instanceof Za && Rl(e), e.__dirty) {
						var t = e.prevStates;
						t && e.useStates(t);
					}
					if (i) {
						e.stateTransition = o;
						var n = e.getTextContent(), a = e.getTextGuideLine();
						n && (n.stateTransition = o), a && (a.stateTransition = o);
					}
					e.__dirty && r(e);
				}
			});
		}
		Ik = function(e) {
			return new (function(t) {
				o(n, t);
				function n() {
					return t !== null && t.apply(this, arguments) || this;
				}
				return n.prototype.getCoordinateSystems = function() {
					return e._coordSysMgr.getCoordinateSystems();
				}, n.prototype.getComponentByElement = function(t) {
					for (; t;) {
						var n = t.__ecComponentInfo;
						if (n != null) return e._model.getComponent(n.mainType, n.index);
						t = t.parent;
					}
				}, n.prototype.enterEmphasis = function(t, n) {
					ll(t, n), Rk(e);
				}, n.prototype.leaveEmphasis = function(t, n) {
					ul(t, n), Rk(e);
				}, n.prototype.enterBlur = function(t) {
					dl(t), Rk(e);
				}, n.prototype.leaveBlur = function(t) {
					fl(t), Rk(e);
				}, n.prototype.enterSelect = function(t) {
					pl(t), Rk(e);
				}, n.prototype.leaveSelect = function(t) {
					ml(t), Rk(e);
				}, n.prototype.getModel = function() {
					return e.getModel();
				}, n.prototype.getViewOfComponentModel = function(t) {
					return e.getViewOfComponentModel(t);
				}, n.prototype.getViewOfSeriesModel = function(t) {
					return e.getViewOfSeriesModel(t);
				}, n.prototype.getECUpdateCycleVersion = function() {
					return e[lk];
				}, n.prototype.usingTHL = function() {
					return e._usingTHL;
				}, n;
			}(Ac))(e);
		}, Lk = function(e) {
			function t(e, t) {
				for (var n = 0; n < e.length; n++) {
					var r = e[n];
					r[pk] = t;
				}
			}
			I(Gk, function(n, r) {
				e._messageCenter.on(r, function(n) {
					if ($k[e.group] && e[pk] !== mk) {
						if (n && n.escapeConnect) return;
						var r = e.makeActionFromEvent(n), i = [];
						I(Qk, function(t) {
							t !== e && t.group === e.group && i.push(t);
						}), t(i, mk), I(i, function(e) {
							e[pk] !== hk && e.dispatchAction(r);
						}), t(i, gk);
					}
				});
			});
		};
	}(), t;
}(hi), Hk = Vk.prototype;
Hk.on = _k("on"), Hk.off = _k("off"), Hk.one = function(e, t, n) {
	var r = this;
	function i() {
		var n = [...arguments];
		t && t.apply && t.apply(this, n), r.off(e, i);
	}
	this.on.call(this, e, i, n);
};
var Uk = [
	"click",
	"dblclick",
	"mouseover",
	"mouseout",
	"mousemove",
	"mousedown",
	"mouseup",
	"globalout",
	"contextmenu"
], Wk = {}, Gk = {}, Kk = {}, qk = [], Jk = [], Yk = [], Xk = {}, Zk = {}, Qk = {}, $k = {}, eA = /* @__PURE__ */ new Date() - 0;
/* @__PURE__ */ new Date() - 0;
var tA = "_echarts_instance_";
function nA(e, t, n) {
	var r = !(n && n.ssr);
	if (r) {
		var i = rA(e);
		if (i) return i;
	}
	var a = new Vk(e, t, n);
	return a.id = "ec_" + eA++, Qk[a.id] = a, r && Zs(e, tA, a.id), Lk(a), VD.trigger("afterinit", a), a;
}
function rA(e) {
	return Qk[Qs(e, tA)];
}
function iA(e, t) {
	Xk[e] = t;
}
function aA(e) {
	N(Jk, e) < 0 && Jk.push(e);
}
function oA(e, t) {
	hA(qk, e, t, YO);
}
function sA(e) {
	lA("afterinit", e);
}
function cA(e) {
	lA("afterupdate", e);
}
function lA(e, t) {
	VD.on(e, t);
}
function uA(e, t, n) {
	var r, i, a, o, s;
	H(t) && (n = t, t = ""), W(e) ? (r = e.type, i = e.event, o = e.update, s = e.publishNonRefinedEvent, n ||= e.action, a = e.refineEvent) : (r = e, i = t);
	function c(e) {
		return e.toLowerCase();
	}
	i = c(i || r);
	var l = a ? c(r) : i;
	Wk[r] || (ve(fk.test(r) && fk.test(i)), a && ve(i !== r), Wk[r] = {
		actionType: r,
		refinedEventType: i,
		nonRefinedEventType: l,
		update: o,
		action: n,
		refineEvent: a
	}, Kk[i] = 1, a && s && (Kk[l] = 1), Gk[l] = r);
}
function dA(e, t) {
	Cm.register(e, t);
}
function fA(e, t) {
	hA(Yk, e, t, ZO, "layout", !0);
}
function pA(e, t) {
	hA(Yk, e, t, ek, "visual", !0);
}
var mA = [];
function hA(e, t, n, r, i, a) {
	if ((H(t) || W(t)) && (n = t, t = r), !(N(mA, n) >= 0)) {
		mA.push(n);
		var o = mD.wrapStageHandler(n, i);
		o.__prio = t, o.__raw = n, e.push(o);
	}
}
function gA(e, t) {
	Zk[e] = t;
}
function _A(e, t, n) {
	var r = WD("registerMap");
	r && r(e, t, n);
}
var vA = $g;
pA($O, cD), pA(nk, uD), pA(nk, dD), pA($O, ID), pA(nk, LD), pA(ok, HO), aA(QE), oA(KO, $E), gA("default", pD), uA({
	type: Rc,
	event: Rc,
	update: Rc
}, je), uA({
	type: zc,
	event: zc,
	update: zc
}, je), uA({
	type: Bc,
	event: Uc,
	update: Bc,
	action: je,
	refineEvent: yA,
	publishNonRefinedEvent: !0
}), uA({
	type: Vc,
	event: Uc,
	update: Vc,
	action: je,
	refineEvent: yA,
	publishNonRefinedEvent: !0
}), uA({
	type: Hc,
	event: Uc,
	update: Hc,
	action: je,
	refineEvent: yA,
	publishNonRefinedEvent: !0
});
function yA(e, t, n, r) {
	return { eventContent: {
		selected: Tl(n),
		isFromClick: t.isFromClick || !1
	} };
}
iA("default", {}), iA("dark", MD);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/extension.js
var bA = [], xA = {
	registerPreprocessor: aA,
	registerProcessor: oA,
	registerPostInit: sA,
	registerPostUpdate: cA,
	registerUpdateLifecycle: lA,
	registerAction: uA,
	registerCoordinateSystem: dA,
	registerLayout: fA,
	registerVisual: pA,
	registerTransform: vA,
	registerLoading: gA,
	registerMap: _A,
	registerImpl: UD,
	PRIORITY: sk,
	ComponentModel: Ng,
	ComponentView: nD,
	SeriesModel: P_,
	ChartView: Ov,
	registerComponentModel: function(e) {
		Ng.registerClass(e);
	},
	registerComponentView: function(e) {
		nD.registerClass(e);
	},
	registerSeriesModel: function(e) {
		P_.registerClass(e);
	},
	registerChartView: function(e) {
		Ov.registerClass(e);
	},
	registerCustomSeries: function(e, t) {
		KD(e, t);
	},
	registerSubTypeDefaulter: function(e, t) {
		Ng.registerSubTypeDefaulter(e, t);
	},
	registerPainter: function(e, t) {
		iE(e, t);
	}
};
function SA(e) {
	if (V(e)) {
		I(e, function(e) {
			SA(e);
		});
		return;
	}
	N(bA, e) >= 0 || (bA.push(e), H(e) && (e = { install: e }), e.install(xA));
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/coord/axisModelCommonMixin.js
var CA = function() {
	function e() {}
	return e.prototype.needIncludeZero = function() {
		return !this.option.scale;
	}, e.prototype.getCoordSysModel = function() {}, e;
}(), wA = function(e) {
	o(t, e);
	function t() {
		return e !== null && e.apply(this, arguments) || this;
	}
	return t.prototype.getCoordSysModel = function() {
		return this.getReferringComponents("grid", Js).models[0];
	}, t.type = "cartesian2dAxis", t;
}(Ng);
P(wA, CA);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/coord/axisDefault.js
var TA = {
	show: !0,
	z: 0,
	inverse: !1,
	name: "",
	nameLocation: "end",
	nameRotate: null,
	nameTruncate: {
		maxWidth: null,
		ellipsis: "...",
		placeholder: "."
	},
	nameTextStyle: {},
	nameGap: 15,
	silent: !1,
	triggerEvent: !1,
	tooltip: { show: !1 },
	axisPointer: {},
	axisLine: {
		show: !0,
		onZero: "auto",
		onZeroAxisIndex: null,
		lineStyle: {
			color: Q.color.axisLine,
			width: 1,
			type: "solid"
		},
		symbol: ["none", "none"],
		symbolSize: [10, 15],
		breakLine: !0
	},
	axisTick: {
		show: !0,
		inside: !1,
		length: 5,
		lineStyle: { width: 1 }
	},
	axisLabel: {
		show: !0,
		inside: !1,
		rotate: 0,
		showMinLabel: null,
		showMaxLabel: null,
		margin: 8,
		fontSize: 12,
		color: Q.color.axisLabel,
		textMargin: [0, 3]
	},
	splitLine: {
		show: !0,
		showMinLine: !0,
		showMaxLine: !0,
		lineStyle: {
			color: Q.color.axisSplitLine,
			width: 1,
			type: "solid"
		}
	},
	splitArea: {
		show: !1,
		areaStyle: { color: [Q.color.backgroundTint, Q.color.backgroundTransparent] }
	},
	breakArea: {
		show: !0,
		itemStyle: {
			color: Q.color.neutral00,
			borderColor: Q.color.border,
			borderWidth: 1,
			borderType: [3, 3],
			opacity: .6
		},
		zigzagAmplitude: 4,
		zigzagMinSpan: 4,
		zigzagMaxSpan: 20,
		zigzagZ: 100,
		expandOnClick: !0
	},
	breakLabelLayout: { moveOverlap: "auto" }
}, EA = A({
	boundaryGap: !0,
	deduplication: null,
	jitter: 0,
	jitterOverlap: !0,
	jitterMargin: 2,
	splitLine: { show: !1 },
	axisTick: {
		alignWithLabel: !1,
		interval: "auto",
		show: "auto"
	},
	axisLabel: { interval: "auto" }
}, TA), DA = A({
	boundaryGap: [0, 0],
	axisLine: { show: "auto" },
	axisTick: { show: "auto" },
	splitNumber: 5,
	minorTick: {
		show: !1,
		splitNumber: 5,
		length: 3,
		lineStyle: {}
	},
	minorSplitLine: {
		show: !1,
		lineStyle: {
			color: Q.color.axisMinorSplitLine,
			width: 1
		}
	}
}, TA), OA = {
	category: EA,
	value: DA,
	time: A({
		splitNumber: 6,
		axisLabel: { rich: { primary: { fontWeight: "bold" } } },
		splitLine: { show: !1 }
	}, DA),
	log: M({ logBase: 10 }, DA)
};
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/coord/axisModelCreator.js
function kA(e, t, n, r) {
	I(Fy, function(i, a) {
		var s = A(A({}, OA[a], !0), r, !0), c = function(e) {
			o(n, e);
			function n() {
				var n = e !== null && e.apply(this, arguments) || this;
				return n.type = t + "Axis." + a, n;
			}
			return n.prototype.mergeDefaultAndTheme = function(e, t) {
				var n = Og(this), r = n ? Ag(e) : {};
				A(e, t.getTheme().get(a + "Axis")), A(e, this.getDefaultOption()), e.type = AA(e), n && kg(e, r, n);
			}, n.prototype.optionUpdated = function() {
				this.option.type === "category" && (this.__ordinalMeta = Bv.createByAxisModel(this));
			}, n.prototype.getCategories = function(e) {
				var t = this.option;
				if (t.type === "category") return e ? t.data : this.__ordinalMeta.categories;
			}, n.prototype.getOrdinalMeta = function() {
				return this.__ordinalMeta;
			}, n.prototype.updateAxisBreaks = function(e) {
				var t = Hx();
				return t ? t.updateModelAxisBreak(this, e) : { breaks: [] };
			}, n.type = t + "Axis." + a, n.defaultOption = s, n;
		}(n);
		e.registerComponentModel(c);
	}), e.registerSubTypeDefaulter(t + "Axis", AA);
}
function AA(e) {
	return e.type || (e.data ? "category" : "value");
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/coord/cartesian/Cartesian.js
var jA = function() {
	function e(e) {
		this.type = "cartesian", this._dimList = [], this._axes = {}, this.name = e || "";
	}
	return e.prototype.getAxis = function(e) {
		return this._axes[e];
	}, e.prototype.getAxes = function() {
		return L(this._dimList, function(e) {
			return this._axes[e];
		}, this);
	}, e.prototype.getAxesByScale = function(e) {
		return e = e.toLowerCase(), re(this.getAxes(), function(t) {
			return t.scale.type === e;
		});
	}, e.prototype.addAxis = function(e) {
		var t = e.dim;
		this._axes[t] = e, this._dimList.push(t);
	}, e;
}(), MA = ["x", "y"];
function NA(e) {
	return (e.type === "interval" || e.type === "time") && !wh(e);
}
var PA = function(e) {
	o(t, e);
	function t() {
		var t = e !== null && e.apply(this, arguments) || this;
		return t.type = nC, t.dimensions = MA, t;
	}
	return t.prototype.calcAffineTransform = function() {
		this._transform = this._invTransform = null;
		var e = this.getAxis("x").scale, t = this.getAxis("y").scale;
		if (!(!NA(e) || !NA(t))) {
			var n = qv(e, null), r = qv(t, null), i = this.dataToPoint([n[0], r[0]]), a = this.dataToPoint([n[1], r[1]]), o = n[1] - n[0], s = r[1] - r[0];
			if (!(!o || !s)) {
				var c = (a[0] - i[0]) / o, l = (a[1] - i[1]) / s, u = i[0] - n[0] * c, d = i[1] - r[0] * l, f = this._transform = [
					c,
					0,
					0,
					l,
					u,
					d
				];
				this._invTransform = pt([], f);
			}
		}
	}, t.prototype.getBaseAxis = function() {
		return this.getAxesByScale("ordinal")[0] || this.getAxesByScale("time")[0] || this.getAxis("x");
	}, t.prototype.containPoint = function(e) {
		var t = this.getAxis("x"), n = this.getAxis("y");
		return t.contain(t.toLocalCoord(e[0])) && n.contain(n.toLocalCoord(e[1]));
	}, t.prototype.containData = function(e) {
		return this.getAxis("x").containData(e[0]) && this.getAxis("y").containData(e[1]);
	}, t.prototype.containZone = function(e, t) {
		var n = this.dataToPoint(e), r = this.dataToPoint(t), i = this.getArea(), a = new Y(n[0], n[1], r[0] - n[0], r[1] - n[1]);
		return i.intersect(a);
	}, t.prototype.dataToPoint = function(e, t, n) {
		n ||= [];
		var r = e[0], i = e[1];
		if (this._transform && r != null && isFinite(r) && i != null && isFinite(i)) return Ot(n, e, this._transform);
		var a = this.getAxis("x"), o = this.getAxis("y");
		return n[0] = a.toGlobalCoord(a.dataToCoord(r, t)), n[1] = o.toGlobalCoord(o.dataToCoord(i, t)), n;
	}, t.prototype.clampData = function(e, t) {
		var n = this.getAxis("x").scale, r = this.getAxis("y").scale, i = n.getExtent(), a = r.getExtent(), o = n.parse(e[0]), s = r.parse(e[1]);
		return t ||= [], t[0] = Math.min(Math.max(Math.min(i[0], i[1]), o), Math.max(i[0], i[1])), t[1] = Math.min(Math.max(Math.min(a[0], a[1]), s), Math.max(a[0], a[1])), t;
	}, t.prototype.pointToData = function(e, t, n) {
		if (n ||= [], this._invTransform) return Ot(n, e, this._invTransform);
		var r = this.getAxis("x"), i = this.getAxis("y");
		return n[0] = r.coordToData(r.toLocalCoord(e[0]), t), n[1] = i.coordToData(i.toLocalCoord(e[1]), t), n;
	}, t.prototype.getOtherAxis = function(e) {
		return this.getAxis(e.dim === "x" ? "y" : "x");
	}, t.prototype.getArea = function(e) {
		e ||= 0;
		var t = this.getAxis("x").getGlobalExtent(), n = this.getAxis("y").getGlobalExtent(), r = Math.min(t[0], t[1]) - e, i = Math.min(n[0], n[1]) - e;
		return new Y(r, i, Math.max(t[0], t[1]) - r + e, Math.max(n[0], n[1]) - i + e);
	}, t;
}(jA);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/coord/axisAlignTicks.js
function FA(e, t) {
	var n = e.scale, r = e.model, i = US(n, r, r.ecModel, e, null), a = ny(n), o = ny(t) ? t.intervalStub : t, s = a ? n.intervalStub : n, c = n.base, l = o.getTicks(), u = o.getTicks({ expandToNicedExtent: !0 }), d = l.length - 1, f, p, m;
	if (d === 1) f = p = 0, m = 1;
	else if (d === 2) {
		var h = Io(l[0].value - l[1].value), g = Io(l[1].value - l[2].value);
		f = p = 0, h === g ? m = 2 : (m = 1, h < g ? f = h / g : p = g / h);
	} else {
		var _ = o.getConfig().interval;
		f = (1 - (l[0].value - u[0].value) / _) % 1, p = (1 - (u[d].value - l[d].value) / _) % 1, m = d - +!!f - !!p;
	}
	var v = i.zoomFixMM, y = v[0] || v[1], b = [i.fixMM[0] || y, i.fixMM[1] || y], x = n.getExtent(), S = s.getExtent(), C = cy(S, b), w, T, E, D, O, k;
	function A(e) {
		for (var t = 50, n = 0; n < t && !e(); n++) E = a ? E * Fo(c, 2) : iy(E), D = ay(E);
	}
	function j() {
		w = Z(k - E * f, D);
	}
	function ee() {
		T = Z(O + E * p, D);
	}
	function M() {
		k = f ? Z(w + E * f, D) : w;
	}
	function N() {
		O = p ? Z(T - E * p, D) : T;
	}
	if (b[0] && b[1]) {
		w = C[0], T = C[1], E = (T - w) / (m + f + p);
		var te = e.getExtent(), P = Io(te[1] - te[0]);
		D = Qo([T, w], P, .5 / m), M(), N(), ms(D) && (E = Z(E, D));
	} else {
		var F = C[1] - C[0];
		E = a ? Fo(os(F), 1) : cs(F / m, 2), D = ay(E), b[0] ? (w = C[0], A(function() {
			if (M(), O = Z(k + E * m, D), ee(), T >= C[1]) return !0;
		})) : b[1] ? (T = C[1], A(function() {
			if (N(), k = Z(O - E * m, D), j(), w <= C[0]) return !0;
		})) : A(function() {
			k = Z(zo(C[0] / E) * E, D), O = Z(Ro(C[1] / E) * E, D);
			var e = Lo((O - k) / E);
			if (e <= m) {
				var t = m - e, n = void 0, r = i.incl0 || a;
				if (r && C[0] === 0) n = [0, t];
				else if (r && C[1] === 0) n = [t, 0];
				else {
					var o = Ro(t / 2);
					n = t % 2 == 0 ? [o, o] : w + T < C[0] + C[1] ? [o, o + 1] : [o + 1, o];
				}
				if (k = Z(k - E * n[0], D), O = Z(O + E * n[1], D), j(), ee(), w <= C[0] && T >= C[1]) return !0;
			}
		});
	}
	Zy(n, b, S, [w, T], x, {
		interval: E,
		intervalCount: m,
		intervalPrecision: D,
		niceExtent: [k, O]
	});
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/coord/axisNiceTicks.js
function IA(e, t) {
	var n = ny(e), r = n ? e.intervalStub : e, i = t.fixMinMax || [], a = n ? e.getExtent() : null, o = r.getExtent(), s = cy(o, i, t.rawExtentResult);
	r.setExtent(s[0], s[1]), s = r.getExtent();
	var c = n ? RA(r, t) : LA(r, t), l = c.intervalPrecision, u = c.interval, d = t.userInterval;
	d != null && (c.interval = d, c.intervalPrecision = ay(d)), i[0] || (s[0] = Z(Ro(s[0] / u) * u, l)), i[1] || (s[1] = Z(zo(s[1] / u) * u, l)), d != null && (c.niceExtent = s.slice()), Zy(e, i, o, s, a, c);
}
function LA(e, t) {
	var n = uy(t.splitNumber, 5), r = Yv(e), i = t.minInterval, a = t.maxInterval, o = cs(r / n, !0);
	i != null && o < i && (o = i), a != null && o > a && (o = a);
	var s = ay(o), c = e.getExtent(), l = [Z(zo(c[0] / o) * o, s), Z(Ro(c[1] / o) * o, s)];
	return {
		interval: o,
		intervalPrecision: s,
		niceExtent: l
	};
}
function RA(e, t) {
	var n = uy(t.splitNumber, 10), r = e.getExtent(), i = Yv(e), a = Fo(os(i), 1);
	n / i * a <= .5 && (a *= 10);
	var o = ay(a), s = [Z(zo(r[0] / a) * a, o), Z(Ro(r[1] / a) * a, o)];
	return {
		intervalPrecision: o,
		interval: a,
		niceExtent: s
	};
}
function zA(e) {
	var t = e.scale, n = e.model, r = n.axis, i = n.ecModel;
	BA(t, n, r, i, null);
}
function BA(e, t, n, r, i) {
	var a = US(e, t, r, n, i), o = ey(e) || ty(e);
	VA(e, {
		splitNumber: t.get("splitNumber"),
		fixMinMax: a.fixMM,
		userInterval: t.get("interval"),
		minInterval: o ? t.get("minInterval") : null,
		maxInterval: o ? t.get("maxInterval") : null,
		rawExtentResult: a
	}), n && r && GS(n, e, a, r);
}
function VA(e, t) {
	HA[e.type](e, t);
}
var HA = {
	interval: IA,
	log: IA,
	time: Dy,
	ordinal: je
}, UA = [[3, 1], [0, 2]], WA = function() {
	function e(e, t, n) {
		this.type = "grid", this._coordsMap = {}, this._coordsList = [], this._axesMap = {}, this._axesList = [], this.axisPointerEnabled = !0, this.dimensions = MA, this._initCartesian(e, t, n), this.model = e;
	}
	return e.prototype.getRect = function() {
		return this._rect;
	}, e.prototype.update = function(e, t) {
		var n = this._axesMap;
		I(this._axesList, function(e) {
			LS(e, 1);
			var t = e.scale;
			ry(t) && t.setSortInfo(e.model.get("categorySortInfo"));
		});
		function r(e) {
			for (var t = R(e), n = [], r = t.length - 1; r >= 0; r--) {
				var i = e[+t[r]];
				i.__alignTo ? n.push(i) : zA(i);
			}
			I(n, function(e) {
				YA(e, e.__alignTo) ? zA(e) : FA(e, e.__alignTo.scale);
			});
		}
		r(n.x), r(n.y);
		var i = {};
		I(n.x, function(e) {
			KA(n, "y", e, i);
		}), I(n.y, function(e) {
			KA(n, "x", e, i);
		}), this.resize(this.model, t);
	}, e.prototype.resize = function(e, t, n) {
		var r = Dg(e, t), i = this._rect = Tg(e.getBoxLayoutParams(), r.refContainer), a = this._axesMap, o = this._coordsList, s = e.get("containLabel");
		if (ZA(a, i), !n) {
			var c = tj(i, o, a, s, t), l = void 0;
			if (s) $A ? ($A(this._axesList, i), ZA(a, i)) : l = ej(i.clone(), "axisLabel", null, i, a, c, r);
			else {
				var u = rj(e, i, r), d = u.outerBoundsRect, f = u.parsedOuterBoundsContain, p = u.outerBoundsClamp;
				d && (l = ej(d, f, p, i, a, c, r));
			}
			nj(i, a, wb.determine, null, l, r), I(this._coordsList, function(e) {
				e.calcAffineTransform();
			});
		}
	}, e.prototype.getAxis = function(e, t) {
		var n = this._axesMap[e];
		if (n != null) return n[t || 0];
	}, e.prototype.getAxes = function() {
		return this._axesList.slice();
	}, e.prototype.getCartesian = function(e, t) {
		if (e != null && t != null) {
			var n = "x" + e + "y" + t;
			return this._coordsMap[n];
		}
		W(e) && (t = e.yAxisIndex, e = e.xAxisIndex);
		for (var r = 0, i = this._coordsList; r < i.length; r++) if (i[r].getAxis("x").index === e || i[r].getAxis("y").index === t) return i[r];
	}, e.prototype.getCartesians = function() {
		return this._coordsList.slice();
	}, e.prototype.convertToPixel = function(e, t, n) {
		var r = this._findConvertTarget(t);
		return r.cartesian ? r.cartesian.dataToPoint(n) : r.axis ? r.axis.toGlobalCoord(r.axis.dataToCoord(n)) : null;
	}, e.prototype.convertFromPixel = function(e, t, n) {
		var r = this._findConvertTarget(t);
		return r.cartesian ? r.cartesian.pointToData(n) : r.axis ? r.axis.coordToData(r.axis.toLocalCoord(n)) : null;
	}, e.prototype._findConvertTarget = function(e) {
		var t = e.seriesModel, n = e.xAxisModel || t && t.getReferringComponents("xAxis", Js).models[0], r = e.yAxisModel || t && t.getReferringComponents("yAxis", Js).models[0], i = e.gridModel, a = this._coordsList, o, s;
		return t ? (o = t.coordinateSystem, N(a, o) < 0 && (o = null)) : n && r ? o = this.getCartesian(n.componentIndex, r.componentIndex) : n ? s = this.getAxis("x", n.componentIndex) : r ? s = this.getAxis("y", r.componentIndex) : i && i.coordinateSystem === this && (o = this._coordsList[0]), {
			cartesian: o,
			axis: s
		};
	}, e.prototype.containPoint = function(e) {
		var t = this._coordsList[0];
		if (t) return t.containPoint(e);
	}, e.prototype._initCartesian = function(e, t, n) {
		var r = this, i = this, a = {
			left: !1,
			right: !1,
			top: !1,
			bottom: !1
		}, o = {
			x: {},
			y: {}
		}, s = {
			x: 0,
			y: 0
		};
		if (t.eachComponent("xAxis", c("x"), this), t.eachComponent("yAxis", c("y"), this), !s.x || !s.y) {
			this._axesMap = {}, this._axesList = [];
			return;
		}
		this._axesMap = o, I(o.x, function(t, n) {
			I(o.y, function(i, a) {
				var o = "x" + n + "y" + a, s = new PA(o);
				s.master = r, s.model = e, r._coordsMap[o] = s, r._coordsList.push(s), s.addAxis(t), s.addAxis(i);
			});
		}), JA(o.x), JA(o.y);
		function c(t) {
			return function(n, r) {
				if (GA(n, e)) {
					var c = n.get("position");
					t === "x" ? c !== "top" && c !== "bottom" && (c = a.bottom ? "top" : "bottom") : c !== "left" && c !== "right" && (c = a.left ? "right" : "left"), a[c] = !0;
					var l = Ly(n), u = new Cx(t, Ry(n, l, !0), [0, 0], l, c);
					u.onBand = $y(u.scale, n), u.inverse = n.get("inverse"), n.axis = u, u.model = n, u.grid = i, u.index = r, i._axesList.push(u), o[t][r] = u, s[t]++;
				}
			};
		}
	}, e.prototype.getTooltipAxes = function(e) {
		var t = [], n = [];
		return I(this.getCartesians(), function(r) {
			var i = e != null && e !== "auto" ? r.getAxis(e) : r.getBaseAxis(), a = r.getOtherAxis(i);
			N(t, i) < 0 && t.push(i), N(n, a) < 0 && n.push(a);
		}), {
			baseAxes: t,
			otherAxes: n
		};
	}, e.create = function(t, n) {
		var r = [];
		return t.eachComponent("grid", function(i, a) {
			var o = new e(i, t, n);
			o.name = "grid_" + a, o.resize(i, n, !0), i.coordinateSystem = o, r.push(o), I(o._axesList, function(t) {
				IS(t, e.dimIdxMap);
			});
		}), t.eachSeries(function(e) {
			var t, n;
			km({
				targetModel: e,
				coordSysType: nC,
				coordSysProvider: r
			});
			function r() {
				var r = TS(e), i = r.xAxisModel, a = r.yAxisModel;
				return t = i.axis, n = a.axis, i.getCoordSysModel().coordinateSystem.getCartesian(i.componentIndex, a.componentIndex);
			}
			t && n && (dx(t, e, nC), dx(n, e, nC));
		}, this), r;
	}, e.dimensions = MA, e.dimIdxMap = em(MA), e;
}();
function GA(e, t) {
	return e.getCoordSysModel() === t;
}
function KA(e, t, n, r) {
	n.getAxesOnZeroOf = function() {
		return a ? [a] : [];
	};
	var i = e[t], a, o = n.model, s = o.get(["axisLine", "onZero"]), c = o.get(["axisLine", "onZeroAxisIndex"]);
	if (!s) return;
	if (c != null) qA(s, i[c]) && (a = i[c]);
	else for (var l in i) if (Ae(i, l) && qA(s, i[l]) && !r[u(i[l])]) {
		a = i[l];
		break;
	}
	a && (r[u(a)] = !0);
	function u(e) {
		return e.dim + "_" + e.index;
	}
}
function qA(e, t) {
	if (!t) return !1;
	var n = t.scale, r = zy(n, 0, !1), i = t && t.type !== "category" && t.type !== "time" && r !== 3;
	return i && e === "auto" && Vy(t) && (i = !1), i;
}
function JA(e) {
	for (var t = R(e), n, r = [], i = t.length - 1; i >= 0; i--) {
		var a = e[+t[i]];
		$v(a.scale) && Yy(a.model, a.type, !0) == null && (a.model.get("alignTicks") && a.model.get("interval") == null ? r.push(a) : n = a);
	}
	n ||= r.pop(), n && I(r, function(e) {
		e.__alignTo = n;
	});
}
function YA(e, t) {
	return wh(e.scale) || wh(t.scale) || t.scale.getTicks().length < 2;
}
function XA(e, t) {
	var n = e.getExtent(), r = n[0] + n[1];
	e.toGlobalCoord = e.dim === "x" ? function(e) {
		return e + t;
	} : function(e) {
		return r - e + t;
	}, e.toLocalCoord = e.dim === "x" ? function(e) {
		return e - t;
	} : function(e) {
		return r - e + t;
	};
}
function ZA(e, t) {
	I(e.x, function(e) {
		return QA(e, t.x, t.width);
	}), I(e.y, function(e) {
		return QA(e, t.y, t.height);
	});
}
function QA(e, t, n) {
	var r = [0, n], i = +!!e.inverse;
	e.setExtent(r[i], r[1 - i]), XA(e, t);
}
var $A;
function ej(e, t, n, r, i, a, o) {
	nj(r, i, wb.estimate, t, !1, o);
	var s = [
		0,
		0,
		0,
		0
	];
	l(0), l(1), u(r, 0, NaN), u(r, 1, NaN);
	var c = ie(s, function(e) {
		return e > 0;
	}) == null;
	return qd(r, s, !0, !0, n), ZA(i, r), c;
	function l(e) {
		I(i[yd[e]], function(t) {
			if (Jy(t.model)) {
				var n = a.ensureRecord(t.model), r = n.labelInfoList;
				if (r) for (var i = 0; i < r.length; i++) {
					var o = r[i], s = t.scale.normalize(Qy(t.scale, qx(o.label).labelInfo.tick));
					s = e === 1 ? 1 - s : s, u(o.rect, e, s), u(o.rect, 1 - e, NaN);
				}
				var c = n.nameLayout;
				if (c) {
					var s = qy(n.nameLocation) ? .5 : NaN;
					u(c.rect, e, s), u(c.rect, 1 - e, NaN);
				}
			}
		});
	}
	function u(t, n, r) {
		var i = e[yd[n]] - t[yd[n]], a = t[bd[n]] + t[yd[n]] - (e[bd[n]] + e[yd[n]]);
		i = d(i, 1 - r), a = d(a, r);
		var o = UA[n][0], c = UA[n][1];
		s[o] = Fo(s[o], i), s[c] = Fo(s[c], a);
	}
	function d(e, t) {
		return e > 0 && !pe(t) && t > 1e-4 && (e /= t), e;
	}
}
function tj(e, t, n, r, i) {
	var a = new Yx(ij);
	return I(n, function(n) {
		return I(n, function(n) {
			if (Jy(n.model)) {
				var o = !r;
				n.axisBuilder = ES(e, t, n.model, i, a, o);
			}
		});
	}), a;
}
function nj(e, t, n, r, i, a) {
	var o = n === wb.determine;
	I(t, function(t) {
		return I(t, function(t) {
			Jy(t.model) && (DS(t.axisBuilder, e, t.model), t.axisBuilder.build(o ? { axisTickLabelDetermine: !0 } : { axisTickLabelEstimate: !0 }, { noPxChange: i }));
		});
	});
	var s = {
		x: 0,
		y: 0
	};
	c(0), c(1);
	function c(t) {
		s[yd[1 - t]] = e[bd[t]] <= a.refContainer[bd[t]] * .5 ? 0 : 1 - t == 1 ? 2 : 1;
	}
	I(t, function(e, t) {
		return I(e, function(e) {
			Jy(e.model) && ((r === "all" || o) && e.axisBuilder.build({ axisName: !0 }, { nameMarginLevel: s[t] }), o && e.axisBuilder.build({ axisLine: !0 }));
		});
	});
}
function rj(e, t, n) {
	var r, i = e.get("outerBoundsMode", !0);
	i === "same" ? r = t.clone() : (i == null || i === "auto") && (r = Tg(e.get("outerBounds", !0) || eC, n.refContainer));
	var a = e.get("outerBoundsContain", !0), o = a == null || a === "auto" || N(["all", "axisLabel"], a) < 0 ? "all" : a, s = [qo(G(e.get("outerBoundsClampWidth", !0), tC[0]), t.width), qo(G(e.get("outerBoundsClampHeight", !0), tC[1]), t.height)];
	return {
		outerBoundsRect: r,
		parsedOuterBoundsContain: o,
		outerBoundsClamp: s
	};
}
var ij = function(e, t, n, r, i, a) {
	var o = n.axis.dim === "x" ? "y" : "x";
	$x(e, t, n, r, i, a), qy(e.nameLocation) || I(t.recordMap[o], function(e) {
		e && e.labelInfoList && e.dirVec && tS(e.labelInfoList, e.dirVec, r, i);
	});
};
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/axisPointer/modelHelper.js
function aj(e, t) {
	var n = {
		axesInfo: {},
		seriesInvolved: !1,
		coordSysAxesInfo: {},
		coordSysMap: {}
	};
	return oj(n, e, t), n.seriesInvolved && cj(n, e), n;
}
function oj(e, t, n) {
	var r = t.getComponent("tooltip"), i = t.getComponent("axisPointer"), a = i.get("link", !0) || [], o = [];
	I(n.getCoordinateSystems(), function(n) {
		if (!n.axisPointerEnabled) return;
		var s = hj(n.model), c = e.coordSysAxesInfo[s] = {};
		e.coordSysMap[s] = n;
		var l = n.model.getModel("tooltip", r);
		if (I(n.getAxes(), B(p, !1, null)), n.getTooltipAxes && r && l.get("show")) {
			var u = l.get("trigger") === "axis", d = l.get(["axisPointer", "type"]) === "cross", f = n.getTooltipAxes(l.get(["axisPointer", "axis"]));
			(u || d) && I(f.baseAxes, B(p, d ? "cross" : !0, u)), d && I(f.otherAxes, B(p, "cross", !1));
		}
		function p(r, s, u) {
			var d = u.model.getModel("axisPointer", i), f = d.get("show");
			if (!(!f || f === "auto" && !r && !mj(d))) {
				s ??= d.get("triggerTooltip"), d = r ? sj(u, l, i, t, r, s) : d;
				var p = d.get("snap"), m = d.get("triggerEmphasis"), h = hj(u.model), g = s || p || u.type === "category", _ = e.axesInfo[h] = {
					key: h,
					axis: u,
					coordSys: n,
					axisPointerModel: d,
					triggerTooltip: s,
					triggerEmphasis: m,
					involveSeries: g,
					snap: p,
					useHandle: mj(d),
					seriesModels: [],
					linkGroup: null
				};
				c[h] = _, e.seriesInvolved = e.seriesInvolved || g;
				var v = lj(a, u);
				if (v != null) {
					var y = o[v] || (o[v] = { axesInfo: {} });
					y.axesInfo[h] = _, y.mapper = a[v].mapper, _.linkGroup = y;
				}
			}
		}
	});
}
function sj(e, t, n, r, i, a) {
	var o = t.getModel("axisPointer"), s = [
		"type",
		"snap",
		"lineStyle",
		"shadowStyle",
		"label",
		"animation",
		"animationDurationUpdate",
		"animationEasingUpdate",
		"z"
	], c = {};
	I(s, function(e) {
		c[e] = k(o.get(e));
	}), c.snap = e.type !== "category" && !!a, o.get("type") === "cross" && (c.type = "line");
	var l = c.label ||= {};
	if (l.show ??= !1, i === "cross" && (l.show = o.get(["label", "show"]) ?? !0, !a)) {
		var u = c.lineStyle = o.get("crossStyle");
		u && M(l, u.textStyle);
	}
	return e.model.getModel("axisPointer", new Bf(c, n, r));
}
function cj(e, t) {
	t.eachSeries(function(t) {
		var n = t.coordinateSystem, r = t.get(["tooltip", "trigger"], !0), i = t.get(["tooltip", "show"], !0);
		!n || !n.model || r === "none" || r === !1 || r === "item" || i === !1 || t.get(["axisPointer", "show"], !0) === !1 || I(e.coordSysAxesInfo[hj(n.model)], function(e) {
			var r = e.axis;
			n.getAxis(r.dim) === r && (e.seriesModels.push(t), e.seriesDataCount ??= 0, e.seriesDataCount += t.getData().count());
		});
	});
}
function lj(e, t) {
	for (var n = t.model, r = t.dim, i = 0; i < e.length; i++) {
		var a = e[i] || {};
		if (uj(a[r + "AxisId"], n.id) || uj(a[r + "AxisIndex"], n.componentIndex) || uj(a[r + "AxisName"], n.name)) return i;
	}
}
function uj(e, t) {
	return e === "all" || V(e) && N(e, t) >= 0 || e === t;
}
function dj(e) {
	var t = fj(e);
	if (t) {
		var n = t.axisPointerModel, r = t.axis.scale, i = n.option, a = n.get("status"), o = n.get("value");
		o != null && (o = r.parse(o));
		var s = mj(n);
		a ?? (i.status = s ? "show" : "hide");
		var c = r.getExtent();
		(o == null || o > c[1]) && (o = c[1]), o < c[0] && (o = c[0]), i.value = o, s && (i.status = t.axis.scale.isBlank() ? "hide" : "show");
	}
}
function fj(e) {
	var t = (e.ecModel.getComponent("axisPointer") || {}).coordSysAxesInfo;
	return t && t.axesInfo[hj(e)];
}
function pj(e) {
	var t = fj(e);
	return t && t.axisPointerModel;
}
function mj(e) {
	return !!e.get(["handle", "show"]);
}
function hj(e) {
	return e.type + "||" + e.id;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/axis/AxisView.js
var gj = {}, _j = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n;
	}
	return t.prototype.render = function(t, n, r, i) {
		this.axisPointerClass && dj(t), e.prototype.render.apply(this, arguments), this._doUpdateAxisPointerClass(t, r, !0);
	}, t.prototype.updateAxisPointer = function(e, t, n, r) {
		this._doUpdateAxisPointerClass(e, n, !1);
	}, t.prototype.remove = function(e, t) {
		var n = this._axisPointer;
		n && n.remove(t);
	}, t.prototype.dispose = function(t, n) {
		this._disposeAxisPointer(n), e.prototype.dispose.apply(this, arguments);
	}, t.prototype._doUpdateAxisPointerClass = function(e, n, r) {
		var i = t.getAxisPointerClass(this.axisPointerClass);
		if (i) {
			var a = pj(e);
			a ? (this._axisPointer ||= new i()).render(e, a, n, r) : this._disposeAxisPointer(n);
		}
	}, t.prototype._disposeAxisPointer = function(e) {
		this._axisPointer && this._axisPointer.dispose(e), this._axisPointer = null;
	}, t.registerAxisPointerClass = function(e, t) {
		gj[e] = t;
	}, t.getAxisPointerClass = function(e) {
		return e && gj[e];
	}, t.type = "axis", t;
}(nD), vj = Ws();
function yj(e, t, n, r) {
	var i = n.axis;
	if (!i.scale.isBlank()) {
		var a = n.getModel("splitArea"), o = a.getModel("areaStyle"), s = o.get("color"), c = r.coordinateSystem.getRect(), l = i.getTicksCoords({
			tickModel: a,
			breakTicks: "none",
			pruneByBreak: "preserve_extent_bound"
		});
		if (l.length) {
			var u = s.length, d = vj(e).splitAreaColors, f = K(), p = 0;
			if (d) for (var m = 0; m < l.length; m++) {
				var h = d.get(l[m].tickValue);
				if (h != null) {
					p = (h + (u - 1) * m) % u;
					break;
				}
			}
			var g = i.toGlobalCoord(l[0].coord), _ = o.getAreaStyle();
			s = V(s) ? s : [s];
			for (var m = 1; m < l.length; m++) {
				var v = i.toGlobalCoord(l[m].coord), y = void 0, b = void 0, x = void 0, S = void 0;
				i.isHorizontal() ? (y = g, b = c.y, x = v - y, S = c.height, g = y + x) : (y = c.x, b = g, x = c.width, S = v - b, g = b + S);
				var C = l[m - 1].tickValue;
				C != null && f.set(C, p), t.add(new fo({
					anid: C == null ? null : "area_" + C,
					shape: {
						x: y,
						y: b,
						width: x,
						height: S
					},
					style: M({ fill: s[p] }, _),
					autoBatch: !0,
					silent: !0
				})), p = (p + 1) % u;
			}
			vj(e).splitAreaColors = f;
		}
	}
}
function bj(e) {
	vj(e).splitAreaColors = null;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/axis/CartesianAxisView.js
var xj = [
	"splitArea",
	"splitLine",
	"minorSplitLine",
	"breakArea"
], Sj = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n.axisPointerClass = "CartesianAxisPointer", n;
	}
	return t.prototype.render = function(t, n, r, i) {
		this.group.removeAll();
		var a = this._axisGroup;
		this._axisGroup = new su(), this.group.add(this._axisGroup), Jy(t) && (this._axisGroup.add(t.axis.axisBuilder.group), I(xj, function(e) {
			t.get([e, "show"]) && Cj[e](this, this._axisGroup, t, t.getCoordSysModel(), r);
		}, this), i && i.type === "changeAxisOrder" && i.isInitSort || zd(a, this._axisGroup, t), e.prototype.render.call(this, t, n, r, i));
	}, t.prototype.remove = function() {
		bj(this);
	}, t.type = "cartesianAxis", t;
}(_j), Cj = {
	splitLine: function(e, t, n, r, i) {
		var a = n.axis;
		if (!a.scale.isBlank()) {
			var o = n.getModel("splitLine"), s = o.getModel("lineStyle"), c = s.get("color"), l = o.get("showMinLine") !== !1, u = o.get("showMaxLine") !== !1;
			c = V(c) ? c : [c];
			for (var d = r.coordinateSystem.getRect(), f = a.isHorizontal(), p = 0, m = a.getTicksCoords({
				tickModel: o,
				breakTicks: "none",
				pruneByBreak: "preserve_extent_bound"
			}), h = [], g = [], _ = s.getLineStyle(), v = 0; v < m.length; v++) {
				var y = a.toGlobalCoord(m[v].coord);
				if (!(v === 0 && !l || v === m.length - 1 && !u)) {
					var b = m[v].tickValue;
					f ? (h[0] = y, h[1] = d.y, g[0] = y, g[1] = d.y + d.height) : (h[0] = d.x, h[1] = y, g[0] = d.x + d.width, g[1] = y);
					var x = p++ % c.length, S = new zu({
						anid: b == null ? null : "line_" + b,
						autoBatch: !0,
						shape: {
							x1: h[0],
							y1: h[1],
							x2: g[0],
							y2: g[1]
						},
						style: M({ stroke: c[x] }, _),
						silent: !0
					});
					jd(S.shape, _.lineWidth), t.add(S);
				}
			}
		}
	},
	minorSplitLine: function(e, t, n, r, i) {
		var a = n.axis, o = n.getModel("minorSplitLine").getModel("lineStyle"), s = r.coordinateSystem.getRect(), c = a.isHorizontal(), l = a.getMinorTicksCoords();
		if (l.length) for (var u = [], d = [], f = o.getLineStyle(), p = 0; p < l.length; p++) for (var m = 0; m < l[p].length; m++) {
			var h = a.toGlobalCoord(l[p][m].coord);
			c ? (u[0] = h, u[1] = s.y, d[0] = h, d[1] = s.y + s.height) : (u[0] = s.x, u[1] = h, d[0] = s.x + s.width, d[1] = h);
			var g = new zu({
				anid: "minor_line_" + l[p][m].tickValue,
				autoBatch: !0,
				shape: {
					x1: u[0],
					y1: u[1],
					x2: d[0],
					y2: d[1]
				},
				style: f,
				silent: !0
			});
			jd(g.shape, f.lineWidth), t.add(g);
		}
	},
	splitArea: function(e, t, n, r, i) {
		yj(e, t, n, r);
	},
	breakArea: function(e, t, n, r, i) {
		var a = Hx(), o = n.axis.scale;
		a && o.type !== "ordinal" && a.rectCoordBuildBreakAxis(t, e, n, r.coordinateSystem.getRect(), i);
	}
}, wj = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n;
	}
	return t.type = "xAxis", t;
}(Sj), Tj = function(e) {
	o(t, e);
	function t() {
		var t = e !== null && e.apply(this, arguments) || this;
		return t.type = wj.type, t;
	}
	return t.type = "yAxis", t;
}(Sj), Ej = function(e) {
	o(t, e);
	function t() {
		var t = e !== null && e.apply(this, arguments) || this;
		return t.type = "grid", t;
	}
	return t.prototype.render = function(e, t) {
		this.group.removeAll(), e.get("show") && this.group.add(new fo({
			shape: e.coordinateSystem.getRect(),
			style: M({ fill: e.get("backgroundColor") }, e.getItemStyle()),
			silent: !0,
			z2: -1
		}));
	}, t.type = "grid", t;
}(nD), Dj = { offset: 0 };
function Oj(e) {
	e.registerComponentView(Ej), e.registerComponentModel(rC), e.registerCoordinateSystem("cartesian2d", WA), kA(e, "x", wA, Dj), kA(e, "y", wA, Dj), e.registerComponentView(wj), e.registerComponentView(Tj), e.registerPreprocessor(function(e) {
		e.xAxis && e.yAxis && !e.grid && (e.grid = {});
	});
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/radar/backwardCompat.js
function kj(e) {
	var t = e.polar;
	if (t) {
		V(t) || (t = [t]);
		var n = [];
		I(t, function(t, r) {
			t.indicator ? (t.type && !t.shape && (t.shape = t.type), e.radar = e.radar || [], V(e.radar) || (e.radar = [e.radar]), e.radar.push(t)) : n.push(t);
		}), e.polar = n;
	}
	I(e.series, function(e) {
		e && e.type === "radar" && e.polarIndex && (e.radarIndex = e.polarIndex);
	});
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/radar/RadarSeries.js
var Aj = "radar", jj = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n.hasSymbolVisual = !0, n;
	}
	return t.prototype.init = function(t) {
		e.prototype.init.apply(this, arguments), this.legendVisualProvider = new sw(z(this.getData, this), z(this.getRawData, this));
	}, t.prototype.getInitialData = function(e, t) {
		return ow(this, {
			generateCoord: "indicator_",
			generateCoordCount: Infinity
		});
	}, t.prototype.formatTooltip = function(e, t, n) {
		var r = this.getData(), i = this.coordinateSystem.getIndicatorAxes(), a = this.getData().getName(e), o = a === "" ? this.name : a, s = D_(this, e);
		return m_("section", {
			header: o,
			sortBlocks: !0,
			blocks: L(i, function(t) {
				var n = r.get(r.mapDimension(t.dim), e);
				return m_("nameValue", {
					markerType: "subItem",
					markerColor: s,
					name: t.name,
					value: n,
					sortParam: n
				});
			})
		});
	}, t.prototype.getTooltipPosition = function(e) {
		if (e != null) {
			for (var t = this.getData(), n = this.coordinateSystem, r = t.getValues(L(n.dimensions, function(e) {
				return t.mapDimension(e);
			}), e), i = 0, a = r.length; i < a; i++) if (!isNaN(r[i])) {
				var o = n.getIndicatorAxes();
				return n.coordToPoint(o[i].dataToCoord(r[i]), i);
			}
		}
	}, t.type = "series." + Aj, t.dependencies = ["radar"], t.defaultOption = {
		z: 2,
		colorBy: "data",
		coordinateSystem: "radar",
		legendHoverLink: !0,
		radarIndex: 0,
		lineStyle: {
			width: 2,
			type: "solid",
			join: "round"
		},
		label: { position: "top" },
		symbolSize: 8
	}, t;
}(P_), Mj = function(e) {
	o(t, e);
	function t() {
		var t = e !== null && e.apply(this, arguments) || this;
		return t.type = Aj, t;
	}
	return t.prototype.render = function(e, t, n) {
		var r = e.coordinateSystem, i = this.group, a = e.getData(), o = this._data;
		function s(e, t) {
			var n = e.getItemVisual(t, "symbol") || "circle";
			if (n !== "none") {
				var r = X_(e.getItemVisual(t, "symbolSize")), i = Y_(n, -1, -1, 2, 2), a = e.getItemVisual(t, "symbolRotate") || 0;
				return i.attr({
					style: { strokeNoScale: !0 },
					z2: 100,
					scaleX: r[0] / 2,
					scaleY: r[1] / 2,
					rotation: a * Math.PI / 180 || 0
				}), i;
			}
		}
		function c(t, n, r, i, a, o) {
			r.removeAll();
			for (var c = 0; c < n.length - 1; c++) {
				var l = s(i, a);
				l && (l.__dimIdx = c, t[c] ? (l.setPosition(t[c]), _d[o ? "initProps" : "updateProps"](l, {
					x: n[c][0],
					y: n[c][1]
				}, e, a)) : l.setPosition(n[c]), r.add(l));
			}
		}
		function l(e) {
			return L(e, function(e) {
				return [r.cx, r.cy];
			});
		}
		a.diff(o).add(function(t) {
			var n = a.getItemLayout(t);
			if (n) {
				var r = new Pu(), i = new Iu(), o = { shape: { points: n } };
				r.shape.points = l(n), i.shape.points = l(n), dd(r, o, e, t), dd(i, o, e, t);
				var s = new su(), u = new su();
				s.add(i), s.add(r), s.add(u), c(i.shape.points, n, u, a, t, !0), a.setItemGraphicEl(t, s);
			}
		}).update(function(t, n) {
			var r = o.getItemGraphicEl(n), i = r.childAt(0), s = r.childAt(1), l = r.childAt(2), u = { shape: { points: a.getItemLayout(t) } };
			u.shape.points && (c(i.shape.points, u.shape.points, l, a, t, !1), gd(s), gd(i), ud(i, u, e), ud(s, u, e), a.setItemGraphicEl(t, r));
		}).remove(function(e) {
			i.remove(o.getItemGraphicEl(e));
		}).execute(), a.eachItemGraphicEl(function(e, t) {
			var n = a.getItemModel(t), r = e.childAt(0), o = e.childAt(1), s = e.childAt(2), c = a.getItemVisual(t, "style"), l = c.fill;
			i.add(e), r.useStyle(M(n.getModel("lineStyle").getLineStyle(), {
				fill: "none",
				stroke: l
			})), Ml(r, n, "lineStyle"), Ml(o, n, "areaStyle");
			var u = n.getModel("areaStyle"), d = u.isEmpty() && u.parentModel.isEmpty();
			o.ignore = d, I([
				"emphasis",
				"select",
				"blur"
			], function(e) {
				var t = n.getModel([e, "areaStyle"]), i = t.isEmpty() && t.parentModel.isEmpty();
				o.ensureState(e).ignore = i && d;
				var a = n.getModel([e, "lineStyle"]).getLineStyle();
				r.ensureState(e).style = a;
				var c = t.getAreaStyle();
				o.ensureState(e).style = c;
				var l = n.getModel([e, "itemStyle"]).getItemStyle();
				s.eachChild(function(t) {
					t.ensureState(e).style = k(l);
				});
			}), o.useStyle(M(n.getModel("areaStyle").getAreaStyle(), {
				fill: l,
				opacity: .7,
				decal: c.decal
			}));
			var f = n.getModel("emphasis");
			s.eachChild(function(e) {
				if (e instanceof ro) {
					var r = e.style;
					e.useStyle(j({
						image: r.image,
						x: r.x,
						y: r.y,
						width: r.width,
						height: r.height
					}, c));
				} else e.useStyle(c), e.setColor(l), e.style.strokeNoScale = !0;
				var i = a.getStore().get(a.getDimensionIndex(e.__dimIdx), t);
				(i == null || isNaN(i)) && (i = ""), hf(e, gf(n), {
					labelFetcher: a.hostModel,
					labelDataIndex: t,
					labelDimIndex: e.__dimIdx,
					defaultText: i,
					inheritColor: l,
					defaultOpacity: c.opacity
				});
			}), Ol(e, f.get("focus"), f.get("blurScope"), f.get("disabled"));
		}), this._data = a;
	}, t.prototype.remove = function() {
		this.group.removeAll(), this._data = null;
	}, t.type = Aj, t;
}(Ov), Nj = OA.value, Pj = "radar", Fj = Pj, Ij = Pj;
function Lj(e, t) {
	return M({ show: t }, e);
}
var Rj = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n;
	}
	return t.prototype.optionUpdated = function() {
		var e = this.get("boundaryGap"), t = this.get("splitNumber"), n = this.get("clockwise"), r = this.get("scale"), i = this.get("axisLine"), a = this.get("axisTick"), o = this.get("axisLabel"), s = this.get("axisName"), c = this.get(["axisName", "show"]), l = this.get(["axisName", "formatter"]), u = this.get("axisNameGap"), d = this.get("triggerEvent"), f = L(this.get("indicator") || [], function(f) {
			f.max != null && f.max > 0 && !f.min ? f.min = 0 : f.min != null && f.min < 0 && !f.max && (f.max = 0);
			var p = s;
			f.color != null && (p = M({ color: f.color }, s));
			var m = A(k(f), {
				boundaryGap: e,
				splitNumber: t,
				clockwise: n,
				scale: r,
				axisLine: i,
				axisTick: a,
				axisLabel: o,
				name: f.text,
				showName: c,
				nameLocation: "end",
				nameGap: u,
				nameTextStyle: p,
				triggerEvent: d
			}, !1);
			if (U(l)) {
				var h = m.name;
				m.name = l.replace("{value}", h ?? "");
			} else H(l) && (m.name = l(m.name, m));
			var g = new Bf(m, null, this.ecModel);
			return P(g, CA.prototype), g.mainType = "radar", g.componentIndex = this.componentIndex, g.uid = Wm("ec_radar"), g;
		}, this);
		this._indicatorModels = f;
	}, t.prototype.getIndicatorModels = function() {
		return this._indicatorModels;
	}, t.type = Fj, t.defaultOption = {
		z: 0,
		center: ["50%", "50%"],
		radius: "50%",
		startAngle: 90,
		clockwise: !1,
		axisName: {
			show: !0,
			color: Q.color.axisLabel
		},
		boundaryGap: [0, 0],
		splitNumber: 5,
		axisNameGap: 15,
		scale: !1,
		shape: "polygon",
		axisLine: A({ lineStyle: { color: Q.color.neutral20 } }, Nj.axisLine),
		axisLabel: Lj(Nj.axisLabel, !1),
		axisTick: Lj(Nj.axisTick, !1),
		splitLine: Lj(Nj.splitLine, !0),
		splitArea: Lj(Nj.splitArea, !0),
		indicator: []
	}, t;
}(Ng), zj = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n;
	}
	return t.prototype.render = function(e, t, n) {
		this.group.removeAll(), this._buildAxes(e, n), this._buildSplitLineAndArea(e);
	}, t.prototype._buildAxes = function(e, t) {
		var n = e.coordinateSystem;
		I(L(n.getIndicatorAxes(), function(e) {
			var r = e.model.get("showName") ? e.name : "";
			return new nS(e.model, t, {
				axisName: r,
				position: [n.cx, n.cy],
				rotation: e.angle,
				labelDirection: -1,
				tickDirection: -1,
				nameDirection: 1
			});
		}), function(e) {
			e.build(), this.group.add(e.group);
		}, this);
	}, t.prototype._buildSplitLineAndArea = function(e) {
		var t = e.coordinateSystem, n = t.getIndicatorAxes();
		if (!n.length) return;
		var r = e.get("shape"), i = e.getModel("splitLine"), a = e.getModel("splitArea"), o = i.getModel("lineStyle"), s = a.getModel("areaStyle"), c = i.get("show"), l = a.get("show"), u = o.get("color"), d = s.get("color"), f = V(u) ? u : [u], p = V(d) ? d : [d], m = [], h = [];
		function g(e, t, n) {
			var r = n % t.length;
			return e[r] = e[r] || [], r;
		}
		if (r === "circle") for (var _ = n[0].getTicksCoords(), v = t.cx, y = t.cy, b = 0; b < _.length; b++) {
			if (c) {
				var x = g(m, f, b);
				m[x].push(new lu({ shape: {
					cx: v,
					cy: y,
					r: _[b].coord
				} }));
			}
			if (l && b < _.length - 1) {
				var x = g(h, p, b);
				h[x].push(new Au({ shape: {
					cx: v,
					cy: y,
					r0: _[b].coord,
					r: _[b + 1].coord
				} }));
			}
		}
		else for (var S, C = L(n, function(e, n) {
			var r = e.getTicksCoords();
			return S = S == null ? r.length - 1 : Math.min(r.length - 1, S), L(r, function(e) {
				return t.coordToPoint(e.coord, n);
			});
		}), w = [], b = 0; b <= S; b++) {
			for (var T = [], E = 0; E < n.length; E++) T.push(C[E][b]);
			if (T[0] && T.push(T[0].slice()), c) {
				var x = g(m, f, b);
				m[x].push(new Iu({ shape: { points: T } }));
			}
			if (l && w) {
				var x = g(h, p, b - 1);
				h[x].push(new Pu({ shape: { points: T.concat(w) } }));
			}
			w = T.slice().reverse();
		}
		var D = o.getLineStyle(), O = s.getAreaStyle();
		I(h, function(e, t) {
			this.group.add(kd(e, {
				style: M({
					stroke: "none",
					fill: p[t % p.length]
				}, O),
				silent: !0
			}));
		}, this), I(m, function(e, t) {
			this.group.add(kd(e, {
				style: M({
					fill: "none",
					stroke: f[t % f.length]
				}, D),
				silent: !0
			}));
		}, this);
	}, t.type = "radar", t;
}(nD), Bj = function(e) {
	o(t, e);
	function t(t, n, r) {
		var i = e.call(this, t, n, r) || this;
		return i.type = "value", i.angle = 0, i.name = "", i;
	}
	return t;
}(bx), Vj = function() {
	function e(e, t, n) {
		this.type = Pj, this.dimensions = [], this._model = e, this._indicatorAxes = L(e.getIndicatorModels(), function(e, t) {
			var n = "indicator_" + t, r = new Bj(n, new my());
			return r.name = e.get("name"), r.model = e, e.axis = r, this.dimensions.push(n), r;
		}, this), this.resize(e, n);
	}
	return e.prototype.getIndicatorAxes = function() {
		return this._indicatorAxes;
	}, e.prototype.dataToPoint = function(e, t) {
		var n = this._indicatorAxes[t];
		return this.coordToPoint(n.dataToCoord(e), t);
	}, e.prototype.coordToPoint = function(e, t) {
		var n = this._indicatorAxes[t].angle;
		return [this.cx + e * Math.cos(n), this.cy - e * Math.sin(n)];
	}, e.prototype.pointToData = function(e) {
		var t = e[0] - this.cx, n = e[1] - this.cy, r = Math.sqrt(t * t + n * n);
		t /= r, n /= r;
		for (var i = Math.atan2(-n, t), a = Infinity, o, s = -1, c = 0; c < this._indicatorAxes.length; c++) {
			var l = this._indicatorAxes[c], u = Math.abs(i - l.angle);
			u < a && (o = l, s = c, a = u);
		}
		return [s, +(o && o.coordToData(r))];
	}, e.prototype.resize = function(e, t) {
		var n = Dg(e, t).refContainer, r = e.get("center"), i = e.get("clockwise") || !1, a = Math.min(n.width, n.height) / 2;
		this.cx = X(r[0], n.width) + n.x, this.cy = X(r[1], n.height) + n.y, this.startAngle = e.get("startAngle") * Math.PI / 180;
		var o = e.get("radius");
		(U(o) || se(o)) && (o = [0, o]), this.r0 = X(o[0], a), this.r = X(o[1], a);
		var s = i ? -1 : 1;
		I(this._indicatorAxes, function(e, t) {
			e.setExtent(this.r0, this.r);
			var n = this.startAngle + s * t * Math.PI * 2 / this._indicatorAxes.length;
			n = Math.atan2(Math.sin(n), Math.cos(n)), e.angle = n;
		}, this);
	}, e.prototype.update = function(e, t) {
		var n = this._indicatorAxes, r = this._model, i = uy(r.get("splitNumber"), 5), a = new my();
		a.setExtent(0, i), a.setConfig({ interval: 1 }), I(n, function(e) {
			LS(e, 1), FA(e, a);
		});
	}, e.prototype.convertToPixel = function(e, t, n) {
		return console.warn("Not implemented."), null;
	}, e.prototype.convertFromPixel = function(e, t, n) {
		return console.warn("Not implemented."), null;
	}, e.prototype.containPoint = function(e) {
		return console.warn("Not implemented."), !1;
	}, e.create = function(t, n) {
		var r = [];
		return t.eachComponent(Fj, function(i) {
			var a = new e(i, t, n);
			r.push(a), i.coordinateSystem = a;
		}), t.eachSeriesByType(Ij, function(e) {
			if (e.get("coordinateSystem") === "radar") {
				var t = e.coordinateSystem = r[e.get("radarIndex") || 0];
				t && I(t.getIndicatorAxes(), function(t) {
					dx(t, e, Pj);
				});
			}
		}), r;
	}, e.dimensions = [], e;
}();
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/radar/install.js
function Hj(e) {
	e.registerCoordinateSystem("radar", Vj), e.registerComponentModel(Rj), e.registerComponentView(zj), e.registerVisual({
		seriesType: "radar",
		reset: function(e) {
			var t = e.getData();
			t.each(function(e) {
				t.setItemVisual(e, "legendIcon", "roundRect");
			}), t.setVisual("legendIcon", "roundRect");
		}
	});
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/radar/radarLayout.js
var Uj = _c(Aj, Wj);
function Wj(e) {
	e.eachSeriesByType(Aj, function(e) {
		var t = e.getData(), n = [], r = e.coordinateSystem;
		if (r) {
			var i = r.getIndicatorAxes();
			I(i, function(e, a) {
				t.each(t.mapDimension(i[a].dim), function(e, t) {
					n[t] = n[t] || [];
					var i = r.dataToPoint(e, a);
					n[t][a] = Gj(i) ? i : Kj(r);
				});
			}), t.each(function(e) {
				var i = ie(n[e], function(e) {
					return Gj(e);
				}) || Kj(r);
				n[e].push(i.slice()), t.setItemLayout(e, n[e]);
			});
		}
	});
}
function Gj(e) {
	return !isNaN(e[0]) && !isNaN(e[1]);
}
function Kj(e) {
	return [e.cx, e.cy];
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/radar/install.js
function qj(e) {
	SA(Hj), e.registerChartView(Mj), e.registerSeriesModel(jj), e.registerLayout(Uj), e.registerProcessor(aw("radar")), e.registerPreprocessor(kj);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/gauge/PointerPath.js
var Jj = function() {
	function e() {
		this.angle = 0, this.width = 10, this.r = 10, this.x = 0, this.y = 0;
	}
	return e;
}(), Yj = function(e) {
	o(t, e);
	function t(t) {
		var n = e.call(this, t) || this;
		return n.type = "pointer", n;
	}
	return t.prototype.getDefaultShape = function() {
		return new Jj();
	}, t.prototype.buildPath = function(e, t) {
		var n = Math.cos, r = Math.sin, i = t.r, a = t.width, o = t.angle, s = t.x - n(o) * a * (a >= i / 3 ? 1 : 2), c = t.y - r(o) * a * (a >= i / 3 ? 1 : 2);
		o = t.angle - Math.PI / 2, e.moveTo(s, c), e.lineTo(t.x + n(o) * a, t.y + r(o) * a), e.lineTo(t.x + n(t.angle) * i, t.y + r(t.angle) * i), e.lineTo(t.x - n(o) * a, t.y - r(o) * a), e.lineTo(s, c);
	}, t;
}(Za);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/gauge/GaugeView.js
function Xj(e, t) {
	var n = e.get("center"), r = t.getWidth(), i = t.getHeight(), a = Math.min(r, i);
	return {
		cx: X(n[0], t.getWidth()),
		cy: X(n[1], t.getHeight()),
		r: X(e.get("radius"), a / 2)
	};
}
function Zj(e, t) {
	var n = e == null ? "" : e + "";
	return t && (U(t) ? n = t.replace("{value}", n) : H(t) && (n = t(e))), n;
}
var Qj = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n;
	}
	return t.prototype.render = function(e, t, n) {
		this.group.removeAll();
		var r = e.get([
			"axisLine",
			"lineStyle",
			"color"
		]), i = Xj(e, n);
		this._renderMain(e, t, n, r, i), this._data = e.getData();
	}, t.prototype.dispose = function() {}, t.prototype._renderMain = function(e, t, n, r, i) {
		var a = this.group, o = e.get("clockwise"), s = -e.get("startAngle") / 180 * Math.PI, c = -e.get("endAngle") / 180 * Math.PI, l = e.getModel("axisLine"), u = l.get("roundCap") ? wC : Ou, d = l.get("show"), f = l.getModel("lineStyle"), p = f.get("width"), m = [s, c];
		Ta(m, !o), s = m[0], c = m[1];
		for (var h = c - s, g = s, _ = [], v = 0; d && v < r.length; v++) {
			var y = Math.min(Math.max(r[v][0], 0), 1);
			c = s + h * y;
			var b = new u({
				shape: {
					startAngle: g,
					endAngle: c,
					cx: i.cx,
					cy: i.cy,
					clockwise: o,
					r0: i.r - p,
					r: i.r
				},
				silent: !0
			});
			b.setStyle({ fill: r[v][1] }), b.setStyle(f.getLineStyle(["color", "width"])), _.push(b), g = c;
		}
		_.reverse(), I(_, function(e) {
			return a.add(e);
		});
		var x = function(e) {
			if (e <= 0) return r[0][1];
			var t;
			for (t = 0; t < r.length; t++) if (r[t][0] >= e && (t === 0 ? 0 : r[t - 1][0]) < e) return r[t][1];
			return r[t - 1][1];
		};
		this._renderTicks(e, t, n, x, i, s, c, o, p), this._renderTitleAndDetail(e, t, n, x, i), this._renderAnchor(e, i), this._renderPointer(e, t, n, x, i, s, c, o, p);
	}, t.prototype._renderTicks = function(e, t, n, r, i, a, o, s, c) {
		for (var l = this.group, u = i.cx, d = i.cy, f = i.r, p = +e.get("min"), m = +e.get("max"), h = e.getModel("splitLine"), g = e.getModel("axisTick"), _ = e.getModel("axisLabel"), v = e.get("splitNumber"), y = g.get("splitNumber"), b = X(h.get("length"), f), x = X(g.get("length"), f), S = a, C = (o - a) / v, w = C / y, T = h.getModel("lineStyle").getLineStyle(), E = g.getModel("lineStyle").getLineStyle(), D = h.get("distance"), O, k, A = 0; A <= v; A++) {
			if (O = Math.cos(S), k = Math.sin(S), h.get("show")) {
				var j = D ? D + c : c, ee = new zu({
					shape: {
						x1: O * (f - j) + u,
						y1: k * (f - j) + d,
						x2: O * (f - b - j) + u,
						y2: k * (f - b - j) + d
					},
					style: T,
					silent: !0
				});
				T.stroke === "auto" && ee.setStyle({ stroke: r(A / v) }), l.add(ee);
			}
			if (_.get("show")) {
				var j = _.get("distance") + D, M = Zj(Z(A * (m - p) / v + p, 14), _.get("formatter")), N = r(A / v), te = O * (f - b - j) + u, P = k * (f - b - j) + d, F = _.get("rotate"), I = 0;
				F === "radial" ? (I = -S + 2 * Math.PI, I > Math.PI / 2 && (I += Math.PI)) : F === "tangential" ? I = -S - Math.PI / 2 : se(F) && (I = F * Math.PI / 180), I === 0 ? l.add(new _o({
					style: _f(_, {
						text: M,
						x: te,
						y: P,
						verticalAlign: k < -.8 ? "top" : k > .8 ? "bottom" : "middle",
						align: O < -.4 ? "left" : O > .4 ? "right" : "center"
					}, { inheritColor: N }),
					silent: !0
				})) : l.add(new _o({
					style: _f(_, {
						text: M,
						x: te,
						y: P,
						verticalAlign: "middle",
						align: "center"
					}, { inheritColor: N }),
					silent: !0,
					originX: te,
					originY: P,
					rotation: I
				}));
			}
			if (g.get("show") && A !== v) {
				var j = g.get("distance");
				j = j ? j + c : c;
				for (var L = 0; L <= y; L++) {
					O = Math.cos(S), k = Math.sin(S);
					var ne = new zu({
						shape: {
							x1: O * (f - j) + u,
							y1: k * (f - j) + d,
							x2: O * (f - x - j) + u,
							y2: k * (f - x - j) + d
						},
						silent: !0,
						style: E
					});
					E.stroke === "auto" && ne.setStyle({ stroke: r((A + L / y) / v) }), l.add(ne), S += w;
				}
				S -= w;
			} else S += C;
		}
	}, t.prototype._renderPointer = function(e, t, n, r, i, a, o, s, c) {
		var l = this.group, u = this._data, d = this._progressEls, f = [], p = e.get(["pointer", "show"]), m = e.getModel("progress"), h = m.get("show"), g = e.getData(), _ = g.mapDimension("value"), v = +e.get("min"), y = +e.get("max"), b = [v, y], x = [a, o];
		function S(t, n) {
			var r = g.getItemModel(t).getModel("pointer"), a = X(r.get("width"), i.r), o = X(r.get("length"), i.r), s = e.get(["pointer", "icon"]), c = r.get("offsetCenter"), l = X(c[0], i.r), u = X(c[1], i.r), d = r.get("keepAspect"), f = s ? Y_(s, l - a / 2, u - o, a, o, null, d) : new Yj({ shape: {
				angle: -Math.PI / 2,
				width: a,
				r: o,
				x: l,
				y: u
			} });
			return f.rotation = -(n + Math.PI / 2), f.x = i.cx, f.y = i.cy, f;
		}
		function C(e, t) {
			var n = m.get("roundCap") ? wC : Ou, r = m.get("overlap"), o = r ? m.get("width") : c / g.count(), l = r ? i.r - o : i.r - (e + 1) * o, u = r ? i.r : i.r - e * o, d = new n({ shape: {
				startAngle: a,
				endAngle: t,
				cx: i.cx,
				cy: i.cy,
				clockwise: s,
				r0: l,
				r: u
			} });
			return r && (d.z2 = Go(g.get(_, e), [v, y], [100, 0], !0)), d;
		}
		(h || p) && (g.diff(u).add(function(t) {
			var n = g.get(_, t);
			if (p) {
				var r = S(t, a);
				dd(r, { rotation: -((isNaN(+n) ? x[0] : Go(n, b, x, !0)) + Math.PI / 2) }, e), l.add(r), g.setItemGraphicEl(t, r);
			}
			if (h) {
				var i = C(t, a);
				dd(i, { shape: { endAngle: Go(n, b, x, m.get("clip")) } }, e), l.add(i), bc(e.seriesIndex, g.dataType, t, i), f[t] = i;
			}
		}).update(function(t, n) {
			var r = g.get(_, t);
			if (p) {
				var i = u.getItemGraphicEl(n), o = i ? i.rotation : a, s = S(t, o);
				s.rotation = o, ud(s, { rotation: -((isNaN(+r) ? x[0] : Go(r, b, x, !0)) + Math.PI / 2) }, e), l.add(s), g.setItemGraphicEl(t, s);
			}
			if (h) {
				var c = d[n], v = C(t, c ? c.shape.endAngle : a);
				ud(v, { shape: { endAngle: Go(r, b, x, m.get("clip")) } }, e), l.add(v), bc(e.seriesIndex, g.dataType, t, v), f[t] = v;
			}
		}).execute(), g.each(function(e) {
			var t = g.getItemModel(e), n = t.getModel("emphasis"), i = n.get("focus"), a = n.get("blurScope"), o = n.get("disabled"), s = r(Go(g.get(_, e), b, [0, 1], !0));
			if (p) {
				var c = g.getItemGraphicEl(e), l = g.getItemVisual(e, "style"), u = l.fill;
				if (c instanceof ro) {
					var d = c.style;
					c.useStyle(j({
						image: d.image,
						x: d.x,
						y: d.y,
						width: d.width,
						height: d.height
					}, l));
				} else c.useStyle(l), c.type !== "pointer" && c.setColor(u);
				c.setStyle(t.getModel(["pointer", "itemStyle"]).getItemStyle()), c.style.fill === "auto" && c.setStyle("fill", s), c.z2EmphasisLift = 0, Ml(c, t), Ol(c, i, a, o);
			}
			if (h) {
				var m = f[e];
				m.useStyle(g.getItemVisual(e, "style")), m.setStyle(t.getModel(["progress", "itemStyle"]).getItemStyle()), m.style.fill === "auto" && m.setStyle("fill", s), m.z2EmphasisLift = 0, Ml(m, t), Ol(m, i, a, o);
			}
		}), this._progressEls = f);
	}, t.prototype._renderAnchor = function(e, t) {
		var n = e.getModel("anchor");
		if (n.get("show")) {
			var r = n.get("size"), i = n.get("icon"), a = n.get("offsetCenter"), o = n.get("keepAspect"), s = Y_(i, t.cx - r / 2 + X(a[0], t.r), t.cy - r / 2 + X(a[1], t.r), r, r, null, o);
			s.z2 = +!!n.get("showAbove"), s.setStyle(n.getModel("itemStyle").getItemStyle()), this.group.add(s);
		}
	}, t.prototype._renderTitleAndDetail = function(e, t, n, r, i) {
		var a = this, o = e.getData(), s = o.mapDimension("value"), c = +e.get("min"), l = +e.get("max"), u = new su(), d = [], f = [], p = e.isAnimationEnabled(), m = e.get(["pointer", "showAbove"]);
		o.diff(this._data).add(function(e) {
			d[e] = new _o({ silent: !0 }), f[e] = new _o({ silent: !0 });
		}).update(function(e, t) {
			d[e] = a._titleEls[t], f[e] = a._detailEls[t];
		}).execute(), o.each(function(t) {
			var n = o.getItemModel(t), a = o.get(s, t), h = new su(), g = r(Go(a, [c, l], [0, 1], !0)), _ = n.getModel("title");
			if (_.get("show")) {
				var v = _.get("offsetCenter"), y = i.cx + X(v[0], i.r), b = i.cy + X(v[1], i.r), x = d[t];
				x.attr({
					z2: m ? 0 : 2,
					style: _f(_, {
						x: y,
						y: b,
						text: o.getName(t),
						align: "center",
						verticalAlign: "middle"
					}, { inheritColor: g })
				}), h.add(x);
			}
			var S = n.getModel("detail");
			if (S.get("show")) {
				var C = S.get("offsetCenter"), w = i.cx + X(C[0], i.r), T = i.cy + X(C[1], i.r), E = X(S.get("width"), i.r), D = X(S.get("height"), i.r), O = e.get(["progress", "show"]) ? o.getItemVisual(t, "style").fill : g, x = f[t], k = S.get("formatter");
				x.attr({
					z2: m ? 0 : 2,
					style: _f(S, {
						x: w,
						y: T,
						text: Zj(a, k),
						width: isNaN(E) ? null : E,
						height: isNaN(D) ? null : D,
						align: "center",
						verticalAlign: "middle"
					}, { inheritColor: O })
				}), Df(x, { normal: S }, a, function(e) {
					return Zj(e, k);
				}), p && Of(x, t, o, e, { getFormattedLabel: function(e, t, n, r, i, o) {
					return Zj(o ? o.interpolatedValue : a, k);
				} }), h.add(x);
			}
			u.add(h);
		}), this.group.add(u), this._titleEls = d, this._detailEls = f;
	}, t.type = "gauge", t;
}(Ov), $j = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n.visualStyleAccessPath = "itemStyle", n;
	}
	return t.prototype.getInitialData = function(e, t) {
		return ow(this, ["value"]);
	}, t.type = "series.gauge", t.defaultOption = {
		z: 2,
		colorBy: "data",
		center: ["50%", "50%"],
		legendHoverLink: !0,
		radius: "75%",
		startAngle: 225,
		endAngle: -45,
		clockwise: !0,
		min: 0,
		max: 100,
		splitNumber: 10,
		axisLine: {
			show: !0,
			roundCap: !1,
			lineStyle: {
				color: [[1, Q.color.neutral10]],
				width: 10
			}
		},
		progress: {
			show: !1,
			overlap: !0,
			width: 10,
			roundCap: !1,
			clip: !0
		},
		splitLine: {
			show: !0,
			length: 10,
			distance: 10,
			lineStyle: {
				color: Q.color.axisTick,
				width: 3,
				type: "solid"
			}
		},
		axisTick: {
			show: !0,
			splitNumber: 5,
			length: 6,
			distance: 10,
			lineStyle: {
				color: Q.color.axisTickMinor,
				width: 1,
				type: "solid"
			}
		},
		axisLabel: {
			show: !0,
			distance: 15,
			color: Q.color.axisLabel,
			fontSize: 12,
			rotate: 0
		},
		pointer: {
			icon: null,
			offsetCenter: [0, 0],
			show: !0,
			showAbove: !0,
			length: "60%",
			width: 6,
			keepAspect: !1
		},
		anchor: {
			show: !1,
			showAbove: !1,
			size: 6,
			icon: "circle",
			offsetCenter: [0, 0],
			keepAspect: !1,
			itemStyle: {
				color: Q.color.neutral00,
				borderWidth: 0,
				borderColor: Q.color.theme[0]
			}
		},
		title: {
			show: !0,
			offsetCenter: [0, "20%"],
			color: Q.color.secondary,
			fontSize: 16,
			valueAnimation: !1
		},
		detail: {
			show: !0,
			backgroundColor: Q.color.transparent,
			borderWidth: 0,
			borderColor: Q.color.neutral40,
			width: 100,
			height: null,
			padding: [5, 10],
			offsetCenter: [0, "40%"],
			color: Q.color.primary,
			fontSize: 30,
			fontWeight: "bold",
			lineHeight: 30,
			valueAnimation: !1
		}
	}, t;
}(P_);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/gauge/install.js
function eM(e) {
	e.registerChartView(Qj), e.registerSeriesModel($j);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/funnel/FunnelSeries.js
var tM = "funnel", nM = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n;
	}
	return t.prototype.init = function(t) {
		e.prototype.init.apply(this, arguments), this.legendVisualProvider = new sw(z(this.getData, this), z(this.getRawData, this)), this._defaultLabelLine(t);
	}, t.prototype.getInitialData = function(e, t) {
		return ow(this, {
			coordDimensions: ["value"],
			encodeDefaulter: B(Jf, this)
		});
	}, t.prototype._defaultLabelLine = function(e) {
		Ts(e, "labelLine", ["show"]);
		var t = e.labelLine, n = e.emphasis.labelLine;
		t.show = t.show && e.label.show, n.show = n.show && e.emphasis.label.show;
	}, t.prototype.getDataParams = function(t) {
		var n = this.getData(), r = e.prototype.getDataParams.call(this, t), i = n.mapDimension("value"), a = n.getSum(i);
		return r.percent = a ? +(n.get(i, t) / a * 100).toFixed(2) : 0, r.$vars.push("percent"), r;
	}, t.type = "series." + tM, t.defaultOption = {
		coordinateSystemUsage: "box",
		z: 2,
		legendHoverLink: !0,
		colorBy: "data",
		left: 80,
		top: 60,
		right: 80,
		bottom: 65,
		minSize: "0%",
		maxSize: "100%",
		sort: "descending",
		orient: "vertical",
		gap: 0,
		funnelAlign: "center",
		label: {
			show: !0,
			position: "outer"
		},
		labelLine: {
			show: !0,
			length: 20,
			lineStyle: { width: 1 }
		},
		itemStyle: {
			borderColor: Q.color.neutral00,
			borderWidth: 1
		},
		emphasis: { label: { show: !0 } },
		select: { itemStyle: { borderColor: Q.color.primary } }
	}, t;
}(P_), rM = ["itemStyle", "opacity"], iM = function(e) {
	o(t, e);
	function t(t, n) {
		var r = e.call(this) || this, i = r, a = new Iu(), o = new _o();
		return i.setTextContent(o), r.setTextGuideLine(a), r.updateData(t, n, !0), r;
	}
	return t.prototype.updateData = function(e, t, n) {
		var r = this, i = e.hostModel, a = e.getItemModel(t), o = e.getItemLayout(t), s = a.getModel("emphasis"), c = a.get(rM);
		c ??= 1, n || gd(r), r.useStyle(e.getItemVisual(t, "style")), r.style.lineJoin = "round", n ? (r.setShape({ points: o.points }), r.style.opacity = 0, dd(r, { style: { opacity: c } }, i, t)) : ud(r, {
			style: { opacity: c },
			shape: { points: o.points }
		}, i, t), Ml(r, a), this._updateLabel(e, t), Ol(this, s.get("focus"), s.get("blurScope"), s.get("disabled"));
	}, t.prototype._updateLabel = function(e, t) {
		var n = this, r = this.getTextGuideLine(), i = n.getTextContent(), a = e.hostModel, o = e.getItemModel(t), s = e.getItemLayout(t).label, c = e.getItemVisual(t, "style"), l = c.fill;
		hf(i, gf(o), {
			labelFetcher: e.hostModel,
			labelDataIndex: t,
			defaultOpacity: c.opacity,
			defaultText: e.getName(t)
		}, { normal: {
			align: s.textAlign,
			verticalAlign: s.verticalAlign
		} });
		var u = o.getModel("label").get("color") === "inherit" ? l : null;
		n.setTextConfig({
			local: !0,
			inside: !!s.inside,
			insideStroke: u,
			outsideFill: u
		});
		var d = s.linePoints;
		r.setShape({ points: d }), n.textGuideLineConfig = { anchor: d ? new J(d[0][0], d[0][1]) : null }, ud(i, { style: {
			x: s.x,
			y: s.y
		} }, a, t), i.attr({
			rotation: s.rotation,
			originX: s.x,
			originY: s.y,
			z2: 10
		}), Sw(n, Cw(o), { stroke: l });
	}, t;
}(Pu), aM = function(e) {
	o(t, e);
	function t() {
		var t = e !== null && e.apply(this, arguments) || this;
		return t.type = tM, t.ignoreLabelLineUpdate = !0, t;
	}
	return t.prototype.render = function(e, t, n) {
		var r = e.getData(), i = this._data, a = this.group;
		r.diff(i).add(function(e) {
			var t = new iM(r, e);
			r.setItemGraphicEl(e, t), a.add(t);
		}).update(function(e, t) {
			var n = i.getItemGraphicEl(t);
			n.updateData(r, e), a.add(n), r.setItemGraphicEl(e, n);
		}).remove(function(t) {
			hd(i.getItemGraphicEl(t), e, t);
		}).execute(), this._data = r;
	}, t.prototype.remove = function() {
		this.group.removeAll(), this._data = null;
	}, t.prototype.dispose = function() {}, t.type = tM, t;
}(Ov);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/funnel/funnelLayout.js
function oM(e, t) {
	for (var n = e.mapDimension("value"), r = e.mapArray(n, function(e) {
		return e;
	}), i = [], a = t === "ascending", o = 0, s = e.count(); o < s; o++) i[o] = o;
	return H(t) ? i.sort(t) : t !== "none" && i.sort(function(e, t) {
		return a ? r[e] - r[t] : r[t] - r[e];
	}), i;
}
function sM(e) {
	var t = e.hostModel, n = uM(t);
	e.each(function(t) {
		var r = e.getItemModel(t), i = r.getModel("label").get("position"), a = r.getModel("labelLine"), o = e.getItemLayout(t), s = o.points, c = i === "inner" || i === "inside" || i === "center" || i === "insideLeft" || i === "insideRight", l, u, d, f;
		if (c) i === "insideLeft" ? (u = (s[0][0] + s[3][0]) / 2 + 5, d = (s[0][1] + s[3][1]) / 2, l = "left") : i === "insideRight" ? (u = (s[1][0] + s[2][0]) / 2 - 5, d = (s[1][1] + s[2][1]) / 2, l = "right") : (u = (s[0][0] + s[1][0] + s[2][0] + s[3][0]) / 4, d = (s[0][1] + s[1][1] + s[2][1] + s[3][1]) / 4, l = "center"), f = [[u, d], [u, d]];
		else {
			var p = void 0, m = void 0, h = void 0, g = void 0, _ = a.get("length");
			U(i) && (!n && N(["top", "bottom"], i) > -1 && (i = "left"), n && N(["left", "right"], i) > -1 && (i = "bottom")), i === "left" ? (p = (s[3][0] + s[0][0]) / 2, m = (s[3][1] + s[0][1]) / 2, h = p - _, u = h - 5, l = "right") : i === "right" ? (p = (s[1][0] + s[2][0]) / 2, m = (s[1][1] + s[2][1]) / 2, h = p + _, u = h + 5, l = "left") : i === "top" ? (p = (s[3][0] + s[0][0]) / 2, m = (s[3][1] + s[0][1]) / 2, g = m - _, d = g - 5, l = "center") : i === "bottom" ? (p = (s[1][0] + s[2][0]) / 2, m = (s[1][1] + s[2][1]) / 2, g = m + _, d = g + 5, l = "center") : i === "rightTop" ? (p = n ? s[3][0] : s[1][0], m = n ? s[3][1] : s[1][1], n ? (g = m - _, d = g - 5, l = "center") : (h = p + _, u = h + 5, l = "top")) : i === "rightBottom" ? (p = s[2][0], m = s[2][1], n ? (g = m + _, d = g + 5, l = "center") : (h = p + _, u = h + 5, l = "bottom")) : i === "leftTop" ? (p = s[0][0], m = n ? s[0][1] : s[1][1], n ? (g = m - _, d = g - 5, l = "center") : (h = p - _, u = h - 5, l = "right")) : i === "leftBottom" ? (p = n ? s[1][0] : s[3][0], m = n ? s[1][1] : s[2][1], n ? (g = m + _, d = g + 5, l = "center") : (h = p - _, u = h - 5, l = "right")) : (p = (s[1][0] + s[2][0]) / 2, m = (s[1][1] + s[2][1]) / 2, n ? (g = m + _, d = g + 5, l = "center") : (h = p + _, u = h + 5, l = "left")), n ? (h = p, u = h) : (g = m, d = g), f = [[p, m], [h, g]];
		}
		o.label = {
			linePoints: f,
			x: u,
			y: d,
			verticalAlign: "middle",
			textAlign: l,
			inside: c
		};
	});
}
var cM = _c(tM, lM);
function lM(e, t) {
	e.eachSeriesByType(tM, function(e) {
		var n = e.getData(), r = n.mapDimension("value"), i = e.get("sort"), a = Dg(e, t), o = Tg(e.getBoxLayoutParams(), a.refContainer), s = uM(e), c = o.width, l = o.height, u = oM(n, i), d = o.x, f = o.y, p = s ? [X(e.get("minSize"), l), X(e.get("maxSize"), l)] : [X(e.get("minSize"), c), X(e.get("maxSize"), c)], m = n.getDataExtent(r), h = e.get("min"), g = e.get("max");
		h ??= Math.min(m[0], 0), g ??= m[1];
		var _ = e.get("funnelAlign"), v = e.get("gap"), y = ((s ? c : l) - v * (n.count() - 1)) / n.count(), b = function(e, t) {
			if (s) {
				var i = Go(n.get(r, e) || 0, [h, g], p, !0), a = void 0;
				switch (_) {
					case "top":
						a = f;
						break;
					case "center":
						a = f + (l - i) / 2;
						break;
					case "bottom":
						a = f + (l - i);
						break;
				}
				return [[t, a], [t, a + i]];
			}
			var o = Go(n.get(r, e) || 0, [h, g], p, !0), u;
			switch (_) {
				case "left":
					u = d;
					break;
				case "center":
					u = d + (c - o) / 2;
					break;
				case "right":
					u = d + c - o;
					break;
			}
			return [[u, t], [u + o, t]];
		};
		i === "ascending" && (y = -y, v = -v, s ? d += c : f += l, u = u.reverse());
		for (var x = 0; x < u.length; x++) {
			var S = u[x], C = u[x + 1], w = n.getItemModel(S);
			if (s) {
				var T = w.get(["itemStyle", "width"]);
				T == null ? T = y : (T = X(T, c), i === "ascending" && (T = -T));
				var E = b(S, d), D = b(C, d + T);
				d += T + v, n.setItemLayout(S, { points: E.concat(D.slice().reverse()) });
			} else {
				var O = w.get(["itemStyle", "height"]);
				O == null ? O = y : (O = X(O, l), i === "ascending" && (O = -O));
				var E = b(S, f), D = b(C, f + O);
				f += O + v, n.setItemLayout(S, { points: E.concat(D.slice().reverse()) });
			}
		}
		sM(n);
	});
}
function uM(e) {
	return e.get("orient") === "horizontal";
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/chart/funnel/install.js
function dM(e) {
	e.registerChartView(aM), e.registerSeriesModel(nM), e.registerLayout(cM), e.registerProcessor(aw(tM));
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/axisPointer/BaseAxisPointer.js
var fM = Ws(), pM = k, mM = z, hM = function() {
	function e() {
		this._dragging = !1, this.animationThreshold = 15;
	}
	return e.prototype.render = function(e, t, n, r) {
		var i = t.get("value"), a = t.get("status");
		if (this._axisModel = e, this._axisPointerModel = t, this._api = n, !(!r && this._lastValue === i && this._lastStatus === a)) {
			this._lastValue = i, this._lastStatus = a;
			var o = this._group, s = this._handle;
			if (!a || a === "hide") {
				o && o.hide(), s && s.hide();
				return;
			}
			o && o.show(), s && s.show();
			var c = {};
			this.makeElOption(c, i, e, t, n);
			var l = c.graphicKey;
			l !== this._lastGraphicKey && this.clear(n), this._lastGraphicKey = l;
			var u = this._moveAnimation = this.determineAnimation(e, t);
			if (!o) o = this._group = new su(), this.createPointerEl(o, c, e, t), this.createLabelEl(o, c, e, t), n.getZr().add(o);
			else {
				var d = B(gM, t, u);
				this.updatePointerEl(o, c, d), this.updateLabelEl(o, c, d, t);
			}
			bM(o, t, !0), this._renderHandle(i);
		}
	}, e.prototype.remove = function(e) {
		this.clear(e);
	}, e.prototype.dispose = function(e) {
		this.clear(e);
	}, e.prototype.determineAnimation = function(e, t) {
		var n = t.get("animation"), r = e.axis, i = r.type === "category", a = t.get("snap");
		if (!a && !i) return !1;
		if (n === "auto" || n == null) {
			var o = this.animationThreshold;
			if (i && gx(r).w > o) return !0;
			if (a) {
				var s = fj(e).seriesDataCount, c = r.getExtent();
				return Math.abs(c[0] - c[1]) / s > o;
			}
			return !1;
		}
		return n === !0;
	}, e.prototype.makeElOption = function(e, t, n, r, i) {}, e.prototype.createPointerEl = function(e, t, n, r) {
		var i = t.pointer;
		if (i) {
			var a = fM(e).pointerEl = new _d[i.type](pM(t.pointer));
			e.add(a);
		}
	}, e.prototype.createLabelEl = function(e, t, n, r) {
		if (t.label) {
			var i = fM(e).labelEl = new _o(pM(t.label));
			e.add(i), vM(i, r);
		}
	}, e.prototype.updatePointerEl = function(e, t, n) {
		var r = fM(e).pointerEl;
		r && t.pointer && (r.setStyle(t.pointer.style), n(r, { shape: t.pointer.shape }));
	}, e.prototype.updateLabelEl = function(e, t, n, r) {
		var i = fM(e).labelEl;
		i && (i.setStyle(t.label.style), n(i, {
			x: t.label.x,
			y: t.label.y
		}), vM(i, r));
	}, e.prototype._renderHandle = function(e) {
		if (!(this._dragging || !this.updateHandleTransform)) {
			var t = this._axisPointerModel, n = this._api.getZr(), r = this._handle, i = t.getModel("handle"), a = t.get("status");
			if (!i.get("show") || !a || a === "hide") {
				r && n.remove(r), this._handle = null;
				return;
			}
			var o;
			this._handle || (o = !0, r = this._handle = Hd(i.get("icon"), {
				cursor: "move",
				draggable: !0,
				onmousemove: function(e) {
					eT(e.event);
				},
				onmousedown: mM(this._onHandleDragMove, this, 0, 0),
				drift: mM(this._onHandleDragMove, this),
				ondragend: mM(this._onHandleDragEnd, this)
			}), n.add(r)), bM(r, t, !1), r.setStyle(i.getItemStyle(null, [
				"color",
				"borderColor",
				"borderWidth",
				"opacity",
				"shadowColor",
				"shadowBlur",
				"shadowOffsetX",
				"shadowOffsetY"
			]));
			var s = i.get("size");
			V(s) || (s = [s, s]), r.scaleX = s[0] / 2, r.scaleY = s[1] / 2, xC(this, "_doDispatchAxisPointer", i.get("throttle") || 0, "fixRate"), this._moveHandleToValue(e, o);
		}
	}, e.prototype._moveHandleToValue = function(e, t) {
		gM(this._axisPointerModel, !t && this._moveAnimation, this._handle, yM(this.getHandleTransform(e, this._axisModel, this._axisPointerModel)));
	}, e.prototype._onHandleDragMove = function(e, t) {
		var n = this._handle;
		if (n) {
			this._dragging = !0;
			var r = this.updateHandleTransform(yM(n), [e, t], this._axisModel, this._axisPointerModel);
			this._payloadInfo = r, n.stopAnimation(), n.attr(yM(r)), fM(n).lastProp = null, this._doDispatchAxisPointer();
		}
	}, e.prototype._doDispatchAxisPointer = function() {
		if (this._handle) {
			var e = this._payloadInfo, t = this._axisModel;
			this._api.dispatchAction({
				type: "updateAxisPointer",
				x: e.cursorPoint[0],
				y: e.cursorPoint[1],
				tooltipOption: e.tooltipOption,
				axesInfo: [{
					axisDim: t.axis.dim,
					axisIndex: t.componentIndex
				}]
			});
		}
	}, e.prototype._onHandleDragEnd = function() {
		if (this._dragging = !1, this._handle) {
			var e = this._axisPointerModel.get("value");
			this._moveHandleToValue(e), this._api.dispatchAction({ type: "hideTip" });
		}
	}, e.prototype.clear = function(e) {
		this._lastValue = null, this._lastStatus = null;
		var t = e.getZr(), n = this._group, r = this._handle;
		t && n && (this._lastGraphicKey = null, n && t.remove(n), r && t.remove(r), this._group = null, this._handle = null, this._payloadInfo = null), SC(this, "_doDispatchAxisPointer");
	}, e.prototype.doClear = function() {}, e.prototype.buildLabel = function(e, t, n) {
		return n ||= 0, {
			x: e[n],
			y: e[1 - n],
			width: t[n],
			height: t[1 - n]
		};
	}, e;
}();
function gM(e, t, n, r) {
	_M(fM(n).lastProp, r) || (fM(n).lastProp = r, t ? ud(n, r, e) : (n.stopAnimation(), n.attr(r)));
}
function _M(e, t) {
	if (W(e) && W(t)) {
		var n = !0;
		return I(t, function(t, r) {
			n &&= _M(e[r], t);
		}), !!n;
	} else return e === t;
}
function vM(e, t) {
	e[t.get(["label", "show"]) ? "show" : "hide"]();
}
function yM(e) {
	return {
		x: e.x || 0,
		y: e.y || 0,
		rotation: e.rotation || 0
	};
}
function bM(e, t, n) {
	var r = t.get("z"), i = t.get("zlevel");
	e && e.traverse(function(e) {
		e.type !== "group" && (r != null && (e.z = r), i != null && (e.zlevel = i), e.silent = n);
	});
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/axisPointer/viewHelper.js
function xM(e) {
	var t = e.get("type"), n = e.getModel(t + "Style"), r;
	return t === "line" ? (r = n.getLineStyle(), r.fill = null) : t === "shadow" && (r = n.getAreaStyle(), r.stroke = null), r;
}
function SM(e, t, n, r, i) {
	var a = wM(n.get("value"), t.axis, t.ecModel, n.get("seriesDataIndices"), {
		precision: n.get(["label", "precision"]),
		formatter: n.get(["label", "formatter"])
	}), o = n.getModel("label"), s = lg(o.get("padding") || 0), c = o.getFont(), l = sn(a, c), u = i.position, d = l.width + s[1] + s[3], f = l.height + s[0] + s[2], p = i.align;
	p === "right" && (u[0] -= d), p === "center" && (u[0] -= d / 2);
	var m = i.verticalAlign;
	m === "bottom" && (u[1] -= f), m === "middle" && (u[1] -= f / 2), CM(u, d, f, r);
	var h = o.get("backgroundColor");
	(!h || h === "auto") && (h = t.get([
		"axisLine",
		"lineStyle",
		"color"
	])), e.label = {
		x: u[0],
		y: u[1],
		style: _f(o, {
			text: a,
			font: c,
			fill: o.getTextColor(),
			padding: s,
			backgroundColor: h
		}),
		z2: 10
	};
}
function CM(e, t, n, r) {
	var i = r.getWidth(), a = r.getHeight();
	e[0] = Math.min(e[0] + t, i) - t, e[1] = Math.min(e[1] + n, a) - n, e[0] = Math.max(e[0], 0), e[1] = Math.max(e[1], 0);
}
function wM(e, t, n, r, i) {
	e = t.scale.parse(e);
	var a = t.scale.getLabel({ value: e }, { precision: i.precision }), o = i.formatter;
	if (o) {
		var s = {
			value: Uy(t, { value: e }),
			axisDimension: t.dim,
			axisIndex: t.index,
			seriesData: []
		};
		I(r, function(e) {
			var t = n.getSeriesByIndex(e.seriesIndex), r = e.dataIndexInside, i = t && t.getDataParams(r);
			i && s.seriesData.push(i);
		}), U(o) ? a = o.replace("{value}", a) : H(o) && (a = o(s));
	}
	return a;
}
function TM(e, t, n) {
	var r = ot();
	return dt(r, r, n.rotation), ut(r, r, n.position), Fd([e.dataToCoord(t), (n.labelOffset || 0) + (n.labelDirection || 1) * (n.labelMargin || 0)], r);
}
function EM(e, t, n, r, i, a) {
	var o = nS.innerTextLayout(n.rotation, 0, n.labelDirection);
	n.labelMargin = i.get(["label", "margin"]), SM(t, r, i, a, {
		position: TM(r.axis, e, n),
		align: o.textAlign,
		verticalAlign: o.textVerticalAlign
	});
}
function DM(e, t, n) {
	return n ||= 0, {
		x1: e[n],
		y1: e[1 - n],
		x2: t[n],
		y2: t[1 - n]
	};
}
function OM(e, t, n) {
	return n ||= 0, {
		x: e[n],
		y: e[1 - n],
		width: t[n],
		height: t[1 - n]
	};
}
function kM(e, t, n) {
	return gx(e, {
		fromStat: { sers: L(t, function(e) {
			return n.getSeriesByIndex(e.seriesIndex);
		}) },
		min: 1
	}).w;
}
function AM(e, t, n) {
	return [Fo(Po(t[0], t[1]), e - n / 2), Po(e + n / 2, Fo(t[0], t[1]))];
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/axisPointer/CartesianAxisPointer.js
var jM = function(e) {
	o(t, e);
	function t() {
		return e !== null && e.apply(this, arguments) || this;
	}
	return t.prototype.makeElOption = function(e, t, n, r, i) {
		var a = n.axis, o = a.grid, s = r.get("type"), c = a.getGlobalExtent(), l = MM(o, a).getOtherAxis(a).getGlobalExtent(), u = a.toGlobalCoord(a.dataToCoord(t, !0));
		if (s && s !== "none") {
			var d = xM(r), f = NM[s](a, u, c, l, r.get("seriesDataIndices"), r.ecModel);
			f.style = d, e.graphicKey = f.type, e.pointer = f;
		}
		EM(t, e, CS(o.getRect(), n), n, r, i);
	}, t.prototype.getHandleTransform = function(e, t, n) {
		var r = CS(t.axis.grid.getRect(), t, { labelInside: !1 });
		r.labelMargin = n.get(["handle", "margin"]);
		var i = TM(t.axis, e, r);
		return {
			x: i[0],
			y: i[1],
			rotation: r.rotation + (r.labelDirection < 0 ? Math.PI : 0)
		};
	}, t.prototype.updateHandleTransform = function(e, t, n, r) {
		var i = n.axis, a = i.grid, o = i.getGlobalExtent(!0), s = MM(a, i).getOtherAxis(i).getGlobalExtent(), c = i.dim === "x" ? 0 : 1, l = [e.x, e.y];
		l[c] += t[c], l[c] = Po(o[1], l[c]), l[c] = Fo(o[0], l[c]);
		var u = (s[1] + s[0]) / 2, d = [u, u];
		return d[c] = l[c], {
			x: l[0],
			y: l[1],
			rotation: e.rotation,
			cursorPoint: d,
			tooltipOption: [{ verticalAlign: "middle" }, { align: "center" }][c]
		};
	}, t;
}(hM);
function MM(e, t) {
	var n = {};
	return n[t.dim + "AxisIndex"] = t.index, e.getCartesian(n);
}
var NM = {
	line: function(e, t, n, r) {
		return {
			type: "Line",
			subPixelOptimize: !0,
			shape: DM([t, r[0]], [t, r[1]], PM(e))
		};
	},
	shadow: function(e, t, n, r, i, a) {
		var o = kM(e, i, a), s = r[1] - r[0], c = AM(t, n, o), l = c[0], u = c[1];
		return {
			type: "Rect",
			shape: OM([l, r[0]], [u - l, s], PM(e))
		};
	}
};
function PM(e) {
	return e.dim === "x" ? 0 : 1;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/axisPointer/AxisPointerModel.js
var FM = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n;
	}
	return t.type = "axisPointer", t.defaultOption = {
		show: "auto",
		z: 50,
		type: "line",
		snap: !1,
		triggerTooltip: !0,
		triggerEmphasis: !0,
		value: null,
		status: null,
		link: [],
		animation: null,
		animationDurationUpdate: 200,
		lineStyle: {
			color: Q.color.border,
			width: 1,
			type: "dashed"
		},
		shadowStyle: { color: Q.color.shadowTint },
		label: {
			show: !0,
			formatter: null,
			precision: "auto",
			margin: 3,
			color: Q.color.neutral00,
			padding: [
				5,
				7,
				5,
				7
			],
			backgroundColor: Q.color.accent60,
			borderColor: null,
			borderWidth: 0,
			borderRadius: 3
		},
		handle: {
			show: !1,
			icon: "M10.7,11.9v-1.3H9.3v1.3c-4.9,0.3-8.8,4.4-8.8,9.4c0,5,3.9,9.1,8.8,9.4h1.3c4.9-0.3,8.8-4.4,8.8-9.4C19.5,16.3,15.6,12.2,10.7,11.9z M13.3,24.4H6.7v-1.2h6.6z M13.3,22H6.7v-1.2h6.6z M13.3,19.6H6.7v-1.2h6.6z",
			size: 45,
			margin: 50,
			color: Q.color.accent40,
			throttle: 40
		}
	}, t;
}(Ng), IM = Ws(), LM = I;
function RM(e, t, n) {
	if (!q.node) {
		var r = t.getZr();
		IM(r).records || (IM(r).records = {}), zM(r, t);
		var i = IM(r).records[e] || (IM(r).records[e] = {});
		i.handler = n;
	}
}
function zM(e, t) {
	if (IM(e).initialized) return;
	IM(e).initialized = !0, n("click", B(HM, "click")), n("mousemove", B(HM, "mousemove")), n("mousewheel", B(HM, "mousewheel")), n("globalout", VM);
	function n(n, r) {
		e.on(n, function(n) {
			var i = UM(t);
			LM(IM(e).records, function(e) {
				e && r(e, n, i.dispatchAction);
			}), BM(i.pendings, t);
		});
	}
}
function BM(e, t) {
	var n = e.showTip.length, r = e.hideTip.length, i;
	n ? i = e.showTip[n - 1] : r && (i = e.hideTip[r - 1]), i && (i.dispatchAction = null, t.dispatchAction(i));
}
function VM(e, t, n) {
	e.handler("leave", null, n);
}
function HM(e, t, n, r) {
	t.handler(e, n, r);
}
function UM(e) {
	var t = {
		showTip: [],
		hideTip: []
	}, n = function(r) {
		var i = t[r.type];
		i ? i.push(r) : (r.dispatchAction = n, e.dispatchAction(r));
	};
	return {
		dispatchAction: n,
		pendings: t
	};
}
function WM(e, t) {
	if (!q.node) {
		var n = t.getZr();
		(IM(n).records || {})[e] && (IM(n).records[e] = null);
	}
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/axisPointer/AxisPointerView.js
var GM = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n;
	}
	return t.prototype.render = function(e, t, n) {
		var r = t.getComponent("tooltip"), i = e.get("triggerOn") || r && r.get("triggerOn") || "mousemove|click|mousewheel";
		RM("axisPointer", n, function(e, t, n) {
			i !== "none" && (e === "leave" || i.indexOf(e) >= 0) && n({
				type: "updateAxisPointer",
				currTrigger: e,
				x: t && t.offsetX,
				y: t && t.offsetY
			});
		});
	}, t.prototype.remove = function(e, t) {
		WM("axisPointer", t);
	}, t.prototype.dispose = function(e, t) {
		WM("axisPointer", t);
	}, t.type = "axisPointer", t;
}(nD);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/axisPointer/findPointFromSeries.js
function KM(e, t) {
	var n = [], r = e.seriesIndex, i;
	if (r == null || !(i = t.getSeriesByIndex(r))) return { point: [] };
	var a = i.getData(), o = Us(a, e);
	if (o == null || o < 0 || V(o)) return { point: [] };
	var s = a.getItemGraphicEl(o), c = i.coordinateSystem;
	if (i.getTooltipPosition) n = i.getTooltipPosition(o) || [];
	else if (c && c.dataToPoint) if (e.isStacked) {
		var l = c.getBaseAxis(), u = c.getOtherAxis(l).dim, d = l.dim, f = +(u === "x" || u === "radius"), p = a.mapDimension(d), m = [];
		m[f] = a.get(p, o), m[1 - f] = a.get(a.getCalculationInfo("stackResultDimension"), o), n = c.dataToPoint(m) || [];
	} else n = c.dataToPoint(a.getValues(L(c.dimensions, function(e) {
		return a.mapDimension(e);
	}), o)) || [];
	else if (s) {
		var h = s.getBoundingRect().clone();
		h.applyTransform(s.transform), n = [h.x + h.width / 2, h.y + h.height / 2];
	}
	return {
		point: n,
		el: s
	};
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/axisPointer/axisTrigger.js
var qM = Ws();
function JM(e, t, n) {
	var r = e.currTrigger, i = [e.x, e.y], a = e, o = e.dispatchAction || z(n.dispatchAction, n), s = t.getComponent("axisPointer").coordSysAxesInfo;
	if (s) {
		iN(i) && (i = KM({
			seriesIndex: a.seriesIndex,
			dataIndex: a.dataIndex
		}, t).point);
		var c = iN(i), l = a.axesInfo, u = s.axesInfo, d = r === "leave" || iN(i), f = {}, p = {}, m = {
			list: [],
			map: {}
		}, h = {
			showPointer: B(ZM, p),
			showTooltip: B(QM, m)
		};
		I(s.coordSysMap, function(e, t) {
			var n = c || e.containPoint(i);
			I(s.coordSysAxesInfo[t], function(e, t) {
				var r = e.axis, a = nN(l, e);
				if (!d && n && (!l || a)) {
					var o = a && a.value;
					o == null && !c && (o = r.pointToData(i)), o != null && YM(e, o, h, !1, f);
				}
			});
		});
		var g = {};
		return I(u, function(e, t) {
			var n = e.linkGroup;
			n && !p[t] && I(n.axesInfo, function(t, r) {
				var i = p[r];
				if (t !== e && i) {
					var a = i.value;
					n.mapper && (a = e.axis.scale.parse(n.mapper(a, rN(t), rN(e)))), g[e.key] = a;
				}
			});
		}), I(g, function(e, t) {
			YM(u[t], e, h, !0, f);
		}), $M(p, u, f), eN(m, i, e, o), tN(u, o, n), f;
	}
}
function YM(e, t, n, r, i) {
	var a = e.axis;
	if (!(a.scale.isBlank() || !a.containData(t))) {
		if (!e.involveSeries) {
			n.showPointer(e, t);
			return;
		}
		var o = XM(t, e), s = o.payloadBatch, c = o.snapToValue;
		s[0] && i.seriesIndex == null && j(i, s[0]), !r && e.snap && a.containData(c) && c != null && (t = c), n.showPointer(e, t, s), n.showTooltip(e, o, c);
	}
}
function XM(e, t) {
	var n = t.axis, r = n.dim, i = e, a = [], o = Number.MAX_VALUE, s = -1;
	return I(t.seriesModels, function(t, c) {
		var l = t.getData().mapDimensionsAll(r), u, d;
		if (t.getAxisTooltipData) {
			var f = t.getAxisTooltipData(l, e, n);
			d = f.dataIndices, u = f.nestestValue;
		} else {
			if (d = t.indicesOfNearest(r, l[0], e, n.type === "category" ? .5 : null), !d.length) return;
			u = t.getData().get(l[0], d[0]);
		}
		if (ms(u)) {
			var p = e - u, m = Math.abs(p);
			m <= o && ((m < o || p >= 0 && s < 0) && (o = m, s = p, i = u, a.length = 0), I(d, function(e) {
				a.push({
					seriesIndex: t.seriesIndex,
					dataIndexInside: e,
					dataIndex: t.getData().getRawIndex(e)
				});
			}));
		}
	}), {
		payloadBatch: a,
		snapToValue: i
	};
}
function ZM(e, t, n, r) {
	e[t.key] = {
		value: n,
		payloadBatch: r
	};
}
function QM(e, t, n, r) {
	var i = n.payloadBatch, a = t.axis, o = a.model, s = t.axisPointerModel;
	if (!(!t.triggerTooltip || !i.length)) {
		var c = t.coordSys.model, l = hj(c), u = e.map[l];
		u || (u = e.map[l] = {
			coordSysId: c.id,
			coordSysIndex: c.componentIndex,
			coordSysType: c.type,
			coordSysMainType: c.mainType,
			dataByAxis: []
		}, e.list.push(u)), u.dataByAxis.push({
			axisDim: a.dim,
			axisIndex: o.componentIndex,
			axisType: o.type,
			axisId: o.id,
			value: r,
			valueLabelOpt: {
				precision: s.get(["label", "precision"]),
				formatter: s.get(["label", "formatter"])
			},
			seriesDataIndices: i.slice()
		});
	}
}
function $M(e, t, n) {
	var r = n.axesInfo = [];
	I(t, function(t, n) {
		var i = t.axisPointerModel.option, a = e[n];
		a ? (!t.useHandle && (i.status = "show"), i.value = a.value, i.seriesDataIndices = (a.payloadBatch || []).slice()) : !t.useHandle && (i.status = "hide"), i.status === "show" && r.push({
			axisDim: t.axis.dim,
			axisIndex: t.axis.model.componentIndex,
			value: i.value
		});
	});
}
function eN(e, t, n, r) {
	if (iN(t) || !e.list.length) {
		r({ type: "hideTip" });
		return;
	}
	var i = ((e.list[0].dataByAxis[0] || {}).seriesDataIndices || [])[0] || {};
	r({
		type: "showTip",
		escapeConnect: !0,
		x: t[0],
		y: t[1],
		tooltipOption: n.tooltipOption,
		position: n.position,
		dataIndexInside: i.dataIndexInside,
		dataIndex: i.dataIndex,
		seriesIndex: i.seriesIndex,
		dataByCoordSys: e.list
	});
}
function tN(e, t, n) {
	var r = n.getZr(), i = "axisPointerLastHighlights", a = qM(r)[i] || {}, o = qM(r)[i] = {};
	I(e, function(e, t) {
		var n = e.axisPointerModel.option;
		n.status === "show" && e.triggerEmphasis && I(n.seriesDataIndices, function(e) {
			o[e.seriesIndex + "|" + e.dataIndex] = e;
		});
	});
	var s = [], c = [];
	function l(e) {
		return {
			seriesIndex: e.seriesIndex,
			dataIndex: e.dataIndex
		};
	}
	I(a, function(e, t) {
		!o[t] && c.push(l(e));
	}), I(o, function(e, t) {
		!a[t] && s.push(l(e));
	}), c.length && n.dispatchAction({
		type: "downplay",
		escapeConnect: !0,
		notBlur: !0,
		batch: c
	}), s.length && n.dispatchAction({
		type: "highlight",
		escapeConnect: !0,
		notBlur: !0,
		batch: s
	});
}
function nN(e, t) {
	for (var n = 0; n < (e || []).length; n++) {
		var r = e[n];
		if (t.axis.dim === r.axisDim && t.axis.model.componentIndex === r.axisIndex) return r;
	}
}
function rN(e) {
	var t = e.axis.model, n = {}, r = n.axisDim = e.axis.dim;
	return n.axisIndex = n[r + "AxisIndex"] = t.componentIndex, n.axisName = n[r + "AxisName"] = t.name, n.axisId = n[r + "AxisId"] = t.id, n;
}
function iN(e) {
	return !e || e[0] == null || isNaN(e[0]) || e[1] == null || isNaN(e[1]);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/axisPointer/install.js
function aN(e) {
	_j.registerAxisPointerClass("CartesianAxisPointer", jM), e.registerComponentModel(FM), e.registerComponentView(GM), e.registerPreprocessor(function(e) {
		if (e) {
			(!e.axisPointer || e.axisPointer.length === 0) && (e.axisPointer = {});
			var t = e.axisPointer.link;
			t && !V(t) && (e.axisPointer.link = [t]);
		}
	}), e.registerProcessor(e.PRIORITY.PROCESSOR.STATISTIC, { overallReset: function(e, t) {
		e.getComponent("axisPointer").coordSysAxesInfo = aj(e, t);
	} }), e.registerAction({
		type: "updateAxisPointer",
		event: "updateAxisPointer",
		update: ":updateAxisPointer"
	}, JM);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/grid/install.js
function oN(e) {
	SA(Oj), SA(aN);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/helper/listComponent.js
function sN(e, t) {
	var n = lg(t.get("padding")), r = t.getItemStyle(["color", "opacity"]);
	return r.fill = t.get("backgroundColor"), new fo({
		shape: {
			x: e.x - n[3],
			y: e.y - n[0],
			width: e.width + n[1] + n[3],
			height: e.height + n[0] + n[2],
			r: t.get("borderRadius")
		},
		style: r,
		silent: !0,
		z2: -1
	});
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/tooltip/TooltipModel.js
var cN = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n;
	}
	return t.type = "tooltip", t.dependencies = ["axisPointer"], t.defaultOption = {
		z: 60,
		show: !0,
		showContent: !0,
		trigger: "item",
		triggerOn: "mousemove|click|mousewheel",
		alwaysShowContent: !1,
		renderMode: "auto",
		confine: null,
		showDelay: 0,
		hideDelay: 100,
		transitionDuration: .4,
		displayTransition: !0,
		enterable: !1,
		backgroundColor: Q.color.neutral00,
		shadowBlur: 10,
		shadowColor: "rgba(0, 0, 0, .2)",
		shadowOffsetX: 1,
		shadowOffsetY: 2,
		borderRadius: 4,
		borderWidth: 1,
		defaultBorderColor: Q.color.border,
		padding: null,
		extraCssText: "",
		axisPointer: {
			type: "line",
			axis: "auto",
			animation: "auto",
			animationDurationUpdate: 200,
			animationEasingUpdate: "exponentialOut",
			crossStyle: {
				color: Q.color.borderShade,
				width: 1,
				type: "dashed",
				textStyle: {}
			}
		},
		textStyle: {
			color: Q.color.tertiary,
			fontSize: 14
		}
	}, t;
}(Ng);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/tooltip/helper.js
function lN(e) {
	var t = e.get("confine");
	return t == null ? e.get("renderMode") === "richText" : !!t;
}
function uN(e) {
	if (q.domSupported) {
		for (var t = document.documentElement.style, n = 0, r = e.length; n < r; n++) if (e[n] in t) return e[n];
	}
}
var dN = uN([
	"transform",
	"webkitTransform",
	"OTransform",
	"MozTransform",
	"msTransform"
]), fN = uN([
	"webkitTransition",
	"transition",
	"OTransition",
	"MozTransition",
	"msTransition"
]);
function pN(e, t) {
	if (!e) return t;
	t = cg(t, !0);
	var n = e.indexOf(t);
	return e = n === -1 ? t : "-" + e.slice(0, n) + "-" + t, e.toLowerCase();
}
function mN(e, t) {
	var n = e.currentStyle || document.defaultView && document.defaultView.getComputedStyle(e);
	return n ? t ? n[t] : n : null;
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/tooltip/TooltipHTMLContent.js
var hN = pN(fN, "transition"), gN = pN(dN, "transform"), _N = "position:absolute;display:block;border-style:solid;white-space:nowrap;z-index:9999999;" + (q.transform3dSupported ? "will-change:transform;" : "");
function vN(e) {
	return e = e === "left" ? "right" : e === "right" ? "left" : e === "top" ? "bottom" : "top", e;
}
function yN(e, t, n) {
	if (!U(n) || n === "inside") return "";
	var r = e.get("backgroundColor"), i = e.get("borderWidth");
	t = hg(t);
	var a = vN(n), o = Math.max(Math.round(i) * 1.5, 6), s = "", c = gN + ":", l;
	N(["left", "right"], a) > -1 ? (s += "top:50%", c += "translateY(-50%) rotate(" + (l = a === "left" ? -225 : -45) + "deg)") : (s += "left:50%", c += "translateX(-50%) rotate(" + (l = a === "top" ? 225 : 45) + "deg)");
	var u = l * Math.PI / 180, d = o + i, f = d * Math.abs(Math.cos(u)) + d * Math.abs(Math.sin(u)), p = Math.round(((f - Math.SQRT2 * i) / 2 + Math.SQRT2 * i - (f - d) / 2) * 100) / 100;
	s += ";" + a + ":-" + p + "px";
	var m = t + " solid " + i + "px;";
	return "<div style=\"" + [
		"position:absolute;width:" + o + "px;height:" + o + "px;z-index:-1;",
		s + ";" + c + ";",
		"border-bottom:" + m,
		"border-right:" + m,
		"background-color:" + r + ";"
	].join("") + "\"></div>";
}
function bN(e, t, n) {
	var r = "cubic-bezier(0.23,1,0.32,1)", i = "", a = "";
	return n && (i = " " + e / 2 + "s " + r, a = "opacity" + i + ",visibility" + i), t || (i = " " + e + "s " + r, a += (a.length ? "," : "") + (q.transformSupported ? "" + gN + i : ",left" + i + ",top" + i)), hN + ":" + a;
}
function xN(e, t, n) {
	var r = e.toFixed(0) + "px", i = t.toFixed(0) + "px";
	if (!q.transformSupported) return n ? "top:" + i + ";left:" + r + ";" : [["top", i], ["left", r]];
	var a = q.transform3dSupported, o = "translate" + (a ? "3d" : "") + "(" + r + "," + i + (a ? ",0" : "") + ")";
	return n ? "top:0;left:0;" + gN + ":" + o + ";" : [
		["top", 0],
		["left", 0],
		[dN, o]
	];
}
function SN(e) {
	var t = [], n = e.get("fontSize"), r = e.getTextColor();
	r && t.push("color:" + r), t.push("font:" + e.getFont());
	var i = G(e.get("lineHeight"), Math.round(n * 3 / 2));
	n && t.push("line-height:" + i + "px");
	var a = e.get("textShadowColor"), o = e.get("textShadowBlur") || 0, s = e.get("textShadowOffsetX") || 0, c = e.get("textShadowOffsetY") || 0;
	return a && o && t.push("text-shadow:" + s + "px " + c + "px " + o + "px " + a), I(["decoration", "align"], function(n) {
		var r = e.get(n);
		r && t.push("text-" + n + ":" + r);
	}), t.join(";");
}
function CN(e, t, n, r) {
	var i = [], a = e.get("transitionDuration"), o = e.get("backgroundColor"), s = e.get("shadowBlur"), c = e.get("shadowColor"), l = e.get("shadowOffsetX"), u = e.get("shadowOffsetY"), d = e.getModel("textStyle"), f = O_(e, "html"), p = l + "px " + u + "px " + s + "px " + c;
	return i.push("box-shadow:" + p), t && a > 0 && i.push(bN(a, n, r)), o && i.push("background-color:" + o), I([
		"width",
		"color",
		"radius"
	], function(t) {
		var n = "border-" + t, r = cg(n), a = e.get(r);
		a != null && i.push(n + ":" + a + (t === "color" ? "" : "px"));
	}), i.push(SN(d)), f != null && i.push("padding:" + lg(f).join("px ") + "px"), i.join(";") + ";";
}
function wN(e, t, n, r, i) {
	var a = t && t.painter;
	if (n) {
		var o = a && a.getViewportRoot();
		o && $m(e, o, n, r, i);
	} else {
		e[0] = r, e[1] = i;
		var s = a && a.getViewportRootOffset();
		s && (e[0] += s.offsetLeft, e[1] += s.offsetTop);
	}
	e[2] = e[0] / t.getWidth(), e[3] = e[1] / t.getHeight();
}
var TN = function() {
	function e(e, t) {
		if (this._show = !1, this._styleCoord = [
			0,
			0,
			0,
			0
		], this._enterable = !0, this._alwaysShowContent = !1, this._firstShow = !0, this._longHide = !0, q.wxa) return null;
		var n = document.createElement("div");
		n.domBelongToZr = !0, this.el = n;
		var r = this._zr = e.getZr(), i = t.appendTo, a = i && (U(i) ? document.querySelector(i) : ue(i) ? i : H(i) && i(e.getDom()));
		wN(this._styleCoord, r, a, e.getWidth() / 2, e.getHeight() / 2), (a || e.getDom()).appendChild(n), this._api = e, this._container = a;
		var o = this;
		n.onmouseenter = function() {
			o._enterable && (clearTimeout(o._hideTimeout), o._show = !0), o._inContent = !0;
		}, n.onmousemove = function(e) {
			if (e ||= window.event, !o._enterable) {
				var t = r.handler;
				Xw(r.painter.getViewportRoot(), e, !0), t.dispatch("mousemove", e);
			}
		}, n.onmouseleave = function() {
			o._inContent = !1, o._enterable && o._show && o.hideLater(o._hideDelay);
		};
	}
	return e.prototype.update = function(e) {
		if (!this._container) {
			var t = this._api.getDom(), n = mN(t, "position"), r = t.style;
			r.position !== "absolute" && n !== "absolute" && (r.position = "relative");
		}
		var i = e.get("alwaysShowContent");
		i && this._moveIfResized(), this._alwaysShowContent = i, this._enableDisplayTransition = e.get("displayTransition") && e.get("transitionDuration") > 0, this.el.className = e.get("className") || "";
	}, e.prototype.show = function(e, t) {
		clearTimeout(this._hideTimeout), clearTimeout(this._longHideTimeout);
		var n = this.el, r = n.style, i = this._styleCoord;
		n.innerHTML ? r.cssText = _N + CN(e, !this._firstShow, this._longHide, this._enableDisplayTransition) + xN(i[0], i[1], !0) + ("border-color:" + hg(t) + ";") + (e.get("extraCssText") || "") + (";pointer-events:" + (this._enterable ? "auto" : "none")) : r.display = "none", this._show = !0, this._firstShow = !1, this._longHide = !1;
	}, e.prototype.setContent = function(e, t, n, r, i) {
		var a = this.el;
		if (e == null) {
			a.innerHTML = "";
			return;
		}
		var o = "";
		if (U(i) && n.get("trigger") === "item" && !lN(n) && (o = yN(n, r, i)), U(e)) a.innerHTML = e + o;
		else if (e) {
			a.innerHTML = "", V(e) || (e = [e]);
			for (var s = 0; s < e.length; s++) ue(e[s]) && e[s].parentNode !== a && a.appendChild(e[s]);
			if (o && a.childNodes.length) {
				var c = document.createElement("div");
				c.innerHTML = o, a.appendChild(c);
			}
		}
	}, e.prototype.setEnterable = function(e) {
		this._enterable = e;
	}, e.prototype.getSize = function() {
		var e = this.el;
		return e ? [e.offsetWidth, e.offsetHeight] : [0, 0];
	}, e.prototype.moveTo = function(e, t) {
		if (this.el) {
			var n = this._styleCoord;
			if (wN(n, this._zr, this._container, e, t), n[0] != null && n[1] != null) {
				var r = this.el.style;
				I(xN(n[0], n[1]), function(e) {
					r[e[0]] = e[1];
				});
			}
		}
	}, e.prototype._moveIfResized = function() {
		var e = this._styleCoord[2], t = this._styleCoord[3];
		this.moveTo(e * this._zr.getWidth(), t * this._zr.getHeight());
	}, e.prototype.hide = function() {
		var e = this, t = this.el.style;
		this._enableDisplayTransition ? (t.visibility = "hidden", t.opacity = "0") : t.display = "none", q.transform3dSupported && (t.willChange = ""), this._show = !1, this._longHideTimeout = setTimeout(function() {
			return e._longHide = !0;
		}, 500);
	}, e.prototype.hideLater = function(e) {
		this._show && !(this._inContent && this._enterable) && !this._alwaysShowContent && (e ? (this._hideDelay = e, this._show = !1, this._hideTimeout = setTimeout(z(this.hide, this), e)) : this.hide());
	}, e.prototype.isShow = function() {
		return this._show;
	}, e.prototype.dispose = function() {
		clearTimeout(this._hideTimeout), clearTimeout(this._longHideTimeout);
		var e = this._zr;
		eh(e && e.painter && e.painter.getViewportRoot(), this._container);
		var t = this.el;
		if (t) {
			t.onmouseenter = t.onmousemove = t.onmouseleave = null;
			var n = t.parentNode;
			n && n.removeChild(t);
		}
		this.el = this._container = null;
	}, e;
}(), EN = function() {
	function e(e) {
		this._show = !1, this._styleCoord = [
			0,
			0,
			0,
			0
		], this._alwaysShowContent = !1, this._enterable = !0, this._zr = e.getZr(), kN(this._styleCoord, this._zr, e.getWidth() / 2, e.getHeight() / 2);
	}
	return e.prototype.update = function(e) {
		var t = e.get("alwaysShowContent");
		t && this._moveIfResized(), this._alwaysShowContent = t;
	}, e.prototype.show = function() {
		this._hideTimeout && clearTimeout(this._hideTimeout), this.el.show(), this._show = !0;
	}, e.prototype.setContent = function(e, t, n, r, i) {
		var a = this;
		W(e) && bs(""), this.el && this._zr.remove(this.el);
		var o = n.getModel("textStyle");
		this.el = new _o({
			style: {
				rich: t.richTextStyles,
				text: e,
				lineHeight: 22,
				borderWidth: 1,
				borderColor: r,
				textShadowColor: o.get("textShadowColor"),
				fill: n.get(["textStyle", "color"]),
				padding: O_(n, "richText"),
				verticalAlign: "top",
				align: "left"
			},
			z: n.get("z")
		}), I([
			"backgroundColor",
			"borderRadius",
			"shadowColor",
			"shadowBlur",
			"shadowOffsetX",
			"shadowOffsetY"
		], function(e) {
			a.el.style[e] = n.get(e);
		}), I([
			"textShadowBlur",
			"textShadowOffsetX",
			"textShadowOffsetY"
		], function(e) {
			a.el.style[e] = o.get(e) || 0;
		}), this._zr.add(this.el);
		var s = this;
		this.el.on("mouseover", function() {
			s._enterable && (clearTimeout(s._hideTimeout), s._show = !0), s._inContent = !0;
		}), this.el.on("mouseout", function() {
			s._enterable && s._show && s.hideLater(s._hideDelay), s._inContent = !1;
		});
	}, e.prototype.setEnterable = function(e) {
		this._enterable = e;
	}, e.prototype.getSize = function() {
		var e = this.el, t = this.el.getBoundingRect(), n = ON(e.style);
		return [t.width + n.left + n.right, t.height + n.top + n.bottom];
	}, e.prototype.moveTo = function(e, t) {
		var n = this.el;
		if (n) {
			var r = this._styleCoord;
			kN(r, this._zr, e, t), e = r[0], t = r[1];
			var i = n.style, a = DN(i.borderWidth || 0), o = ON(i);
			n.x = e + a + o.left, n.y = t + a + o.top, n.markRedraw();
		}
	}, e.prototype._moveIfResized = function() {
		var e = this._styleCoord[2], t = this._styleCoord[3];
		this.moveTo(e * this._zr.getWidth(), t * this._zr.getHeight());
	}, e.prototype.hide = function() {
		this.el && this.el.hide(), this._show = !1;
	}, e.prototype.hideLater = function(e) {
		this._show && !(this._inContent && this._enterable) && !this._alwaysShowContent && (e ? (this._hideDelay = e, this._show = !1, this._hideTimeout = setTimeout(z(this.hide, this), e)) : this.hide());
	}, e.prototype.isShow = function() {
		return this._show;
	}, e.prototype.dispose = function() {
		this._zr.remove(this.el);
	}, e;
}();
function DN(e) {
	return Math.max(0, e);
}
function ON(e) {
	var t = DN(e.shadowBlur || 0), n = DN(e.shadowOffsetX || 0), r = DN(e.shadowOffsetY || 0);
	return {
		left: DN(t - n),
		right: DN(t + n),
		top: DN(t - r),
		bottom: DN(t + r)
	};
}
function kN(e, t, n, r) {
	e[0] = n, e[1] = r, e[2] = e[0] / t.getWidth(), e[3] = e[1] / t.getHeight();
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/tooltip/TooltipView.js
var AN = new fo({ shape: {
	x: -1,
	y: -1,
	width: 2,
	height: 2
} }), jN = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n;
	}
	return t.prototype.init = function(e, t) {
		if (!(q.node || !t.getDom())) {
			var n = e.getComponent("tooltip"), r = this._renderMode = $s(n.get("renderMode"));
			this._tooltipContent = r === "richText" ? new EN(t) : new TN(t, { appendTo: n.get("appendToBody", !0) ? "body" : n.get("appendTo", !0) });
		}
	}, t.prototype.render = function(e, t, n) {
		if (!(q.node || !n.getDom())) {
			this.group.removeAll(), this._tooltipModel = e, this._ecModel = t, this._api = n;
			var r = this._tooltipContent;
			r.update(e), r.setEnterable(e.get("enterable")), this._initGlobalListener(), this._keepShow(), this._renderMode !== "richText" && e.get("transitionDuration") ? xC(this, "_updatePosition", 50, "fixRate") : SC(this, "_updatePosition");
		}
	}, t.prototype._initGlobalListener = function() {
		var e = this._tooltipModel.get("triggerOn");
		RM("itemTooltip", this._api, z(function(t, n, r) {
			e !== "none" && (e.indexOf(t) >= 0 ? this._tryShow(n, r) : t === "leave" && this._hide(r));
		}, this));
	}, t.prototype._keepShow = function() {
		var e = this._tooltipModel, t = this._ecModel, n = this._api, r = e.get("triggerOn");
		if (e.get("trigger") !== "axis" && (this._lastDataByCoordSys = null, this._cbParamsList = null), this._lastX != null && this._lastY != null && r !== "none" && r !== "click") {
			var i = this;
			clearTimeout(this._refreshUpdateTimeout), this._refreshUpdateTimeout = setTimeout(function() {
				!n.isDisposed() && i.manuallyShowTip(e, t, n, {
					x: i._lastX,
					y: i._lastY,
					dataByCoordSys: i._lastDataByCoordSys
				});
			});
		}
	}, t.prototype.manuallyShowTip = function(e, t, n, r) {
		if (!(r.from === this.uid || q.node || !n.getDom())) {
			var i = NN(r, n);
			this._ticket = "";
			var a = r.dataByCoordSys, o = RN(r, t, n);
			if (o) {
				var s = o.el.getBoundingRect().clone();
				s.applyTransform(o.el.transform), this._tryShow({
					offsetX: s.x + s.width / 2,
					offsetY: s.y + s.height / 2,
					target: o.el,
					position: r.position,
					positionDefault: "bottom"
				}, i);
			} else if (r.tooltip && r.x != null && r.y != null) {
				var c = AN;
				c.x = r.x, c.y = r.y, c.update(), yc(c).tooltipConfig = {
					name: null,
					option: r.tooltip
				}, this._tryShow({
					offsetX: r.x,
					offsetY: r.y,
					target: c
				}, i);
			} else if (a) this._tryShow({
				offsetX: r.x,
				offsetY: r.y,
				position: r.position,
				dataByCoordSys: a,
				tooltipOption: r.tooltipOption
			}, i);
			else if (r.seriesIndex != null) {
				if (this._manuallyAxisShowTip(e, t, n, r)) return;
				var l = KM(r, t), u = l.point[0], d = l.point[1];
				u != null && d != null && this._tryShow({
					offsetX: u,
					offsetY: d,
					target: l.el,
					position: r.position,
					positionDefault: "bottom"
				}, i);
			} else r.x != null && r.y != null && (n.dispatchAction({
				type: "updateAxisPointer",
				x: r.x,
				y: r.y
			}), this._tryShow({
				offsetX: r.x,
				offsetY: r.y,
				position: r.position,
				target: n.getZr().findHover(r.x, r.y).target
			}, i));
		}
	}, t.prototype.manuallyHideTip = function(e, t, n, r) {
		var i = this._tooltipContent;
		this._tooltipModel && i.hideLater(this._tooltipModel.get("hideDelay")), this._lastX = this._lastY = this._lastDataByCoordSys = null, this._cbParamsList = null, r.from !== this.uid && this._hide(NN(r, n));
	}, t.prototype._manuallyAxisShowTip = function(e, t, n, r) {
		var i = r.seriesIndex, a = r.dataIndex, o = t.getComponent("axisPointer").coordSysAxesInfo;
		if (!(i == null || a == null || o == null)) {
			var s = t.getSeriesByIndex(i);
			if (s && MN([
				s.getData().getItemModel(a),
				s,
				(s.coordinateSystem || {}).model
			], this._tooltipModel).get("trigger") === "axis") return n.dispatchAction({
				type: "updateAxisPointer",
				seriesIndex: i,
				dataIndex: a,
				position: r.position
			}), !0;
		}
	}, t.prototype._tryShow = function(e, t) {
		var n = e.target;
		if (this._tooltipModel) {
			this._lastX = e.offsetX, this._lastY = e.offsetY;
			var r = e.dataByCoordSys;
			if (r && r.length) this._showAxisTooltip(r, e);
			else if (n) {
				if (yc(n).ssrType === "legend") return;
				this._lastDataByCoordSys = null, this._cbParamsList = null;
				var i, a;
				BD(n, function(e) {
					if (e.tooltipDisabled) return i = a = null, !0;
					i || a || (yc(e).dataIndex == null ? yc(e).tooltipConfig != null && (a = e) : i = e);
				}, !0), i ? this._showSeriesItemTooltip(e, i, t) : a ? this._showComponentItemTooltip(e, a, t) : this._hide(t);
			} else this._lastDataByCoordSys = null, this._cbParamsList = null, this._hide(t);
		}
	}, t.prototype._showOrMove = function(e, t) {
		var n = e.get("showDelay");
		t = z(t, this), clearTimeout(this._showTimout), n > 0 ? this._showTimout = setTimeout(t, n) : t();
	}, t.prototype._showAxisTooltip = function(e, t) {
		var n = this._ecModel, r = this._tooltipModel, i = [t.offsetX, t.offsetY], a = MN([t.tooltipOption], r), o = this._renderMode, s = [], c = m_("section", {
			blocks: [],
			noHeader: !0
		}), l = [], u = new k_();
		I(e, function(e) {
			I(e.dataByAxis, function(e) {
				var t = n.getComponent(e.axisDim + "Axis", e.axisIndex), i = e.value, a = t.axis, d = a.scale.parse(i);
				if (!(!t || i == null)) {
					var f = wM(i, a, n, e.seriesDataIndices, e.valueLabelOpt), p = m_("section", {
						header: f,
						noHeader: !ye(f),
						sortBlocks: !0,
						blocks: []
					});
					c.blocks.push(p), I(e.seriesDataIndices, function(i) {
						var a = n.getSeriesByIndex(i.seriesIndex), c = i.dataIndexInside, m = a.getDataParams(c);
						if (!(m.dataIndex < 0)) {
							m.axisDim = e.axisDim, m.axisIndex = e.axisIndex, m.axisType = e.axisType, m.axisId = e.axisId, m.axisValue = Uy(t.axis, { value: d }), m.axisValueLabel = f, m.marker = u.makeTooltipMarker("item", hg(m.color), o);
							var h = Hg(a.formatTooltip(c, !0, null)), g = h.frag;
							if (g) {
								var _ = MN([a], r).get("valueFormatter");
								p.blocks.push(_ ? j({ valueFormatter: _ }, g) : g);
							}
							h.text && l.push(h.text), s.push(m);
						}
					});
				}
			});
		}), c.blocks.reverse(), l.reverse();
		var d = t.position, f = b_(c, u, o, a.get("order"), n.get("useUTC"), a.get("textStyle"));
		f && l.unshift(f);
		var p = o === "richText" ? "\n\n" : "<br/>", m = l.join(p);
		this._showOrMove(a, function() {
			this._updateContentNotChangedOnAxis(e, s) ? this._updatePosition(a, d, i[0], i[1], this._tooltipContent, s) : this._showTooltipContent(a, m, s, Math.random() + "", i[0], i[1], d, null, u);
		});
	}, t.prototype._showSeriesItemTooltip = function(e, t, n) {
		var r = this._ecModel, i = yc(t), a = i.seriesIndex, o = r.getSeriesByIndex(a), s = i.dataModel || o, c = i.dataIndex, l = i.dataType, u = s.getData(l), d = this._renderMode, f = e.positionDefault, p = MN([
			u.getItemModel(c),
			s,
			o && (o.coordinateSystem || {}).model
		], this._tooltipModel, f ? { position: f } : null), m = p.get("trigger");
		if (!(m != null && m !== "item")) {
			var h = s.getDataParams(c, l), g = new k_();
			h.marker = g.makeTooltipMarker("item", hg(h.color), d);
			var _ = Hg(s.formatTooltip(c, !1, l)), v = p.get("order"), y = p.get("valueFormatter"), b = _.frag, x = b ? b_(y ? j({ valueFormatter: y }, b) : b, g, d, v, r.get("useUTC"), p.get("textStyle")) : _.text, S = "item_" + s.name + "_" + c;
			this._showOrMove(p, function() {
				this._showTooltipContent(p, x, h, S, e.offsetX, e.offsetY, e.position, e.target, g);
			}), n({
				type: "showTip",
				dataIndexInside: c,
				dataIndex: u.getRawIndex(c),
				seriesIndex: a,
				from: this.uid
			});
		}
	}, t.prototype._showComponentItemTooltip = function(e, t, n) {
		var r = this._renderMode === "html", i = yc(t), a = i.tooltipConfig.option || {}, o = a.encodeHTMLContent;
		if (U(a)) {
			var s = a;
			a = {
				content: s,
				formatter: s
			}, o = !0;
		}
		o && r && a.content && (a = k(a), a.content = sh(a.content));
		var c = [a], l = this._ecModel.getComponent(i.componentMainType, i.componentIndex);
		l && c.push(l), c.push({ formatter: a.content });
		var u = e.positionDefault, d = MN(c, this._tooltipModel, u ? { position: u } : null), f = d.get("content"), p = Math.random() + "", m = new k_();
		this._showOrMove(d, function() {
			var n = k(d.get("formatterParams") || {});
			this._showTooltipContent(d, f, n, p, e.offsetX, e.offsetY, e.position, t, m);
		}), n({
			type: "showTip",
			from: this.uid
		});
	}, t.prototype._showTooltipContent = function(e, t, n, r, i, a, o, s, c) {
		if (this._ticket = "", !(!e.get("showContent") || !e.get("show"))) {
			var l = this._tooltipContent;
			l.setEnterable(e.get("enterable"));
			var u = e.get("formatter");
			o ||= e.get("position");
			var d = t, f = this._getNearestPoint([i, a], n, e.get("trigger"), e.get("borderColor"), e.get("defaultBorderColor", !0)).color;
			if (u) if (U(u)) {
				var p = e.ecModel.get("useUTC"), m = V(n) ? n[0] : n, h = m && m.axisType && m.axisType.indexOf("time") >= 0;
				d = u, h && (d = Uh(m.axisValue, d, p)), d = pg(d, n, !0);
			} else if (H(u)) {
				var g = z(function(t, r) {
					t === this._ticket && (l.setContent(r, c, e, f, o), this._updatePosition(e, o, i, a, l, n, s));
				}, this);
				this._ticket = r, d = u(n, r, g);
			} else d = u;
			l.setContent(d, c, e, f, o), l.show(e, f), this._updatePosition(e, o, i, a, l, n, s);
		}
	}, t.prototype._getNearestPoint = function(e, t, n, r, i) {
		if (n === "axis" || V(t)) return { color: r || i };
		if (!V(t)) return { color: r || t.color || t.borderColor };
	}, t.prototype._updatePosition = function(e, t, n, r, i, a, o) {
		var s = this._api.getWidth(), c = this._api.getHeight();
		t ||= e.get("position");
		var l = i.getSize(), u = e.get("align"), d = e.get("verticalAlign"), f = o && o.getBoundingRect().clone();
		if (o && f.applyTransform(o.transform), H(t) && (t = t([n, r], a, i.el, f, {
			viewSize: [s, c],
			contentSize: l.slice()
		})), V(t)) n = X(t[0], s), r = X(t[1], c);
		else if (W(t)) {
			var p = t;
			p.width = l[0], p.height = l[1];
			var m = Tg(p, {
				width: s,
				height: c
			});
			n = m.x, r = m.y, u = null, d = null;
		} else if (U(t) && o) {
			var h = IN(t, f, l, e.get("borderWidth"));
			n = h[0], r = h[1];
		} else {
			var h = PN(n, r, i, s, c, u ? null : 20, d ? null : 20);
			n = h[0], r = h[1];
		}
		if (u && (n -= LN(u) ? l[0] / 2 : u === "right" ? l[0] : 0), d && (r -= LN(d) ? l[1] / 2 : d === "bottom" ? l[1] : 0), lN(e)) {
			var h = FN(n, r, i, s, c);
			n = h[0], r = h[1];
		}
		i.moveTo(n, r);
	}, t.prototype._updateContentNotChangedOnAxis = function(e, t) {
		var n = this._lastDataByCoordSys, r = this._cbParamsList, i = !!n && n.length === e.length;
		return i && I(n, function(n, a) {
			var o = n.dataByAxis || [], s = (e[a] || {}).dataByAxis || [];
			i &&= o.length === s.length, i && I(o, function(e, n) {
				var a = s[n] || {}, o = e.seriesDataIndices || [], c = a.seriesDataIndices || [];
				i = i && e.value === a.value && e.axisType === a.axisType && e.axisId === a.axisId && o.length === c.length, i && I(o, function(e, t) {
					var n = c[t];
					i = i && e.seriesIndex === n.seriesIndex && e.dataIndex === n.dataIndex;
				}), r && I(e.seriesDataIndices, function(e) {
					var n = e.seriesIndex, a = t[n], o = r[n];
					a && o && o.data !== a.data && (i = !1);
				});
			});
		}), this._lastDataByCoordSys = e, this._cbParamsList = t, !!i;
	}, t.prototype._hide = function(e) {
		this._lastDataByCoordSys = null, this._cbParamsList = null, e({
			type: "hideTip",
			from: this.uid
		});
	}, t.prototype.dispose = function(e, t) {
		q.node || !t.getDom() || (SC(this, "_updatePosition"), this._tooltipContent.dispose(), WM("itemTooltip", t), this._tooltipContent = null, this._tooltipModel = null, this._lastDataByCoordSys = null, this._cbParamsList = null);
	}, t.type = "tooltip", t;
}(nD);
function MN(e, t, n) {
	var r = t.ecModel, i;
	n ? (i = new Bf(n, r, r), i = new Bf(t.option, i, r)) : i = t;
	for (var a = e.length - 1; a >= 0; a--) {
		var o = e[a];
		o && (o instanceof Bf && (o = o.get("tooltip", !0)), U(o) && (o = { formatter: o }), o && (i = new Bf(o, i, r)));
	}
	return i;
}
function NN(e, t) {
	return e.dispatchAction || z(t.dispatchAction, t);
}
function PN(e, t, n, r, i, a, o) {
	var s = n.getSize(), c = s[0], l = s[1];
	return a != null && (e + c + a + 2 > r ? e -= c + a : e += a), o != null && (t + l + o > i ? t -= l + o : t += o), [e, t];
}
function FN(e, t, n, r, i) {
	var a = n.getSize(), o = a[0], s = a[1];
	return e = Math.min(e + o, r) - o, t = Math.min(t + s, i) - s, e = Math.max(e, 0), t = Math.max(t, 0), [e, t];
}
function IN(e, t, n, r) {
	var i = n[0], a = n[1], o = Math.ceil(Math.SQRT2 * r) + 8, s = 0, c = 0, l = t.width, u = t.height;
	switch (e) {
		case "inside":
			s = t.x + l / 2 - i / 2, c = t.y + u / 2 - a / 2;
			break;
		case "top":
			s = t.x + l / 2 - i / 2, c = t.y - a - o;
			break;
		case "bottom":
			s = t.x + l / 2 - i / 2, c = t.y + u + o;
			break;
		case "left":
			s = t.x - i - o, c = t.y + u / 2 - a / 2;
			break;
		case "right": s = t.x + l + o, c = t.y + u / 2 - a / 2;
	}
	return [s, c];
}
function LN(e) {
	return e === "center" || e === "middle";
}
function RN(e, t, n) {
	var r = qs(e).queryOptionMap, i = r.keys()[0];
	if (!(!i || i === "series")) {
		var a = Ys(t, i, r.get(i), {
			useDefault: !1,
			enableAll: !1,
			enableNone: !1
		}).models[0];
		if (a) {
			var o = n.getViewOfComponentModel(a), s;
			if (o.group.traverse(function(t) {
				var n = yc(t).tooltipConfig;
				if (n && n.name === e.name) return s = t, !0;
			}), s) return {
				componentMainType: i,
				componentIndex: a.componentIndex,
				el: s
			};
		}
	}
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/tooltip/install.js
function zN(e) {
	SA(aN), e.registerComponentModel(cN), e.registerComponentView(jN), e.registerAction({
		type: "showTip",
		event: "showTip",
		update: "tooltip:manuallyShowTip"
	}, je), e.registerAction({
		type: "hideTip",
		event: "hideTip",
		update: "tooltip:manuallyHideTip"
	}, je);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/title/install.js
var BN = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n.layoutMode = {
			type: "box",
			ignoreSize: !0
		}, n;
	}
	return t.type = "title", t.defaultOption = {
		z: 6,
		show: !0,
		text: "",
		target: "blank",
		subtext: "",
		subtarget: "blank",
		left: "center",
		top: Q.size.m,
		backgroundColor: Q.color.transparent,
		borderColor: Q.color.primary,
		borderWidth: 0,
		padding: 5,
		itemGap: 10,
		textStyle: {
			fontSize: 18,
			fontWeight: "bold",
			color: Q.color.primary
		},
		subtextStyle: {
			fontSize: 12,
			color: Q.color.quaternary
		}
	}, t;
}(Ng), VN = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n;
	}
	return t.prototype.render = function(e, t, n) {
		if (this.group.removeAll(), e.get("show")) {
			var r = this.group, i = e.getModel("textStyle"), a = e.getModel("subtextStyle"), o = e.get("textAlign"), s = G(e.get("textBaseline"), e.get("textVerticalAlign")), c = new _o({
				style: _f(i, {
					text: e.get("text"),
					fill: i.getTextColor()
				}, { disableBox: !0 }),
				z2: 10
			}), l = c.getBoundingRect(), u = e.get("subtext"), d = new _o({
				style: _f(a, {
					text: u,
					fill: a.getTextColor(),
					y: l.height + e.get("itemGap"),
					verticalAlign: "top"
				}, { disableBox: !0 }),
				z2: 10
			}), f = e.get("link"), p = e.get("sublink"), m = e.get("triggerEvent", !0);
			c.silent = !f && !m, d.silent = !p && !m, f && c.on("click", function() {
				gg(f, "_" + e.get("target"));
			}), p && d.on("click", function() {
				gg(p, "_" + e.get("subtarget"));
			}), yc(c).eventData = yc(d).eventData = m ? {
				componentType: "title",
				componentIndex: e.componentIndex
			} : null, r.add(c), u && r.add(d);
			var h = r.getBoundingRect(), g = e.getBoxLayoutParams();
			g.width = h.width, g.height = h.height;
			var _ = Tg(g, Dg(e, n).refContainer, e.get("padding"));
			o || (o = e.get("left") || e.get("right"), o === "middle" && (o = "center"), o === "right" ? _.x += _.width : o === "center" && (_.x += _.width / 2)), s || (s = e.get("top") || e.get("bottom"), s === "center" && (s = "middle"), s === "bottom" ? _.y += _.height : s === "middle" && (_.y += _.height / 2), s ||= "top"), r.x = _.x, r.y = _.y, r.markRedraw();
			var v = {
				align: o,
				verticalAlign: s
			};
			c.setStyle(v), d.setStyle(v), h = r.getBoundingRect();
			var y = _.margin, b = e.getItemStyle(["color", "opacity"]);
			b.fill = e.get("backgroundColor");
			var x = new fo({
				shape: {
					x: h.x - y[3],
					y: h.y - y[0],
					width: h.width + y[1] + y[3],
					height: h.height + y[0] + y[2],
					r: e.get("borderRadius")
				},
				style: b,
				subPixelOptimize: !0,
				silent: !0
			});
			r.add(x);
		}
	}, t.type = "title", t;
}(nD);
function HN(e) {
	e.registerComponentModel(BN), e.registerComponentView(VN);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/legend/LegendModel.js
var UN = function(e, t) {
	if (t === "all") return {
		type: "all",
		title: e.getLocaleModel().get([
			"legend",
			"selector",
			"all"
		])
	};
	if (t === "inverse") return {
		type: "inverse",
		title: e.getLocaleModel().get([
			"legend",
			"selector",
			"inverse"
		])
	};
}, WN = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n.layoutMode = {
			type: "box",
			ignoreSize: !0
		}, n;
	}
	return t.prototype.init = function(e, t, n) {
		this.mergeDefaultAndTheme(e, n), e.selected = e.selected || {}, this._updateSelector(e);
	}, t.prototype.mergeOption = function(t, n) {
		e.prototype.mergeOption.call(this, t, n), this._updateSelector(t);
	}, t.prototype._updateSelector = function(e) {
		var t = e.selector, n = this.ecModel;
		t === !0 && (t = e.selector = ["all", "inverse"]), V(t) && I(t, function(e, r) {
			U(e) && (e = { type: e }), t[r] = A(e, UN(n, e.type));
		});
	}, t.prototype.optionUpdated = function() {
		this._updateData(this.ecModel);
		var e = this._data;
		if (e[0] && this.get("selectedMode") === "single") {
			for (var t = !1, n = 0; n < e.length; n++) {
				var r = e[n].get("name");
				if (this.isSelected(r)) {
					this.select(r), t = !0;
					break;
				}
			}
			!t && this.select(e[0].get("name"));
		}
	}, t.prototype._updateData = function(e) {
		var t = [], n = [];
		e.eachRawSeries(function(r) {
			var i = r.name;
			n.push(i);
			var a;
			if (r.legendVisualProvider) {
				var o = r.legendVisualProvider.getAllNames();
				e.isSeriesFiltered(r) || (n = n.concat(o)), o.length ? t = t.concat(o) : a = !0;
			} else a = !0;
			a && zs(r) && t.push(r.name);
		}), this._availableNames = n;
		var r = this.get("data") || t, i = K(), a = L(r, function(e) {
			return (U(e) || se(e)) && (e = { name: e }), i.get(e.name) ? null : (i.set(e.name, !0), new Bf(e, this, this.ecModel));
		}, this);
		this._data = re(a, function(e) {
			return !!e;
		});
	}, t.prototype.getData = function() {
		return this._data;
	}, t.prototype.select = function(e) {
		var t = this.option.selected;
		if (this.get("selectedMode") === "single") {
			var n = this._data;
			I(n, function(e) {
				t[e.get("name")] = !1;
			});
		}
		t[e] = !0;
	}, t.prototype.unSelect = function(e) {
		this.get("selectedMode") !== "single" && (this.option.selected[e] = !1);
	}, t.prototype.toggleSelected = function(e) {
		var t = this.option.selected;
		t.hasOwnProperty(e) || (t[e] = !0), this[t[e] ? "unSelect" : "select"](e);
	}, t.prototype.allSelect = function() {
		var e = this._data, t = this.option.selected;
		I(e, function(e) {
			t[e.get("name", !0)] = !0;
		});
	}, t.prototype.inverseSelect = function() {
		var e = this._data, t = this.option.selected;
		I(e, function(e) {
			var n = e.get("name", !0);
			t.hasOwnProperty(n) || (t[n] = !0), t[n] = !t[n];
		});
	}, t.prototype.isSelected = function(e) {
		var t = this.option.selected;
		return !(t.hasOwnProperty(e) && !t[e]) && N(this._availableNames, e) >= 0;
	}, t.prototype.getOrient = function() {
		return this.get("orient") === "vertical" ? {
			index: 1,
			name: "vertical"
		} : {
			index: 0,
			name: "horizontal"
		};
	}, t.type = "legend.plain", t.dependencies = ["series"], t.defaultOption = {
		z: 4,
		show: !0,
		orient: "horizontal",
		left: "center",
		bottom: Q.size.m,
		align: "auto",
		backgroundColor: Q.color.transparent,
		borderColor: Q.color.border,
		borderRadius: 0,
		borderWidth: 0,
		padding: 5,
		itemGap: 8,
		itemWidth: 25,
		itemHeight: 14,
		symbolRotate: "inherit",
		symbolKeepAspect: !0,
		inactiveColor: Q.color.disabled,
		inactiveBorderColor: Q.color.disabled,
		inactiveBorderWidth: "auto",
		itemStyle: {
			color: "inherit",
			opacity: "inherit",
			borderColor: "inherit",
			borderWidth: "auto",
			borderCap: "inherit",
			borderJoin: "inherit",
			borderDashOffset: "inherit",
			borderMiterLimit: "inherit"
		},
		lineStyle: {
			width: "auto",
			color: "inherit",
			inactiveColor: Q.color.disabled,
			inactiveWidth: 2,
			opacity: "inherit",
			type: "inherit",
			cap: "inherit",
			join: "inherit",
			dashOffset: "inherit",
			miterLimit: "inherit"
		},
		textStyle: { color: Q.color.secondary },
		selectedMode: !0,
		selector: !1,
		selectorLabel: {
			show: !0,
			borderRadius: 10,
			padding: [
				3,
				5,
				3,
				5
			],
			fontSize: 12,
			fontFamily: "sans-serif",
			color: Q.color.tertiary,
			borderWidth: 1,
			borderColor: Q.color.border
		},
		emphasis: { selectorLabel: {
			show: !0,
			color: Q.color.quaternary
		} },
		selectorPosition: "auto",
		selectorItemGap: 7,
		selectorButtonGap: 10,
		tooltip: { show: !1 },
		triggerEvent: !1
	}, t;
}(Ng), GN = B, KN = I, qN = su, JN = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n.newlineDisabled = !1, n;
	}
	return t.prototype.init = function() {
		this.group.add(this._contentGroup = new qN()), this.group.add(this._selectorGroup = new qN()), this._isFirstRender = !0;
	}, t.prototype.getContentGroup = function() {
		return this._contentGroup;
	}, t.prototype.getSelectorGroup = function() {
		return this._selectorGroup;
	}, t.prototype.render = function(e, t, n) {
		var r = this._isFirstRender;
		if (this._isFirstRender = !1, this.resetInner(), e.get("show", !0)) {
			var i = e.get("align"), a = e.get("orient");
			(!i || i === "auto") && (i = e.get("left") === "right" && a === "vertical" ? "right" : "left");
			var o = e.get("selector", !0), s = e.get("selectorPosition", !0);
			o && (!s || s === "auto") && (s = a === "horizontal" ? "end" : "start"), this.renderInner(i, e, t, n, o, a, s);
			var c = Dg(e, n).refContainer, l = e.getBoxLayoutParams(), u = e.get("padding"), d = Tg(l, c, u), f = this.layoutInner(e, i, d, r, o, s), p = Tg(M({
				width: f.width,
				height: f.height
			}, l), c, u);
			this.group.x = p.x - f.x, this.group.y = p.y - f.y, this.group.markRedraw(), this.group.add(this._backgroundEl = sN(f, e));
		}
	}, t.prototype.resetInner = function() {
		this.getContentGroup().removeAll(), this._backgroundEl && this.group.remove(this._backgroundEl), this.getSelectorGroup().removeAll();
	}, t.prototype.renderInner = function(e, t, n, r, i, a, o) {
		var s = this.getContentGroup(), c = K(), l = t.get("selectedMode"), u = t.get("triggerEvent"), d = [];
		n.eachRawSeries(function(e) {
			!e.get("legendHoverLink") && d.push(e.id);
		}), KN(t.getData(), function(i, a) {
			var o = this, f = i.get("name");
			if (!this.newlineDisabled && (f === "" || f === "\n")) {
				var p = new qN();
				p.newline = !0, s.add(p);
				return;
			}
			var m = n.getSeriesByName(f)[0];
			if (!c.get(f)) if (m) {
				var h = m.getData(), g = h.getVisual("legendLineStyle") || {}, _ = h.getVisual("legendIcon"), v = h.getVisual("style"), y = this._createItem(m, f, a, i, t, e, g, v, _, l, r);
				y.on("click", GN(ZN, f, null, r, d)).on("mouseover", GN(QN, m.name, null, r, d)).on("mouseout", GN($N, m.name, null, r, d)), n.ssr && y.eachChild(function(e) {
					var t = yc(e);
					t.seriesIndex = m.seriesIndex, t.dataIndex = a, t.ssrType = "legend";
				}), u && y.eachChild(function(e) {
					o.packEventData(e, t, m, a, f);
				}), c.set(f, !0);
			} else n.eachRawSeries(function(o) {
				var s = this;
				if (!c.get(f) && o.legendVisualProvider) {
					var p = o.legendVisualProvider;
					if (!p.containName(f)) return;
					var m = p.indexOfName(f), h = p.getItemVisual(m, "style"), g = p.getItemVisual(m, "legendIcon"), _ = Pr(h.fill);
					_ && _[3] === 0 && (_[3] = .2, h = j(j({}, h), { fill: Br(_, "rgba") }));
					var v = this._createItem(o, f, a, i, t, e, {}, h, g, l, r);
					v.on("click", GN(ZN, null, f, r, d)).on("mouseover", GN(QN, null, f, r, d)).on("mouseout", GN($N, null, f, r, d)), n.ssr && v.eachChild(function(e) {
						var t = yc(e);
						t.seriesIndex = o.seriesIndex, t.dataIndex = a, t.ssrType = "legend";
					}), u && v.eachChild(function(e) {
						s.packEventData(e, t, o, a, f);
					}), c.set(f, !0);
				}
			}, this);
		}, this), i && this._createSelector(i, t, r, a, o);
	}, t.prototype.packEventData = function(e, t, n, r, i) {
		var a = {
			componentType: "legend",
			componentIndex: t.componentIndex,
			dataIndex: r,
			value: i,
			seriesIndex: n.seriesIndex
		};
		yc(e).eventData = a;
	}, t.prototype._createSelector = function(e, t, n, r, i) {
		var a = this.getSelectorGroup();
		KN(e, function(e) {
			var r = e.type, i = new _o({
				style: {
					x: 0,
					y: 0,
					align: "center",
					verticalAlign: "middle"
				},
				onclick: function() {
					n.dispatchAction({
						type: r === "all" ? "legendAllSelect" : "legendInverseSelect",
						legendId: t.id
					});
				}
			});
			a.add(i), hf(i, {
				normal: t.getModel("selectorLabel"),
				emphasis: t.getModel(["emphasis", "selectorLabel"])
			}, { defaultText: e.title }), El(i);
		});
	}, t.prototype._createItem = function(e, t, n, r, i, a, o, s, c, l, u) {
		var d = e.visualDrawType, f = i.get("itemWidth"), p = i.get("itemHeight"), m = i.isSelected(t), h = r.get("symbolRotate"), g = r.get("symbolKeepAspect"), _ = r.get("icon");
		c = _ || c || "roundRect";
		var v = YN(c, r, o, s, d, m, u), y = new qN(), b = r.getModel("textStyle");
		if (H(e.getLegendIcon) && (!_ || _ === "inherit")) y.add(e.getLegendIcon({
			itemWidth: f,
			itemHeight: p,
			icon: c,
			iconRotate: h,
			itemStyle: v.itemStyle,
			lineStyle: v.lineStyle,
			symbolKeepAspect: g
		}));
		else {
			var x = _ === "inherit" && e.getData().getVisual("symbol") ? h === "inherit" ? e.getData().getVisual("symbolRotate") : h : 0;
			y.add(XN({
				itemWidth: f,
				itemHeight: p,
				icon: c,
				iconRotate: x,
				itemStyle: v.itemStyle,
				lineStyle: v.lineStyle,
				symbolKeepAspect: g
			}));
		}
		var S = a === "left" ? f + 5 : -5, C = a, w = i.get("formatter"), T = t;
		U(w) && w ? T = w.replace("{name}", t ?? "") : H(w) && (T = w(t));
		var E = m ? b.getTextColor() : r.get("inactiveColor");
		y.add(new _o({ style: _f(b, {
			text: T,
			x: S,
			y: p / 2,
			fill: E,
			align: C,
			verticalAlign: "middle"
		}, { inheritColor: E }) }));
		var D = new fo({
			shape: y.getBoundingRect(),
			style: { fill: "transparent" }
		}), O = r.getModel("tooltip");
		return O.get("show") && Xd({
			el: D,
			componentModel: i,
			itemName: t,
			itemTooltipOption: O.option
		}), y.add(D), y.eachChild(function(e) {
			e.silent = !0;
		}), D.silent = !l, this.getContentGroup().add(y), El(y), y.__legendDataIndex = n, y;
	}, t.prototype.layoutInner = function(e, t, n, r, i, a) {
		var o = this.getContentGroup(), s = this.getSelectorGroup();
		xg(e.get("orient"), o, e.get("itemGap"), n.width, n.height);
		var c = o.getBoundingRect(), l = [-c.x, -c.y];
		if (s.markRedraw(), o.markRedraw(), i) {
			xg("horizontal", s, e.get("selectorItemGap", !0));
			var u = s.getBoundingRect(), d = [-u.x, -u.y], f = e.get("selectorButtonGap", !0), p = e.getOrient().index, m = p === 0 ? "width" : "height", h = p === 0 ? "height" : "width", g = p === 0 ? "y" : "x";
			a === "end" ? d[p] += c[m] + f : l[p] += u[m] + f, d[1 - p] += c[h] / 2 - u[h] / 2, s.x = d[0], s.y = d[1], o.x = l[0], o.y = l[1];
			var _ = {
				x: 0,
				y: 0
			};
			return _[m] = c[m] + f + u[m], _[h] = Math.max(c[h], u[h]), _[g] = Math.min(0, u[g] + d[1 - p]), _;
		} else return o.x = l[0], o.y = l[1], this.group.getBoundingRect();
	}, t.prototype.remove = function() {
		this.getContentGroup().removeAll(), this._isFirstRender = !0;
	}, t.type = "legend.plain", t;
}(nD);
function YN(e, t, n, r, i, a, o) {
	function s(e, t) {
		e.lineWidth === "auto" && (e.lineWidth = t.lineWidth > 0 ? 2 : 0), KN(e, function(n, r) {
			e[r] === "inherit" && (e[r] = t[r]);
		});
	}
	var c = t.getModel("itemStyle"), l = c.getItemStyle(), u = e.lastIndexOf("empty", 0) === 0 ? "fill" : "stroke", d = c.getShallow("decal");
	l.decal = !d || d === "inherit" ? r.decal : IO(d, o), l.fill === "inherit" && (l.fill = r[i]), l.stroke === "inherit" && (l.stroke = r[u]), l.opacity === "inherit" && (l.opacity = (i === "fill" ? r : n).opacity), s(l, r);
	var f = t.getModel("lineStyle"), p = f.getLineStyle();
	if (s(p, n), l.fill === "auto" && (l.fill = r.fill), l.stroke === "auto" && (l.stroke = r.fill), p.stroke === "auto" && (p.stroke = r.fill), !a) {
		var m = t.get("inactiveBorderWidth"), h = l[u];
		l.lineWidth = m === "auto" ? r.lineWidth > 0 && h ? 2 : 0 : l.lineWidth, l.fill = t.get("inactiveColor"), l.stroke = t.get("inactiveBorderColor"), p.stroke = f.get("inactiveColor"), p.lineWidth = f.get("inactiveWidth");
	}
	return {
		itemStyle: l,
		lineStyle: p
	};
}
function XN(e) {
	var t = e.icon || "roundRect", n = Y_(t, 0, 0, e.itemWidth, e.itemHeight, e.itemStyle.fill, e.symbolKeepAspect);
	return n.setStyle(e.itemStyle), n.rotation = (e.iconRotate || 0) * Math.PI / 180, n.setOrigin([e.itemWidth / 2, e.itemHeight / 2]), t.indexOf("empty") > -1 && (n.style.stroke = n.style.fill, n.style.fill = Q.color.neutral00, n.style.lineWidth = 2), n;
}
function ZN(e, t, n, r) {
	$N(e, t, n, r), n.dispatchAction({
		type: "legendToggleSelect",
		name: e ?? t
	}), QN(e, t, n, r);
}
function QN(e, t, n, r) {
	n.usingTHL() || n.dispatchAction({
		type: "highlight",
		seriesName: e,
		name: t,
		excludeSeriesId: r
	});
}
function $N(e, t, n, r) {
	n.usingTHL() || n.dispatchAction({
		type: "downplay",
		seriesName: e,
		name: t,
		excludeSeriesId: r
	});
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/legend/legendAction.js
function eP(e, t, n) {
	var r = e === "allSelect" || e === "inverseSelect", i = {}, a = [];
	n.eachComponent({
		mainType: "legend",
		query: t
	}, function(n) {
		r ? n[e]() : n[e](t.name), tP(n, i), a.push(n.componentIndex);
	});
	var o = {};
	return n.eachComponent("legend", function(e) {
		I(i, function(t, n) {
			e[t ? "select" : "unSelect"](n);
		}), tP(e, o);
	}), r ? {
		selected: o,
		legendIndex: a
	} : {
		name: t.name,
		selected: o
	};
}
function tP(e, t) {
	var n = t || {};
	return I(e.getData(), function(t) {
		var r = t.get("name");
		if (!(r === "\n" || r === "")) {
			var i = e.isSelected(r);
			Ae(n, r) ? n[r] = n[r] && i : n[r] = i;
		}
	}), n;
}
function nP(e) {
	e.registerAction("legendToggleSelect", "legendselectchanged", B(eP, "toggleSelected")), e.registerAction("legendAllSelect", "legendselectall", B(eP, "allSelect")), e.registerAction("legendInverseSelect", "legendinverseselect", B(eP, "inverseSelect")), e.registerAction("legendSelect", "legendselected", B(eP, "select")), e.registerAction("legendUnSelect", "legendunselected", B(eP, "unSelect"));
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/legend/legendFilter.js
var rP = vc(iP);
function iP(e) {
	var t = e.findComponents({ mainType: "legend" });
	t && t.length && e.filterSeries(function(e) {
		for (var n = 0; n < t.length; n++) if (!t[n].isSelected(e.name)) return !1;
		return !0;
	});
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/legend/installLegendPlain.js
function aP(e) {
	e.registerComponentModel(WN), e.registerComponentView(JN), e.registerProcessor(e.PRIORITY.PROCESSOR.SERIES_FILTER, rP), e.registerSubTypeDefaulter("legend", function() {
		return "plain";
	}), nP(e);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/legend/ScrollableLegendModel.js
var oP = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n;
	}
	return t.prototype.setScrollDataIndex = function(e) {
		this.option.scrollDataIndex = e;
	}, t.prototype.init = function(t, n, r) {
		var i = Ag(t);
		e.prototype.init.call(this, t, n, r), sP(this, t, i);
	}, t.prototype.mergeOption = function(t, n) {
		e.prototype.mergeOption.call(this, t, n), sP(this, this.option, t);
	}, t.type = "legend.scroll", t.defaultOption = qm(WN.defaultOption, {
		scrollDataIndex: 0,
		pageButtonItemGap: 5,
		pageButtonGap: null,
		pageButtonPosition: "end",
		pageFormatter: "{current}/{total}",
		pageIcons: {
			horizontal: ["M0,0L12,-10L12,10z", "M0,0L-12,-10L-12,10z"],
			vertical: ["M0,0L20,0L10,-20z", "M0,0L20,0L10,20z"]
		},
		pageIconColor: Q.color.accent50,
		pageIconInactiveColor: Q.color.accent10,
		pageIconSize: 15,
		pageTextStyle: { color: Q.color.tertiary },
		animationDurationUpdate: 800
	}), t;
}(WN);
function sP(e, t, n) {
	var r = e.getOrient(), i = [1, 1];
	i[r.index] = 0, kg(t, n, {
		type: "box",
		ignoreSize: !!i
	});
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/legend/ScrollableLegendView.js
var cP = su, lP = ["width", "height"], uP = ["x", "y"], dP = function(e) {
	o(t, e);
	function t() {
		var n = e !== null && e.apply(this, arguments) || this;
		return n.type = t.type, n.newlineDisabled = !0, n._currentIndex = 0, n;
	}
	return t.prototype.init = function() {
		e.prototype.init.call(this), this.group.add(this._containerGroup = new cP()), this._containerGroup.add(this.getContentGroup()), this.group.add(this._controllerGroup = new cP());
	}, t.prototype.resetInner = function() {
		e.prototype.resetInner.call(this), this._controllerGroup.removeAll(), this._containerGroup.removeClipPath(), this._containerGroup.__rectSize = null;
	}, t.prototype.renderInner = function(t, n, r, i, a, o, s) {
		var c = this;
		e.prototype.renderInner.call(this, t, n, r, i, a, o, s);
		var l = this._controllerGroup, u = n.get("pageIconSize", !0), d = V(u) ? u : [u, u];
		p("pagePrev", 0);
		var f = n.getModel("pageTextStyle");
		l.add(new _o({
			name: "pageText",
			style: {
				text: "xx/xx",
				fill: f.getTextColor(),
				font: f.getFont(),
				verticalAlign: "middle",
				align: "center"
			},
			silent: !0
		})), p("pageNext", 1);
		function p(e, t) {
			var r = e + "DataIndex", a = Hd(n.get("pageIcons", !0)[n.getOrient().name][t], { onclick: z(c._pageGo, c, r, n, i) }, {
				x: -d[0] / 2,
				y: -d[1] / 2,
				width: d[0],
				height: d[1]
			});
			a.name = e, l.add(a);
		}
	}, t.prototype.layoutInner = function(e, t, n, r, i, a) {
		var o = this.getSelectorGroup(), s = e.getOrient().index, c = lP[s], l = uP[s], u = lP[1 - s], d = uP[1 - s];
		i && xg("horizontal", o, e.get("selectorItemGap", !0));
		var f = e.get("selectorButtonGap", !0), p = o.getBoundingRect(), m = [-p.x, -p.y], h = k(n);
		i && (h[c] = n[c] - p[c] - f);
		var g = this._layoutContentAndController(e, r, h, s, c, u, d, l);
		if (i) {
			if (a === "end") m[s] += g[c] + f;
			else {
				var _ = p[c] + f;
				m[s] -= _, g[l] -= _;
			}
			g[c] += p[c] + f, m[1 - s] += g[d] + g[u] / 2 - p[u] / 2, g[u] = Math.max(g[u], p[u]), g[d] = Math.min(g[d], p[d] + m[1 - s]), o.x = m[0], o.y = m[1], o.markRedraw();
		}
		return g;
	}, t.prototype._layoutContentAndController = function(e, t, n, r, i, a, o, s) {
		var c = this.getContentGroup(), l = this._containerGroup, u = this._controllerGroup;
		xg(e.get("orient"), c, e.get("itemGap"), r ? n.width : null, r ? null : n.height), xg("horizontal", u, e.get("pageButtonItemGap", !0));
		var d = c.getBoundingRect(), f = u.getBoundingRect(), p = this._showController = d[i] > n[i], m = [-d.x, -d.y];
		t || (m[r] = c[s]);
		var h = [0, 0], g = [-f.x, -f.y], _ = G(e.get("pageButtonGap", !0), e.get("itemGap", !0));
		p && (e.get("pageButtonPosition", !0) === "end" ? g[r] += n[i] - f[i] : h[r] += f[i] + _), g[1 - r] += d[a] / 2 - f[a] / 2, c.setPosition(m), l.setPosition(h), u.setPosition(g);
		var v = {
			x: 0,
			y: 0
		};
		if (v[i] = p ? n[i] : d[i], v[a] = Math.max(d[a], f[a]), v[o] = Math.min(0, f[o] + g[1 - r]), l.__rectSize = n[i], p) {
			var y = {
				x: 0,
				y: 0
			};
			y[i] = Math.max(n[i] - f[i] - _, 0), y[a] = v[a], l.setClipPath(new fo({ shape: y })), l.__rectSize = y[i];
		} else u.eachChild(function(e) {
			e.attr({
				invisible: !0,
				silent: !0
			});
		});
		var b = this._getPageInfo(e);
		return b.pageIndex != null && ud(c, {
			x: b.contentPosition[0],
			y: b.contentPosition[1]
		}, p ? e : null), this._updatePageInfoView(e, b), v;
	}, t.prototype._pageGo = function(e, t, n) {
		var r = this._getPageInfo(t)[e];
		r != null && n.dispatchAction({
			type: "legendScroll",
			scrollDataIndex: r,
			legendId: t.id
		});
	}, t.prototype._updatePageInfoView = function(e, t) {
		var n = this._controllerGroup;
		I(["pagePrev", "pageNext"], function(r) {
			var i = t[r + "DataIndex"] != null, a = n.childOfName(r);
			a && (a.setStyle("fill", i ? e.get("pageIconColor", !0) : e.get("pageIconInactiveColor", !0)), a.cursor = i ? "pointer" : "default");
		});
		var r = n.childOfName("pageText"), i = e.get("pageFormatter"), a = t.pageIndex, o = a == null ? 0 : a + 1, s = t.pageCount;
		r && i && r.setStyle("text", U(i) ? i.replace("{current}", o == null ? "" : o + "").replace("{total}", s == null ? "" : s + "") : i({
			current: o,
			total: s
		}));
	}, t.prototype._getPageInfo = function(e) {
		var t = e.get("scrollDataIndex", !0), n = this.getContentGroup(), r = this._containerGroup.__rectSize, i = e.getOrient().index, a = lP[i], o = uP[i], s = this._findTargetItemIndex(t), c = n.children(), l = c[s], u = c.length, d = +!!u, f = {
			contentPosition: [n.x, n.y],
			pageCount: d,
			pageIndex: d - 1,
			pagePrevDataIndex: null,
			pageNextDataIndex: null
		};
		if (!l) return f;
		var p = v(l);
		f.contentPosition[i] = -p.s;
		for (var m = s + 1, h = p, g = p, _ = null; m <= u; ++m) _ = v(c[m]), (!_ && g.e > h.s + r || _ && !y(_, h.s)) && (h = g.i > h.i ? g : _, h && (f.pageNextDataIndex ??= h.i, ++f.pageCount)), g = _;
		for (var m = s - 1, h = p, g = p, _ = null; m >= -1; --m) _ = v(c[m]), (!_ || !y(g, _.s)) && h.i < g.i && (g = h, f.pagePrevDataIndex ??= h.i, ++f.pageCount, ++f.pageIndex), h = _;
		return f;
		function v(e) {
			if (e) {
				var t = e.getBoundingRect(), n = t[o] + e[o];
				return {
					s: n,
					e: n + t[a],
					i: e.__legendDataIndex
				};
			}
		}
		function y(e, t) {
			return e.e >= t && e.s <= t + r;
		}
	}, t.prototype._findTargetItemIndex = function(e) {
		if (!this._showController) return 0;
		var t, n = this.getContentGroup(), r;
		return n.eachChild(function(n, i) {
			var a = n.__legendDataIndex;
			r == null && a != null && (r = i), a === e && (t = i);
		}), t ?? r;
	}, t.type = "legend.scroll", t;
}(JN);
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/legend/scrollableLegendAction.js
function fP(e) {
	e.registerAction("legendScroll", "legendscroll", function(e, t) {
		var n = e.scrollDataIndex;
		n != null && t.eachComponent({
			mainType: "legend",
			subType: "scroll",
			query: e
		}, function(e) {
			e.setScrollDataIndex(n);
		});
	});
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/legend/installLegendScroll.js
function pP(e) {
	SA(aP), e.registerComponentModel(oP), e.registerComponentView(dP), fP(e);
}
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/component/legend/install.js
function mP(e) {
	SA(aP), SA(pP);
}
//#endregion
//#region node_modules/.pnpm/zrender@6.1.0/node_modules/zrender/lib/canvas/Layer.js
function hP(e, t, n) {
	var r = p.createCanvas(), i = t.getWidth(), a = t.getHeight(), o = r.style;
	return o && (o.position = "absolute", o.left = "0", o.top = "0", o.width = i + "px", o.height = a + "px", r.setAttribute("data-zr-dom-id", e)), r.width = i * n, r.height = a * n, r;
}
function gP(e) {
	return !e.__cursors.get(0);
}
function _P(e) {
	var t = e.__cursors.get(0);
	return {
		startIdx: t ? t.startIdx : 0,
		endIdx: t ? t.endIdx : 0
	};
}
var vP = function(e) {
	o(t, e);
	function t(t, n, r) {
		var i = e.call(this) || this;
		i.motionBlur = !1, i.lastFrameAlpha = .7, i.dpr = 1, i.virtual = !1, i.config = {}, i.zlevel = 0, i.zlevel2 = 0, i.maxRepaintRectCount = 5, i.__dirty = !0, i.__firstTimePaint = !0, i.__prevIdx = {
			startIdx: 0,
			endIdx: 0
		};
		var a;
		r ||= _i, typeof t == "string" ? a = hP(t, n, r) : W(t) && (a = t, t = a.id), i.id = t, i.dom = a;
		var o = a.style;
		return o && (ke(a), a.onselectstart = function() {
			return !1;
		}, o.padding = "0", o.margin = "0", o.borderWidth = "0"), i.painter = n, i.dpr = r, i;
	}
	return t.prototype.afterBrush = function() {
		this.__prevIdx = _P(this);
	}, t.prototype.initContext = function() {
		this.ctx = this.dom.getContext("2d"), this.ctx.dpr = this.dpr;
	}, t.prototype.setUnpainted = function() {
		this.__firstTimePaint = !0;
	}, t.prototype.createBackBuffer = function() {
		var e = this.dpr;
		this.domBack = hP("back-" + this.id, this.painter, e), this.ctxBack = this.domBack.getContext("2d"), e !== 1 && this.ctxBack.scale(e, e);
	}, t.prototype.createRepaintRects = function(e, t, n, r) {
		if (this.__firstTimePaint) return this.__firstTimePaint = !1, null;
		var i = [], a = this.maxRepaintRectCount, o = !1, s = new Y(0, 0, 0, 0);
		function c(e) {
			if (!(!e.isFinite() || e.isZero())) if (i.length === 0) {
				var t = new Y(0, 0, 0, 0);
				t.copy(e), i.push(t);
			} else {
				for (var n = !1, r = Infinity, c = 0, l = 0; l < i.length; ++l) {
					var u = i[l];
					if (u.intersect(e)) {
						var d = new Y(0, 0, 0, 0);
						d.copy(u), d.union(e), i[l] = d, n = !0;
						break;
					} else if (o) {
						s.copy(e), s.union(u);
						var f = e.width * e.height, p = u.width * u.height, m = s.width * s.height - f - p;
						m < r && (r = m, c = l);
					}
				}
				if (o && (i[c].union(e), n = !0), !n) {
					var t = new Y(0, 0, 0, 0);
					t.copy(e), i.push(t);
				}
				o ||= i.length >= a;
			}
		}
		for (var l = _P(this), u = l.startIdx; u < l.endIdx; ++u) {
			var d = e[u];
			if (d) {
				var f = d.shouldBePainted(n, r, !0, !0), p = d.__isRendered && (d.__dirty & 1 || !f) ? d.getPrevPaintRect() : null;
				p && c(p);
				var m = f && (d.__dirty & 1 || !d.__isRendered) ? d.getPaintRect() : null;
				m && c(m);
			}
		}
		for (var h = this.__prevIdx, u = h.startIdx; u < h.endIdx; ++u) {
			var d = t[u], f = d && d.shouldBePainted(n, r, !0, !0);
			if (d && (!f || !d.__zr) && d.__isRendered) {
				var p = d.getPrevPaintRect();
				p && c(p);
			}
		}
		var g;
		do {
			g = !1;
			for (var u = 0; u < i.length;) {
				if (i[u].isZero()) {
					i.splice(u, 1);
					continue;
				}
				for (var _ = u + 1; _ < i.length;) i[u].intersect(i[_]) ? (g = !0, i[u].union(i[_]), i.splice(_, 1)) : _++;
				u++;
			}
		} while (g);
		return this._paintRects = i, i;
	}, t.prototype.debugGetPaintRects = function() {
		return (this._paintRects || []).slice();
	}, t.prototype.resize = function(e, t) {
		var n = this.dpr, r = this.dom, i = r.style, a = this.domBack;
		i && (i.width = e + "px", i.height = t + "px"), r.width = e * n, r.height = t * n, a && (a.width = e * n, a.height = t * n, n !== 1 && this.ctxBack.scale(n, n));
	}, t.prototype.clear = function(e, t, n) {
		var r = this.dom, i = this.ctx, a = r.width, o = r.height;
		t ||= this.clearColor;
		var s = this.motionBlur && !e, c = this.lastFrameAlpha, l = this.dpr, u = this;
		s && (this.domBack || this.createBackBuffer(), this.ctxBack.globalCompositeOperation = "copy", this.ctxBack.drawImage(r, 0, 0, a / l, o / l));
		var d = this.domBack;
		function f(e, n, r, a) {
			if (i.clearRect(e, n, r, a), t && t !== "transparent") {
				var o = void 0;
				de(t) ? (o = (t.global || t.__width === r && t.__height === a) && t.__canvasGradient || $D(i, t, {
					x: 0,
					y: 0,
					width: r,
					height: a
				}), t.__canvasGradient = o, t.__width = r, t.__height = a) : fe(t) && (t.scaleX = t.scaleX || l, t.scaleY = t.scaleY || l, o = dO(i, t, { dirty: function() {
					u.setUnpainted(), u.painter.refresh();
				} })), i.save(), i.fillStyle = o || t, i.fillRect(e, n, r, a), i.restore();
			}
			s && (i.save(), i.globalAlpha = c, i.drawImage(d, e, n, r, a), i.restore());
		}
		!n || s ? f(0, 0, a, o) : n.length && I(n, function(e) {
			f(e.x * l, e.y * l, e.width * l, e.height * l);
		});
	}, t;
}(hi), yP = 1e5, bP = 314159, xP = void 0, SP = 1, CP = 2;
function wP(e) {
	return e ? e.__builtin__ ? !0 : !(typeof e.resize != "function" || typeof e.refresh != "function") : !1;
}
function TP(e, t) {
	var n = document.createElement("div");
	return n.style.cssText = [
		"position:relative",
		"width:" + e + "px",
		"height:" + t + "px",
		"padding:0",
		"margin:0",
		"border-width:0"
	].join(";") + ";", n;
}
function EP(e, t, n, r) {
	var i = new vP(e, t, t.dpr);
	return i.zlevel = n, i.zlevel2 = r, i.__builtin__ = !0, DP(i), i;
}
function DP(e) {
	e.__cursorStack = [], e.__cursors = K();
}
function OP(e) {
	return e.startIdx = e.drawIdx = e.endIdx = e.endIdxNew = 0, e.used = !1, e.first = e.last = NaN, e.notClearIdx = -1, e;
}
function kP(e, t) {
	var n = e.__cursors, r = +t;
	return n.get(r) || (e.__cursorStack.push(r), n.set(r, OP({ key: r })));
}
function AP(e, t) {
	for (var n = e.__cursorStack, r = 0; r < n.length; r++) t(e.__cursors.get(n[r]));
}
function jP(e, t) {
	var n = e.layers;
	return n[t] || (n[t] = [
		,
		,
		,
	]);
}
function MP(e, t, n) {
	for (var r = e.layerStack, i = 0; i < r.length; i++) {
		var a = r[i].zl, o = r[i].zl2, s = e.layers[a][o];
		(!n || (!(n & NP) || s.__builtin__) && (!(n & PP) || !s.__builtin__) && (!(n & FP) || s !== e.hoverlayer)) && t(s, a, o, i);
	}
}
var NP = 1, PP = 2, FP = 4, IP = NP | FP, LP = function() {
	function e(e, t, n, r) {
		this.type = "canvas", this._prevDisplayList = [], this._layerConfig = {}, this._needsManuallyCompositing = !1, this.type = "canvas", this._i = {
			layerStack: [],
			layers: []
		};
		var i = !e.nodeName || e.nodeName.toUpperCase() === "CANVAS";
		if (this._opts = n = j({}, n || {}), this.dpr = n.devicePixelRatio || _i, this._singleCanvas = i, this.root = e, e.style && (ke(e), e.innerHTML = ""), this.storage = t, this._prevDisplayList = [], i) {
			var a = e, o = a.width, s = a.height;
			n.width != null && (o = n.width), n.height != null && (s = n.height), this.dpr = n.devicePixelRatio || 1, a.width = o * this.dpr, a.height = s * this.dpr, this._width = o, this._height = s;
			var c = EP(a, this, bP, 0);
			c.initContext(), this._insertLayer(c, bP, 0, !0), this._domRoot = e;
		} else {
			this._width = nO(e, 0, n), this._height = nO(e, 1, n);
			var l = this._domRoot = TP(this._width, this._height);
			e.appendChild(l);
		}
	}
	return e.prototype.getType = function() {
		return "canvas";
	}, e.prototype.isSingleCanvas = function() {
		return this._singleCanvas;
	}, e.prototype.getViewportRoot = function() {
		return this._domRoot;
	}, e.prototype.getViewportRootOffset = function() {
		var e = this.getViewportRoot();
		if (e) return {
			offsetLeft: e.offsetLeft || 0,
			offsetTop: e.offsetTop || 0
		};
	}, e.prototype.refresh = function(e) {
		var t = e && !W(e) ? { paintAll: !!e } : e || {}, n = G(t.refresh, !0), r = G(t.refreshHover, !1);
		if (r && (this._hoverLayerDirty = CP), !n) return r && this._paintHoverList(this.storage.getDisplayList(!1)), this;
		var i = this.storage.getDisplayList(!0);
		this._updateLayerStatus(i, t.paintAll), this._redrawId = Math.random();
		var a = this._prevDisplayList;
		this._paintList(i, a, this._redrawId);
		var o = this._backgroundColor;
		return MP(this._i, function(e, t, n, r) {
			e.refresh && e.refresh(r === 0 ? o : null);
		}, PP), this._opts.useDirtyRect && (this._prevDisplayList = i.slice()), this;
	}, e.prototype._paintHoverList = function(e) {
		var t = this._i.hoverlayer, n = this._hoverLayerDirty;
		if (this._hoverLayerDirty = xP, n !== xP && (!t && n === CP && (t = this._i.hoverlayer = this._ensureLayer(yP)), t)) {
			t.clear();
			for (var r = {
				inHover: !0,
				viewWidth: this._width,
				viewHeight: this._height,
				beforeBrushParam: {}
			}, i, a = 0, o = e.length; a < o; a++) {
				var s = e[a];
				if (s.__inHover) {
					i || (i = t.ctx, i.save());
					var c = s.__hoverStyle, l = void 0;
					c && (l = s.style, s.style = c), AO(i, s, r), c && (s.style = l);
				}
			}
			i && (jO(i, r), i.restore());
		}
	}, e.prototype.getHoverLayer = function() {
		return this._ensureLayer(yP);
	}, e.prototype.paintOne = function(e, t) {
		kO(e, t);
	}, e.prototype._paintList = function(e, t, n) {
		if (this._redrawId === n) {
			var r = this._doPaintList(e, t);
			if (this._needsManuallyCompositing && this._compositeManually(), r) MP(this._i, function(e) {
				e.afterBrush && e.afterBrush();
			}, IP), this._paintHoverList(e);
			else {
				var i = this;
				AT(function() {
					i._paintList(e, t, n);
				});
			}
		}
	}, e.prototype._compositeManually = function() {
		var e = this._ensureLayer(bP).ctx, t = this._domRoot.width, n = this._domRoot.height;
		e.clearRect(0, 0, t, n), MP(this._i, function(r) {
			r.virtual && e.drawImage(r.dom, 0, 0, t, n);
		}, NP);
	}, e.prototype._doPaintList = function(e, t) {
		var n = this, r = !0;
		return MP(this._i, function(i) {
			var a = !1;
			if (AP(i, function(e) {
				(e.drawIdx < e.endIdx || e.notClearIdx >= 0) && (a = !0);
			}), !(!a && !i.__dirty)) {
				var o = n._opts.useDirtyRect && !gP(i) ? i.createRepaintRects(e, t, n._width, n._height) : null, s = n._i.layerStack[0], c = !0;
				if (i.__dirty) {
					c = !1, i.__dirty = !1;
					var l = i.zlevel === s.zl && i.zlevel2 === s.zl2 ? n._backgroundColor : null;
					i.clear(!1, l, o);
				}
				AP(i, function(t) {
					var a = n._paintPerCursor(i, t, e, o, c);
					r &&= a;
				});
			}
		}, IP), q.wxa && MP(this._i, function(e) {
			e && e.ctx && e.ctx.draw && e.ctx.draw();
		}), r;
	}, e.prototype._paintPerCursor = function(e, t, n, r, i) {
		var a = e.ctx;
		if (r) if (!r.length) t.drawIdx = t.endIdx;
		else for (var o = this.dpr, s = 0; s < r.length; ++s) {
			var c = r[s];
			a.save(), a.beginPath(), a.rect(c.x * o, c.y * o, c.width * o, c.height * o), a.clip(), this._paintPerCursorInRect(e, t, n, c, i), a.restore();
		}
		else a.save(), this._paintPerCursorInRect(e, t, n, null, i), a.restore();
		return t.drawIdx >= t.endIdx;
	}, e.prototype._paintPerCursorInRect = function(e, t, n, r, i) {
		for (var a = {
			inHover: !1,
			allClipped: !1,
			prevEl: null,
			viewWidth: this._width,
			viewHeight: this._height,
			beforeBrushParam: { contentRetained: i }
		}, o = e.ctx, s = gP(e), c = s && p.getTime(), l = t.drawIdx, u = t.notClearIdx, d = u >= 0 ? Math.min(u, l) : l; d < t.endIdx; d++) {
			var f = n[d];
			if (!(d < l && !f.notClear)) {
				if (f.__inHover && (this._hoverLayerDirty = CP), r != null) {
					var m = f.getPaintRect();
					m && m.intersect(r) && (AO(o, f, a), f.setPrevPaintRect(m));
				} else AO(o, f, a);
				if (s && p.getTime() - c > 15) {
					d++;
					break;
				}
			}
		}
		jO(o, a), t.drawIdx = Math.max(d, l);
	}, e.prototype.getLayer = function(e, t) {
		return this._ensureLayer(e, 0, t);
	}, e.prototype._ensureLayer = function(e, t, n) {
		t ||= 0;
		var r = this._singleCanvas;
		r && !this._needsManuallyCompositing && (e = bP, t = 0);
		var i = jP(this._i, e)[t];
		return i || (i = EP("zr_" + e + "." + t, this, e, t), this._layerConfig[e] && A(i, this._layerConfig[e], !0), (n || r && e !== bP) && (i.virtual = !0), this._insertLayer(i, e, t, !1), i.initContext()), i;
	}, e.prototype.insertLayer = function(e, t) {
		this._insertLayer(t, e, 0, !1);
	}, e.prototype._insertLayer = function(e, t, n, r) {
		var i = this._i, a = i.layers, o = i.layerStack, s = this._domRoot, c = null;
		if (!(a[t] && a[t][n]) && wP(e)) {
			for (var l = o.length, u = 0; u < l && (o[u].zl < t || o[u].zl === t && o[u].zl2 < n);) u++;
			if (u > 0 && (c = jP(i, o[u - 1].zl)[o[u - 1].zl2]), o.splice(u, 0, {
				zl: t,
				zl2: n
			}), jP(i, t)[n] = e, !r && !e.virtual) if (c) {
				var d = c.dom;
				d.nextSibling ? s.insertBefore(e.dom, d.nextSibling) : s.appendChild(e.dom);
			} else s.firstChild ? s.insertBefore(e.dom, s.firstChild) : s.appendChild(e.dom);
			e.painter ||= this;
		}
	}, e.prototype.eachLayer = function(e, t) {
		return MP(this._i, function(n, r) {
			e.call(t, n, r);
		});
	}, e.prototype.eachBuiltinLayer = function(e, t) {
		return MP(this._i, function(n, r) {
			e.call(t, n, r);
		}, NP);
	}, e.prototype.eachOtherLayer = function(e, t) {
		return MP(this._i, function(n, r) {
			e.call(t, n, r);
		}, PP);
	}, e.prototype.getLayers = function() {
		var e = {};
		return MP(this._i, function(t, n, r) {
			e[t.id] = t;
		}), e;
	}, e.prototype._updateLayerStatus = function(e, t) {
		var n = this;
		if (n._singleCanvas) for (var r = 1; r < e.length; r++) {
			var i = e[r];
			if (i.zlevel !== e[r - 1].zlevel || i.incremental) {
				n._needsManuallyCompositing = !0;
				break;
			}
		}
		MP(n._i, function(e) {
			e.__dirty = !1, AP(e, function(e) {
				e.used = !1, e.endIdxNew = 0, e.notClearIdx = -1;
			});
		}, IP);
		for (var a, o = null, s = null, c = !1, l = 0, u = e.length; l < u; l++) {
			var i = e[l], d = i.zlevel, f = i.incremental, p = void 0;
			if (a !== d && (a = d, c = !1), f ? (c = !0, p = 1) : p = c ? 2 : 0, (!o || d !== o.zlevel || p !== o.zlevel2) && (o = n._ensureLayer(d, p), s = null, !o.__builtin__)) {
				O("ZLevel " + d + " has been used by unknown layer " + o.id);
				continue;
			}
			if ((!s || f !== s.key) && (s = kP(o, f), !s.used)) if (s.used = !0, !t && s.first === i.id) {
				var m = l - s.startIdx;
				s.startIdx = l, s.drawIdx += m, s.endIdx += m;
			} else o.__dirty = !0, s.first = i.id, s.startIdx = s.drawIdx = l, s.endIdx = l + 1;
			s.endIdxNew = l + 1, i.__dirty & 1 && !i.__inHover && ((!f || !i.notClear && l < s.drawIdx) && (o.__dirty = !0), f && i.notClear && s.notClearIdx < 0 && (s.notClearIdx = l));
		}
		MP(n._i, function(t) {
			for (var r = t.__cursorStack, i = t.__cursors, a = r.length - 1; a >= 0; a--) {
				var o = i.get(r[a]);
				if (!o.used) t.__dirty = !0, i.removeKey(r[a]), r.splice(a, 1);
				else {
					var s = o.endIdxNew;
					(gP(t) ? s < o.drawIdx : s !== o.endIdx || !s || e[s - 1].id !== o.last) && (t.__dirty = !0), o.endIdx = o.endIdxNew, o.last = s ? e[s - 1].id : NaN;
				}
			}
			t.__dirty && (AP(t, function(e) {
				e.drawIdx = e.startIdx;
			}), n._hoverLayerDirty === xP && (n._hoverLayerDirty = SP));
		}, IP);
	}, e.prototype.clear = function() {
		return MP(this._i, function(e) {
			e.clear(), DP(e);
		}, NP), this;
	}, e.prototype.setBackgroundColor = function(e) {
		this._backgroundColor = e, MP(this._i, function(e) {
			e.setUnpainted();
		});
	}, e.prototype.configLayer = function(e, t) {
		if (t) {
			var n = this._layerConfig;
			n[e] ? A(n[e], t, !0) : n[e] = t, MP(this._i, function(e, t) {
				A(e, n[t], !0);
			});
		}
	}, e.prototype.delLayer = function(e) {
		for (var t = this._i.layerStack, n = this._i.layers, r = t.length - 1; r >= 0; r--) {
			var i = t[r];
			if (i.zl === e) {
				var a = n[e][i.zl2];
				if (a.__builtin__) continue;
				if (t.splice(r, 1), n[e][i.zl2] = void 0, !a.virtual) {
					var o = a.dom.parentNode;
					o && o.removeChild(a.dom);
				}
			}
		}
	}, e.prototype.resize = function(e, t) {
		if (this._domRoot.style) {
			var n = this._domRoot;
			n.style.display = "none";
			var r = this._opts, i = this.root;
			e != null && (r.width = e), t != null && (r.height = t), e = nO(i, 0, r), t = nO(i, 1, r), n.style.display = "", (this._width !== e || t !== this._height) && (n.style.width = e + "px", n.style.height = t + "px", MP(this._i, function(n) {
				n.resize(e, t);
			}), this.refresh({ paintAll: !0 })), this._width = e, this._height = t;
		} else {
			if (e == null || t == null) return;
			this._width = e, this._height = t, this._ensureLayer(bP).resize(e, t);
		}
		return this;
	}, e.prototype.clearLayer = function(e) {
		I(this._i.layers[e], function(e) {
			e && !e.__builtin__ && e.clear();
		});
	}, e.prototype.dispose = function() {
		this.root.innerHTML = "", this.root = this.storage = this._domRoot = this._i = null;
	}, e.prototype.getRenderedCanvas = function(e) {
		if (e ||= {}, this._singleCanvas && !this._compositeManually) return this._i.layers[bP][0].dom;
		var t = new vP("image", this, e.pixelRatio || this.dpr);
		t.initContext(), t.clear(!1, e.backgroundColor || this._backgroundColor);
		var n = t.ctx;
		if (e.pixelRatio <= this.dpr) {
			this.refresh();
			var r = t.dom.width, i = t.dom.height;
			MP(this._i, function(e) {
				e.__builtin__ ? n.drawImage(e.dom, 0, 0, r, i) : e.renderToCanvas && (n.save(), e.renderToCanvas(n), n.restore());
			});
		} else {
			for (var a = {
				inHover: !1,
				viewWidth: this._width,
				viewHeight: this._height,
				beforeBrushParam: {}
			}, o = this.storage.getDisplayList(!0), s = 0, c = o.length; s < c; s++) {
				var l = o[s];
				AO(n, l, a);
			}
			jO(n, a);
		}
		return t.dom;
	}, e.prototype.getWidth = function() {
		return this._width;
	}, e.prototype.getHeight = function() {
		return this._height;
	}, e;
}();
//#endregion
//#region node_modules/.pnpm/echarts@6.1.0/node_modules/echarts/lib/renderer/installCanvasRenderer.js
function RP(e) {
	e.registerPainter("canvas", LP);
}
//#endregion
//#region packages/charts/src/safe-option.ts
var zP = new Set([
	"formatter",
	"renderItem",
	"renderMode",
	"map",
	"geoJSON",
	"geoJson"
]), BP = /^(?:https?:)?\/\//i;
function VP(e) {
	HP(e, "option", /* @__PURE__ */ new WeakSet());
}
function HP(e, t, n) {
	if (e === null || typeof e == "string" || typeof e == "boolean") {
		if (typeof e == "string" && (BP.test(e) || e.startsWith("image://"))) throw TypeError(`${t} cannot reference an external image or URL.`);
		return;
	}
	if (typeof e == "number") {
		if (!Number.isFinite(e)) throw TypeError(`${t} must contain finite JSON numbers only.`);
		return;
	}
	if (typeof e != "object") throw TypeError(`${t} must contain JSON values only.`);
	if (n.has(e)) throw TypeError(`${t} must not contain circular references.`);
	if (n.add(e), Array.isArray(e)) {
		e.forEach((e, r) => HP(e, `${t}[${r}]`, n)), n.delete(e);
		return;
	}
	let r = Object.getPrototypeOf(e);
	if (r !== Object.prototype && r !== null) throw TypeError(`${t} must contain plain JSON objects only.`);
	for (let [r, i] of Object.entries(e)) {
		if (zP.has(r)) throw TypeError(`${t}.${r} is not supported by controlled EChart.`);
		if (r === "type" && (i === "custom" || i === "map")) throw TypeError(`${t}.type cannot use custom or map series.`);
		HP(i, `${t}.${r}`, n);
	}
	n.delete(e);
}
//#endregion
//#region packages/charts/src/index.tsx
SA([
	tw,
	dM,
	eM,
	xb,
	Vw,
	qj,
	oN,
	mP,
	Hj,
	HN,
	zN,
	RP
]);
function UP({ ariaLabel: r, className: i, option: a, style: o }) {
	let s = t(null), c = t(null);
	return VP(a), e(() => {
		let e = s.current;
		if (!e) return;
		let t = nA(e);
		c.current = t;
		let n = typeof ResizeObserver > "u" ? null : new ResizeObserver(() => t.resize());
		return n?.observe(e), () => {
			n?.disconnect(), c.current = null, t.dispose();
		};
	}, []), e(() => {
		let e = {
			...a,
			tooltip: a.tooltip && typeof a.tooltip == "object" ? {
				...a.tooltip,
				renderMode: "richText"
			} : a.tooltip
		};
		c.current?.setOption(e, {
			notMerge: !0,
			lazyUpdate: !0
		});
	}, [a]), /* @__PURE__ */ n("div", {
		ref: s,
		"aria-label": r,
		className: i,
		role: "img",
		style: o
	});
}
//#endregion
export { UP as EChart };
