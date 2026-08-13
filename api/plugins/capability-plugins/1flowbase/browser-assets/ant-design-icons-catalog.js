import * as React$1 from "react";
import React, { createContext, useContext, useEffect } from "react";
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/components/Context.js
var IconContext = /*#__PURE__*/ createContext({});
//#endregion
//#region node_modules/.pnpm/clsx@2.1.1/node_modules/clsx/dist/clsx.mjs
function r(e) {
	var t, f, n = "";
	if ("string" == typeof e || "number" == typeof e) n += e;
	else if ("object" == typeof e) if (Array.isArray(e)) {
		var o = e.length;
		for (t = 0; t < o; t++) e[t] && (f = r(e[t])) && (n && (n += " "), n += f);
	} else for (f in e) e[f] && (n && (n += " "), n += f);
	return n;
}
function clsx() {
	for (var e, t, f = 0, n = "", o = arguments.length; f < o; f++) (e = arguments[f]) && (t = r(e)) && (n && (n += " "), n += t);
	return n;
}
//#endregion
//#region node_modules/.pnpm/@ant-design+fast-color@3.0.1/node_modules/@ant-design/fast-color/es/presetColors.js
var presetColors_default = {
	aliceblue: "9ehhb",
	antiquewhite: "9sgk7",
	aqua: "1ekf",
	aquamarine: "4zsno",
	azure: "9eiv3",
	beige: "9lhp8",
	bisque: "9zg04",
	black: "0",
	blanchedalmond: "9zhe5",
	blue: "73",
	blueviolet: "5e31e",
	brown: "6g016",
	burlywood: "8ouiv",
	cadetblue: "3qba8",
	chartreuse: "4zshs",
	chocolate: "87k0u",
	coral: "9yvyo",
	cornflowerblue: "3xael",
	cornsilk: "9zjz0",
	crimson: "8l4xo",
	cyan: "1ekf",
	darkblue: "3v",
	darkcyan: "rkb",
	darkgoldenrod: "776yz",
	darkgray: "6mbhl",
	darkgreen: "jr4",
	darkgrey: "6mbhl",
	darkkhaki: "7ehkb",
	darkmagenta: "5f91n",
	darkolivegreen: "3bzfz",
	darkorange: "9yygw",
	darkorchid: "5z6x8",
	darkred: "5f8xs",
	darksalmon: "9441m",
	darkseagreen: "5lwgf",
	darkslateblue: "2th1n",
	darkslategray: "1ugcv",
	darkslategrey: "1ugcv",
	darkturquoise: "14up",
	darkviolet: "5rw7n",
	deeppink: "9yavn",
	deepskyblue: "11xb",
	dimgray: "442g9",
	dimgrey: "442g9",
	dodgerblue: "16xof",
	firebrick: "6y7tu",
	floralwhite: "9zkds",
	forestgreen: "1cisi",
	fuchsia: "9y70f",
	gainsboro: "8m8kc",
	ghostwhite: "9pq0v",
	goldenrod: "8j4f4",
	gold: "9zda8",
	gray: "50i2o",
	green: "pa8",
	greenyellow: "6senj",
	grey: "50i2o",
	honeydew: "9eiuo",
	hotpink: "9yrp0",
	indianred: "80gnw",
	indigo: "2xcoy",
	ivory: "9zldc",
	khaki: "9edu4",
	lavenderblush: "9ziet",
	lavender: "90c8q",
	lawngreen: "4vk74",
	lemonchiffon: "9zkct",
	lightblue: "6s73a",
	lightcoral: "9dtog",
	lightcyan: "8s1rz",
	lightgoldenrodyellow: "9sjiq",
	lightgray: "89jo3",
	lightgreen: "5nkwg",
	lightgrey: "89jo3",
	lightpink: "9z6wx",
	lightsalmon: "9z2ii",
	lightseagreen: "19xgq",
	lightskyblue: "5arju",
	lightslategray: "4nwk9",
	lightslategrey: "4nwk9",
	lightsteelblue: "6wau6",
	lightyellow: "9zlcw",
	lime: "1edc",
	limegreen: "1zcxe",
	linen: "9shk6",
	magenta: "9y70f",
	maroon: "4zsow",
	mediumaquamarine: "40eju",
	mediumblue: "5p",
	mediumorchid: "79qkz",
	mediumpurple: "5r3rv",
	mediumseagreen: "2d9ip",
	mediumslateblue: "4tcku",
	mediumspringgreen: "1di2",
	mediumturquoise: "2uabw",
	mediumvioletred: "7rn9h",
	midnightblue: "z980",
	mintcream: "9ljp6",
	mistyrose: "9zg0x",
	moccasin: "9zfzp",
	navajowhite: "9zest",
	navy: "3k",
	oldlace: "9wq92",
	olive: "50hz4",
	olivedrab: "472ub",
	orange: "9z3eo",
	orangered: "9ykg0",
	orchid: "8iu3a",
	palegoldenrod: "9bl4a",
	palegreen: "5yw0o",
	paleturquoise: "6v4ku",
	palevioletred: "8k8lv",
	papayawhip: "9zi6t",
	peachpuff: "9ze0p",
	peru: "80oqn",
	pink: "9z8wb",
	plum: "8nba5",
	powderblue: "6wgdi",
	purple: "4zssg",
	rebeccapurple: "3zk49",
	red: "9y6tc",
	rosybrown: "7cv4f",
	royalblue: "2jvtt",
	saddlebrown: "5fmkz",
	salmon: "9rvci",
	sandybrown: "9jn1c",
	seagreen: "1tdnb",
	seashell: "9zje6",
	sienna: "6973h",
	silver: "7ir40",
	skyblue: "5arjf",
	slateblue: "45e4t",
	slategray: "4e100",
	slategrey: "4e100",
	snow: "9zke2",
	springgreen: "1egv",
	steelblue: "2r1kk",
	tan: "87yx8",
	teal: "pds",
	thistle: "8ggk8",
	tomato: "9yqfb",
	turquoise: "2j4r4",
	violet: "9b10u",
	wheat: "9ld4j",
	white: "9zldr",
	whitesmoke: "9lhpx",
	yellow: "9zl6o",
	yellowgreen: "61fzm"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+fast-color@3.0.1/node_modules/@ant-design/fast-color/es/FastColor.js
var round = Math.round;
/**
* Support format, alpha unit will check the % mark:
* - rgba(102, 204, 255, .5)      -> [102, 204, 255, 0.5]
* - rgb(102 204 255 / .5)        -> [102, 204, 255, 0.5]
* - rgb(100%, 50%, 0% / 50%)     -> [255, 128, 0, 0.5]
* - hsl(270, 60, 40, .5)         -> [270, 60, 40, 0.5]
* - hsl(270deg 60% 40% / 50%)   -> [270, 60, 40, 0.5]
*
* When `base` is provided, the percentage value will be divided by `base`.
*/
function splitColorStr(str, parseNum) {
	const match = str.replace(/^[^(]*\((.*)/, "$1").replace(/\).*/, "").match(/\d*\.?\d+%?/g) || [];
	const numList = match.map((item) => parseFloat(item));
	for (let i = 0; i < 3; i += 1) numList[i] = parseNum(numList[i] || 0, match[i] || "", i);
	if (match[3]) numList[3] = match[3].includes("%") ? numList[3] / 100 : numList[3];
	else numList[3] = 1;
	return numList;
}
var parseHSVorHSL = (num, _, index) => index === 0 ? num : num / 100;
/** round and limit number to integer between 0-255 */
function limitRange(value, max) {
	const mergedMax = max || 255;
	if (value > mergedMax) return mergedMax;
	if (value < 0) return 0;
	return value;
}
var FastColor = class FastColor {
	/**
	* All FastColor objects are valid. So isValid is always true. This property is kept to be compatible with TinyColor.
	*/
	isValid = true;
	/**
	* Red, R in RGB
	*/
	r = 0;
	/**
	* Green, G in RGB
	*/
	g = 0;
	/**
	* Blue, B in RGB
	*/
	b = 0;
	/**
	* Alpha/Opacity, A in RGBA/HSLA
	*/
	a = 1;
	_h;
	_hsl_s;
	_hsv_s;
	_l;
	_v;
	_max;
	_min;
	_brightness;
	constructor(input) {
		/**
		* Always check 3 char in the object to determine the format.
		* We not use function in check to save bundle size.
		* e.g. 'rgb' -> { r: 0, g: 0, b: 0 }.
		*/
		function matchFormat(str) {
			return str[0] in input && str[1] in input && str[2] in input;
		}
		if (!input) {} else if (typeof input === "string") {
			const trimStr = input.trim();
			function matchPrefix(prefix) {
				return trimStr.startsWith(prefix);
			}
			if (/^#?[A-F\d]{3,8}$/i.test(trimStr)) this.fromHexString(trimStr);
			else if (matchPrefix("rgb")) this.fromRgbString(trimStr);
			else if (matchPrefix("hsl")) this.fromHslString(trimStr);
			else if (matchPrefix("hsv") || matchPrefix("hsb")) this.fromHsvString(trimStr);
			else {
				const presetColor = presetColors_default[trimStr.toLowerCase()];
				if (presetColor) this.fromHexString(parseInt(presetColor, 36).toString(16).padStart(6, "0"));
			}
		} else if (input instanceof FastColor) {
			this.r = input.r;
			this.g = input.g;
			this.b = input.b;
			this.a = input.a;
			this._h = input._h;
			this._hsl_s = input._hsl_s;
			this._hsv_s = input._hsv_s;
			this._l = input._l;
			this._v = input._v;
		} else if (matchFormat("rgb")) {
			this.r = limitRange(input.r);
			this.g = limitRange(input.g);
			this.b = limitRange(input.b);
			this.a = typeof input.a === "number" ? limitRange(input.a, 1) : 1;
		} else if (matchFormat("hsl")) this.fromHsl(input);
		else if (matchFormat("hsv")) this.fromHsv(input);
		else throw new Error("@ant-design/fast-color: unsupported input " + JSON.stringify(input));
	}
	setR(value) {
		return this._sc("r", value);
	}
	setG(value) {
		return this._sc("g", value);
	}
	setB(value) {
		return this._sc("b", value);
	}
	setA(value) {
		return this._sc("a", value, 1);
	}
	setHue(value) {
		const hsv = this.toHsv();
		hsv.h = value;
		return this._c(hsv);
	}
	/**
	* Returns the perceived luminance of a color, from 0-1.
	* @see http://www.w3.org/TR/2008/REC-WCAG20-20081211/#relativeluminancedef
	*/
	getLuminance() {
		function adjustGamma(raw) {
			const val = raw / 255;
			return val <= .03928 ? val / 12.92 : Math.pow((val + .055) / 1.055, 2.4);
		}
		const R = adjustGamma(this.r);
		const G = adjustGamma(this.g);
		const B = adjustGamma(this.b);
		return .2126 * R + .7152 * G + .0722 * B;
	}
	getHue() {
		if (typeof this._h === "undefined") {
			const delta = this.getMax() - this.getMin();
			if (delta === 0) this._h = 0;
			else this._h = round(60 * (this.r === this.getMax() ? (this.g - this.b) / delta + (this.g < this.b ? 6 : 0) : this.g === this.getMax() ? (this.b - this.r) / delta + 2 : (this.r - this.g) / delta + 4));
		}
		return this._h;
	}
	/**
	* @deprecated should use getHSVSaturation or getHSLSaturation instead
	*/
	getSaturation() {
		return this.getHSVSaturation();
	}
	getHSVSaturation() {
		if (typeof this._hsv_s === "undefined") {
			const delta = this.getMax() - this.getMin();
			if (delta === 0) this._hsv_s = 0;
			else this._hsv_s = delta / this.getMax();
		}
		return this._hsv_s;
	}
	getHSLSaturation() {
		if (typeof this._hsl_s === "undefined") {
			const delta = this.getMax() - this.getMin();
			if (delta === 0) this._hsl_s = 0;
			else {
				const l = this.getLightness();
				this._hsl_s = delta / 255 / (1 - Math.abs(2 * l - 1));
			}
		}
		return this._hsl_s;
	}
	getLightness() {
		if (typeof this._l === "undefined") this._l = (this.getMax() + this.getMin()) / 510;
		return this._l;
	}
	getValue() {
		if (typeof this._v === "undefined") this._v = this.getMax() / 255;
		return this._v;
	}
	/**
	* Returns the perceived brightness of the color, from 0-255.
	* Note: this is not the b of HSB
	* @see http://www.w3.org/TR/AERT#color-contrast
	*/
	getBrightness() {
		if (typeof this._brightness === "undefined") this._brightness = (this.r * 299 + this.g * 587 + this.b * 114) / 1e3;
		return this._brightness;
	}
	darken(amount = 10) {
		const h = this.getHue();
		const s = this.getSaturation();
		let l = this.getLightness() - amount / 100;
		if (l < 0) l = 0;
		return this._c({
			h,
			s,
			l,
			a: this.a
		});
	}
	lighten(amount = 10) {
		const h = this.getHue();
		const s = this.getSaturation();
		let l = this.getLightness() + amount / 100;
		if (l > 1) l = 1;
		return this._c({
			h,
			s,
			l,
			a: this.a
		});
	}
	/**
	* Mix the current color a given amount with another color, from 0 to 100.
	* 0 means no mixing (return current color).
	*/
	mix(input, amount = 50) {
		const color = this._c(input);
		const p = amount / 100;
		const calc = (key) => (color[key] - this[key]) * p + this[key];
		const rgba = {
			r: round(calc("r")),
			g: round(calc("g")),
			b: round(calc("b")),
			a: round(calc("a") * 100) / 100
		};
		return this._c(rgba);
	}
	/**
	* Mix the color with pure white, from 0 to 100.
	* Providing 0 will do nothing, providing 100 will always return white.
	*/
	tint(amount = 10) {
		return this.mix({
			r: 255,
			g: 255,
			b: 255,
			a: 1
		}, amount);
	}
	/**
	* Mix the color with pure black, from 0 to 100.
	* Providing 0 will do nothing, providing 100 will always return black.
	*/
	shade(amount = 10) {
		return this.mix({
			r: 0,
			g: 0,
			b: 0,
			a: 1
		}, amount);
	}
	onBackground(background) {
		const bg = this._c(background);
		const alpha = this.a + bg.a * (1 - this.a);
		const calc = (key) => {
			return round((this[key] * this.a + bg[key] * bg.a * (1 - this.a)) / alpha);
		};
		return this._c({
			r: calc("r"),
			g: calc("g"),
			b: calc("b"),
			a: alpha
		});
	}
	isDark() {
		return this.getBrightness() < 128;
	}
	isLight() {
		return this.getBrightness() >= 128;
	}
	equals(other) {
		return this.r === other.r && this.g === other.g && this.b === other.b && this.a === other.a;
	}
	clone() {
		return this._c(this);
	}
	toHexString() {
		let hex = "#";
		const rHex = (this.r || 0).toString(16);
		hex += rHex.length === 2 ? rHex : "0" + rHex;
		const gHex = (this.g || 0).toString(16);
		hex += gHex.length === 2 ? gHex : "0" + gHex;
		const bHex = (this.b || 0).toString(16);
		hex += bHex.length === 2 ? bHex : "0" + bHex;
		if (typeof this.a === "number" && this.a >= 0 && this.a < 1) {
			const aHex = round(this.a * 255).toString(16);
			hex += aHex.length === 2 ? aHex : "0" + aHex;
		}
		return hex;
	}
	/** CSS support color pattern */
	toHsl() {
		return {
			h: this.getHue(),
			s: this.getHSLSaturation(),
			l: this.getLightness(),
			a: this.a
		};
	}
	/** CSS support color pattern */
	toHslString() {
		const h = this.getHue();
		const s = round(this.getHSLSaturation() * 100);
		const l = round(this.getLightness() * 100);
		return this.a !== 1 ? `hsla(${h},${s}%,${l}%,${this.a})` : `hsl(${h},${s}%,${l}%)`;
	}
	/** Same as toHsb */
	toHsv() {
		return {
			h: this.getHue(),
			s: this.getHSVSaturation(),
			v: this.getValue(),
			a: this.a
		};
	}
	toRgb() {
		return {
			r: this.r,
			g: this.g,
			b: this.b,
			a: this.a
		};
	}
	toRgbString() {
		return this.a !== 1 ? `rgba(${this.r},${this.g},${this.b},${this.a})` : `rgb(${this.r},${this.g},${this.b})`;
	}
	toString() {
		return this.toRgbString();
	}
	/** Return a new FastColor object with one channel changed */
	_sc(rgb, value, max) {
		const clone = this.clone();
		clone[rgb] = limitRange(value, max);
		return clone;
	}
	_c(input) {
		return new this.constructor(input);
	}
	getMax() {
		if (typeof this._max === "undefined") this._max = Math.max(this.r, this.g, this.b);
		return this._max;
	}
	getMin() {
		if (typeof this._min === "undefined") this._min = Math.min(this.r, this.g, this.b);
		return this._min;
	}
	fromHexString(trimStr) {
		const withoutPrefix = trimStr.replace("#", "");
		function connectNum(index1, index2) {
			return parseInt(withoutPrefix[index1] + withoutPrefix[index2 || index1], 16);
		}
		if (withoutPrefix.length < 6) {
			this.r = connectNum(0);
			this.g = connectNum(1);
			this.b = connectNum(2);
			this.a = withoutPrefix[3] ? connectNum(3) / 255 : 1;
		} else {
			this.r = connectNum(0, 1);
			this.g = connectNum(2, 3);
			this.b = connectNum(4, 5);
			this.a = withoutPrefix[6] ? connectNum(6, 7) / 255 : 1;
		}
	}
	fromHsl({ h: _h, s, l, a }) {
		const h = (_h % 360 + 360) % 360;
		this._h = h;
		this._hsl_s = s;
		this._l = l;
		this.a = typeof a === "number" ? a : 1;
		if (s <= 0) {
			const rgb = round(l * 255);
			this.r = rgb;
			this.g = rgb;
			this.b = rgb;
			return;
		}
		let r = 0, g = 0, b = 0;
		const huePrime = h / 60;
		const chroma = (1 - Math.abs(2 * l - 1)) * s;
		const secondComponent = chroma * (1 - Math.abs(huePrime % 2 - 1));
		if (huePrime >= 0 && huePrime < 1) {
			r = chroma;
			g = secondComponent;
		} else if (huePrime >= 1 && huePrime < 2) {
			r = secondComponent;
			g = chroma;
		} else if (huePrime >= 2 && huePrime < 3) {
			g = chroma;
			b = secondComponent;
		} else if (huePrime >= 3 && huePrime < 4) {
			g = secondComponent;
			b = chroma;
		} else if (huePrime >= 4 && huePrime < 5) {
			r = secondComponent;
			b = chroma;
		} else if (huePrime >= 5 && huePrime < 6) {
			r = chroma;
			b = secondComponent;
		}
		const lightnessModification = l - chroma / 2;
		this.r = round((r + lightnessModification) * 255);
		this.g = round((g + lightnessModification) * 255);
		this.b = round((b + lightnessModification) * 255);
	}
	fromHsv({ h: _h, s, v, a }) {
		const h = (_h % 360 + 360) % 360;
		this._h = h;
		this._hsv_s = s;
		this._v = v;
		this.a = typeof a === "number" ? a : 1;
		const vv = round(v * 255);
		this.r = vv;
		this.g = vv;
		this.b = vv;
		if (s <= 0) return;
		const hh = h / 60;
		const i = Math.floor(hh);
		const ff = hh - i;
		const p = round(v * (1 - s) * 255);
		const q = round(v * (1 - s * ff) * 255);
		const t = round(v * (1 - s * (1 - ff)) * 255);
		switch (i) {
			case 0:
				this.g = t;
				this.b = p;
				break;
			case 1:
				this.r = q;
				this.b = p;
				break;
			case 2:
				this.r = p;
				this.b = t;
				break;
			case 3:
				this.r = p;
				this.g = q;
				break;
			case 4:
				this.r = t;
				this.g = p;
				break;
			default:
				this.g = p;
				this.b = q;
				break;
		}
	}
	fromHsvString(trimStr) {
		const cells = splitColorStr(trimStr, parseHSVorHSL);
		this.fromHsv({
			h: cells[0],
			s: cells[1],
			v: cells[2],
			a: cells[3]
		});
	}
	fromHslString(trimStr) {
		const cells = splitColorStr(trimStr, parseHSVorHSL);
		this.fromHsl({
			h: cells[0],
			s: cells[1],
			l: cells[2],
			a: cells[3]
		});
	}
	fromRgbString(trimStr) {
		const cells = splitColorStr(trimStr, (num, txt) => txt.includes("%") ? round(num / 100 * 255) : num);
		this.r = cells[0];
		this.g = cells[1];
		this.b = cells[2];
		this.a = cells[3];
	}
};
//#endregion
//#region node_modules/.pnpm/@ant-design+colors@8.0.1/node_modules/@ant-design/colors/es/generate.js
var hueStep = 2;
var saturationStep = .16;
var saturationStep2 = .05;
var brightnessStep1 = .05;
var brightnessStep2 = .15;
var lightColorCount = 5;
var darkColorCount = 4;
var darkColorMap = [
	{
		index: 7,
		amount: 15
	},
	{
		index: 6,
		amount: 25
	},
	{
		index: 5,
		amount: 30
	},
	{
		index: 5,
		amount: 45
	},
	{
		index: 5,
		amount: 65
	},
	{
		index: 5,
		amount: 85
	},
	{
		index: 4,
		amount: 90
	},
	{
		index: 3,
		amount: 95
	},
	{
		index: 2,
		amount: 97
	},
	{
		index: 1,
		amount: 98
	}
];
function getHue(hsv, i, light) {
	let hue;
	if (Math.round(hsv.h) >= 60 && Math.round(hsv.h) <= 240) hue = light ? Math.round(hsv.h) - hueStep * i : Math.round(hsv.h) + hueStep * i;
	else hue = light ? Math.round(hsv.h) + hueStep * i : Math.round(hsv.h) - hueStep * i;
	if (hue < 0) hue += 360;
	else if (hue >= 360) hue -= 360;
	return hue;
}
function getSaturation(hsv, i, light) {
	if (hsv.h === 0 && hsv.s === 0) return hsv.s;
	let saturation;
	if (light) saturation = hsv.s - saturationStep * i;
	else if (i === darkColorCount) saturation = hsv.s + saturationStep;
	else saturation = hsv.s + saturationStep2 * i;
	if (saturation > 1) saturation = 1;
	if (light && i === lightColorCount && saturation > .1) saturation = .1;
	if (saturation < .06) saturation = .06;
	return Math.round(saturation * 100) / 100;
}
function getValue(hsv, i, light) {
	let value;
	if (light) value = hsv.v + brightnessStep1 * i;
	else value = hsv.v - brightnessStep2 * i;
	value = Math.max(0, Math.min(1, value));
	return Math.round(value * 100) / 100;
}
function generate$1(color, opts = {}) {
	const patterns = [];
	const pColor = new FastColor(color);
	const hsv = pColor.toHsv();
	for (let i = lightColorCount; i > 0; i -= 1) {
		const c = new FastColor({
			h: getHue(hsv, i, true),
			s: getSaturation(hsv, i, true),
			v: getValue(hsv, i, true)
		});
		patterns.push(c);
	}
	patterns.push(pColor);
	for (let i = 1; i <= darkColorCount; i += 1) {
		const c = new FastColor({
			h: getHue(hsv, i),
			s: getSaturation(hsv, i),
			v: getValue(hsv, i)
		});
		patterns.push(c);
	}
	if (opts.theme === "dark") return darkColorMap.map(({ index, amount }) => new FastColor(opts.backgroundColor || "#141414").mix(patterns[index], amount).toHexString());
	return patterns.map((c) => c.toHexString());
}
//#endregion
//#region node_modules/.pnpm/@ant-design+colors@8.0.1/node_modules/@ant-design/colors/es/presets.js
var red = [
	"#fff1f0",
	"#ffccc7",
	"#ffa39e",
	"#ff7875",
	"#ff4d4f",
	"#f5222d",
	"#cf1322",
	"#a8071a",
	"#820014",
	"#5c0011"
];
red.primary = red[5];
var volcano = [
	"#fff2e8",
	"#ffd8bf",
	"#ffbb96",
	"#ff9c6e",
	"#ff7a45",
	"#fa541c",
	"#d4380d",
	"#ad2102",
	"#871400",
	"#610b00"
];
volcano.primary = volcano[5];
var orange = [
	"#fff7e6",
	"#ffe7ba",
	"#ffd591",
	"#ffc069",
	"#ffa940",
	"#fa8c16",
	"#d46b08",
	"#ad4e00",
	"#873800",
	"#612500"
];
orange.primary = orange[5];
var gold = [
	"#fffbe6",
	"#fff1b8",
	"#ffe58f",
	"#ffd666",
	"#ffc53d",
	"#faad14",
	"#d48806",
	"#ad6800",
	"#874d00",
	"#613400"
];
gold.primary = gold[5];
var yellow = [
	"#feffe6",
	"#ffffb8",
	"#fffb8f",
	"#fff566",
	"#ffec3d",
	"#fadb14",
	"#d4b106",
	"#ad8b00",
	"#876800",
	"#614700"
];
yellow.primary = yellow[5];
var lime = [
	"#fcffe6",
	"#f4ffb8",
	"#eaff8f",
	"#d3f261",
	"#bae637",
	"#a0d911",
	"#7cb305",
	"#5b8c00",
	"#3f6600",
	"#254000"
];
lime.primary = lime[5];
var green = [
	"#f6ffed",
	"#d9f7be",
	"#b7eb8f",
	"#95de64",
	"#73d13d",
	"#52c41a",
	"#389e0d",
	"#237804",
	"#135200",
	"#092b00"
];
green.primary = green[5];
var cyan = [
	"#e6fffb",
	"#b5f5ec",
	"#87e8de",
	"#5cdbd3",
	"#36cfc9",
	"#13c2c2",
	"#08979c",
	"#006d75",
	"#00474f",
	"#002329"
];
cyan.primary = cyan[5];
var blue = [
	"#e6f4ff",
	"#bae0ff",
	"#91caff",
	"#69b1ff",
	"#4096ff",
	"#1677ff",
	"#0958d9",
	"#003eb3",
	"#002c8c",
	"#001d66"
];
blue.primary = blue[5];
var geekblue = [
	"#f0f5ff",
	"#d6e4ff",
	"#adc6ff",
	"#85a5ff",
	"#597ef7",
	"#2f54eb",
	"#1d39c4",
	"#10239e",
	"#061178",
	"#030852"
];
geekblue.primary = geekblue[5];
var purple = [
	"#f9f0ff",
	"#efdbff",
	"#d3adf7",
	"#b37feb",
	"#9254de",
	"#722ed1",
	"#531dab",
	"#391085",
	"#22075e",
	"#120338"
];
purple.primary = purple[5];
var magenta = [
	"#fff0f6",
	"#ffd6e7",
	"#ffadd2",
	"#ff85c0",
	"#f759ab",
	"#eb2f96",
	"#c41d7f",
	"#9e1068",
	"#780650",
	"#520339"
];
magenta.primary = magenta[5];
var grey = [
	"#a6a6a6",
	"#999999",
	"#8c8c8c",
	"#808080",
	"#737373",
	"#666666",
	"#404040",
	"#1a1a1a",
	"#000000",
	"#000000"
];
grey.primary = grey[5];
var redDark = [
	"#2a1215",
	"#431418",
	"#58181c",
	"#791a1f",
	"#a61d24",
	"#d32029",
	"#e84749",
	"#f37370",
	"#f89f9a",
	"#fac8c3"
];
redDark.primary = redDark[5];
var volcanoDark = [
	"#2b1611",
	"#441d12",
	"#592716",
	"#7c3118",
	"#aa3e19",
	"#d84a1b",
	"#e87040",
	"#f3956a",
	"#f8b692",
	"#fad4bc"
];
volcanoDark.primary = volcanoDark[5];
var orangeDark = [
	"#2b1d11",
	"#442a11",
	"#593815",
	"#7c4a15",
	"#aa6215",
	"#d87a16",
	"#e89a3c",
	"#f3b765",
	"#f8cf8d",
	"#fae3b7"
];
orangeDark.primary = orangeDark[5];
var goldDark = [
	"#2b2111",
	"#443111",
	"#594214",
	"#7c5914",
	"#aa7714",
	"#d89614",
	"#e8b339",
	"#f3cc62",
	"#f8df8b",
	"#faedb5"
];
goldDark.primary = goldDark[5];
var yellowDark = [
	"#2b2611",
	"#443b11",
	"#595014",
	"#7c6e14",
	"#aa9514",
	"#d8bd14",
	"#e8d639",
	"#f3ea62",
	"#f8f48b",
	"#fafab5"
];
yellowDark.primary = yellowDark[5];
var limeDark = [
	"#1f2611",
	"#2e3c10",
	"#3e4f13",
	"#536d13",
	"#6f9412",
	"#8bbb11",
	"#a9d134",
	"#c9e75d",
	"#e4f88b",
	"#f0fab5"
];
limeDark.primary = limeDark[5];
var greenDark = [
	"#162312",
	"#1d3712",
	"#274916",
	"#306317",
	"#3c8618",
	"#49aa19",
	"#6abe39",
	"#8fd460",
	"#b2e58b",
	"#d5f2bb"
];
greenDark.primary = greenDark[5];
var cyanDark = [
	"#112123",
	"#113536",
	"#144848",
	"#146262",
	"#138585",
	"#13a8a8",
	"#33bcb7",
	"#58d1c9",
	"#84e2d8",
	"#b2f1e8"
];
cyanDark.primary = cyanDark[5];
var blueDark = [
	"#111a2c",
	"#112545",
	"#15325b",
	"#15417e",
	"#1554ad",
	"#1668dc",
	"#3c89e8",
	"#65a9f3",
	"#8dc5f8",
	"#b7dcfa"
];
blueDark.primary = blueDark[5];
var geekblueDark = [
	"#131629",
	"#161d40",
	"#1c2755",
	"#203175",
	"#263ea0",
	"#2b4acb",
	"#5273e0",
	"#7f9ef3",
	"#a8c1f8",
	"#d2e0fa"
];
geekblueDark.primary = geekblueDark[5];
var purpleDark = [
	"#1a1325",
	"#24163a",
	"#301c4d",
	"#3e2069",
	"#51258f",
	"#642ab5",
	"#854eca",
	"#ab7ae0",
	"#cda8f0",
	"#ebd7fa"
];
purpleDark.primary = purpleDark[5];
var magentaDark = [
	"#291321",
	"#40162f",
	"#551c3b",
	"#75204f",
	"#a02669",
	"#cb2b83",
	"#e0529c",
	"#f37fb7",
	"#f8a8cc",
	"#fad2e3"
];
magentaDark.primary = magentaDark[5];
var greyDark = [
	"#151515",
	"#1f1f1f",
	"#2d2d2d",
	"#393939",
	"#494949",
	"#5a5a5a",
	"#6a6a6a",
	"#7b7b7b",
	"#888888",
	"#969696"
];
greyDark.primary = greyDark[5];
//#endregion
//#region node_modules/.pnpm/@rc-component+util@1.12.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@rc-component/util/es/Dom/canUseDom.js
function canUseDom() {
	return !!(typeof window !== "undefined" && window.document && window.document.createElement);
}
//#endregion
//#region node_modules/.pnpm/@rc-component+util@1.12.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@rc-component/util/es/Dom/contains.js
function contains(root, n) {
	if (!root) return false;
	if (root.contains) return root.contains(n);
	let node = n;
	while (node) {
		if (node === root) return true;
		node = node.parentNode;
	}
	return false;
}
//#endregion
//#region node_modules/.pnpm/@rc-component+util@1.12.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@rc-component/util/es/Dom/dynamicCSS.js
var APPEND_ORDER = "data-rc-order";
var APPEND_PRIORITY = "data-rc-priority";
var MARK_KEY = `rc-util-key`;
var containerCache = /* @__PURE__ */ new Map();
function getMark({ mark } = {}) {
	if (mark) return mark.startsWith("data-") ? mark : `data-${mark}`;
	return MARK_KEY;
}
function getContainer(option) {
	if (option.attachTo) return option.attachTo;
	return document.querySelector("head") || document.body;
}
function getOrder(prepend) {
	if (prepend === "queue") return "prependQueue";
	return prepend ? "prepend" : "append";
}
/**
* Find style which inject by rc-util
*/
function findStyles(container) {
	return Array.from((containerCache.get(container) || container).children).filter((node) => node.tagName === "STYLE");
}
function injectCSS(css, option = {}) {
	if (!canUseDom()) return null;
	const { csp, prepend, priority = 0 } = option;
	const mergedOrder = getOrder(prepend);
	const isPrependQueue = mergedOrder === "prependQueue";
	const styleNode = document.createElement("style");
	styleNode.setAttribute(APPEND_ORDER, mergedOrder);
	if (isPrependQueue && priority) styleNode.setAttribute(APPEND_PRIORITY, `${priority}`);
	if (csp?.nonce) styleNode.nonce = csp?.nonce;
	styleNode.innerHTML = css;
	const container = getContainer(option);
	const { firstChild } = container;
	if (prepend) {
		if (isPrependQueue) {
			const existStyle = (option.styles || findStyles(container)).filter((node) => {
				if (!["prepend", "prependQueue"].includes(node.getAttribute(APPEND_ORDER))) return false;
				return priority >= Number(node.getAttribute(APPEND_PRIORITY) || 0);
			});
			if (existStyle.length) {
				container.insertBefore(styleNode, existStyle[existStyle.length - 1].nextSibling);
				return styleNode;
			}
		}
		container.insertBefore(styleNode, firstChild);
	} else container.appendChild(styleNode);
	return styleNode;
}
function findExistNode(key, option = {}) {
	let { styles } = option;
	styles ||= findStyles(getContainer(option));
	return styles.find((node) => node.getAttribute(getMark(option)) === key);
}
/**
* qiankun will inject `appendChild` to insert into other
*/
function syncRealContainer(container, option) {
	const cachedRealContainer = containerCache.get(container);
	if (!cachedRealContainer || !contains(document, cachedRealContainer)) {
		const placeholderStyle = injectCSS("", option);
		const { parentNode } = placeholderStyle;
		containerCache.set(container, parentNode);
		container.removeChild(placeholderStyle);
	}
}
function updateCSS(css, key, originOption = {}) {
	const container = getContainer(originOption);
	const styles = findStyles(container);
	const option = {
		...originOption,
		styles
	};
	syncRealContainer(container, option);
	const existNode = findExistNode(key, option);
	if (existNode) {
		if (option.csp?.nonce && existNode.nonce !== option.csp?.nonce) existNode.nonce = option.csp?.nonce;
		if (existNode.innerHTML !== css) existNode.innerHTML = css;
		return existNode;
	}
	const newNode = injectCSS(css, option);
	newNode.setAttribute(getMark(option), key);
	return newNode;
}
//#endregion
//#region node_modules/.pnpm/@rc-component+util@1.12.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@rc-component/util/es/Dom/shadow.js
function getRoot(ele) {
	return ele?.getRootNode?.();
}
/**
* Check if is in shadowRoot
*/
function inShadow(ele) {
	return getRoot(ele) instanceof ShadowRoot;
}
/**
* Return shadowRoot if possible
*/
function getShadowRoot(ele) {
	return inShadow(ele) ? getRoot(ele) : null;
}
//#endregion
//#region node_modules/.pnpm/@rc-component+util@1.12.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@rc-component/util/es/warning.js
var warned = {};
var preWarningFns = [];
/**
* Pre warning enable you to parse content before console.error.
* Modify to null will prevent warning.
*/
var preMessage = (fn) => {
	preWarningFns.push(fn);
};
/**
* Warning if condition not match.
* @param valid Condition
* @param message Warning message
* @example
* ```js
* warning(false, 'some error'); // print some error
* warning(true, 'some error'); // print nothing
* warning(1 === 2, 'some error'); // print some error
* ```
*/
function warning$1(valid, message) {}
/** @see Similar to {@link warning} */
function note(valid, message) {}
function resetWarned() {
	warned = {};
}
function call(method, valid, message) {
	if (!valid && !warned[message]) {
		method(false, message);
		warned[message] = true;
	}
}
/** @see Same as {@link warning}, but only warn once for the same message */
function warningOnce(valid, message) {
	call(warning$1, valid, message);
}
/** @see Same as {@link warning}, but only warn once for the same message */
function noteOnce(valid, message) {
	call(note, valid, message);
}
warningOnce.preMessage = preMessage;
warningOnce.resetWarned = resetWarned;
warningOnce.noteOnce = noteOnce;
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/utils.js
function camelCase(input) {
	return input.replace(/-(.)/g, (match, g) => g.toUpperCase());
}
function warning(valid, message) {
	warningOnce(valid, `[@ant-design/icons] ${message}`);
}
function isIconDefinition(target) {
	return typeof target === "object" && typeof target.name === "string" && typeof target.theme === "string" && (typeof target.icon === "object" || typeof target.icon === "function");
}
function normalizeAttrs(attrs = {}) {
	return Object.keys(attrs).reduce((acc, key) => {
		const val = attrs[key];
		switch (key) {
			case "class":
				acc.className = val;
				delete acc.class;
				break;
			default:
				delete acc[key];
				acc[camelCase(key)] = val;
		}
		return acc;
	}, {});
}
function generate(node, key, rootProps) {
	if (!rootProps) return /*#__PURE__*/ React.createElement(node.tag, {
		key,
		...normalizeAttrs(node.attrs)
	}, (node.children || []).map((child, index) => generate(child, `${key}-${node.tag}-${index}`)));
	return /*#__PURE__*/ React.createElement(node.tag, {
		key,
		...normalizeAttrs(node.attrs),
		...rootProps
	}, (node.children || []).map((child, index) => generate(child, `${key}-${node.tag}-${index}`)));
}
function getSecondaryColor(primaryColor) {
	return generate$1(primaryColor)[0];
}
function normalizeTwoToneColors(twoToneColor) {
	if (!twoToneColor) return [];
	return Array.isArray(twoToneColor) ? twoToneColor : [twoToneColor];
}
var iconStyles = `
.anticon {
  display: inline-flex;
  align-items: center;
  color: inherit;
  font-style: normal;
  line-height: 0;
  text-align: center;
  text-transform: none;
  vertical-align: -0.125em;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

.anticon > * {
  line-height: 1;
}

.anticon svg {
  display: inline-block;
  vertical-align: inherit;
}

.anticon::before {
  display: none;
}

.anticon .anticon-icon {
  display: block;
}

.anticon[tabindex] {
  cursor: pointer;
}

.anticon-spin::before,
.anticon-spin {
  display: inline-block;
  -webkit-animation: loadingCircle 1s infinite linear;
  animation: loadingCircle 1s infinite linear;
}

@-webkit-keyframes loadingCircle {
  100% {
    -webkit-transform: rotate(360deg);
    transform: rotate(360deg);
  }
}

@keyframes loadingCircle {
  100% {
    -webkit-transform: rotate(360deg);
    transform: rotate(360deg);
  }
}
`;
var useInsertStyles = (eleRef) => {
	const { csp, prefixCls, layer } = useContext(IconContext);
	let mergedStyleStr = iconStyles;
	if (prefixCls) mergedStyleStr = mergedStyleStr.replace(/anticon/g, prefixCls);
	if (layer) mergedStyleStr = `@layer ${layer} {\n${mergedStyleStr}\n}`;
	useEffect(() => {
		const ele = eleRef.current;
		const shadowRoot = getShadowRoot(ele);
		updateCSS(mergedStyleStr, "@ant-design-icons", {
			prepend: !layer,
			csp,
			attachTo: shadowRoot
		});
	}, []);
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/components/IconBase.js
var twoToneColorPalette = {
	primaryColor: "#333",
	secondaryColor: "#E6E6E6",
	calculated: false
};
function setTwoToneColors({ primaryColor, secondaryColor }) {
	twoToneColorPalette.primaryColor = primaryColor;
	twoToneColorPalette.secondaryColor = secondaryColor || getSecondaryColor(primaryColor);
	twoToneColorPalette.calculated = !!secondaryColor;
}
function getTwoToneColors() {
	return { ...twoToneColorPalette };
}
var IconBase = (props) => {
	const { icon, className, onClick, style, primaryColor, secondaryColor, ...restProps } = props;
	const svgRef = React$1.useRef(null);
	let colors = twoToneColorPalette;
	if (primaryColor) colors = {
		primaryColor,
		secondaryColor: secondaryColor || getSecondaryColor(primaryColor)
	};
	useInsertStyles(svgRef);
	warning(isIconDefinition(icon), `icon should be icon definiton, but got ${icon}`);
	if (!isIconDefinition(icon)) return null;
	let target = icon;
	if (target && typeof target.icon === "function") target = {
		...target,
		icon: target.icon(colors.primaryColor, colors.secondaryColor)
	};
	return generate(target.icon, `svg-${target.name}`, {
		className,
		onClick,
		style,
		"data-icon": target.name,
		width: "1em",
		height: "1em",
		fill: "currentColor",
		"aria-hidden": "true",
		...restProps,
		ref: svgRef
	});
};
IconBase.displayName = "IconReact";
IconBase.getTwoToneColors = getTwoToneColors;
IconBase.setTwoToneColors = setTwoToneColors;
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/components/twoTonePrimaryColor.js
function setTwoToneColor(twoToneColor) {
	const [primaryColor, secondaryColor] = normalizeTwoToneColors(twoToneColor);
	return IconBase.setTwoToneColors({
		primaryColor,
		secondaryColor
	});
}
function getTwoToneColor() {
	const colors = IconBase.getTwoToneColors();
	if (!colors.calculated) return colors.primaryColor;
	return [colors.primaryColor, colors.secondaryColor];
}
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/components/AntdIcon.js
function _extends$40() {
	_extends$40 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$40.apply(this, arguments);
}
setTwoToneColor(blue.primary);
var Icon = /*#__PURE__*/ React$1.forwardRef((props, ref) => {
	const { className, icon, spin, rotate, tabIndex, onClick, twoToneColor, ...restProps } = props;
	const { prefixCls = "anticon", rootClassName } = React$1.useContext(IconContext);
	const classString = clsx(rootClassName, prefixCls, {
		[`${prefixCls}-${icon.name}`]: !!icon.name,
		[`${prefixCls}-spin`]: !!spin || icon.name === "loading"
	}, className);
	let iconTabIndex = tabIndex;
	if (iconTabIndex === void 0 && onClick) iconTabIndex = -1;
	const svgStyle = rotate ? {
		msTransform: `rotate(${rotate}deg)`,
		transform: `rotate(${rotate}deg)`
	} : void 0;
	const [primaryColor, secondaryColor] = normalizeTwoToneColors(twoToneColor);
	return /*#__PURE__*/ React$1.createElement("span", _extends$40({
		role: "img",
		"aria-label": icon.name
	}, restProps, {
		ref,
		tabIndex: iconTabIndex,
		onClick,
		className: classString
	}), /*#__PURE__*/ React$1.createElement(IconBase, {
		icon,
		primaryColor,
		secondaryColor,
		style: svgStyle
	}));
});
Icon.getTwoToneColor = getTwoToneColor;
Icon.setTwoToneColor = setTwoToneColor;
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/ArrowDownOutlined.js
var ArrowDownOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M862 465.3h-81c-4.6 0-9 2-12.1 5.5L550 723.1V160c0-4.4-3.6-8-8-8h-60c-4.4 0-8 3.6-8 8v563.1L255.1 470.8c-3-3.5-7.4-5.5-12.1-5.5h-81c-6.8 0-10.5 8.1-6 13.2L487.9 861a31.96 31.96 0 0048.3 0L868 478.5c4.5-5.2.8-13.2-6-13.2z" }
		}]
	},
	"name": "arrow-down",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/ArrowDownOutlined.js
function _extends$39() {
	_extends$39 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$39.apply(this, arguments);
}
var ArrowDownOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$39({}, props, {
	ref,
	icon: ArrowDownOutlined$1
}));
/**![arrow-down](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTg2MiA0NjUuM2gtODFjLTQuNiAwLTkgMi0xMi4xIDUuNUw1NTAgNzIzLjFWMTYwYzAtNC40LTMuNi04LTgtOGgtNjBjLTQuNCAwLTggMy42LTggOHY1NjMuMUwyNTUuMSA0NzAuOGMtMy0zLjUtNy40LTUuNS0xMi4xLTUuNWgtODFjLTYuOCAwLTEwLjUgOC4xLTYgMTMuMkw0ODcuOSA4NjFhMzEuOTYgMzEuOTYgMCAwMDQ4LjMgMEw4NjggNDc4LjVjNC41LTUuMi44LTEzLjItNi0xMy4yeiIgLz48L3N2Zz4=) */
var RefIcon = /*#__PURE__*/ React$1.forwardRef(ArrowDownOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/ArrowLeftOutlined.js
var ArrowLeftOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M872 474H286.9l350.2-304c5.6-4.9 2.2-14-5.2-14h-88.5c-3.9 0-7.6 1.4-10.5 3.9L155 487.8a31.96 31.96 0 000 48.3L535.1 866c1.5 1.3 3.3 2 5.2 2h91.5c7.4 0 10.8-9.2 5.2-14L286.9 550H872c4.4 0 8-3.6 8-8v-60c0-4.4-3.6-8-8-8z" }
		}]
	},
	"name": "arrow-left",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/ArrowLeftOutlined.js
function _extends$38() {
	_extends$38 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$38.apply(this, arguments);
}
var ArrowLeftOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$38({}, props, {
	ref,
	icon: ArrowLeftOutlined$1
}));
/**![arrow-left](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTg3MiA0NzRIMjg2LjlsMzUwLjItMzA0YzUuNi00LjkgMi4yLTE0LTUuMi0xNGgtODguNWMtMy45IDAtNy42IDEuNC0xMC41IDMuOUwxNTUgNDg3LjhhMzEuOTYgMzEuOTYgMCAwMDAgNDguM0w1MzUuMSA4NjZjMS41IDEuMyAzLjMgMiA1LjIgMmg5MS41YzcuNCAwIDEwLjgtOS4yIDUuMi0xNEwyODYuOSA1NTBIODcyYzQuNCAwIDgtMy42IDgtOHYtNjBjMC00LjQtMy42LTgtOC04eiIgLz48L3N2Zz4=) */
var RefIcon$1 = /*#__PURE__*/ React$1.forwardRef(ArrowLeftOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/ArrowRightOutlined.js
var ArrowRightOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M869 487.8L491.2 159.9c-2.9-2.5-6.6-3.9-10.5-3.9h-88.5c-7.4 0-10.8 9.2-5.2 14l350.2 304H152c-4.4 0-8 3.6-8 8v60c0 4.4 3.6 8 8 8h585.1L386.9 854c-5.6 4.9-2.2 14 5.2 14h91.5c1.9 0 3.8-.7 5.2-2L869 536.2a32.07 32.07 0 000-48.4z" }
		}]
	},
	"name": "arrow-right",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/ArrowRightOutlined.js
function _extends$37() {
	_extends$37 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$37.apply(this, arguments);
}
var ArrowRightOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$37({}, props, {
	ref,
	icon: ArrowRightOutlined$1
}));
/**![arrow-right](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTg2OSA0ODcuOEw0OTEuMiAxNTkuOWMtMi45LTIuNS02LjYtMy45LTEwLjUtMy45aC04OC41Yy03LjQgMC0xMC44IDkuMi01LjIgMTRsMzUwLjIgMzA0SDE1MmMtNC40IDAtOCAzLjYtOCA4djYwYzAgNC40IDMuNiA4IDggOGg1ODUuMUwzODYuOSA4NTRjLTUuNiA0LjktMi4yIDE0IDUuMiAxNGg5MS41YzEuOSAwIDMuOC0uNyA1LjItMkw4NjkgNTM2LjJhMzIuMDcgMzIuMDcgMCAwMDAtNDguNHoiIC8+PC9zdmc+) */
var RefIcon$2 = /*#__PURE__*/ React$1.forwardRef(ArrowRightOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/ArrowUpOutlined.js
var ArrowUpOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M868 545.5L536.1 163a31.96 31.96 0 00-48.3 0L156 545.5a7.97 7.97 0 006 13.2h81c4.6 0 9-2 12.1-5.5L474 300.9V864c0 4.4 3.6 8 8 8h60c4.4 0 8-3.6 8-8V300.9l218.9 252.3c3 3.5 7.4 5.5 12.1 5.5h81c6.8 0 10.5-8 6-13.2z" }
		}]
	},
	"name": "arrow-up",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/ArrowUpOutlined.js
function _extends$36() {
	_extends$36 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$36.apply(this, arguments);
}
var ArrowUpOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$36({}, props, {
	ref,
	icon: ArrowUpOutlined$1
}));
/**![arrow-up](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTg2OCA1NDUuNUw1MzYuMSAxNjNhMzEuOTYgMzEuOTYgMCAwMC00OC4zIDBMMTU2IDU0NS41YTcuOTcgNy45NyAwIDAwNiAxMy4yaDgxYzQuNiAwIDktMiAxMi4xLTUuNUw0NzQgMzAwLjlWODY0YzAgNC40IDMuNiA4IDggOGg2MGM0LjQgMCA4LTMuNiA4LThWMzAwLjlsMjE4LjkgMjUyLjNjMyAzLjUgNy40IDUuNSAxMi4xIDUuNWg4MWM2LjggMCAxMC41LTggNi0xMy4yeiIgLz48L3N2Zz4=) */
var RefIcon$3 = /*#__PURE__*/ React$1.forwardRef(ArrowUpOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/CalendarOutlined.js
var CalendarOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M880 184H712v-64c0-4.4-3.6-8-8-8h-56c-4.4 0-8 3.6-8 8v64H384v-64c0-4.4-3.6-8-8-8h-56c-4.4 0-8 3.6-8 8v64H144c-17.7 0-32 14.3-32 32v664c0 17.7 14.3 32 32 32h736c17.7 0 32-14.3 32-32V216c0-17.7-14.3-32-32-32zm-40 656H184V460h656v380zM184 392V256h128v48c0 4.4 3.6 8 8 8h56c4.4 0 8-3.6 8-8v-48h256v48c0 4.4 3.6 8 8 8h56c4.4 0 8-3.6 8-8v-48h128v136H184z" }
		}]
	},
	"name": "calendar",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/CalendarOutlined.js
function _extends$35() {
	_extends$35 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$35.apply(this, arguments);
}
var CalendarOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$35({}, props, {
	ref,
	icon: CalendarOutlined$1
}));
/**![calendar](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTg4MCAxODRINzEydi02NGMwLTQuNC0zLjYtOC04LThoLTU2Yy00LjQgMC04IDMuNi04IDh2NjRIMzg0di02NGMwLTQuNC0zLjYtOC04LThoLTU2Yy00LjQgMC04IDMuNi04IDh2NjRIMTQ0Yy0xNy43IDAtMzIgMTQuMy0zMiAzMnY2NjRjMCAxNy43IDE0LjMgMzIgMzIgMzJoNzM2YzE3LjcgMCAzMi0xNC4zIDMyLTMyVjIxNmMwLTE3LjctMTQuMy0zMi0zMi0zMnptLTQwIDY1NkgxODRWNDYwaDY1NnYzODB6TTE4NCAzOTJWMjU2aDEyOHY0OGMwIDQuNCAzLjYgOCA4IDhoNTZjNC40IDAgOC0zLjYgOC04di00OGgyNTZ2NDhjMCA0LjQgMy42IDggOCA4aDU2YzQuNCAwIDgtMy42IDgtOHYtNDhoMTI4djEzNkgxODR6IiAvPjwvc3ZnPg==) */
var RefIcon$4 = /*#__PURE__*/ React$1.forwardRef(CalendarOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/CheckCircleOutlined.js
var CheckCircleOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M699 353h-46.9c-10.2 0-19.9 4.9-25.9 13.3L469 584.3l-71.2-98.8c-6-8.3-15.6-13.3-25.9-13.3H325c-6.5 0-10.3 7.4-6.5 12.7l124.6 172.8a31.8 31.8 0 0051.7 0l210.6-292c3.9-5.3.1-12.7-6.4-12.7z" }
		}, {
			"tag": "path",
			"attrs": { "d": "M512 64C264.6 64 64 264.6 64 512s200.6 448 448 448 448-200.6 448-448S759.4 64 512 64zm0 820c-205.4 0-372-166.6-372-372s166.6-372 372-372 372 166.6 372 372-166.6 372-372 372z" }
		}]
	},
	"name": "check-circle",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/CheckCircleOutlined.js
function _extends$34() {
	_extends$34 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$34.apply(this, arguments);
}
var CheckCircleOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$34({}, props, {
	ref,
	icon: CheckCircleOutlined$1
}));
/**![check-circle](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTY5OSAzNTNoLTQ2LjljLTEwLjIgMC0xOS45IDQuOS0yNS45IDEzLjNMNDY5IDU4NC4zbC03MS4yLTk4LjhjLTYtOC4zLTE1LjYtMTMuMy0yNS45LTEzLjNIMzI1Yy02LjUgMC0xMC4zIDcuNC02LjUgMTIuN2wxMjQuNiAxNzIuOGEzMS44IDMxLjggMCAwMDUxLjcgMGwyMTAuNi0yOTJjMy45LTUuMy4xLTEyLjctNi40LTEyLjd6IiAvPjxwYXRoIGQ9Ik01MTIgNjRDMjY0LjYgNjQgNjQgMjY0LjYgNjQgNTEyczIwMC42IDQ0OCA0NDggNDQ4IDQ0OC0yMDAuNiA0NDgtNDQ4Uzc1OS40IDY0IDUxMiA2NHptMCA4MjBjLTIwNS40IDAtMzcyLTE2Ni42LTM3Mi0zNzJzMTY2LjYtMzcyIDM3Mi0zNzIgMzcyIDE2Ni42IDM3MiAzNzItMTY2LjYgMzcyLTM3MiAzNzJ6IiAvPjwvc3ZnPg==) */
var RefIcon$5 = /*#__PURE__*/ React$1.forwardRef(CheckCircleOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/CheckOutlined.js
var CheckOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M912 190h-69.9c-9.8 0-19.1 4.5-25.1 12.2L404.7 724.5 207 474a32 32 0 00-25.1-12.2H112c-6.7 0-10.4 7.7-6.3 12.9l273.9 347c12.8 16.2 37.4 16.2 50.3 0l488.4-618.9c4.1-5.1.4-12.8-6.3-12.8z" }
		}]
	},
	"name": "check",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/CheckOutlined.js
function _extends$33() {
	_extends$33 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$33.apply(this, arguments);
}
var CheckOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$33({}, props, {
	ref,
	icon: CheckOutlined$1
}));
/**![check](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTkxMiAxOTBoLTY5LjljLTkuOCAwLTE5LjEgNC41LTI1LjEgMTIuMkw0MDQuNyA3MjQuNSAyMDcgNDc0YTMyIDMyIDAgMDAtMjUuMS0xMi4ySDExMmMtNi43IDAtMTAuNCA3LjctNi4zIDEyLjlsMjczLjkgMzQ3YzEyLjggMTYuMiAzNy40IDE2LjIgNTAuMyAwbDQ4OC40LTYxOC45YzQuMS01LjEuNC0xMi44LTYuMy0xMi44eiIgLz48L3N2Zz4=) */
var RefIcon$6 = /*#__PURE__*/ React$1.forwardRef(CheckOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/ClockCircleOutlined.js
var ClockCircleOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M512 64C264.6 64 64 264.6 64 512s200.6 448 448 448 448-200.6 448-448S759.4 64 512 64zm0 820c-205.4 0-372-166.6-372-372s166.6-372 372-372 372 166.6 372 372-166.6 372-372 372z" }
		}, {
			"tag": "path",
			"attrs": { "d": "M686.7 638.6L544.1 535.5V288c0-4.4-3.6-8-8-8H488c-4.4 0-8 3.6-8 8v275.4c0 2.6 1.2 5 3.3 6.5l165.4 120.6c3.6 2.6 8.6 1.8 11.2-1.7l28.6-39c2.6-3.7 1.8-8.7-1.8-11.2z" }
		}]
	},
	"name": "clock-circle",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/ClockCircleOutlined.js
function _extends$32() {
	_extends$32 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$32.apply(this, arguments);
}
var ClockCircleOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$32({}, props, {
	ref,
	icon: ClockCircleOutlined$1
}));
/**![clock-circle](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTUxMiA2NEMyNjQuNiA2NCA2NCAyNjQuNiA2NCA1MTJzMjAwLjYgNDQ4IDQ0OCA0NDggNDQ4LTIwMC42IDQ0OC00NDhTNzU5LjQgNjQgNTEyIDY0em0wIDgyMGMtMjA1LjQgMC0zNzItMTY2LjYtMzcyLTM3MnMxNjYuNi0zNzIgMzcyLTM3MiAzNzIgMTY2LjYgMzcyIDM3Mi0xNjYuNiAzNzItMzcyIDM3MnoiIC8+PHBhdGggZD0iTTY4Ni43IDYzOC42TDU0NC4xIDUzNS41VjI4OGMwLTQuNC0zLjYtOC04LThINDg4Yy00LjQgMC04IDMuNi04IDh2Mjc1LjRjMCAyLjYgMS4yIDUgMy4zIDYuNWwxNjUuNCAxMjAuNmMzLjYgMi42IDguNiAxLjggMTEuMi0xLjdsMjguNi0zOWMyLjYtMy43IDEuOC04LjctMS44LTExLjJ6IiAvPjwvc3ZnPg==) */
var RefIcon$7 = /*#__PURE__*/ React$1.forwardRef(ClockCircleOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/CloseCircleOutlined.js
var CloseCircleOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"fill-rule": "evenodd",
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M512 64c247.4 0 448 200.6 448 448S759.4 960 512 960 64 759.4 64 512 264.6 64 512 64zm0 76c-205.4 0-372 166.6-372 372s166.6 372 372 372 372-166.6 372-372-166.6-372-372-372zm128.01 198.83c.03 0 .05.01.09.06l45.02 45.01a.2.2 0 01.05.09.12.12 0 010 .07c0 .02-.01.04-.05.08L557.25 512l127.87 127.86a.27.27 0 01.05.06v.02a.12.12 0 010 .07c0 .03-.01.05-.05.09l-45.02 45.02a.2.2 0 01-.09.05.12.12 0 01-.07 0c-.02 0-.04-.01-.08-.05L512 557.25 384.14 685.12c-.04.04-.06.05-.08.05a.12.12 0 01-.07 0c-.03 0-.05-.01-.09-.05l-45.02-45.02a.2.2 0 01-.05-.09.12.12 0 010-.07c0-.02.01-.04.06-.08L466.75 512 338.88 384.14a.27.27 0 01-.05-.06l-.01-.02a.12.12 0 010-.07c0-.03.01-.05.05-.09l45.02-45.02a.2.2 0 01.09-.05.12.12 0 01.07 0c.02 0 .04.01.08.06L512 466.75l127.86-127.86c.04-.05.06-.06.08-.06a.12.12 0 01.07 0z" }
		}]
	},
	"name": "close-circle",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/CloseCircleOutlined.js
function _extends$31() {
	_extends$31 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$31.apply(this, arguments);
}
var CloseCircleOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$31({}, props, {
	ref,
	icon: CloseCircleOutlined$1
}));
/**![close-circle](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIGZpbGwtcnVsZT0iZXZlbm9kZCIgdmlld0JveD0iNjQgNjQgODk2IDg5NiIgZm9jdXNhYmxlPSJmYWxzZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48cGF0aCBkPSJNNTEyIDY0YzI0Ny40IDAgNDQ4IDIwMC42IDQ0OCA0NDhTNzU5LjQgOTYwIDUxMiA5NjAgNjQgNzU5LjQgNjQgNTEyIDI2NC42IDY0IDUxMiA2NHptMCA3NmMtMjA1LjQgMC0zNzIgMTY2LjYtMzcyIDM3MnMxNjYuNiAzNzIgMzcyIDM3MiAzNzItMTY2LjYgMzcyLTM3Mi0xNjYuNi0zNzItMzcyLTM3MnptMTI4LjAxIDE5OC44M2MuMDMgMCAuMDUuMDEuMDkuMDZsNDUuMDIgNDUuMDFhLjIuMiAwIDAxLjA1LjA5LjEyLjEyIDAgMDEwIC4wN2MwIC4wMi0uMDEuMDQtLjA1LjA4TDU1Ny4yNSA1MTJsMTI3Ljg3IDEyNy44NmEuMjcuMjcgMCAwMS4wNS4wNnYuMDJhLjEyLjEyIDAgMDEwIC4wN2MwIC4wMy0uMDEuMDUtLjA1LjA5bC00NS4wMiA0NS4wMmEuMi4yIDAgMDEtLjA5LjA1LjEyLjEyIDAgMDEtLjA3IDBjLS4wMiAwLS4wNC0uMDEtLjA4LS4wNUw1MTIgNTU3LjI1IDM4NC4xNCA2ODUuMTJjLS4wNC4wNC0uMDYuMDUtLjA4LjA1YS4xMi4xMiAwIDAxLS4wNyAwYy0uMDMgMC0uMDUtLjAxLS4wOS0uMDVsLTQ1LjAyLTQ1LjAyYS4yLjIgMCAwMS0uMDUtLjA5LjEyLjEyIDAgMDEwLS4wN2MwLS4wMi4wMS0uMDQuMDYtLjA4TDQ2Ni43NSA1MTIgMzM4Ljg4IDM4NC4xNGEuMjcuMjcgMCAwMS0uMDUtLjA2bC0uMDEtLjAyYS4xMi4xMiAwIDAxMC0uMDdjMC0uMDMuMDEtLjA1LjA1LS4wOWw0NS4wMi00NS4wMmEuMi4yIDAgMDEuMDktLjA1LjEyLjEyIDAgMDEuMDcgMGMuMDIgMCAuMDQuMDEuMDguMDZMNTEyIDQ2Ni43NWwxMjcuODYtMTI3Ljg2Yy4wNC0uMDUuMDYtLjA2LjA4LS4wNmEuMTIuMTIgMCAwMS4wNyAweiIgLz48L3N2Zz4=) */
var RefIcon$8 = /*#__PURE__*/ React$1.forwardRef(CloseCircleOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/CloseOutlined.js
var CloseOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"fill-rule": "evenodd",
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M799.86 166.31c.02 0 .04.02.08.06l57.69 57.7c.04.03.05.05.06.08a.12.12 0 010 .06c0 .03-.02.05-.06.09L569.93 512l287.7 287.7c.04.04.05.06.06.09a.12.12 0 010 .07c0 .02-.02.04-.06.08l-57.7 57.69c-.03.04-.05.05-.07.06a.12.12 0 01-.07 0c-.03 0-.05-.02-.09-.06L512 569.93l-287.7 287.7c-.04.04-.06.05-.09.06a.12.12 0 01-.07 0c-.02 0-.04-.02-.08-.06l-57.69-57.7c-.04-.03-.05-.05-.06-.07a.12.12 0 010-.07c0-.03.02-.05.06-.09L454.07 512l-287.7-287.7c-.04-.04-.05-.06-.06-.09a.12.12 0 010-.07c0-.02.02-.04.06-.08l57.7-57.69c.03-.04.05-.05.07-.06a.12.12 0 01.07 0c.03 0 .05.02.09.06L512 454.07l287.7-287.7c.04-.04.06-.05.09-.06a.12.12 0 01.07 0z" }
		}]
	},
	"name": "close",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/CloseOutlined.js
function _extends$30() {
	_extends$30 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$30.apply(this, arguments);
}
var CloseOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$30({}, props, {
	ref,
	icon: CloseOutlined$1
}));
/**![close](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIGZpbGwtcnVsZT0iZXZlbm9kZCIgdmlld0JveD0iNjQgNjQgODk2IDg5NiIgZm9jdXNhYmxlPSJmYWxzZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48cGF0aCBkPSJNNzk5Ljg2IDE2Ni4zMWMuMDIgMCAuMDQuMDIuMDguMDZsNTcuNjkgNTcuN2MuMDQuMDMuMDUuMDUuMDYuMDhhLjEyLjEyIDAgMDEwIC4wNmMwIC4wMy0uMDIuMDUtLjA2LjA5TDU2OS45MyA1MTJsMjg3LjcgMjg3LjdjLjA0LjA0LjA1LjA2LjA2LjA5YS4xMi4xMiAwIDAxMCAuMDdjMCAuMDItLjAyLjA0LS4wNi4wOGwtNTcuNyA1Ny42OWMtLjAzLjA0LS4wNS4wNS0uMDcuMDZhLjEyLjEyIDAgMDEtLjA3IDBjLS4wMyAwLS4wNS0uMDItLjA5LS4wNkw1MTIgNTY5LjkzbC0yODcuNyAyODcuN2MtLjA0LjA0LS4wNi4wNS0uMDkuMDZhLjEyLjEyIDAgMDEtLjA3IDBjLS4wMiAwLS4wNC0uMDItLjA4LS4wNmwtNTcuNjktNTcuN2MtLjA0LS4wMy0uMDUtLjA1LS4wNi0uMDdhLjEyLjEyIDAgMDEwLS4wN2MwLS4wMy4wMi0uMDUuMDYtLjA5TDQ1NC4wNyA1MTJsLTI4Ny43LTI4Ny43Yy0uMDQtLjA0LS4wNS0uMDYtLjA2LS4wOWEuMTIuMTIgMCAwMTAtLjA3YzAtLjAyLjAyLS4wNC4wNi0uMDhsNTcuNy01Ny42OWMuMDMtLjA0LjA1LS4wNS4wNy0uMDZhLjEyLjEyIDAgMDEuMDcgMGMuMDMgMCAuMDUuMDIuMDkuMDZMNTEyIDQ1NC4wN2wyODcuNy0yODcuN2MuMDQtLjA0LjA2LS4wNS4wOS0uMDZhLjEyLjEyIDAgMDEuMDcgMHoiIC8+PC9zdmc+) */
var RefIcon$9 = /*#__PURE__*/ React$1.forwardRef(CloseOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/CopyOutlined.js
var CopyOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M832 64H296c-4.4 0-8 3.6-8 8v56c0 4.4 3.6 8 8 8h496v688c0 4.4 3.6 8 8 8h56c4.4 0 8-3.6 8-8V96c0-17.7-14.3-32-32-32zM704 192H192c-17.7 0-32 14.3-32 32v530.7c0 8.5 3.4 16.6 9.4 22.6l173.3 173.3c2.2 2.2 4.7 4 7.4 5.5v1.9h4.2c3.5 1.3 7.2 2 11 2H704c17.7 0 32-14.3 32-32V224c0-17.7-14.3-32-32-32zM350 856.2L263.9 770H350v86.2zM664 888H414V746c0-22.1-17.9-40-40-40H232V264h432v624z" }
		}]
	},
	"name": "copy",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/CopyOutlined.js
function _extends$29() {
	_extends$29 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$29.apply(this, arguments);
}
var CopyOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$29({}, props, {
	ref,
	icon: CopyOutlined$1
}));
/**![copy](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTgzMiA2NEgyOTZjLTQuNCAwLTggMy42LTggOHY1NmMwIDQuNCAzLjYgOCA4IDhoNDk2djY4OGMwIDQuNCAzLjYgOCA4IDhoNTZjNC40IDAgOC0zLjYgOC04Vjk2YzAtMTcuNy0xNC4zLTMyLTMyLTMyek03MDQgMTkySDE5MmMtMTcuNyAwLTMyIDE0LjMtMzIgMzJ2NTMwLjdjMCA4LjUgMy40IDE2LjYgOS40IDIyLjZsMTczLjMgMTczLjNjMi4yIDIuMiA0LjcgNCA3LjQgNS41djEuOWg0LjJjMy41IDEuMyA3LjIgMiAxMSAySDcwNGMxNy43IDAgMzItMTQuMyAzMi0zMlYyMjRjMC0xNy43LTE0LjMtMzItMzItMzJ6TTM1MCA4NTYuMkwyNjMuOSA3NzBIMzUwdjg2LjJ6TTY2NCA4ODhINDE0Vjc0NmMwLTIyLjEtMTcuOS00MC00MC00MEgyMzJWMjY0aDQzMnY2MjR6IiAvPjwvc3ZnPg==) */
var RefIcon$10 = /*#__PURE__*/ React$1.forwardRef(CopyOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/DeleteOutlined.js
var DeleteOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M360 184h-8c4.4 0 8-3.6 8-8v8h304v-8c0 4.4 3.6 8 8 8h-8v72h72v-80c0-35.3-28.7-64-64-64H352c-35.3 0-64 28.7-64 64v80h72v-72zm504 72H160c-17.7 0-32 14.3-32 32v32c0 4.4 3.6 8 8 8h60.4l24.7 523c1.6 34.1 29.8 61 63.9 61h454c34.2 0 62.3-26.8 63.9-61l24.7-523H888c4.4 0 8-3.6 8-8v-32c0-17.7-14.3-32-32-32zM731.3 840H292.7l-24.2-512h487l-24.2 512z" }
		}]
	},
	"name": "delete",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/DeleteOutlined.js
function _extends$28() {
	_extends$28 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$28.apply(this, arguments);
}
var DeleteOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$28({}, props, {
	ref,
	icon: DeleteOutlined$1
}));
/**![delete](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTM2MCAxODRoLThjNC40IDAgOC0zLjYgOC04djhoMzA0di04YzAgNC40IDMuNiA4IDggOGgtOHY3Mmg3MnYtODBjMC0zNS4zLTI4LjctNjQtNjQtNjRIMzUyYy0zNS4zIDAtNjQgMjguNy02NCA2NHY4MGg3MnYtNzJ6bTUwNCA3MkgxNjBjLTE3LjcgMC0zMiAxNC4zLTMyIDMydjMyYzAgNC40IDMuNiA4IDggOGg2MC40bDI0LjcgNTIzYzEuNiAzNC4xIDI5LjggNjEgNjMuOSA2MWg0NTRjMzQuMiAwIDYyLjMtMjYuOCA2My45LTYxbDI0LjctNTIzSDg4OGM0LjQgMCA4LTMuNiA4LTh2LTMyYzAtMTcuNy0xNC4zLTMyLTMyLTMyek03MzEuMyA4NDBIMjkyLjdsLTI0LjItNTEyaDQ4N2wtMjQuMiA1MTJ6IiAvPjwvc3ZnPg==) */
var RefIcon$11 = /*#__PURE__*/ React$1.forwardRef(DeleteOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/DownloadOutlined.js
var DownloadOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M505.7 661a8 8 0 0012.6 0l112-141.7c4.1-5.2.4-12.9-6.3-12.9h-74.1V168c0-4.4-3.6-8-8-8h-60c-4.4 0-8 3.6-8 8v338.3H400c-6.7 0-10.4 7.7-6.3 12.9l112 141.8zM878 626h-60c-4.4 0-8 3.6-8 8v154H214V634c0-4.4-3.6-8-8-8h-60c-4.4 0-8 3.6-8 8v198c0 17.7 14.3 32 32 32h684c17.7 0 32-14.3 32-32V634c0-4.4-3.6-8-8-8z" }
		}]
	},
	"name": "download",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/DownloadOutlined.js
function _extends$27() {
	_extends$27 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$27.apply(this, arguments);
}
var DownloadOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$27({}, props, {
	ref,
	icon: DownloadOutlined$1
}));
/**![download](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTUwNS43IDY2MWE4IDggMCAwMDEyLjYgMGwxMTItMTQxLjdjNC4xLTUuMi40LTEyLjktNi4zLTEyLjloLTc0LjFWMTY4YzAtNC40LTMuNi04LTgtOGgtNjBjLTQuNCAwLTggMy42LTggOHYzMzguM0g0MDBjLTYuNyAwLTEwLjQgNy43LTYuMyAxMi45bDExMiAxNDEuOHpNODc4IDYyNmgtNjBjLTQuNCAwLTggMy42LTggOHYxNTRIMjE0VjYzNGMwLTQuNC0zLjYtOC04LThoLTYwYy00LjQgMC04IDMuNi04IDh2MTk4YzAgMTcuNyAxNC4zIDMyIDMyIDMyaDY4NGMxNy43IDAgMzItMTQuMyAzMi0zMlY2MzRjMC00LjQtMy42LTgtOC04eiIgLz48L3N2Zz4=) */
var RefIcon$12 = /*#__PURE__*/ React$1.forwardRef(DownloadOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/EditOutlined.js
var EditOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M257.7 752c2 0 4-.2 6-.5L431.9 722c2-.4 3.9-1.3 5.3-2.8l423.9-423.9a9.96 9.96 0 000-14.1L694.9 114.9c-1.9-1.9-4.4-2.9-7.1-2.9s-5.2 1-7.1 2.9L256.8 538.8c-1.5 1.5-2.4 3.3-2.8 5.3l-29.5 168.2a33.5 33.5 0 009.4 29.8c6.6 6.4 14.9 9.9 23.8 9.9zm67.4-174.4L687.8 215l73.3 73.3-362.7 362.6-88.9 15.7 15.6-89zM880 836H144c-17.7 0-32 14.3-32 32v36c0 4.4 3.6 8 8 8h784c4.4 0 8-3.6 8-8v-36c0-17.7-14.3-32-32-32z" }
		}]
	},
	"name": "edit",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/EditOutlined.js
function _extends$26() {
	_extends$26 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$26.apply(this, arguments);
}
var EditOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$26({}, props, {
	ref,
	icon: EditOutlined$1
}));
/**![edit](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTI1Ny43IDc1MmMyIDAgNC0uMiA2LS41TDQzMS45IDcyMmMyLS40IDMuOS0xLjMgNS4zLTIuOGw0MjMuOS00MjMuOWE5Ljk2IDkuOTYgMCAwMDAtMTQuMUw2OTQuOSAxMTQuOWMtMS45LTEuOS00LjQtMi45LTcuMS0yLjlzLTUuMiAxLTcuMSAyLjlMMjU2LjggNTM4LjhjLTEuNSAxLjUtMi40IDMuMy0yLjggNS4zbC0yOS41IDE2OC4yYTMzLjUgMzMuNSAwIDAwOS40IDI5LjhjNi42IDYuNCAxNC45IDkuOSAyMy44IDkuOXptNjcuNC0xNzQuNEw2ODcuOCAyMTVsNzMuMyA3My4zLTM2Mi43IDM2Mi42LTg4LjkgMTUuNyAxNS42LTg5ek04ODAgODM2SDE0NGMtMTcuNyAwLTMyIDE0LjMtMzIgMzJ2MzZjMCA0LjQgMy42IDggOCA4aDc4NGM0LjQgMCA4LTMuNiA4LTh2LTM2YzAtMTcuNy0xNC4zLTMyLTMyLTMyeiIgLz48L3N2Zz4=) */
var RefIcon$13 = /*#__PURE__*/ React$1.forwardRef(EditOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/ExclamationCircleOutlined.js
var ExclamationCircleOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M512 64C264.6 64 64 264.6 64 512s200.6 448 448 448 448-200.6 448-448S759.4 64 512 64zm0 820c-205.4 0-372-166.6-372-372s166.6-372 372-372 372 166.6 372 372-166.6 372-372 372z" }
		}, {
			"tag": "path",
			"attrs": { "d": "M464 688a48 48 0 1096 0 48 48 0 10-96 0zm24-112h48c4.4 0 8-3.6 8-8V296c0-4.4-3.6-8-8-8h-48c-4.4 0-8 3.6-8 8v272c0 4.4 3.6 8 8 8z" }
		}]
	},
	"name": "exclamation-circle",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/ExclamationCircleOutlined.js
function _extends$25() {
	_extends$25 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$25.apply(this, arguments);
}
var ExclamationCircleOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$25({}, props, {
	ref,
	icon: ExclamationCircleOutlined$1
}));
/**![exclamation-circle](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTUxMiA2NEMyNjQuNiA2NCA2NCAyNjQuNiA2NCA1MTJzMjAwLjYgNDQ4IDQ0OCA0NDggNDQ4LTIwMC42IDQ0OC00NDhTNzU5LjQgNjQgNTEyIDY0em0wIDgyMGMtMjA1LjQgMC0zNzItMTY2LjYtMzcyLTM3MnMxNjYuNi0zNzIgMzcyLTM3MiAzNzIgMTY2LjYgMzcyIDM3Mi0xNjYuNiAzNzItMzcyIDM3MnoiIC8+PHBhdGggZD0iTTQ2NCA2ODhhNDggNDggMCAxMDk2IDAgNDggNDggMCAxMC05NiAwem0yNC0xMTJoNDhjNC40IDAgOC0zLjYgOC04VjI5NmMwLTQuNC0zLjYtOC04LThoLTQ4Yy00LjQgMC04IDMuNi04IDh2MjcyYzAgNC40IDMuNiA4IDggOHoiIC8+PC9zdmc+) */
var RefIcon$14 = /*#__PURE__*/ React$1.forwardRef(ExclamationCircleOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/EyeInvisibleOutlined.js
var EyeInvisibleOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M942.2 486.2Q889.47 375.11 816.7 305l-50.88 50.88C807.31 395.53 843.45 447.4 874.7 512 791.5 684.2 673.4 766 512 766q-72.67 0-133.87-22.38L323 798.75Q408 838 512 838q288.3 0 430.2-300.3a60.29 60.29 0 000-51.5zm-63.57-320.64L836 122.88a8 8 0 00-11.32 0L715.31 232.2Q624.86 186 512 186q-288.3 0-430.2 300.3a60.3 60.3 0 000 51.5q56.69 119.4 136.5 191.41L112.48 835a8 8 0 000 11.31L155.17 889a8 8 0 0011.31 0l712.15-712.12a8 8 0 000-11.32zM149.3 512C232.6 339.8 350.7 258 512 258c54.54 0 104.13 9.36 149.12 28.39l-70.3 70.3a176 176 0 00-238.13 238.13l-83.42 83.42C223.1 637.49 183.3 582.28 149.3 512zm246.7 0a112.11 112.11 0 01146.2-106.69L401.31 546.2A112 112 0 01396 512z" }
		}, {
			"tag": "path",
			"attrs": { "d": "M508 624c-3.46 0-6.87-.16-10.25-.47l-52.82 52.82a176.09 176.09 0 00227.42-227.42l-52.82 52.82c.31 3.38.47 6.79.47 10.25a111.94 111.94 0 01-112 112z" }
		}]
	},
	"name": "eye-invisible",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/EyeInvisibleOutlined.js
function _extends$24() {
	_extends$24 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$24.apply(this, arguments);
}
var EyeInvisibleOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$24({}, props, {
	ref,
	icon: EyeInvisibleOutlined$1
}));
/**![eye-invisible](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTk0Mi4yIDQ4Ni4yUTg4OS40NyAzNzUuMTEgODE2LjcgMzA1bC01MC44OCA1MC44OEM4MDcuMzEgMzk1LjUzIDg0My40NSA0NDcuNCA4NzQuNyA1MTIgNzkxLjUgNjg0LjIgNjczLjQgNzY2IDUxMiA3NjZxLTcyLjY3IDAtMTMzLjg3LTIyLjM4TDMyMyA3OTguNzVRNDA4IDgzOCA1MTIgODM4cTI4OC4zIDAgNDMwLjItMzAwLjNhNjAuMjkgNjAuMjkgMCAwMDAtNTEuNXptLTYzLjU3LTMyMC42NEw4MzYgMTIyLjg4YTggOCAwIDAwLTExLjMyIDBMNzE1LjMxIDIzMi4yUTYyNC44NiAxODYgNTEyIDE4NnEtMjg4LjMgMC00MzAuMiAzMDAuM2E2MC4zIDYwLjMgMCAwMDAgNTEuNXE1Ni42OSAxMTkuNCAxMzYuNSAxOTEuNDFMMTEyLjQ4IDgzNWE4IDggMCAwMDAgMTEuMzFMMTU1LjE3IDg4OWE4IDggMCAwMDExLjMxIDBsNzEyLjE1LTcxMi4xMmE4IDggMCAwMDAtMTEuMzJ6TTE0OS4zIDUxMkMyMzIuNiAzMzkuOCAzNTAuNyAyNTggNTEyIDI1OGM1NC41NCAwIDEwNC4xMyA5LjM2IDE0OS4xMiAyOC4zOWwtNzAuMyA3MC4zYTE3NiAxNzYgMCAwMC0yMzguMTMgMjM4LjEzbC04My40MiA4My40MkMyMjMuMSA2MzcuNDkgMTgzLjMgNTgyLjI4IDE0OS4zIDUxMnptMjQ2LjcgMGExMTIuMTEgMTEyLjExIDAgMDExNDYuMi0xMDYuNjlMNDAxLjMxIDU0Ni4yQTExMiAxMTIgMCAwMTM5NiA1MTJ6IiAvPjxwYXRoIGQ9Ik01MDggNjI0Yy0zLjQ2IDAtNi44Ny0uMTYtMTAuMjUtLjQ3bC01Mi44MiA1Mi44MmExNzYuMDkgMTc2LjA5IDAgMDAyMjcuNDItMjI3LjQybC01Mi44MiA1Mi44MmMuMzEgMy4zOC40NyA2Ljc5LjQ3IDEwLjI1YTExMS45NCAxMTEuOTQgMCAwMS0xMTIgMTEyeiIgLz48L3N2Zz4=) */
var RefIcon$15 = /*#__PURE__*/ React$1.forwardRef(EyeInvisibleOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/EyeOutlined.js
var EyeOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M942.2 486.2C847.4 286.5 704.1 186 512 186c-192.2 0-335.4 100.5-430.2 300.3a60.3 60.3 0 000 51.5C176.6 737.5 319.9 838 512 838c192.2 0 335.4-100.5 430.2-300.3 7.7-16.2 7.7-35 0-51.5zM512 766c-161.3 0-279.4-81.8-362.7-254C232.6 339.8 350.7 258 512 258c161.3 0 279.4 81.8 362.7 254C791.5 684.2 673.4 766 512 766zm-4-430c-97.2 0-176 78.8-176 176s78.8 176 176 176 176-78.8 176-176-78.8-176-176-176zm0 288c-61.9 0-112-50.1-112-112s50.1-112 112-112 112 50.1 112 112-50.1 112-112 112z" }
		}]
	},
	"name": "eye",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/EyeOutlined.js
function _extends$23() {
	_extends$23 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$23.apply(this, arguments);
}
var EyeOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$23({}, props, {
	ref,
	icon: EyeOutlined$1
}));
/**![eye](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTk0Mi4yIDQ4Ni4yQzg0Ny40IDI4Ni41IDcwNC4xIDE4NiA1MTIgMTg2Yy0xOTIuMiAwLTMzNS40IDEwMC41LTQzMC4yIDMwMC4zYTYwLjMgNjAuMyAwIDAwMCA1MS41QzE3Ni42IDczNy41IDMxOS45IDgzOCA1MTIgODM4YzE5Mi4yIDAgMzM1LjQtMTAwLjUgNDMwLjItMzAwLjMgNy43LTE2LjIgNy43LTM1IDAtNTEuNXpNNTEyIDc2NmMtMTYxLjMgMC0yNzkuNC04MS44LTM2Mi43LTI1NEMyMzIuNiAzMzkuOCAzNTAuNyAyNTggNTEyIDI1OGMxNjEuMyAwIDI3OS40IDgxLjggMzYyLjcgMjU0Qzc5MS41IDY4NC4yIDY3My40IDc2NiA1MTIgNzY2em0tNC00MzBjLTk3LjIgMC0xNzYgNzguOC0xNzYgMTc2czc4LjggMTc2IDE3NiAxNzYgMTc2LTc4LjggMTc2LTE3Ni03OC44LTE3Ni0xNzYtMTc2em0wIDI4OGMtNjEuOSAwLTExMi01MC4xLTExMi0xMTJzNTAuMS0xMTIgMTEyLTExMiAxMTIgNTAuMSAxMTIgMTEyLTUwLjEgMTEyLTExMiAxMTJ6IiAvPjwvc3ZnPg==) */
var RefIcon$16 = /*#__PURE__*/ React$1.forwardRef(EyeOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/FileOutlined.js
var FileOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M854.6 288.6L639.4 73.4c-6-6-14.1-9.4-22.6-9.4H192c-17.7 0-32 14.3-32 32v832c0 17.7 14.3 32 32 32h640c17.7 0 32-14.3 32-32V311.3c0-8.5-3.4-16.7-9.4-22.7zM790.2 326H602V137.8L790.2 326zm1.8 562H232V136h302v216a42 42 0 0042 42h216v494z" }
		}]
	},
	"name": "file",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/FileOutlined.js
function _extends$22() {
	_extends$22 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$22.apply(this, arguments);
}
var FileOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$22({}, props, {
	ref,
	icon: FileOutlined$1
}));
/**![file](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTg1NC42IDI4OC42TDYzOS40IDczLjRjLTYtNi0xNC4xLTkuNC0yMi42LTkuNEgxOTJjLTE3LjcgMC0zMiAxNC4zLTMyIDMydjgzMmMwIDE3LjcgMTQuMyAzMiAzMiAzMmg2NDBjMTcuNyAwIDMyLTE0LjMgMzItMzJWMzExLjNjMC04LjUtMy40LTE2LjctOS40LTIyLjd6TTc5MC4yIDMyNkg2MDJWMTM3LjhMNzkwLjIgMzI2em0xLjggNTYySDIzMlYxMzZoMzAydjIxNmE0MiA0MiAwIDAwNDIgNDJoMjE2djQ5NHoiIC8+PC9zdmc+) */
var RefIcon$17 = /*#__PURE__*/ React$1.forwardRef(FileOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/FolderOpenOutlined.js
var FolderOpenOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M928 444H820V330.4c0-17.7-14.3-32-32-32H473L355.7 186.2a8.15 8.15 0 00-5.5-2.2H96c-17.7 0-32 14.3-32 32v592c0 17.7 14.3 32 32 32h698c13 0 24.8-7.9 29.7-20l134-332c1.5-3.8 2.3-7.9 2.3-12 0-17.7-14.3-32-32-32zM136 256h188.5l119.6 114.4H748V444H238c-13 0-24.8 7.9-29.7 20L136 643.2V256zm635.3 512H159l103.3-256h612.4L771.3 768z" }
		}]
	},
	"name": "folder-open",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/FolderOpenOutlined.js
function _extends$21() {
	_extends$21 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$21.apply(this, arguments);
}
var FolderOpenOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$21({}, props, {
	ref,
	icon: FolderOpenOutlined$1
}));
/**![folder-open](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTkyOCA0NDRIODIwVjMzMC40YzAtMTcuNy0xNC4zLTMyLTMyLTMySDQ3M0wzNTUuNyAxODYuMmE4LjE1IDguMTUgMCAwMC01LjUtMi4ySDk2Yy0xNy43IDAtMzIgMTQuMy0zMiAzMnY1OTJjMCAxNy43IDE0LjMgMzIgMzIgMzJoNjk4YzEzIDAgMjQuOC03LjkgMjkuNy0yMGwxMzQtMzMyYzEuNS0zLjggMi4zLTcuOSAyLjMtMTIgMC0xNy43LTE0LjMtMzItMzItMzJ6TTEzNiAyNTZoMTg4LjVsMTE5LjYgMTE0LjRINzQ4VjQ0NEgyMzhjLTEzIDAtMjQuOCA3LjktMjkuNyAyMEwxMzYgNjQzLjJWMjU2em02MzUuMyA1MTJIMTU5bDEwMy4zLTI1Nmg2MTIuNEw3NzEuMyA3Njh6IiAvPjwvc3ZnPg==) */
var RefIcon$18 = /*#__PURE__*/ React$1.forwardRef(FolderOpenOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/FolderOutlined.js
var FolderOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M880 298.4H521L403.7 186.2a8.15 8.15 0 00-5.5-2.2H144c-17.7 0-32 14.3-32 32v592c0 17.7 14.3 32 32 32h736c17.7 0 32-14.3 32-32V330.4c0-17.7-14.3-32-32-32zM840 768H184V256h188.5l119.6 114.4H840V768z" }
		}]
	},
	"name": "folder",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/FolderOutlined.js
function _extends$20() {
	_extends$20 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$20.apply(this, arguments);
}
var FolderOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$20({}, props, {
	ref,
	icon: FolderOutlined$1
}));
/**![folder](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTg4MCAyOTguNEg1MjFMNDAzLjcgMTg2LjJhOC4xNSA4LjE1IDAgMDAtNS41LTIuMkgxNDRjLTE3LjcgMC0zMiAxNC4zLTMyIDMydjU5MmMwIDE3LjcgMTQuMyAzMiAzMiAzMmg3MzZjMTcuNyAwIDMyLTE0LjMgMzItMzJWMzMwLjRjMC0xNy43LTE0LjMtMzItMzItMzJ6TTg0MCA3NjhIMTg0VjI1NmgxODguNWwxMTkuNiAxMTQuNEg4NDBWNzY4eiIgLz48L3N2Zz4=) */
var RefIcon$19 = /*#__PURE__*/ React$1.forwardRef(FolderOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/HomeOutlined.js
var HomeOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M946.5 505L560.1 118.8l-25.9-25.9a31.5 31.5 0 00-44.4 0L77.5 505a63.9 63.9 0 00-18.8 46c.4 35.2 29.7 63.3 64.9 63.3h42.5V940h691.8V614.3h43.4c17.1 0 33.2-6.7 45.3-18.8a63.6 63.6 0 0018.7-45.3c0-17-6.7-33.1-18.8-45.2zM568 868H456V664h112v204zm217.9-325.7V868H632V640c0-22.1-17.9-40-40-40H432c-22.1 0-40 17.9-40 40v228H238.1V542.3h-96l370-369.7 23.1 23.1L882 542.3h-96.1z" }
		}]
	},
	"name": "home",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/HomeOutlined.js
function _extends$19() {
	_extends$19 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$19.apply(this, arguments);
}
var HomeOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$19({}, props, {
	ref,
	icon: HomeOutlined$1
}));
/**![home](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTk0Ni41IDUwNUw1NjAuMSAxMTguOGwtMjUuOS0yNS45YTMxLjUgMzEuNSAwIDAwLTQ0LjQgMEw3Ny41IDUwNWE2My45IDYzLjkgMCAwMC0xOC44IDQ2Yy40IDM1LjIgMjkuNyA2My4zIDY0LjkgNjMuM2g0Mi41Vjk0MGg2OTEuOFY2MTQuM2g0My40YzE3LjEgMCAzMy4yLTYuNyA0NS4zLTE4LjhhNjMuNiA2My42IDAgMDAxOC43LTQ1LjNjMC0xNy02LjctMzMuMS0xOC44LTQ1LjJ6TTU2OCA4NjhINDU2VjY2NGgxMTJ2MjA0em0yMTcuOS0zMjUuN1Y4NjhINjMyVjY0MGMwLTIyLjEtMTcuOS00MC00MC00MEg0MzJjLTIyLjEgMC00MCAxNy45LTQwIDQwdjIyOEgyMzguMVY1NDIuM2gtOTZsMzcwLTM2OS43IDIzLjEgMjMuMUw4ODIgNTQyLjNoLTk2LjF6IiAvPjwvc3ZnPg==) */
var RefIcon$20 = /*#__PURE__*/ React$1.forwardRef(HomeOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/InfoCircleOutlined.js
var InfoCircleOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M512 64C264.6 64 64 264.6 64 512s200.6 448 448 448 448-200.6 448-448S759.4 64 512 64zm0 820c-205.4 0-372-166.6-372-372s166.6-372 372-372 372 166.6 372 372-166.6 372-372 372z" }
		}, {
			"tag": "path",
			"attrs": { "d": "M464 336a48 48 0 1096 0 48 48 0 10-96 0zm72 112h-48c-4.4 0-8 3.6-8 8v272c0 4.4 3.6 8 8 8h48c4.4 0 8-3.6 8-8V456c0-4.4-3.6-8-8-8z" }
		}]
	},
	"name": "info-circle",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/InfoCircleOutlined.js
function _extends$18() {
	_extends$18 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$18.apply(this, arguments);
}
var InfoCircleOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$18({}, props, {
	ref,
	icon: InfoCircleOutlined$1
}));
/**![info-circle](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTUxMiA2NEMyNjQuNiA2NCA2NCAyNjQuNiA2NCA1MTJzMjAwLjYgNDQ4IDQ0OCA0NDggNDQ4LTIwMC42IDQ0OC00NDhTNzU5LjQgNjQgNTEyIDY0em0wIDgyMGMtMjA1LjQgMC0zNzItMTY2LjYtMzcyLTM3MnMxNjYuNi0zNzIgMzcyLTM3MiAzNzIgMTY2LjYgMzcyIDM3Mi0xNjYuNiAzNzItMzcyIDM3MnoiIC8+PHBhdGggZD0iTTQ2NCAzMzZhNDggNDggMCAxMDk2IDAgNDggNDggMCAxMC05NiAwem03MiAxMTJoLTQ4Yy00LjQgMC04IDMuNi04IDh2MjcyYzAgNC40IDMuNiA4IDggOGg0OGM0LjQgMCA4LTMuNiA4LThWNDU2YzAtNC40LTMuNi04LTgtOHoiIC8+PC9zdmc+) */
var RefIcon$21 = /*#__PURE__*/ React$1.forwardRef(InfoCircleOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/LeftOutlined.js
var LeftOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M724 218.3V141c0-6.7-7.7-10.4-12.9-6.3L260.3 486.8a31.86 31.86 0 000 50.3l450.8 352.1c5.3 4.1 12.9.4 12.9-6.3v-77.3c0-4.9-2.3-9.6-6.1-12.6l-360-281 360-281.1c3.8-3 6.1-7.7 6.1-12.6z" }
		}]
	},
	"name": "left",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/LeftOutlined.js
function _extends$17() {
	_extends$17 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$17.apply(this, arguments);
}
var LeftOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$17({}, props, {
	ref,
	icon: LeftOutlined$1
}));
/**![left](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTcyNCAyMTguM1YxNDFjMC02LjctNy43LTEwLjQtMTIuOS02LjNMMjYwLjMgNDg2LjhhMzEuODYgMzEuODYgMCAwMDAgNTAuM2w0NTAuOCAzNTIuMWM1LjMgNC4xIDEyLjkuNCAxMi45LTYuM3YtNzcuM2MwLTQuOS0yLjMtOS42LTYuMS0xMi42bC0zNjAtMjgxIDM2MC0yODEuMWMzLjgtMyA2LjEtNy43IDYuMS0xMi42eiIgLz48L3N2Zz4=) */
var RefIcon$22 = /*#__PURE__*/ React$1.forwardRef(LeftOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/LinkOutlined.js
var LinkOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M574 665.4a8.03 8.03 0 00-11.3 0L446.5 781.6c-53.8 53.8-144.6 59.5-204 0-59.5-59.5-53.8-150.2 0-204l116.2-116.2c3.1-3.1 3.1-8.2 0-11.3l-39.8-39.8a8.03 8.03 0 00-11.3 0L191.4 526.5c-84.6 84.6-84.6 221.5 0 306s221.5 84.6 306 0l116.2-116.2c3.1-3.1 3.1-8.2 0-11.3L574 665.4zm258.6-474c-84.6-84.6-221.5-84.6-306 0L410.3 307.6a8.03 8.03 0 000 11.3l39.7 39.7c3.1 3.1 8.2 3.1 11.3 0l116.2-116.2c53.8-53.8 144.6-59.5 204 0 59.5 59.5 53.8 150.2 0 204L665.3 562.6a8.03 8.03 0 000 11.3l39.8 39.8c3.1 3.1 8.2 3.1 11.3 0l116.2-116.2c84.5-84.6 84.5-221.5 0-306.1zM610.1 372.3a8.03 8.03 0 00-11.3 0L372.3 598.7a8.03 8.03 0 000 11.3l39.6 39.6c3.1 3.1 8.2 3.1 11.3 0l226.4-226.4c3.1-3.1 3.1-8.2 0-11.3l-39.5-39.6z" }
		}]
	},
	"name": "link",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/LinkOutlined.js
function _extends$16() {
	_extends$16 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$16.apply(this, arguments);
}
var LinkOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$16({}, props, {
	ref,
	icon: LinkOutlined$1
}));
/**![link](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTU3NCA2NjUuNGE4LjAzIDguMDMgMCAwMC0xMS4zIDBMNDQ2LjUgNzgxLjZjLTUzLjggNTMuOC0xNDQuNiA1OS41LTIwNCAwLTU5LjUtNTkuNS01My44LTE1MC4yIDAtMjA0bDExNi4yLTExNi4yYzMuMS0zLjEgMy4xLTguMiAwLTExLjNsLTM5LjgtMzkuOGE4LjAzIDguMDMgMCAwMC0xMS4zIDBMMTkxLjQgNTI2LjVjLTg0LjYgODQuNi04NC42IDIyMS41IDAgMzA2czIyMS41IDg0LjYgMzA2IDBsMTE2LjItMTE2LjJjMy4xLTMuMSAzLjEtOC4yIDAtMTEuM0w1NzQgNjY1LjR6bTI1OC42LTQ3NGMtODQuNi04NC42LTIyMS41LTg0LjYtMzA2IDBMNDEwLjMgMzA3LjZhOC4wMyA4LjAzIDAgMDAwIDExLjNsMzkuNyAzOS43YzMuMSAzLjEgOC4yIDMuMSAxMS4zIDBsMTE2LjItMTE2LjJjNTMuOC01My44IDE0NC42LTU5LjUgMjA0IDAgNTkuNSA1OS41IDUzLjggMTUwLjIgMCAyMDRMNjY1LjMgNTYyLjZhOC4wMyA4LjAzIDAgMDAwIDExLjNsMzkuOCAzOS44YzMuMSAzLjEgOC4yIDMuMSAxMS4zIDBsMTE2LjItMTE2LjJjODQuNS04NC42IDg0LjUtMjIxLjUgMC0zMDYuMXpNNjEwLjEgMzcyLjNhOC4wMyA4LjAzIDAgMDAtMTEuMyAwTDM3Mi4zIDU5OC43YTguMDMgOC4wMyAwIDAwMCAxMS4zbDM5LjYgMzkuNmMzLjEgMy4xIDguMiAzLjEgMTEuMyAwbDIyNi40LTIyNi40YzMuMS0zLjEgMy4xLTguMiAwLTExLjNsLTM5LjUtMzkuNnoiIC8+PC9zdmc+) */
var RefIcon$23 = /*#__PURE__*/ React$1.forwardRef(LinkOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/LoadingOutlined.js
var LoadingOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "0 0 1024 1024",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M988 548c-19.9 0-36-16.1-36-36 0-59.4-11.6-117-34.6-171.3a440.45 440.45 0 00-94.3-139.9 437.71 437.71 0 00-139.9-94.3C629 83.6 571.4 72 512 72c-19.9 0-36-16.1-36-36s16.1-36 36-36c69.1 0 136.2 13.5 199.3 40.3C772.3 66 827 103 874 150c47 47 83.9 101.8 109.7 162.7 26.7 63.1 40.2 130.2 40.2 199.3.1 19.9-16 36-35.9 36z" }
		}]
	},
	"name": "loading",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/LoadingOutlined.js
function _extends$15() {
	_extends$15 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$15.apply(this, arguments);
}
var LoadingOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$15({}, props, {
	ref,
	icon: LoadingOutlined$1
}));
/**![loading](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjAgMCAxMDI0IDEwMjQiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTk4OCA1NDhjLTE5LjkgMC0zNi0xNi4xLTM2LTM2IDAtNTkuNC0xMS42LTExNy0zNC42LTE3MS4zYTQ0MC40NSA0NDAuNDUgMCAwMC05NC4zLTEzOS45IDQzNy43MSA0MzcuNzEgMCAwMC0xMzkuOS05NC4zQzYyOSA4My42IDU3MS40IDcyIDUxMiA3MmMtMTkuOSAwLTM2LTE2LjEtMzYtMzZzMTYuMS0zNiAzNi0zNmM2OS4xIDAgMTM2LjIgMTMuNSAxOTkuMyA0MC4zQzc3Mi4zIDY2IDgyNyAxMDMgODc0IDE1MGM0NyA0NyA4My45IDEwMS44IDEwOS43IDE2Mi43IDI2LjcgNjMuMSA0MC4yIDEzMC4yIDQwLjIgMTk5LjMuMSAxOS45LTE2IDM2LTM1LjkgMzZ6IiAvPjwvc3ZnPg==) */
var RefIcon$24 = /*#__PURE__*/ React$1.forwardRef(LoadingOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/LockOutlined.js
var LockOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M832 464h-68V240c0-70.7-57.3-128-128-128H388c-70.7 0-128 57.3-128 128v224h-68c-17.7 0-32 14.3-32 32v384c0 17.7 14.3 32 32 32h640c17.7 0 32-14.3 32-32V496c0-17.7-14.3-32-32-32zM332 240c0-30.9 25.1-56 56-56h248c30.9 0 56 25.1 56 56v224H332V240zm460 600H232V536h560v304zM484 701v53c0 4.4 3.6 8 8 8h40c4.4 0 8-3.6 8-8v-53a48.01 48.01 0 10-56 0z" }
		}]
	},
	"name": "lock",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/LockOutlined.js
function _extends$14() {
	_extends$14 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$14.apply(this, arguments);
}
var LockOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$14({}, props, {
	ref,
	icon: LockOutlined$1
}));
/**![lock](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTgzMiA0NjRoLTY4VjI0MGMwLTcwLjctNTcuMy0xMjgtMTI4LTEyOEgzODhjLTcwLjcgMC0xMjggNTcuMy0xMjggMTI4djIyNGgtNjhjLTE3LjcgMC0zMiAxNC4zLTMyIDMydjM4NGMwIDE3LjcgMTQuMyAzMiAzMiAzMmg2NDBjMTcuNyAwIDMyLTE0LjMgMzItMzJWNDk2YzAtMTcuNy0xNC4zLTMyLTMyLTMyek0zMzIgMjQwYzAtMzAuOSAyNS4xLTU2IDU2LTU2aDI0OGMzMC45IDAgNTYgMjUuMSA1NiA1NnYyMjRIMzMyVjI0MHptNDYwIDYwMEgyMzJWNTM2aDU2MHYzMDR6TTQ4NCA3MDF2NTNjMCA0LjQgMy42IDggOCA4aDQwYzQuNCAwIDgtMy42IDgtOHYtNTNhNDguMDEgNDguMDEgMCAxMC01NiAweiIgLz48L3N2Zz4=) */
var RefIcon$25 = /*#__PURE__*/ React$1.forwardRef(LockOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/MailOutlined.js
var MailOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M928 160H96c-17.7 0-32 14.3-32 32v640c0 17.7 14.3 32 32 32h832c17.7 0 32-14.3 32-32V192c0-17.7-14.3-32-32-32zm-40 110.8V792H136V270.8l-27.6-21.5 39.3-50.5 42.8 33.3h643.1l42.8-33.3 39.3 50.5-27.7 21.5zM833.6 232L512 482 190.4 232l-42.8-33.3-39.3 50.5 27.6 21.5 341.6 265.6a55.99 55.99 0 0068.7 0L888 270.8l27.6-21.5-39.3-50.5-42.7 33.2z" }
		}]
	},
	"name": "mail",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/MailOutlined.js
function _extends$13() {
	_extends$13 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$13.apply(this, arguments);
}
var MailOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$13({}, props, {
	ref,
	icon: MailOutlined$1
}));
/**![mail](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTkyOCAxNjBIOTZjLTE3LjcgMC0zMiAxNC4zLTMyIDMydjY0MGMwIDE3LjcgMTQuMyAzMiAzMiAzMmg4MzJjMTcuNyAwIDMyLTE0LjMgMzItMzJWMTkyYzAtMTcuNy0xNC4zLTMyLTMyLTMyem0tNDAgMTEwLjhWNzkySDEzNlYyNzAuOGwtMjcuNi0yMS41IDM5LjMtNTAuNSA0Mi44IDMzLjNoNjQzLjFsNDIuOC0zMy4zIDM5LjMgNTAuNS0yNy43IDIxLjV6TTgzMy42IDIzMkw1MTIgNDgyIDE5MC40IDIzMmwtNDIuOC0zMy4zLTM5LjMgNTAuNSAyNy42IDIxLjUgMzQxLjYgMjY1LjZhNTUuOTkgNTUuOTkgMCAwMDY4LjcgMEw4ODggMjcwLjhsMjcuNi0yMS41LTM5LjMtNTAuNS00Mi43IDMzLjJ6IiAvPjwvc3ZnPg==) */
var RefIcon$26 = /*#__PURE__*/ React$1.forwardRef(MailOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/MenuOutlined.js
var MenuOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M904 160H120c-4.4 0-8 3.6-8 8v64c0 4.4 3.6 8 8 8h784c4.4 0 8-3.6 8-8v-64c0-4.4-3.6-8-8-8zm0 624H120c-4.4 0-8 3.6-8 8v64c0 4.4 3.6 8 8 8h784c4.4 0 8-3.6 8-8v-64c0-4.4-3.6-8-8-8zm0-312H120c-4.4 0-8 3.6-8 8v64c0 4.4 3.6 8 8 8h784c4.4 0 8-3.6 8-8v-64c0-4.4-3.6-8-8-8z" }
		}]
	},
	"name": "menu",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/MenuOutlined.js
function _extends$12() {
	_extends$12 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$12.apply(this, arguments);
}
var MenuOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$12({}, props, {
	ref,
	icon: MenuOutlined$1
}));
/**![menu](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTkwNCAxNjBIMTIwYy00LjQgMC04IDMuNi04IDh2NjRjMCA0LjQgMy42IDggOCA4aDc4NGM0LjQgMCA4LTMuNiA4LTh2LTY0YzAtNC40LTMuNi04LTgtOHptMCA2MjRIMTIwYy00LjQgMC04IDMuNi04IDh2NjRjMCA0LjQgMy42IDggOCA4aDc4NGM0LjQgMCA4LTMuNiA4LTh2LTY0YzAtNC40LTMuNi04LTgtOHptMC0zMTJIMTIwYy00LjQgMC04IDMuNi04IDh2NjRjMCA0LjQgMy42IDggOCA4aDc4NGM0LjQgMCA4LTMuNiA4LTh2LTY0YzAtNC40LTMuNi04LTgtOHoiIC8+PC9zdmc+) */
var RefIcon$27 = /*#__PURE__*/ React$1.forwardRef(MenuOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/MinusOutlined.js
var MinusOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M872 474H152c-4.4 0-8 3.6-8 8v60c0 4.4 3.6 8 8 8h720c4.4 0 8-3.6 8-8v-60c0-4.4-3.6-8-8-8z" }
		}]
	},
	"name": "minus",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/MinusOutlined.js
function _extends$11() {
	_extends$11 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$11.apply(this, arguments);
}
var MinusOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$11({}, props, {
	ref,
	icon: MinusOutlined$1
}));
/**![minus](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTg3MiA0NzRIMTUyYy00LjQgMC04IDMuNi04IDh2NjBjMCA0LjQgMy42IDggOCA4aDcyMGM0LjQgMCA4LTMuNiA4LTh2LTYwYzAtNC40LTMuNi04LTgtOHoiIC8+PC9zdmc+) */
var RefIcon$28 = /*#__PURE__*/ React$1.forwardRef(MinusOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/MoreOutlined.js
var MoreOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M456 231a56 56 0 10112 0 56 56 0 10-112 0zm0 280a56 56 0 10112 0 56 56 0 10-112 0zm0 280a56 56 0 10112 0 56 56 0 10-112 0z" }
		}]
	},
	"name": "more",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/MoreOutlined.js
function _extends$10() {
	_extends$10 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$10.apply(this, arguments);
}
var MoreOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$10({}, props, {
	ref,
	icon: MoreOutlined$1
}));
/**![more](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTQ1NiAyMzFhNTYgNTYgMCAxMDExMiAwIDU2IDU2IDAgMTAtMTEyIDB6bTAgMjgwYTU2IDU2IDAgMTAxMTIgMCA1NiA1NiAwIDEwLTExMiAwem0wIDI4MGE1NiA1NiAwIDEwMTEyIDAgNTYgNTYgMCAxMC0xMTIgMHoiIC8+PC9zdmc+) */
var RefIcon$29 = /*#__PURE__*/ React$1.forwardRef(MoreOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/PlusOutlined.js
var PlusOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M482 152h60q8 0 8 8v704q0 8-8 8h-60q-8 0-8-8V160q0-8 8-8z" }
		}, {
			"tag": "path",
			"attrs": { "d": "M192 474h672q8 0 8 8v60q0 8-8 8H160q-8 0-8-8v-60q0-8 8-8z" }
		}]
	},
	"name": "plus",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/PlusOutlined.js
function _extends$9() {
	_extends$9 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$9.apply(this, arguments);
}
var PlusOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$9({}, props, {
	ref,
	icon: PlusOutlined$1
}));
/**![plus](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTQ4MiAxNTJoNjBxOCAwIDggOHY3MDRxMCA4LTggOGgtNjBxLTggMC04LThWMTYwcTAtOCA4LTh6IiAvPjxwYXRoIGQ9Ik0xOTIgNDc0aDY3MnE4IDAgOCA4djYwcTAgOC04IDhIMTYwcS04IDAtOC04di02MHEwLTggOC04eiIgLz48L3N2Zz4=) */
var RefIcon$30 = /*#__PURE__*/ React$1.forwardRef(PlusOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/QuestionCircleOutlined.js
var QuestionCircleOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M512 64C264.6 64 64 264.6 64 512s200.6 448 448 448 448-200.6 448-448S759.4 64 512 64zm0 820c-205.4 0-372-166.6-372-372s166.6-372 372-372 372 166.6 372 372-166.6 372-372 372z" }
		}, {
			"tag": "path",
			"attrs": { "d": "M623.6 316.7C593.6 290.4 554 276 512 276s-81.6 14.5-111.6 40.7C369.2 344 352 380.7 352 420v7.6c0 4.4 3.6 8 8 8h48c4.4 0 8-3.6 8-8V420c0-44.1 43.1-80 96-80s96 35.9 96 80c0 31.1-22 59.6-56.1 72.7-21.2 8.1-39.2 22.3-52.1 40.9-13.1 19-19.9 41.8-19.9 64.9V620c0 4.4 3.6 8 8 8h48c4.4 0 8-3.6 8-8v-22.7a48.3 48.3 0 0130.9-44.8c59-22.7 97.1-74.7 97.1-132.5.1-39.3-17.1-76-48.3-103.3zM472 732a40 40 0 1080 0 40 40 0 10-80 0z" }
		}]
	},
	"name": "question-circle",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/QuestionCircleOutlined.js
function _extends$8() {
	_extends$8 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$8.apply(this, arguments);
}
var QuestionCircleOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$8({}, props, {
	ref,
	icon: QuestionCircleOutlined$1
}));
/**![question-circle](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTUxMiA2NEMyNjQuNiA2NCA2NCAyNjQuNiA2NCA1MTJzMjAwLjYgNDQ4IDQ0OCA0NDggNDQ4LTIwMC42IDQ0OC00NDhTNzU5LjQgNjQgNTEyIDY0em0wIDgyMGMtMjA1LjQgMC0zNzItMTY2LjYtMzcyLTM3MnMxNjYuNi0zNzIgMzcyLTM3MiAzNzIgMTY2LjYgMzcyIDM3Mi0xNjYuNiAzNzItMzcyIDM3MnoiIC8+PHBhdGggZD0iTTYyMy42IDMxNi43QzU5My42IDI5MC40IDU1NCAyNzYgNTEyIDI3NnMtODEuNiAxNC41LTExMS42IDQwLjdDMzY5LjIgMzQ0IDM1MiAzODAuNyAzNTIgNDIwdjcuNmMwIDQuNCAzLjYgOCA4IDhoNDhjNC40IDAgOC0zLjYgOC04VjQyMGMwLTQ0LjEgNDMuMS04MCA5Ni04MHM5NiAzNS45IDk2IDgwYzAgMzEuMS0yMiA1OS42LTU2LjEgNzIuNy0yMS4yIDguMS0zOS4yIDIyLjMtNTIuMSA0MC45LTEzLjEgMTktMTkuOSA0MS44LTE5LjkgNjQuOVY2MjBjMCA0LjQgMy42IDggOCA4aDQ4YzQuNCAwIDgtMy42IDgtOHYtMjIuN2E0OC4zIDQ4LjMgMCAwMTMwLjktNDQuOGM1OS0yMi43IDk3LjEtNzQuNyA5Ny4xLTEzMi41LjEtMzkuMy0xNy4xLTc2LTQ4LjMtMTAzLjN6TTQ3MiA3MzJhNDAgNDAgMCAxMDgwIDAgNDAgNDAgMCAxMC04MCAweiIgLz48L3N2Zz4=) */
var RefIcon$31 = /*#__PURE__*/ React$1.forwardRef(QuestionCircleOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/RightOutlined.js
var RightOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M765.7 486.8L314.9 134.7A7.97 7.97 0 00302 141v77.3c0 4.9 2.3 9.6 6.1 12.6l360 281.1-360 281.1c-3.9 3-6.1 7.7-6.1 12.6V883c0 6.7 7.7 10.4 12.9 6.3l450.8-352.1a31.96 31.96 0 000-50.4z" }
		}]
	},
	"name": "right",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/RightOutlined.js
function _extends$7() {
	_extends$7 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$7.apply(this, arguments);
}
var RightOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$7({}, props, {
	ref,
	icon: RightOutlined$1
}));
/**![right](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTc2NS43IDQ4Ni44TDMxNC45IDEzNC43QTcuOTcgNy45NyAwIDAwMzAyIDE0MXY3Ny4zYzAgNC45IDIuMyA5LjYgNi4xIDEyLjZsMzYwIDI4MS4xLTM2MCAyODEuMWMtMy45IDMtNi4xIDcuNy02LjEgMTIuNlY4ODNjMCA2LjcgNy43IDEwLjQgMTIuOSA2LjNsNDUwLjgtMzUyLjFhMzEuOTYgMzEuOTYgMCAwMDAtNTAuNHoiIC8+PC9zdmc+) */
var RefIcon$32 = /*#__PURE__*/ React$1.forwardRef(RightOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/SaveOutlined.js
var SaveOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M893.3 293.3L730.7 130.7c-7.5-7.5-16.7-13-26.7-16V112H144c-17.7 0-32 14.3-32 32v736c0 17.7 14.3 32 32 32h736c17.7 0 32-14.3 32-32V338.5c0-17-6.7-33.2-18.7-45.2zM384 184h256v104H384V184zm456 656H184V184h136v136c0 17.7 14.3 32 32 32h320c17.7 0 32-14.3 32-32V205.8l136 136V840zM512 442c-79.5 0-144 64.5-144 144s64.5 144 144 144 144-64.5 144-144-64.5-144-144-144zm0 224c-44.2 0-80-35.8-80-80s35.8-80 80-80 80 35.8 80 80-35.8 80-80 80z" }
		}]
	},
	"name": "save",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/SaveOutlined.js
function _extends$6() {
	_extends$6 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$6.apply(this, arguments);
}
var SaveOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$6({}, props, {
	ref,
	icon: SaveOutlined$1
}));
/**![save](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTg5My4zIDI5My4zTDczMC43IDEzMC43Yy03LjUtNy41LTE2LjctMTMtMjYuNy0xNlYxMTJIMTQ0Yy0xNy43IDAtMzIgMTQuMy0zMiAzMnY3MzZjMCAxNy43IDE0LjMgMzIgMzIgMzJoNzM2YzE3LjcgMCAzMi0xNC4zIDMyLTMyVjMzOC41YzAtMTctNi43LTMzLjItMTguNy00NS4yek0zODQgMTg0aDI1NnYxMDRIMzg0VjE4NHptNDU2IDY1NkgxODRWMTg0aDEzNnYxMzZjMCAxNy43IDE0LjMgMzIgMzIgMzJoMzIwYzE3LjcgMCAzMi0xNC4zIDMyLTMyVjIwNS44bDEzNiAxMzZWODQwek01MTIgNDQyYy03OS41IDAtMTQ0IDY0LjUtMTQ0IDE0NHM2NC41IDE0NCAxNDQgMTQ0IDE0NC02NC41IDE0NC0xNDQtNjQuNS0xNDQtMTQ0LTE0NHptMCAyMjRjLTQ0LjIgMC04MC0zNS44LTgwLTgwczM1LjgtODAgODAtODAgODAgMzUuOCA4MCA4MC0zNS44IDgwLTgwIDgweiIgLz48L3N2Zz4=) */
var RefIcon$33 = /*#__PURE__*/ React$1.forwardRef(SaveOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/SearchOutlined.js
var SearchOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M909.6 854.5L649.9 594.8C690.2 542.7 712 479 712 412c0-80.2-31.3-155.4-87.9-212.1-56.6-56.7-132-87.9-212.1-87.9s-155.5 31.3-212.1 87.9C143.2 256.5 112 331.8 112 412c0 80.1 31.3 155.5 87.9 212.1C256.5 680.8 331.8 712 412 712c67 0 130.6-21.8 182.7-62l259.7 259.6a8.2 8.2 0 0011.6 0l43.6-43.5a8.2 8.2 0 000-11.6zM570.4 570.4C528 612.7 471.8 636 412 636s-116-23.3-158.4-65.6C211.3 528 188 471.8 188 412s23.3-116.1 65.6-158.4C296 211.3 352.2 188 412 188s116.1 23.2 158.4 65.6S636 352.2 636 412s-23.3 116.1-65.6 158.4z" }
		}]
	},
	"name": "search",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/SearchOutlined.js
function _extends$5() {
	_extends$5 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$5.apply(this, arguments);
}
var SearchOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$5({}, props, {
	ref,
	icon: SearchOutlined$1
}));
/**![search](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTkwOS42IDg1NC41TDY0OS45IDU5NC44QzY5MC4yIDU0Mi43IDcxMiA0NzkgNzEyIDQxMmMwLTgwLjItMzEuMy0xNTUuNC04Ny45LTIxMi4xLTU2LjYtNTYuNy0xMzItODcuOS0yMTIuMS04Ny45cy0xNTUuNSAzMS4zLTIxMi4xIDg3LjlDMTQzLjIgMjU2LjUgMTEyIDMzMS44IDExMiA0MTJjMCA4MC4xIDMxLjMgMTU1LjUgODcuOSAyMTIuMUMyNTYuNSA2ODAuOCAzMzEuOCA3MTIgNDEyIDcxMmM2NyAwIDEzMC42LTIxLjggMTgyLjctNjJsMjU5LjcgMjU5LjZhOC4yIDguMiAwIDAwMTEuNiAwbDQzLjYtNDMuNWE4LjIgOC4yIDAgMDAwLTExLjZ6TTU3MC40IDU3MC40QzUyOCA2MTIuNyA0NzEuOCA2MzYgNDEyIDYzNnMtMTE2LTIzLjMtMTU4LjQtNjUuNkMyMTEuMyA1MjggMTg4IDQ3MS44IDE4OCA0MTJzMjMuMy0xMTYuMSA2NS42LTE1OC40QzI5NiAyMTEuMyAzNTIuMiAxODggNDEyIDE4OHMxMTYuMSAyMy4yIDE1OC40IDY1LjZTNjM2IDM1Mi4yIDYzNiA0MTJzLTIzLjMgMTE2LjEtNjUuNiAxNTguNHoiIC8+PC9zdmc+) */
var RefIcon$34 = /*#__PURE__*/ React$1.forwardRef(SearchOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/SettingOutlined.js
var SettingOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M924.8 625.7l-65.5-56c3.1-19 4.7-38.4 4.7-57.8s-1.6-38.8-4.7-57.8l65.5-56a32.03 32.03 0 009.3-35.2l-.9-2.6a443.74 443.74 0 00-79.7-137.9l-1.8-2.1a32.12 32.12 0 00-35.1-9.5l-81.3 28.9c-30-24.6-63.5-44-99.7-57.6l-15.7-85a32.05 32.05 0 00-25.8-25.7l-2.7-.5c-52.1-9.4-106.9-9.4-159 0l-2.7.5a32.05 32.05 0 00-25.8 25.7l-15.8 85.4a351.86 351.86 0 00-99 57.4l-81.9-29.1a32 32 0 00-35.1 9.5l-1.8 2.1a446.02 446.02 0 00-79.7 137.9l-.9 2.6c-4.5 12.5-.8 26.5 9.3 35.2l66.3 56.6c-3.1 18.8-4.6 38-4.6 57.1 0 19.2 1.5 38.4 4.6 57.1L99 625.5a32.03 32.03 0 00-9.3 35.2l.9 2.6c18.1 50.4 44.9 96.9 79.7 137.9l1.8 2.1a32.12 32.12 0 0035.1 9.5l81.9-29.1c29.8 24.5 63.1 43.9 99 57.4l15.8 85.4a32.05 32.05 0 0025.8 25.7l2.7.5a449.4 449.4 0 00159 0l2.7-.5a32.05 32.05 0 0025.8-25.7l15.7-85a350 350 0 0099.7-57.6l81.3 28.9a32 32 0 0035.1-9.5l1.8-2.1c34.8-41.1 61.6-87.5 79.7-137.9l.9-2.6c4.5-12.3.8-26.3-9.3-35zM788.3 465.9c2.5 15.1 3.8 30.6 3.8 46.1s-1.3 31-3.8 46.1l-6.6 40.1 74.7 63.9a370.03 370.03 0 01-42.6 73.6L721 702.8l-31.4 25.8c-23.9 19.6-50.5 35-79.3 45.8l-38.1 14.3-17.9 97a377.5 377.5 0 01-85 0l-17.9-97.2-37.8-14.5c-28.5-10.8-55-26.2-78.7-45.7l-31.4-25.9-93.4 33.2c-17-22.9-31.2-47.6-42.6-73.6l75.5-64.5-6.5-40c-2.4-14.9-3.7-30.3-3.7-45.5 0-15.3 1.2-30.6 3.7-45.5l6.5-40-75.5-64.5c11.3-26.1 25.6-50.7 42.6-73.6l93.4 33.2 31.4-25.9c23.7-19.5 50.2-34.9 78.7-45.7l37.9-14.3 17.9-97.2c28.1-3.2 56.8-3.2 85 0l17.9 97 38.1 14.3c28.7 10.8 55.4 26.2 79.3 45.8l31.4 25.8 92.8-32.9c17 22.9 31.2 47.6 42.6 73.6L781.8 426l6.5 39.9zM512 326c-97.2 0-176 78.8-176 176s78.8 176 176 176 176-78.8 176-176-78.8-176-176-176zm79.2 255.2A111.6 111.6 0 01512 614c-29.9 0-58-11.7-79.2-32.8A111.6 111.6 0 01400 502c0-29.9 11.7-58 32.8-79.2C454 401.6 482.1 390 512 390c29.9 0 58 11.6 79.2 32.8A111.6 111.6 0 01624 502c0 29.9-11.7 58-32.8 79.2z" }
		}]
	},
	"name": "setting",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/SettingOutlined.js
function _extends$4() {
	_extends$4 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$4.apply(this, arguments);
}
var SettingOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$4({}, props, {
	ref,
	icon: SettingOutlined$1
}));
/**![setting](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTkyNC44IDYyNS43bC02NS41LTU2YzMuMS0xOSA0LjctMzguNCA0LjctNTcuOHMtMS42LTM4LjgtNC43LTU3LjhsNjUuNS01NmEzMi4wMyAzMi4wMyAwIDAwOS4zLTM1LjJsLS45LTIuNmE0NDMuNzQgNDQzLjc0IDAgMDAtNzkuNy0xMzcuOWwtMS44LTIuMWEzMi4xMiAzMi4xMiAwIDAwLTM1LjEtOS41bC04MS4zIDI4LjljLTMwLTI0LjYtNjMuNS00NC05OS43LTU3LjZsLTE1LjctODVhMzIuMDUgMzIuMDUgMCAwMC0yNS44LTI1LjdsLTIuNy0uNWMtNTIuMS05LjQtMTA2LjktOS40LTE1OSAwbC0yLjcuNWEzMi4wNSAzMi4wNSAwIDAwLTI1LjggMjUuN2wtMTUuOCA4NS40YTM1MS44NiAzNTEuODYgMCAwMC05OSA1Ny40bC04MS45LTI5LjFhMzIgMzIgMCAwMC0zNS4xIDkuNWwtMS44IDIuMWE0NDYuMDIgNDQ2LjAyIDAgMDAtNzkuNyAxMzcuOWwtLjkgMi42Yy00LjUgMTIuNS0uOCAyNi41IDkuMyAzNS4ybDY2LjMgNTYuNmMtMy4xIDE4LjgtNC42IDM4LTQuNiA1Ny4xIDAgMTkuMiAxLjUgMzguNCA0LjYgNTcuMUw5OSA2MjUuNWEzMi4wMyAzMi4wMyAwIDAwLTkuMyAzNS4ybC45IDIuNmMxOC4xIDUwLjQgNDQuOSA5Ni45IDc5LjcgMTM3LjlsMS44IDIuMWEzMi4xMiAzMi4xMiAwIDAwMzUuMSA5LjVsODEuOS0yOS4xYzI5LjggMjQuNSA2My4xIDQzLjkgOTkgNTcuNGwxNS44IDg1LjRhMzIuMDUgMzIuMDUgMCAwMDI1LjggMjUuN2wyLjcuNWE0NDkuNCA0NDkuNCAwIDAwMTU5IDBsMi43LS41YTMyLjA1IDMyLjA1IDAgMDAyNS44LTI1LjdsMTUuNy04NWEzNTAgMzUwIDAgMDA5OS43LTU3LjZsODEuMyAyOC45YTMyIDMyIDAgMDAzNS4xLTkuNWwxLjgtMi4xYzM0LjgtNDEuMSA2MS42LTg3LjUgNzkuNy0xMzcuOWwuOS0yLjZjNC41LTEyLjMuOC0yNi4zLTkuMy0zNXpNNzg4LjMgNDY1LjljMi41IDE1LjEgMy44IDMwLjYgMy44IDQ2LjFzLTEuMyAzMS0zLjggNDYuMWwtNi42IDQwLjEgNzQuNyA2My45YTM3MC4wMyAzNzAuMDMgMCAwMS00Mi42IDczLjZMNzIxIDcwMi44bC0zMS40IDI1LjhjLTIzLjkgMTkuNi01MC41IDM1LTc5LjMgNDUuOGwtMzguMSAxNC4zLTE3LjkgOTdhMzc3LjUgMzc3LjUgMCAwMS04NSAwbC0xNy45LTk3LjItMzcuOC0xNC41Yy0yOC41LTEwLjgtNTUtMjYuMi03OC43LTQ1LjdsLTMxLjQtMjUuOS05My40IDMzLjJjLTE3LTIyLjktMzEuMi00Ny42LTQyLjYtNzMuNmw3NS41LTY0LjUtNi41LTQwYy0yLjQtMTQuOS0zLjctMzAuMy0zLjctNDUuNSAwLTE1LjMgMS4yLTMwLjYgMy43LTQ1LjVsNi41LTQwLTc1LjUtNjQuNWMxMS4zLTI2LjEgMjUuNi01MC43IDQyLjYtNzMuNmw5My40IDMzLjIgMzEuNC0yNS45YzIzLjctMTkuNSA1MC4yLTM0LjkgNzguNy00NS43bDM3LjktMTQuMyAxNy45LTk3LjJjMjguMS0zLjIgNTYuOC0zLjIgODUgMGwxNy45IDk3IDM4LjEgMTQuM2MyOC43IDEwLjggNTUuNCAyNi4yIDc5LjMgNDUuOGwzMS40IDI1LjggOTIuOC0zMi45YzE3IDIyLjkgMzEuMiA0Ny42IDQyLjYgNzMuNkw3ODEuOCA0MjZsNi41IDM5Ljl6TTUxMiAzMjZjLTk3LjIgMC0xNzYgNzguOC0xNzYgMTc2czc4LjggMTc2IDE3NiAxNzYgMTc2LTc4LjggMTc2LTE3Ni03OC44LTE3Ni0xNzYtMTc2em03OS4yIDI1NS4yQTExMS42IDExMS42IDAgMDE1MTIgNjE0Yy0yOS45IDAtNTgtMTEuNy03OS4yLTMyLjhBMTExLjYgMTExLjYgMCAwMTQwMCA1MDJjMC0yOS45IDExLjctNTggMzIuOC03OS4yQzQ1NCA0MDEuNiA0ODIuMSAzOTAgNTEyIDM5MGMyOS45IDAgNTggMTEuNiA3OS4yIDMyLjhBMTExLjYgMTExLjYgMCAwMTYyNCA1MDJjMCAyOS45LTExLjcgNTgtMzIuOCA3OS4yeiIgLz48L3N2Zz4=) */
var RefIcon$35 = /*#__PURE__*/ React$1.forwardRef(SettingOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/UpOutlined.js
var UpOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M890.5 755.3L537.9 269.2c-12.8-17.6-39-17.6-51.7 0L133.5 755.3A8 8 0 00140 768h75c5.1 0 9.9-2.5 12.9-6.6L512 369.8l284.1 391.6c3 4.1 7.8 6.6 12.9 6.6h75c6.5 0 10.3-7.4 6.5-12.7z" }
		}]
	},
	"name": "up",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/UpOutlined.js
function _extends$3() {
	_extends$3 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$3.apply(this, arguments);
}
var UpOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$3({}, props, {
	ref,
	icon: UpOutlined$1
}));
/**![up](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTg5MC41IDc1NS4zTDUzNy45IDI2OS4yYy0xMi44LTE3LjYtMzktMTcuNi01MS43IDBMMTMzLjUgNzU1LjNBOCA4IDAgMDAxNDAgNzY4aDc1YzUuMSAwIDkuOS0yLjUgMTIuOS02LjZMNTEyIDM2OS44bDI4NC4xIDM5MS42YzMgNC4xIDcuOCA2LjYgMTIuOSA2LjZoNzVjNi41IDAgMTAuMy03LjQgNi41LTEyLjd6IiAvPjwvc3ZnPg==) */
var RefIcon$36 = /*#__PURE__*/ React$1.forwardRef(UpOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/UploadOutlined.js
var UploadOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M400 317.7h73.9V656c0 4.4 3.6 8 8 8h60c4.4 0 8-3.6 8-8V317.7H624c6.7 0 10.4-7.7 6.3-12.9L518.3 163a8 8 0 00-12.6 0l-112 141.7c-4.1 5.3-.4 13 6.3 13zM878 626h-60c-4.4 0-8 3.6-8 8v154H214V634c0-4.4-3.6-8-8-8h-60c-4.4 0-8 3.6-8 8v198c0 17.7 14.3 32 32 32h684c17.7 0 32-14.3 32-32V634c0-4.4-3.6-8-8-8z" }
		}]
	},
	"name": "upload",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/UploadOutlined.js
function _extends$2() {
	_extends$2 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$2.apply(this, arguments);
}
var UploadOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$2({}, props, {
	ref,
	icon: UploadOutlined$1
}));
/**![upload](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTQwMCAzMTcuN2g3My45VjY1NmMwIDQuNCAzLjYgOCA4IDhoNjBjNC40IDAgOC0zLjYgOC04VjMxNy43SDYyNGM2LjcgMCAxMC40LTcuNyA2LjMtMTIuOUw1MTguMyAxNjNhOCA4IDAgMDAtMTIuNiAwbC0xMTIgMTQxLjdjLTQuMSA1LjMtLjQgMTMgNi4zIDEzek04NzggNjI2aC02MGMtNC40IDAtOCAzLjYtOCA4djE1NEgyMTRWNjM0YzAtNC40LTMuNi04LTgtOGgtNjBjLTQuNCAwLTggMy42LTggOHYxOThjMCAxNy43IDE0LjMgMzIgMzIgMzJoNjg0YzE3LjcgMCAzMi0xNC4zIDMyLTMyVjYzNGMwLTQuNC0zLjYtOC04LTh6IiAvPjwvc3ZnPg==) */
var RefIcon$37 = /*#__PURE__*/ React$1.forwardRef(UploadOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/UserOutlined.js
var UserOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M858.5 763.6a374 374 0 00-80.6-119.5 375.63 375.63 0 00-119.5-80.6c-.4-.2-.8-.3-1.2-.5C719.5 518 760 444.7 760 362c0-137-111-248-248-248S264 225 264 362c0 82.7 40.5 156 102.8 201.1-.4.2-.8.3-1.2.5-44.8 18.9-85 46-119.5 80.6a375.63 375.63 0 00-80.6 119.5A371.7 371.7 0 00136 901.8a8 8 0 008 8.2h60c4.4 0 7.9-3.5 8-7.8 2-77.2 33-149.5 87.8-204.3 56.7-56.7 132-87.9 212.2-87.9s155.5 31.2 212.2 87.9C779 752.7 810 825 812 902.2c.1 4.4 3.6 7.8 8 7.8h60a8 8 0 008-8.2c-1-47.8-10.9-94.3-29.5-138.2zM512 534c-45.9 0-89.1-17.9-121.6-50.4S340 407.9 340 362c0-45.9 17.9-89.1 50.4-121.6S466.1 190 512 190s89.1 17.9 121.6 50.4S684 316.1 684 362c0 45.9-17.9 89.1-50.4 121.6S557.9 534 512 534z" }
		}]
	},
	"name": "user",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/UserOutlined.js
function _extends$1() {
	_extends$1 = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends$1.apply(this, arguments);
}
var UserOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends$1({}, props, {
	ref,
	icon: UserOutlined$1
}));
/**![user](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTg1OC41IDc2My42YTM3NCAzNzQgMCAwMC04MC42LTExOS41IDM3NS42MyAzNzUuNjMgMCAwMC0xMTkuNS04MC42Yy0uNC0uMi0uOC0uMy0xLjItLjVDNzE5LjUgNTE4IDc2MCA0NDQuNyA3NjAgMzYyYzAtMTM3LTExMS0yNDgtMjQ4LTI0OFMyNjQgMjI1IDI2NCAzNjJjMCA4Mi43IDQwLjUgMTU2IDEwMi44IDIwMS4xLS40LjItLjguMy0xLjIuNS00NC44IDE4LjktODUgNDYtMTE5LjUgODAuNmEzNzUuNjMgMzc1LjYzIDAgMDAtODAuNiAxMTkuNUEzNzEuNyAzNzEuNyAwIDAwMTM2IDkwMS44YTggOCAwIDAwOCA4LjJoNjBjNC40IDAgNy45LTMuNSA4LTcuOCAyLTc3LjIgMzMtMTQ5LjUgODcuOC0yMDQuMyA1Ni43LTU2LjcgMTMyLTg3LjkgMjEyLjItODcuOXMxNTUuNSAzMS4yIDIxMi4yIDg3LjlDNzc5IDc1Mi43IDgxMCA4MjUgODEyIDkwMi4yYy4xIDQuNCAzLjYgNy44IDggNy44aDYwYTggOCAwIDAwOC04LjJjLTEtNDcuOC0xMC45LTk0LjMtMjkuNS0xMzguMnpNNTEyIDUzNGMtNDUuOSAwLTg5LjEtMTcuOS0xMjEuNi01MC40UzM0MCA0MDcuOSAzNDAgMzYyYzAtNDUuOSAxNy45LTg5LjEgNTAuNC0xMjEuNlM0NjYuMSAxOTAgNTEyIDE5MHM4OS4xIDE3LjkgMTIxLjYgNTAuNFM2ODQgMzE2LjEgNjg0IDM2MmMwIDQ1LjktMTcuOSA4OS4xLTUwLjQgMTIxLjZTNTU3LjkgNTM0IDUxMiA1MzR6IiAvPjwvc3ZnPg==) */
var RefIcon$38 = /*#__PURE__*/ React$1.forwardRef(UserOutlined);
//#endregion
//#region node_modules/.pnpm/@ant-design+icons-svg@4.5.0/node_modules/@ant-design/icons-svg/es/asn/WarningOutlined.js
var WarningOutlined$1 = {
	"icon": {
		"tag": "svg",
		"attrs": {
			"viewBox": "64 64 896 896",
			"focusable": "false"
		},
		"children": [{
			"tag": "path",
			"attrs": { "d": "M464 720a48 48 0 1096 0 48 48 0 10-96 0zm16-304v184c0 4.4 3.6 8 8 8h48c4.4 0 8-3.6 8-8V416c0-4.4-3.6-8-8-8h-48c-4.4 0-8 3.6-8 8zm475.7 440l-416-720c-6.2-10.7-16.9-16-27.7-16s-21.6 5.3-27.7 16l-416 720C56 877.4 71.4 904 96 904h832c24.6 0 40-26.6 27.7-48zm-783.5-27.9L512 239.9l339.8 588.2H172.2z" }
		}]
	},
	"name": "warning",
	"theme": "outlined"
};
//#endregion
//#region node_modules/.pnpm/@ant-design+icons@6.1.0_react-dom@19.2.5_react@19.2.5__react@19.2.5/node_modules/@ant-design/icons/es/icons/WarningOutlined.js
function _extends() {
	_extends = Object.assign ? Object.assign.bind() : function(target) {
		for (var i = 1; i < arguments.length; i++) {
			var source = arguments[i];
			for (var key in source) if (Object.prototype.hasOwnProperty.call(source, key)) target[key] = source[key];
		}
		return target;
	};
	return _extends.apply(this, arguments);
}
var WarningOutlined = (props, ref) => /*#__PURE__*/ React$1.createElement(Icon, _extends({}, props, {
	ref,
	icon: WarningOutlined$1
}));
/**![warning](data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAiIGhlaWdodD0iNTAiIGZpbGw9IiNjYWNhY2EiIHZpZXdCb3g9IjY0IDY0IDg5NiA4OTYiIGZvY3VzYWJsZT0iZmFsc2UiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHBhdGggZD0iTTQ2NCA3MjBhNDggNDggMCAxMDk2IDAgNDggNDggMCAxMC05NiAwem0xNi0zMDR2MTg0YzAgNC40IDMuNiA4IDggOGg0OGM0LjQgMCA4LTMuNiA4LThWNDE2YzAtNC40LTMuNi04LTgtOGgtNDhjLTQuNCAwLTggMy42LTggOHptNDc1LjcgNDQwbC00MTYtNzIwYy02LjItMTAuNy0xNi45LTE2LTI3LjctMTZzLTIxLjYgNS4zLTI3LjcgMTZsLTQxNiA3MjBDNTYgODc3LjQgNzEuNCA5MDQgOTYgOTA0aDgzMmMyNC42IDAgNDAtMjYuNiAyNy43LTQ4em0tNzgzLjUtMjcuOUw1MTIgMjM5LjlsMzM5LjggNTg4LjJIMTcyLjJ6IiAvPjwvc3ZnPg==) */
var RefIcon$39 = /*#__PURE__*/ React$1.forwardRef(WarningOutlined);
//#endregion
export { RefIcon as ArrowDownOutlined, RefIcon$1 as ArrowLeftOutlined, RefIcon$2 as ArrowRightOutlined, RefIcon$3 as ArrowUpOutlined, RefIcon$4 as CalendarOutlined, RefIcon$5 as CheckCircleOutlined, RefIcon$6 as CheckOutlined, RefIcon$7 as ClockCircleOutlined, RefIcon$8 as CloseCircleOutlined, RefIcon$9 as CloseOutlined, RefIcon$10 as CopyOutlined, RefIcon$11 as DeleteOutlined, RefIcon$12 as DownloadOutlined, RefIcon$13 as EditOutlined, RefIcon$14 as ExclamationCircleOutlined, RefIcon$15 as EyeInvisibleOutlined, RefIcon$16 as EyeOutlined, RefIcon$17 as FileOutlined, RefIcon$18 as FolderOpenOutlined, RefIcon$19 as FolderOutlined, RefIcon$20 as HomeOutlined, RefIcon$21 as InfoCircleOutlined, RefIcon$22 as LeftOutlined, RefIcon$23 as LinkOutlined, RefIcon$24 as LoadingOutlined, RefIcon$25 as LockOutlined, RefIcon$26 as MailOutlined, RefIcon$27 as MenuOutlined, RefIcon$28 as MinusOutlined, RefIcon$29 as MoreOutlined, RefIcon$30 as PlusOutlined, RefIcon$31 as QuestionCircleOutlined, RefIcon$32 as RightOutlined, RefIcon$33 as SaveOutlined, RefIcon$34 as SearchOutlined, RefIcon$35 as SettingOutlined, RefIcon$36 as UpOutlined, RefIcon$37 as UploadOutlined, RefIcon$38 as UserOutlined, RefIcon$39 as WarningOutlined };
