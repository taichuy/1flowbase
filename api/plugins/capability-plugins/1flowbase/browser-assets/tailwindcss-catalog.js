var e = Object.create, t = Object.defineProperty, n = Object.getOwnPropertyDescriptor, r = Object.getOwnPropertyNames, i = Object.getPrototypeOf, a = Object.prototype.hasOwnProperty, o = (e, t) => () => (t || (e((t = { exports: {} }).exports, t), e = null), t.exports), s = (e, i, o, s) => {
	if (i && typeof i == "object" || typeof i == "function") for (var c = r(i), l = 0, u = c.length, d; l < u; l++) d = c[l], !a.call(e, d) && d !== o && t(e, d, {
		get: ((e) => i[e]).bind(null, d),
		enumerable: !(s = n(i, d)) || s.enumerable
	});
	return e;
}, c = (n, r, a) => (a = n == null ? {} : e(i(n)), s(r || !n || !n.__esModule ? t(a, "default", {
	value: n,
	enumerable: !0
}) : a, n)), l;
(function(e) {
	e[e.NONE = 0] = "NONE", e[e._abstract = 1] = "_abstract", e[e._accessor = 2] = "_accessor", e[e._as = 3] = "_as", e[e._assert = 4] = "_assert", e[e._asserts = 5] = "_asserts", e[e._async = 6] = "_async", e[e._await = 7] = "_await", e[e._checks = 8] = "_checks", e[e._constructor = 9] = "_constructor", e[e._declare = 10] = "_declare", e[e._enum = 11] = "_enum", e[e._exports = 12] = "_exports", e[e._from = 13] = "_from", e[e._get = 14] = "_get", e[e._global = 15] = "_global", e[e._implements = 16] = "_implements", e[e._infer = 17] = "_infer", e[e._interface = 18] = "_interface", e[e._is = 19] = "_is", e[e._keyof = 20] = "_keyof", e[e._mixins = 21] = "_mixins", e[e._module = 22] = "_module", e[e._namespace = 23] = "_namespace", e[e._of = 24] = "_of", e[e._opaque = 25] = "_opaque", e[e._out = 26] = "_out", e[e._override = 27] = "_override", e[e._private = 28] = "_private", e[e._protected = 29] = "_protected", e[e._proto = 30] = "_proto", e[e._public = 31] = "_public", e[e._readonly = 32] = "_readonly", e[e._require = 33] = "_require", e[e._satisfies = 34] = "_satisfies", e[e._set = 35] = "_set", e[e._static = 36] = "_static", e[e._symbol = 37] = "_symbol", e[e._type = 38] = "_type", e[e._unique = 39] = "_unique", e[e._using = 40] = "_using";
})(l ||= {});
/* istanbul ignore file */
var u;
(function(e) {
	e[e.PRECEDENCE_MASK = 15] = "PRECEDENCE_MASK", e[e.IS_KEYWORD = 16] = "IS_KEYWORD", e[e.IS_ASSIGN = 32] = "IS_ASSIGN", e[e.IS_RIGHT_ASSOCIATIVE = 64] = "IS_RIGHT_ASSOCIATIVE", e[e.IS_PREFIX = 128] = "IS_PREFIX", e[e.IS_POSTFIX = 256] = "IS_POSTFIX", e[e.IS_EXPRESSION_START = 512] = "IS_EXPRESSION_START", e[e.num = 512] = "num", e[e.bigint = 1536] = "bigint", e[e.decimal = 2560] = "decimal", e[e.regexp = 3584] = "regexp", e[e.string = 4608] = "string", e[e.name = 5632] = "name", e[e.eof = 6144] = "eof", e[e.bracketL = 7680] = "bracketL", e[e.bracketR = 8192] = "bracketR", e[e.braceL = 9728] = "braceL", e[e.braceBarL = 10752] = "braceBarL", e[e.braceR = 11264] = "braceR", e[e.braceBarR = 12288] = "braceBarR", e[e.parenL = 13824] = "parenL", e[e.parenR = 14336] = "parenR", e[e.comma = 15360] = "comma", e[e.semi = 16384] = "semi", e[e.colon = 17408] = "colon", e[e.doubleColon = 18432] = "doubleColon", e[e.dot = 19456] = "dot", e[e.question = 20480] = "question", e[e.questionDot = 21504] = "questionDot", e[e.arrow = 22528] = "arrow", e[e.template = 23552] = "template", e[e.ellipsis = 24576] = "ellipsis", e[e.backQuote = 25600] = "backQuote", e[e.dollarBraceL = 27136] = "dollarBraceL", e[e.at = 27648] = "at", e[e.hash = 29184] = "hash", e[e.eq = 29728] = "eq", e[e.assign = 30752] = "assign", e[e.preIncDec = 32640] = "preIncDec", e[e.postIncDec = 33664] = "postIncDec", e[e.bang = 34432] = "bang", e[e.tilde = 35456] = "tilde", e[e.pipeline = 35841] = "pipeline", e[e.nullishCoalescing = 36866] = "nullishCoalescing", e[e.logicalOR = 37890] = "logicalOR", e[e.logicalAND = 38915] = "logicalAND", e[e.bitwiseOR = 39940] = "bitwiseOR", e[e.bitwiseXOR = 40965] = "bitwiseXOR", e[e.bitwiseAND = 41990] = "bitwiseAND", e[e.equality = 43015] = "equality", e[e.lessThan = 44040] = "lessThan", e[e.greaterThan = 45064] = "greaterThan", e[e.relationalOrEqual = 46088] = "relationalOrEqual", e[e.bitShiftL = 47113] = "bitShiftL", e[e.bitShiftR = 48137] = "bitShiftR", e[e.plus = 49802] = "plus", e[e.minus = 50826] = "minus", e[e.modulo = 51723] = "modulo", e[e.star = 52235] = "star", e[e.slash = 53259] = "slash", e[e.exponent = 54348] = "exponent", e[e.jsxName = 55296] = "jsxName", e[e.jsxText = 56320] = "jsxText", e[e.jsxEmptyText = 57344] = "jsxEmptyText", e[e.jsxTagStart = 58880] = "jsxTagStart", e[e.jsxTagEnd = 59392] = "jsxTagEnd", e[e.typeParameterStart = 60928] = "typeParameterStart", e[e.nonNullAssertion = 61440] = "nonNullAssertion", e[e._break = 62480] = "_break", e[e._case = 63504] = "_case", e[e._catch = 64528] = "_catch", e[e._continue = 65552] = "_continue", e[e._debugger = 66576] = "_debugger", e[e._default = 67600] = "_default", e[e._do = 68624] = "_do", e[e._else = 69648] = "_else", e[e._finally = 70672] = "_finally", e[e._for = 71696] = "_for", e[e._function = 73232] = "_function", e[e._if = 73744] = "_if", e[e._return = 74768] = "_return", e[e._switch = 75792] = "_switch", e[e._throw = 77456] = "_throw", e[e._try = 77840] = "_try", e[e._var = 78864] = "_var", e[e._let = 79888] = "_let", e[e._const = 80912] = "_const", e[e._while = 81936] = "_while", e[e._with = 82960] = "_with", e[e._new = 84496] = "_new", e[e._this = 85520] = "_this", e[e._super = 86544] = "_super", e[e._class = 87568] = "_class", e[e._extends = 88080] = "_extends", e[e._export = 89104] = "_export", e[e._import = 90640] = "_import", e[e._yield = 91664] = "_yield", e[e._null = 92688] = "_null", e[e._true = 93712] = "_true", e[e._false = 94736] = "_false", e[e._in = 95256] = "_in", e[e._instanceof = 96280] = "_instanceof", e[e._typeof = 97936] = "_typeof", e[e._void = 98960] = "_void", e[e._delete = 99984] = "_delete", e[e._async = 100880] = "_async", e[e._get = 101904] = "_get", e[e._set = 102928] = "_set", e[e._declare = 103952] = "_declare", e[e._readonly = 104976] = "_readonly", e[e._abstract = 106e3] = "_abstract", e[e._static = 107024] = "_static", e[e._public = 107536] = "_public", e[e._private = 108560] = "_private", e[e._protected = 109584] = "_protected", e[e._override = 110608] = "_override", e[e._as = 112144] = "_as", e[e._enum = 113168] = "_enum", e[e._type = 114192] = "_type", e[e._implements = 115216] = "_implements";
})(u ||= {});
function d(e) {
	switch (e) {
		case u.num: return "num";
		case u.bigint: return "bigint";
		case u.decimal: return "decimal";
		case u.regexp: return "regexp";
		case u.string: return "string";
		case u.name: return "name";
		case u.eof: return "eof";
		case u.bracketL: return "[";
		case u.bracketR: return "]";
		case u.braceL: return "{";
		case u.braceBarL: return "{|";
		case u.braceR: return "}";
		case u.braceBarR: return "|}";
		case u.parenL: return "(";
		case u.parenR: return ")";
		case u.comma: return ",";
		case u.semi: return ";";
		case u.colon: return ":";
		case u.doubleColon: return "::";
		case u.dot: return ".";
		case u.question: return "?";
		case u.questionDot: return "?.";
		case u.arrow: return "=>";
		case u.template: return "template";
		case u.ellipsis: return "...";
		case u.backQuote: return "`";
		case u.dollarBraceL: return "${";
		case u.at: return "@";
		case u.hash: return "#";
		case u.eq: return "=";
		case u.assign: return "_=";
		case u.preIncDec: return "++/--";
		case u.postIncDec: return "++/--";
		case u.bang: return "!";
		case u.tilde: return "~";
		case u.pipeline: return "|>";
		case u.nullishCoalescing: return "??";
		case u.logicalOR: return "||";
		case u.logicalAND: return "&&";
		case u.bitwiseOR: return "|";
		case u.bitwiseXOR: return "^";
		case u.bitwiseAND: return "&";
		case u.equality: return "==/!=";
		case u.lessThan: return "<";
		case u.greaterThan: return ">";
		case u.relationalOrEqual: return "<=/>=";
		case u.bitShiftL: return "<<";
		case u.bitShiftR: return ">>/>>>";
		case u.plus: return "+";
		case u.minus: return "-";
		case u.modulo: return "%";
		case u.star: return "*";
		case u.slash: return "/";
		case u.exponent: return "**";
		case u.jsxName: return "jsxName";
		case u.jsxText: return "jsxText";
		case u.jsxEmptyText: return "jsxEmptyText";
		case u.jsxTagStart: return "jsxTagStart";
		case u.jsxTagEnd: return "jsxTagEnd";
		case u.typeParameterStart: return "typeParameterStart";
		case u.nonNullAssertion: return "nonNullAssertion";
		case u._break: return "break";
		case u._case: return "case";
		case u._catch: return "catch";
		case u._continue: return "continue";
		case u._debugger: return "debugger";
		case u._default: return "default";
		case u._do: return "do";
		case u._else: return "else";
		case u._finally: return "finally";
		case u._for: return "for";
		case u._function: return "function";
		case u._if: return "if";
		case u._return: return "return";
		case u._switch: return "switch";
		case u._throw: return "throw";
		case u._try: return "try";
		case u._var: return "var";
		case u._let: return "let";
		case u._const: return "const";
		case u._while: return "while";
		case u._with: return "with";
		case u._new: return "new";
		case u._this: return "this";
		case u._super: return "super";
		case u._class: return "class";
		case u._extends: return "extends";
		case u._export: return "export";
		case u._import: return "import";
		case u._yield: return "yield";
		case u._null: return "null";
		case u._true: return "true";
		case u._false: return "false";
		case u._in: return "in";
		case u._instanceof: return "instanceof";
		case u._typeof: return "typeof";
		case u._void: return "void";
		case u._delete: return "delete";
		case u._async: return "async";
		case u._get: return "get";
		case u._set: return "set";
		case u._declare: return "declare";
		case u._readonly: return "readonly";
		case u._abstract: return "abstract";
		case u._static: return "static";
		case u._public: return "public";
		case u._private: return "private";
		case u._protected: return "protected";
		case u._override: return "override";
		case u._as: return "as";
		case u._enum: return "enum";
		case u._type: return "type";
		case u._implements: return "implements";
		default: return "";
	}
}
var f = class {
	constructor(e, t, n) {
		this.startTokenIndex = e, this.endTokenIndex = t, this.isFunctionScope = n;
	}
}, p = class {
	constructor(e, t, n, r, i, a, o, s, c, l, u, d, f) {
		this.potentialArrowAt = e, this.noAnonFunctionType = t, this.inDisallowConditionalTypesContext = n, this.tokensLength = r, this.scopesLength = i, this.pos = a, this.type = o, this.contextualKeyword = s, this.start = c, this.end = l, this.isType = u, this.scopeDepth = d, this.error = f;
	}
}, m = class e {
	constructor() {
		e.prototype.__init.call(this), e.prototype.__init2.call(this), e.prototype.__init3.call(this), e.prototype.__init4.call(this), e.prototype.__init5.call(this), e.prototype.__init6.call(this), e.prototype.__init7.call(this), e.prototype.__init8.call(this), e.prototype.__init9.call(this), e.prototype.__init10.call(this), e.prototype.__init11.call(this), e.prototype.__init12.call(this), e.prototype.__init13.call(this);
	}
	__init() {
		this.potentialArrowAt = -1;
	}
	__init2() {
		this.noAnonFunctionType = !1;
	}
	__init3() {
		this.inDisallowConditionalTypesContext = !1;
	}
	__init4() {
		this.tokens = [];
	}
	__init5() {
		this.scopes = [];
	}
	__init6() {
		this.pos = 0;
	}
	__init7() {
		this.type = u.eof;
	}
	__init8() {
		this.contextualKeyword = l.NONE;
	}
	__init9() {
		this.start = 0;
	}
	__init10() {
		this.end = 0;
	}
	__init11() {
		this.isType = !1;
	}
	__init12() {
		this.scopeDepth = 0;
	}
	__init13() {
		this.error = null;
	}
	snapshot() {
		return new p(this.potentialArrowAt, this.noAnonFunctionType, this.inDisallowConditionalTypesContext, this.tokens.length, this.scopes.length, this.pos, this.type, this.contextualKeyword, this.start, this.end, this.isType, this.scopeDepth, this.error);
	}
	restoreFromSnapshot(e) {
		this.potentialArrowAt = e.potentialArrowAt, this.noAnonFunctionType = e.noAnonFunctionType, this.inDisallowConditionalTypesContext = e.inDisallowConditionalTypesContext, this.tokens.length = e.tokensLength, this.scopes.length = e.scopesLength, this.pos = e.pos, this.type = e.type, this.contextualKeyword = e.contextualKeyword, this.start = e.start, this.end = e.end, this.isType = e.isType, this.scopeDepth = e.scopeDepth, this.error = e.error;
	}
}, h;
(function(e) {
	e[e.backSpace = 8] = "backSpace", e[e.lineFeed = 10] = "lineFeed", e[e.tab = 9] = "tab", e[e.carriageReturn = 13] = "carriageReturn", e[e.shiftOut = 14] = "shiftOut", e[e.space = 32] = "space", e[e.exclamationMark = 33] = "exclamationMark", e[e.quotationMark = 34] = "quotationMark", e[e.numberSign = 35] = "numberSign", e[e.dollarSign = 36] = "dollarSign", e[e.percentSign = 37] = "percentSign", e[e.ampersand = 38] = "ampersand", e[e.apostrophe = 39] = "apostrophe", e[e.leftParenthesis = 40] = "leftParenthesis", e[e.rightParenthesis = 41] = "rightParenthesis", e[e.asterisk = 42] = "asterisk", e[e.plusSign = 43] = "plusSign", e[e.comma = 44] = "comma", e[e.dash = 45] = "dash", e[e.dot = 46] = "dot", e[e.slash = 47] = "slash", e[e.digit0 = 48] = "digit0", e[e.digit1 = 49] = "digit1", e[e.digit2 = 50] = "digit2", e[e.digit3 = 51] = "digit3", e[e.digit4 = 52] = "digit4", e[e.digit5 = 53] = "digit5", e[e.digit6 = 54] = "digit6", e[e.digit7 = 55] = "digit7", e[e.digit8 = 56] = "digit8", e[e.digit9 = 57] = "digit9", e[e.colon = 58] = "colon", e[e.semicolon = 59] = "semicolon", e[e.lessThan = 60] = "lessThan", e[e.equalsTo = 61] = "equalsTo", e[e.greaterThan = 62] = "greaterThan", e[e.questionMark = 63] = "questionMark", e[e.atSign = 64] = "atSign", e[e.uppercaseA = 65] = "uppercaseA", e[e.uppercaseB = 66] = "uppercaseB", e[e.uppercaseC = 67] = "uppercaseC", e[e.uppercaseD = 68] = "uppercaseD", e[e.uppercaseE = 69] = "uppercaseE", e[e.uppercaseF = 70] = "uppercaseF", e[e.uppercaseG = 71] = "uppercaseG", e[e.uppercaseH = 72] = "uppercaseH", e[e.uppercaseI = 73] = "uppercaseI", e[e.uppercaseJ = 74] = "uppercaseJ", e[e.uppercaseK = 75] = "uppercaseK", e[e.uppercaseL = 76] = "uppercaseL", e[e.uppercaseM = 77] = "uppercaseM", e[e.uppercaseN = 78] = "uppercaseN", e[e.uppercaseO = 79] = "uppercaseO", e[e.uppercaseP = 80] = "uppercaseP", e[e.uppercaseQ = 81] = "uppercaseQ", e[e.uppercaseR = 82] = "uppercaseR", e[e.uppercaseS = 83] = "uppercaseS", e[e.uppercaseT = 84] = "uppercaseT", e[e.uppercaseU = 85] = "uppercaseU", e[e.uppercaseV = 86] = "uppercaseV", e[e.uppercaseW = 87] = "uppercaseW", e[e.uppercaseX = 88] = "uppercaseX", e[e.uppercaseY = 89] = "uppercaseY", e[e.uppercaseZ = 90] = "uppercaseZ", e[e.leftSquareBracket = 91] = "leftSquareBracket", e[e.backslash = 92] = "backslash", e[e.rightSquareBracket = 93] = "rightSquareBracket", e[e.caret = 94] = "caret", e[e.underscore = 95] = "underscore", e[e.graveAccent = 96] = "graveAccent", e[e.lowercaseA = 97] = "lowercaseA", e[e.lowercaseB = 98] = "lowercaseB", e[e.lowercaseC = 99] = "lowercaseC", e[e.lowercaseD = 100] = "lowercaseD", e[e.lowercaseE = 101] = "lowercaseE", e[e.lowercaseF = 102] = "lowercaseF", e[e.lowercaseG = 103] = "lowercaseG", e[e.lowercaseH = 104] = "lowercaseH", e[e.lowercaseI = 105] = "lowercaseI", e[e.lowercaseJ = 106] = "lowercaseJ", e[e.lowercaseK = 107] = "lowercaseK", e[e.lowercaseL = 108] = "lowercaseL", e[e.lowercaseM = 109] = "lowercaseM", e[e.lowercaseN = 110] = "lowercaseN", e[e.lowercaseO = 111] = "lowercaseO", e[e.lowercaseP = 112] = "lowercaseP", e[e.lowercaseQ = 113] = "lowercaseQ", e[e.lowercaseR = 114] = "lowercaseR", e[e.lowercaseS = 115] = "lowercaseS", e[e.lowercaseT = 116] = "lowercaseT", e[e.lowercaseU = 117] = "lowercaseU", e[e.lowercaseV = 118] = "lowercaseV", e[e.lowercaseW = 119] = "lowercaseW", e[e.lowercaseX = 120] = "lowercaseX", e[e.lowercaseY = 121] = "lowercaseY", e[e.lowercaseZ = 122] = "lowercaseZ", e[e.leftCurlyBrace = 123] = "leftCurlyBrace", e[e.verticalBar = 124] = "verticalBar", e[e.rightCurlyBrace = 125] = "rightCurlyBrace", e[e.tilde = 126] = "tilde", e[e.nonBreakingSpace = 160] = "nonBreakingSpace", e[e.oghamSpaceMark = 5760] = "oghamSpaceMark", e[e.lineSeparator = 8232] = "lineSeparator", e[e.paragraphSeparator = 8233] = "paragraphSeparator";
})(h ||= {});
var g, _, v, y, b, ee;
function te() {
	return ee++;
}
function ne(e) {
	if ("pos" in e) {
		let t = ie(e.pos);
		e.message += ` (${t.line}:${t.column})`, e.loc = t;
	}
	return e;
}
var re = class {
	constructor(e, t) {
		this.line = e, this.column = t;
	}
};
function ie(e) {
	let t = 1, n = 1;
	for (let r = 0; r < e; r++) b.charCodeAt(r) === h.lineFeed ? (t++, n = 1) : n++;
	return new re(t, n);
}
function ae(e, t, n, r) {
	b = e, y = new m(), ee = 1, g = t, _ = n, v = r;
}
function x(e) {
	return y.contextualKeyword === e;
}
function oe(e) {
	let t = Ee();
	return t.type === u.name && t.contextualKeyword === e;
}
function S(e) {
	return y.contextualKeyword === e && I(u.name);
}
function C(e) {
	S(e) || k();
}
function w() {
	return L(u.eof) || L(u.braceR) || T();
}
function T() {
	let e = y.tokens[y.tokens.length - 1], t = e ? e.end : 0;
	for (let e = t; e < y.start; e++) {
		let t = b.charCodeAt(e);
		if (t === h.lineFeed || t === h.carriageReturn || t === 8232 || t === 8233) return !0;
	}
	return !1;
}
function se() {
	let e = De();
	for (let t = y.end; t < e; t++) {
		let e = b.charCodeAt(t);
		if (e === h.lineFeed || e === h.carriageReturn || e === 8232 || e === 8233) return !0;
	}
	return !1;
}
function E() {
	return I(u.semi) || w();
}
function D() {
	E() || k("Unexpected token, expected \";\"");
}
function O(e) {
	I(e) || k(`Unexpected token, expected "${d(e)}"`);
}
function k(e = "Unexpected token", t = y.start) {
	if (y.error) return;
	let n = SyntaxError(e);
	n.pos = t, y.error = n, y.pos = b.length, z(u.eof);
}
var ce = [
	9,
	11,
	12,
	h.space,
	h.nonBreakingSpace,
	h.oghamSpaceMark,
	8192,
	8193,
	8194,
	8195,
	8196,
	8197,
	8198,
	8199,
	8200,
	8201,
	8202,
	8239,
	8287,
	12288,
	65279
], le = /(?:\s|\/\/.*|\/\*[^]*?\*\/)*/g, ue = new Uint8Array(65536);
for (let e of ce) ue[e] = 1;
function de(e) {
	if (e < 48) return e === 36;
	if (e < 58) return !0;
	if (e < 65) return !1;
	if (e < 91) return !0;
	if (e < 97) return e === 95;
	if (e < 123) return !0;
	if (e < 128) return !1;
	throw Error("Should not be called with non-ASCII char code.");
}
var A = new Uint8Array(65536);
for (let e = 0; e < 128; e++) A[e] = +!!de(e);
for (let e = 128; e < 65536; e++) A[e] = 1;
for (let e of ce) A[e] = 0;
A[8232] = 0, A[8233] = 0;
var fe = A.slice();
for (let e = h.digit0; e <= h.digit9; e++) fe[e] = 0;
var pe = new Int32Array([
	-1,
	27,
	783,
	918,
	1755,
	2376,
	2862,
	3483,
	-1,
	3699,
	-1,
	4617,
	4752,
	4833,
	5130,
	5508,
	5940,
	-1,
	6480,
	6939,
	7749,
	8181,
	8451,
	8613,
	-1,
	8829,
	-1,
	-1,
	-1,
	54,
	243,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	432,
	-1,
	-1,
	-1,
	675,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	81,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	108,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	135,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	162,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	189,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	216,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._abstract << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	270,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	297,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	324,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	351,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	378,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	405,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._accessor << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._as << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	459,
	-1,
	-1,
	-1,
	-1,
	-1,
	594,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	486,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	513,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	540,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._assert << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	567,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._asserts << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	621,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	648,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._async << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	702,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	729,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	756,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._await << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	810,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	837,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	864,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	891,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._break << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	945,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1107,
	-1,
	-1,
	-1,
	1242,
	-1,
	-1,
	1350,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	972,
	1026,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	999,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._case << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1053,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1080,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._catch << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1134,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1161,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1188,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1215,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._checks << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1269,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1296,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1323,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._class << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1377,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1404,
	1620,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1431,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._const << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1458,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1485,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1512,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1539,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1566,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1593,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._constructor << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1647,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1674,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1701,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1728,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._continue << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1782,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2349,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1809,
	1971,
	-1,
	-1,
	2106,
	-1,
	-1,
	-1,
	-1,
	-1,
	2241,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1836,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1863,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1890,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1917,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1944,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._debugger << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	1998,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2025,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2052,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2079,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._declare << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2133,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2160,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2187,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2214,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._default << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2268,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2295,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2322,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._delete << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._do << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2403,
	-1,
	2484,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2565,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2430,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2457,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._else << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2511,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2538,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._enum << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2592,
	-1,
	-1,
	-1,
	2727,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2619,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2646,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2673,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._export << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2700,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._exports << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2754,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2781,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2808,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2835,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._extends << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2889,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2997,
	-1,
	-1,
	-1,
	-1,
	-1,
	3159,
	-1,
	-1,
	3213,
	-1,
	-1,
	3294,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2916,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2943,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	2970,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._false << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3024,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3051,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3078,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3105,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3132,
	-1,
	(u._finally << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3186,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._for << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3240,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3267,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._from << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3321,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3348,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3375,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3402,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3429,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3456,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._function << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3510,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3564,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3537,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._get << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3591,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3618,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3645,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3672,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._global << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3726,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3753,
	4077,
	-1,
	-1,
	-1,
	-1,
	4590,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._if << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3780,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3807,
	-1,
	-1,
	3996,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3834,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3861,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3888,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3915,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3942,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	3969,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._implements << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4023,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4050,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._import << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._in << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4104,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4185,
	4401,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4131,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4158,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._infer << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4212,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4239,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4266,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4293,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4320,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4347,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4374,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._instanceof << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4428,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4455,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4482,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4509,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4536,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4563,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._interface << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._is << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4644,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4671,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4698,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4725,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._keyof << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4779,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4806,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._let << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4860,
	-1,
	-1,
	-1,
	-1,
	-1,
	4995,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4887,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4914,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4941,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	4968,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._mixins << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5022,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5049,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5076,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5103,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._module << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5157,
	-1,
	-1,
	-1,
	5373,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5427,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5184,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5211,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5238,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5265,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5292,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5319,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5346,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._namespace << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5400,
	-1,
	-1,
	-1,
	(u._new << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5454,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5481,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._null << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5535,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5562,
	-1,
	-1,
	-1,
	-1,
	5697,
	5751,
	-1,
	-1,
	-1,
	-1,
	l._of << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5589,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5616,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5643,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5670,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._opaque << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5724,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._out << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5778,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5805,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5832,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5859,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5886,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5913,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._override << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5967,
	-1,
	-1,
	6345,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	5994,
	-1,
	-1,
	-1,
	-1,
	-1,
	6129,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6021,
	-1,
	-1,
	-1,
	-1,
	-1,
	6048,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6075,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6102,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._private << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6156,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6183,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6318,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6210,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6237,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6264,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6291,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._protected << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._proto << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6372,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6399,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6426,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6453,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._public << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6507,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6534,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6696,
	-1,
	-1,
	6831,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6561,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6588,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6615,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6642,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6669,
	-1,
	l._readonly << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6723,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6750,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6777,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6804,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._require << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6858,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6885,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6912,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._return << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6966,
	-1,
	-1,
	-1,
	7182,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7236,
	7371,
	-1,
	7479,
	-1,
	7614,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	6993,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7020,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7047,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7074,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7101,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7128,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7155,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._satisfies << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7209,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._set << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7263,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7290,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7317,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7344,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._static << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7398,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7425,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7452,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._super << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7506,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7533,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7560,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7587,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._switch << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7641,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7668,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7695,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7722,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._symbol << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7776,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7938,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8046,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7803,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7857,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7830,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._this << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7884,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7911,
	-1,
	-1,
	-1,
	(u._throw << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7965,
	-1,
	-1,
	-1,
	8019,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	7992,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._true << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._try << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8073,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8100,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._type << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8127,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8154,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._typeof << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8208,
	-1,
	-1,
	-1,
	-1,
	8343,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8235,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8262,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8289,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8316,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._unique << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8370,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8397,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8424,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	l._using << 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8478,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8532,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8505,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._var << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8559,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8586,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._void << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8640,
	8748,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8667,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8694,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8721,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._while << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8775,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8802,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._with << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8856,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8883,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8910,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	8937,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	(u._yield << 1) + 1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1,
	-1
]);
function me() {
	let e = 0, t = 0, n = y.pos;
	for (; n < b.length && (t = b.charCodeAt(n), !(t < h.lowercaseA || t > h.lowercaseZ));) {
		let r = pe[e + (t - h.lowercaseA) + 1];
		if (r === -1) break;
		e = r, n++;
	}
	let r = pe[e];
	if (r > -1 && !A[t]) {
		y.pos = n, r & 1 ? z(r >>> 1) : z(u.name, r >>> 1);
		return;
	}
	for (; n < b.length;) {
		let e = b.charCodeAt(n);
		if (A[e]) n++;
		else if (e === h.backslash) {
			if (n += 2, b.charCodeAt(n) === h.leftCurlyBrace) {
				for (; n < b.length && b.charCodeAt(n) !== h.rightCurlyBrace;) n++;
				n++;
			}
		} else if (e === h.atSign && b.charCodeAt(n + 1) === h.atSign) n += 2;
		else break;
	}
	y.pos = n, z(u.name);
}
var j;
(function(e) {
	e[e.Access = 0] = "Access", e[e.ExportAccess = 1] = "ExportAccess", e[e.TopLevelDeclaration = 2] = "TopLevelDeclaration", e[e.FunctionScopedDeclaration = 3] = "FunctionScopedDeclaration", e[e.BlockScopedDeclaration = 4] = "BlockScopedDeclaration", e[e.ObjectShorthandTopLevelDeclaration = 5] = "ObjectShorthandTopLevelDeclaration", e[e.ObjectShorthandFunctionScopedDeclaration = 6] = "ObjectShorthandFunctionScopedDeclaration", e[e.ObjectShorthandBlockScopedDeclaration = 7] = "ObjectShorthandBlockScopedDeclaration", e[e.ObjectShorthand = 8] = "ObjectShorthand", e[e.ImportDeclaration = 9] = "ImportDeclaration", e[e.ObjectKey = 10] = "ObjectKey", e[e.ImportAccess = 11] = "ImportAccess";
})(j ||= {});
var M;
(function(e) {
	e[e.NoChildren = 0] = "NoChildren", e[e.OneChild = 1] = "OneChild", e[e.StaticChildren = 2] = "StaticChildren", e[e.KeyAfterPropSpread = 3] = "KeyAfterPropSpread";
})(M ||= {});
function he(e) {
	let t = e.identifierRole;
	return t === j.TopLevelDeclaration || t === j.FunctionScopedDeclaration || t === j.BlockScopedDeclaration || t === j.ObjectShorthandTopLevelDeclaration || t === j.ObjectShorthandFunctionScopedDeclaration || t === j.ObjectShorthandBlockScopedDeclaration;
}
function ge(e) {
	let t = e.identifierRole;
	return t === j.FunctionScopedDeclaration || t === j.BlockScopedDeclaration || t === j.ObjectShorthandFunctionScopedDeclaration || t === j.ObjectShorthandBlockScopedDeclaration;
}
function _e(e) {
	let t = e.identifierRole;
	return t === j.TopLevelDeclaration || t === j.ObjectShorthandTopLevelDeclaration || t === j.ImportDeclaration;
}
function ve(e) {
	let t = e.identifierRole;
	return t === j.TopLevelDeclaration || t === j.BlockScopedDeclaration || t === j.ObjectShorthandTopLevelDeclaration || t === j.ObjectShorthandBlockScopedDeclaration;
}
function ye(e) {
	let t = e.identifierRole;
	return t === j.FunctionScopedDeclaration || t === j.ObjectShorthandFunctionScopedDeclaration;
}
function be(e) {
	return e.identifierRole === j.ObjectShorthandTopLevelDeclaration || e.identifierRole === j.ObjectShorthandBlockScopedDeclaration || e.identifierRole === j.ObjectShorthandFunctionScopedDeclaration;
}
var xe = class {
	constructor() {
		this.type = y.type, this.contextualKeyword = y.contextualKeyword, this.start = y.start, this.end = y.end, this.scopeDepth = y.scopeDepth, this.isType = y.isType, this.identifierRole = null, this.jsxRole = null, this.shadowsGlobal = !1, this.isAsyncOperation = !1, this.contextId = null, this.rhsEndIndex = null, this.isExpression = !1, this.numNullishCoalesceStarts = 0, this.numNullishCoalesceEnds = 0, this.isOptionalChainStart = !1, this.isOptionalChainEnd = !1, this.subscriptStartIndex = null, this.nullishStartIndex = null;
	}
};
function N() {
	y.tokens.push(new xe()), Ae();
}
function Se() {
	y.tokens.push(new xe()), y.start = y.pos, Qe();
}
function Ce() {
	y.type === u.assign && --y.pos, qe();
}
function P(e) {
	for (let t = y.tokens.length - e; t < y.tokens.length; t++) y.tokens[t].isType = !0;
	let t = y.isType;
	return y.isType = !0, t;
}
function F(e) {
	y.isType = e;
}
function I(e) {
	return L(e) ? (N(), !0) : !1;
}
function we(e) {
	let t = y.isType;
	y.isType = !0, I(e), y.isType = t;
}
function L(e) {
	return y.type === e;
}
function R() {
	let e = y.snapshot();
	N();
	let t = y.type;
	return y.restoreFromSnapshot(e), t;
}
var Te = class {
	constructor(e, t) {
		this.type = e, this.contextualKeyword = t;
	}
};
function Ee() {
	let e = y.snapshot();
	N();
	let t = y.type, n = y.contextualKeyword;
	return y.restoreFromSnapshot(e), new Te(t, n);
}
function De() {
	return Oe(y.pos);
}
function Oe(e) {
	return le.lastIndex = e, e + le.exec(b)[0].length;
}
function ke() {
	return b.charCodeAt(De());
}
function Ae() {
	if (Pe(), y.start = y.pos, y.pos >= b.length) {
		let e = y.tokens;
		e.length >= 2 && e[e.length - 1].start >= b.length && e[e.length - 2].start >= b.length && k("Unexpectedly reached the end of input."), z(u.eof);
		return;
	}
	je(b.charCodeAt(y.pos));
}
function je(e) {
	fe[e] || e === h.backslash || e === h.atSign && b.charCodeAt(y.pos + 1) === h.atSign ? me() : Ke(e);
}
function Me() {
	for (; b.charCodeAt(y.pos) !== h.asterisk || b.charCodeAt(y.pos + 1) !== h.slash;) if (y.pos++, y.pos > b.length) {
		k("Unterminated comment", y.pos - 2);
		return;
	}
	y.pos += 2;
}
function Ne(e) {
	let t = b.charCodeAt(y.pos += e);
	if (y.pos < b.length) for (; t !== h.lineFeed && t !== h.carriageReturn && t !== h.lineSeparator && t !== h.paragraphSeparator && ++y.pos < b.length;) t = b.charCodeAt(y.pos);
}
function Pe() {
	for (; y.pos < b.length;) {
		let e = b.charCodeAt(y.pos);
		switch (e) {
			case h.carriageReturn: b.charCodeAt(y.pos + 1) === h.lineFeed && ++y.pos;
			case h.lineFeed:
			case h.lineSeparator:
			case h.paragraphSeparator:
				++y.pos;
				break;
			case h.slash:
				switch (b.charCodeAt(y.pos + 1)) {
					case h.asterisk:
						y.pos += 2, Me();
						break;
					case h.slash:
						Ne(2);
						break;
					default: return;
				}
				break;
			default: if (ue[e]) ++y.pos;
			else return;
		}
	}
}
function z(e, t = l.NONE) {
	y.end = y.pos, y.type = e, y.contextualKeyword = t;
}
function Fe() {
	let e = b.charCodeAt(y.pos + 1);
	if (e >= h.digit0 && e <= h.digit9) {
		Xe(!0);
		return;
	}
	e === h.dot && b.charCodeAt(y.pos + 2) === h.dot ? (y.pos += 3, z(u.ellipsis)) : (++y.pos, z(u.dot));
}
function Ie() {
	b.charCodeAt(y.pos + 1) === h.equalsTo ? B(u.assign, 2) : B(u.slash, 1);
}
function Le(e) {
	let t = e === h.asterisk ? u.star : u.modulo, n = 1, r = b.charCodeAt(y.pos + 1);
	e === h.asterisk && r === h.asterisk && (n++, r = b.charCodeAt(y.pos + 2), t = u.exponent), r === h.equalsTo && b.charCodeAt(y.pos + 2) !== h.greaterThan && (n++, t = u.assign), B(t, n);
}
function Re(e) {
	let t = b.charCodeAt(y.pos + 1);
	if (t === e) {
		b.charCodeAt(y.pos + 2) === h.equalsTo ? B(u.assign, 3) : B(e === h.verticalBar ? u.logicalOR : u.logicalAND, 2);
		return;
	}
	if (e === h.verticalBar) {
		if (t === h.greaterThan) {
			B(u.pipeline, 2);
			return;
		} else if (t === h.rightCurlyBrace && v) {
			B(u.braceBarR, 2);
			return;
		}
	}
	if (t === h.equalsTo) {
		B(u.assign, 2);
		return;
	}
	B(e === h.verticalBar ? u.bitwiseOR : u.bitwiseAND, 1);
}
function ze() {
	b.charCodeAt(y.pos + 1) === h.equalsTo ? B(u.assign, 2) : B(u.bitwiseXOR, 1);
}
function Be(e) {
	let t = b.charCodeAt(y.pos + 1);
	if (t === e) {
		B(u.preIncDec, 2);
		return;
	}
	t === h.equalsTo ? B(u.assign, 2) : e === h.plusSign ? B(u.plus, 1) : B(u.minus, 1);
}
function Ve() {
	let e = b.charCodeAt(y.pos + 1);
	if (e === h.lessThan) {
		if (b.charCodeAt(y.pos + 2) === h.equalsTo) {
			B(u.assign, 3);
			return;
		}
		y.isType ? B(u.lessThan, 1) : B(u.bitShiftL, 2);
		return;
	}
	e === h.equalsTo ? B(u.relationalOrEqual, 2) : B(u.lessThan, 1);
}
function He() {
	if (y.isType) {
		B(u.greaterThan, 1);
		return;
	}
	let e = b.charCodeAt(y.pos + 1);
	if (e === h.greaterThan) {
		let e = b.charCodeAt(y.pos + 2) === h.greaterThan ? 3 : 2;
		if (b.charCodeAt(y.pos + e) === h.equalsTo) {
			B(u.assign, e + 1);
			return;
		}
		B(u.bitShiftR, e);
		return;
	}
	e === h.equalsTo ? B(u.relationalOrEqual, 2) : B(u.greaterThan, 1);
}
function Ue() {
	y.type === u.greaterThan && (--y.pos, He());
}
function We(e) {
	let t = b.charCodeAt(y.pos + 1);
	if (t === h.equalsTo) {
		B(u.equality, b.charCodeAt(y.pos + 2) === h.equalsTo ? 3 : 2);
		return;
	}
	if (e === h.equalsTo && t === h.greaterThan) {
		y.pos += 2, z(u.arrow);
		return;
	}
	B(e === h.equalsTo ? u.eq : u.bang, 1);
}
function Ge() {
	let e = b.charCodeAt(y.pos + 1), t = b.charCodeAt(y.pos + 2);
	e === h.questionMark && !(v && y.isType) ? t === h.equalsTo ? B(u.assign, 3) : B(u.nullishCoalescing, 2) : e === h.dot && !(t >= h.digit0 && t <= h.digit9) ? (y.pos += 2, z(u.questionDot)) : (++y.pos, z(u.question));
}
function Ke(e) {
	switch (e) {
		case h.numberSign:
			++y.pos, z(u.hash);
			return;
		case h.dot:
			Fe();
			return;
		case h.leftParenthesis:
			++y.pos, z(u.parenL);
			return;
		case h.rightParenthesis:
			++y.pos, z(u.parenR);
			return;
		case h.semicolon:
			++y.pos, z(u.semi);
			return;
		case h.comma:
			++y.pos, z(u.comma);
			return;
		case h.leftSquareBracket:
			++y.pos, z(u.bracketL);
			return;
		case h.rightSquareBracket:
			++y.pos, z(u.bracketR);
			return;
		case h.leftCurlyBrace:
			v && b.charCodeAt(y.pos + 1) === h.verticalBar ? B(u.braceBarL, 2) : (++y.pos, z(u.braceL));
			return;
		case h.rightCurlyBrace:
			++y.pos, z(u.braceR);
			return;
		case h.colon:
			b.charCodeAt(y.pos + 1) === h.colon ? B(u.doubleColon, 2) : (++y.pos, z(u.colon));
			return;
		case h.questionMark:
			Ge();
			return;
		case h.atSign:
			++y.pos, z(u.at);
			return;
		case h.graveAccent:
			++y.pos, z(u.backQuote);
			return;
		case h.digit0: {
			let e = b.charCodeAt(y.pos + 1);
			if (e === h.lowercaseX || e === h.uppercaseX || e === h.lowercaseO || e === h.uppercaseO || e === h.lowercaseB || e === h.uppercaseB) {
				Ye();
				return;
			}
		}
		case h.digit1:
		case h.digit2:
		case h.digit3:
		case h.digit4:
		case h.digit5:
		case h.digit6:
		case h.digit7:
		case h.digit8:
		case h.digit9:
			Xe(!1);
			return;
		case h.quotationMark:
		case h.apostrophe:
			Ze(e);
			return;
		case h.slash:
			Ie();
			return;
		case h.percentSign:
		case h.asterisk:
			Le(e);
			return;
		case h.verticalBar:
		case h.ampersand:
			Re(e);
			return;
		case h.caret:
			ze();
			return;
		case h.plusSign:
		case h.dash:
			Be(e);
			return;
		case h.lessThan:
			Ve();
			return;
		case h.greaterThan:
			He();
			return;
		case h.equalsTo:
		case h.exclamationMark:
			We(e);
			return;
		case h.tilde:
			B(u.tilde, 1);
			return;
		default: break;
	}
	k(`Unexpected character '${String.fromCharCode(e)}'`, y.pos);
}
function B(e, t) {
	y.pos += t, z(e);
}
function qe() {
	let e = y.pos, t = !1, n = !1;
	for (;;) {
		if (y.pos >= b.length) {
			k("Unterminated regular expression", e);
			return;
		}
		let r = b.charCodeAt(y.pos);
		if (t) t = !1;
		else {
			if (r === h.leftSquareBracket) n = !0;
			else if (r === h.rightSquareBracket && n) n = !1;
			else if (r === h.slash && !n) break;
			t = r === h.backslash;
		}
		++y.pos;
	}
	++y.pos, $e(), z(u.regexp);
}
function Je() {
	for (;;) {
		let e = b.charCodeAt(y.pos);
		if (e >= h.digit0 && e <= h.digit9 || e === h.underscore) y.pos++;
		else break;
	}
}
function Ye() {
	for (y.pos += 2;;) {
		let e = b.charCodeAt(y.pos);
		if (e >= h.digit0 && e <= h.digit9 || e >= h.lowercaseA && e <= h.lowercaseF || e >= h.uppercaseA && e <= h.uppercaseF || e === h.underscore) y.pos++;
		else break;
	}
	b.charCodeAt(y.pos) === h.lowercaseN ? (++y.pos, z(u.bigint)) : z(u.num);
}
function Xe(e) {
	let t = !1, n = !1;
	e || Je();
	let r = b.charCodeAt(y.pos);
	if (r === h.dot && (++y.pos, Je(), r = b.charCodeAt(y.pos)), (r === h.uppercaseE || r === h.lowercaseE) && (r = b.charCodeAt(++y.pos), (r === h.plusSign || r === h.dash) && ++y.pos, Je(), r = b.charCodeAt(y.pos)), r === h.lowercaseN ? (++y.pos, t = !0) : r === h.lowercaseM && (++y.pos, n = !0), t) {
		z(u.bigint);
		return;
	}
	if (n) {
		z(u.decimal);
		return;
	}
	z(u.num);
}
function Ze(e) {
	for (y.pos++;;) {
		if (y.pos >= b.length) {
			k("Unterminated string constant");
			return;
		}
		let t = b.charCodeAt(y.pos);
		if (t === h.backslash) y.pos++;
		else if (t === e) break;
		y.pos++;
	}
	y.pos++, z(u.string);
}
function Qe() {
	for (;;) {
		if (y.pos >= b.length) {
			k("Unterminated template");
			return;
		}
		let e = b.charCodeAt(y.pos);
		if (e === h.graveAccent || e === h.dollarSign && b.charCodeAt(y.pos + 1) === h.leftCurlyBrace) {
			if (y.pos === y.start && L(u.template)) if (e === h.dollarSign) {
				y.pos += 2, z(u.dollarBraceL);
				return;
			} else {
				++y.pos, z(u.backQuote);
				return;
			}
			z(u.template);
			return;
		}
		e === h.backslash && y.pos++, y.pos++;
	}
}
function $e() {
	for (; y.pos < b.length;) {
		let e = b.charCodeAt(y.pos);
		if (A[e]) y.pos++;
		else if (e === h.backslash) {
			if (y.pos += 2, b.charCodeAt(y.pos) === h.leftCurlyBrace) {
				for (; y.pos < b.length && b.charCodeAt(y.pos) !== h.rightCurlyBrace;) y.pos++;
				y.pos++;
			}
		} else break;
	}
}
function et(e, t = e.currentIndex()) {
	let n = t + 1;
	if (tt(e, n)) {
		let r = e.identifierNameAtIndex(t);
		return {
			isType: !1,
			leftName: r,
			rightName: r,
			endIndex: n
		};
	}
	if (n++, tt(e, n)) return {
		isType: !0,
		leftName: null,
		rightName: null,
		endIndex: n
	};
	if (n++, tt(e, n)) return {
		isType: !1,
		leftName: e.identifierNameAtIndex(t),
		rightName: e.identifierNameAtIndex(t + 2),
		endIndex: n
	};
	if (n++, tt(e, n)) return {
		isType: !0,
		leftName: null,
		rightName: null,
		endIndex: n
	};
	throw Error(`Unexpected import/export specifier at ${t}`);
}
function tt(e, t) {
	let n = e.tokens[t];
	return n.type === u.braceR || n.type === u.comma;
}
var nt = new Map([
	["quot", "\""],
	["amp", "&"],
	["apos", "'"],
	["lt", "<"],
	["gt", ">"],
	["nbsp", "\xA0"],
	["iexcl", "¡"],
	["cent", "¢"],
	["pound", "£"],
	["curren", "¤"],
	["yen", "¥"],
	["brvbar", "¦"],
	["sect", "§"],
	["uml", "¨"],
	["copy", "©"],
	["ordf", "ª"],
	["laquo", "«"],
	["not", "¬"],
	["shy", "­"],
	["reg", "®"],
	["macr", "¯"],
	["deg", "°"],
	["plusmn", "±"],
	["sup2", "²"],
	["sup3", "³"],
	["acute", "´"],
	["micro", "µ"],
	["para", "¶"],
	["middot", "·"],
	["cedil", "¸"],
	["sup1", "¹"],
	["ordm", "º"],
	["raquo", "»"],
	["frac14", "¼"],
	["frac12", "½"],
	["frac34", "¾"],
	["iquest", "¿"],
	["Agrave", "À"],
	["Aacute", "Á"],
	["Acirc", "Â"],
	["Atilde", "Ã"],
	["Auml", "Ä"],
	["Aring", "Å"],
	["AElig", "Æ"],
	["Ccedil", "Ç"],
	["Egrave", "È"],
	["Eacute", "É"],
	["Ecirc", "Ê"],
	["Euml", "Ë"],
	["Igrave", "Ì"],
	["Iacute", "Í"],
	["Icirc", "Î"],
	["Iuml", "Ï"],
	["ETH", "Ð"],
	["Ntilde", "Ñ"],
	["Ograve", "Ò"],
	["Oacute", "Ó"],
	["Ocirc", "Ô"],
	["Otilde", "Õ"],
	["Ouml", "Ö"],
	["times", "×"],
	["Oslash", "Ø"],
	["Ugrave", "Ù"],
	["Uacute", "Ú"],
	["Ucirc", "Û"],
	["Uuml", "Ü"],
	["Yacute", "Ý"],
	["THORN", "Þ"],
	["szlig", "ß"],
	["agrave", "à"],
	["aacute", "á"],
	["acirc", "â"],
	["atilde", "ã"],
	["auml", "ä"],
	["aring", "å"],
	["aelig", "æ"],
	["ccedil", "ç"],
	["egrave", "è"],
	["eacute", "é"],
	["ecirc", "ê"],
	["euml", "ë"],
	["igrave", "ì"],
	["iacute", "í"],
	["icirc", "î"],
	["iuml", "ï"],
	["eth", "ð"],
	["ntilde", "ñ"],
	["ograve", "ò"],
	["oacute", "ó"],
	["ocirc", "ô"],
	["otilde", "õ"],
	["ouml", "ö"],
	["divide", "÷"],
	["oslash", "ø"],
	["ugrave", "ù"],
	["uacute", "ú"],
	["ucirc", "û"],
	["uuml", "ü"],
	["yacute", "ý"],
	["thorn", "þ"],
	["yuml", "ÿ"],
	["OElig", "Œ"],
	["oelig", "œ"],
	["Scaron", "Š"],
	["scaron", "š"],
	["Yuml", "Ÿ"],
	["fnof", "ƒ"],
	["circ", "ˆ"],
	["tilde", "˜"],
	["Alpha", "Α"],
	["Beta", "Β"],
	["Gamma", "Γ"],
	["Delta", "Δ"],
	["Epsilon", "Ε"],
	["Zeta", "Ζ"],
	["Eta", "Η"],
	["Theta", "Θ"],
	["Iota", "Ι"],
	["Kappa", "Κ"],
	["Lambda", "Λ"],
	["Mu", "Μ"],
	["Nu", "Ν"],
	["Xi", "Ξ"],
	["Omicron", "Ο"],
	["Pi", "Π"],
	["Rho", "Ρ"],
	["Sigma", "Σ"],
	["Tau", "Τ"],
	["Upsilon", "Υ"],
	["Phi", "Φ"],
	["Chi", "Χ"],
	["Psi", "Ψ"],
	["Omega", "Ω"],
	["alpha", "α"],
	["beta", "β"],
	["gamma", "γ"],
	["delta", "δ"],
	["epsilon", "ε"],
	["zeta", "ζ"],
	["eta", "η"],
	["theta", "θ"],
	["iota", "ι"],
	["kappa", "κ"],
	["lambda", "λ"],
	["mu", "μ"],
	["nu", "ν"],
	["xi", "ξ"],
	["omicron", "ο"],
	["pi", "π"],
	["rho", "ρ"],
	["sigmaf", "ς"],
	["sigma", "σ"],
	["tau", "τ"],
	["upsilon", "υ"],
	["phi", "φ"],
	["chi", "χ"],
	["psi", "ψ"],
	["omega", "ω"],
	["thetasym", "ϑ"],
	["upsih", "ϒ"],
	["piv", "ϖ"],
	["ensp", " "],
	["emsp", " "],
	["thinsp", " "],
	["zwnj", "‌"],
	["zwj", "‍"],
	["lrm", "‎"],
	["rlm", "‏"],
	["ndash", "–"],
	["mdash", "—"],
	["lsquo", "‘"],
	["rsquo", "’"],
	["sbquo", "‚"],
	["ldquo", "“"],
	["rdquo", "”"],
	["bdquo", "„"],
	["dagger", "†"],
	["Dagger", "‡"],
	["bull", "•"],
	["hellip", "…"],
	["permil", "‰"],
	["prime", "′"],
	["Prime", "″"],
	["lsaquo", "‹"],
	["rsaquo", "›"],
	["oline", "‾"],
	["frasl", "⁄"],
	["euro", "€"],
	["image", "ℑ"],
	["weierp", "℘"],
	["real", "ℜ"],
	["trade", "™"],
	["alefsym", "ℵ"],
	["larr", "←"],
	["uarr", "↑"],
	["rarr", "→"],
	["darr", "↓"],
	["harr", "↔"],
	["crarr", "↵"],
	["lArr", "⇐"],
	["uArr", "⇑"],
	["rArr", "⇒"],
	["dArr", "⇓"],
	["hArr", "⇔"],
	["forall", "∀"],
	["part", "∂"],
	["exist", "∃"],
	["empty", "∅"],
	["nabla", "∇"],
	["isin", "∈"],
	["notin", "∉"],
	["ni", "∋"],
	["prod", "∏"],
	["sum", "∑"],
	["minus", "−"],
	["lowast", "∗"],
	["radic", "√"],
	["prop", "∝"],
	["infin", "∞"],
	["ang", "∠"],
	["and", "∧"],
	["or", "∨"],
	["cap", "∩"],
	["cup", "∪"],
	["int", "∫"],
	["there4", "∴"],
	["sim", "∼"],
	["cong", "≅"],
	["asymp", "≈"],
	["ne", "≠"],
	["equiv", "≡"],
	["le", "≤"],
	["ge", "≥"],
	["sub", "⊂"],
	["sup", "⊃"],
	["nsub", "⊄"],
	["sube", "⊆"],
	["supe", "⊇"],
	["oplus", "⊕"],
	["otimes", "⊗"],
	["perp", "⊥"],
	["sdot", "⋅"],
	["lceil", "⌈"],
	["rceil", "⌉"],
	["lfloor", "⌊"],
	["rfloor", "⌋"],
	["lang", "〈"],
	["rang", "〉"],
	["loz", "◊"],
	["spades", "♠"],
	["clubs", "♣"],
	["hearts", "♥"],
	["diams", "♦"]
]);
function rt(e) {
	let [t, n] = it(e.jsxPragma || "React.createElement"), [r, i] = it(e.jsxFragmentPragma || "React.Fragment");
	return {
		base: t,
		suffix: n,
		fragmentBase: r,
		fragmentSuffix: i
	};
}
function it(e) {
	let t = e.indexOf(".");
	return t === -1 && (t = e.length), [e.slice(0, t), e.slice(t)];
}
var V = class {
	getPrefixCode() {
		return "";
	}
	getHoistedCode() {
		return "";
	}
	getSuffixCode() {
		return "";
	}
}, at = class e extends V {
	__init() {
		this.lastLineNumber = 1;
	}
	__init2() {
		this.lastIndex = 0;
	}
	__init3() {
		this.filenameVarName = null;
	}
	__init4() {
		this.esmAutomaticImportNameResolutions = {};
	}
	__init5() {
		this.cjsAutomaticModuleNameResolutions = {};
	}
	constructor(t, n, r, i, a) {
		super(), this.rootTransformer = t, this.tokens = n, this.importProcessor = r, this.nameManager = i, this.options = a, e.prototype.__init.call(this), e.prototype.__init2.call(this), e.prototype.__init3.call(this), e.prototype.__init4.call(this), e.prototype.__init5.call(this), this.jsxPragmaInfo = rt(a), this.isAutomaticRuntime = a.jsxRuntime === "automatic", this.jsxImportSource = a.jsxImportSource || "react";
	}
	process() {
		return this.tokens.matches1(u.jsxTagStart) ? (this.processJSXTag(), !0) : !1;
	}
	getPrefixCode() {
		let e = "";
		if (this.filenameVarName && (e += `const ${this.filenameVarName} = ${JSON.stringify(this.options.filePath || "")};`), this.isAutomaticRuntime) if (this.importProcessor) for (let [t, n] of Object.entries(this.cjsAutomaticModuleNameResolutions)) e += `var ${n} = require("${t}");`;
		else {
			let { createElement: t, ...n } = this.esmAutomaticImportNameResolutions;
			t && (e += `import {createElement as ${t}} from "${this.jsxImportSource}";`);
			let r = Object.entries(n).map(([e, t]) => `${e} as ${t}`).join(", ");
			if (r) {
				let t = this.jsxImportSource + (this.options.production ? "/jsx-runtime" : "/jsx-dev-runtime");
				e += `import {${r}} from "${t}";`;
			}
		}
		return e;
	}
	processJSXTag() {
		let { jsxRole: e, start: t } = this.tokens.currentToken(), n = this.options.production ? null : this.getElementLocationCode(t);
		this.isAutomaticRuntime && e !== M.KeyAfterPropSpread ? this.transformTagToJSXFunc(n, e) : this.transformTagToCreateElement(n);
	}
	getElementLocationCode(e) {
		return `lineNumber: ${this.getLineNumberForIndex(e)}`;
	}
	getLineNumberForIndex(e) {
		let t = this.tokens.code;
		for (; this.lastIndex < e && this.lastIndex < t.length;) t[this.lastIndex] === "\n" && this.lastLineNumber++, this.lastIndex++;
		return this.lastLineNumber;
	}
	transformTagToJSXFunc(e, t) {
		let n = t === M.StaticChildren;
		this.tokens.replaceToken(this.getJSXFuncInvocationCode(n));
		let r = null;
		if (this.tokens.matches1(u.jsxTagEnd)) this.tokens.replaceToken(`${this.getFragmentCode()}, {`), this.processAutomaticChildrenAndEndProps(t);
		else {
			if (this.processTagIntro(), this.tokens.appendCode(", {"), r = this.processProps(!0), this.tokens.matches2(u.slash, u.jsxTagEnd)) this.tokens.appendCode("}");
			else if (this.tokens.matches1(u.jsxTagEnd)) this.tokens.removeToken(), this.processAutomaticChildrenAndEndProps(t);
			else throw Error("Expected either /> or > at the end of the tag.");
			r && this.tokens.appendCode(`, ${r}`);
		}
		for (this.options.production || (r === null && this.tokens.appendCode(", void 0"), this.tokens.appendCode(`, ${n}, ${this.getDevSource(e)}, this`)), this.tokens.removeInitialToken(); !this.tokens.matches1(u.jsxTagEnd);) this.tokens.removeToken();
		this.tokens.replaceToken(")");
	}
	transformTagToCreateElement(e) {
		if (this.tokens.replaceToken(this.getCreateElementInvocationCode()), this.tokens.matches1(u.jsxTagEnd)) this.tokens.replaceToken(`${this.getFragmentCode()}, null`), this.processChildren(!0);
		else if (this.processTagIntro(), this.processPropsObjectWithDevInfo(e), !this.tokens.matches2(u.slash, u.jsxTagEnd)) if (this.tokens.matches1(u.jsxTagEnd)) this.tokens.removeToken(), this.processChildren(!0);
		else throw Error("Expected either /> or > at the end of the tag.");
		for (this.tokens.removeInitialToken(); !this.tokens.matches1(u.jsxTagEnd);) this.tokens.removeToken();
		this.tokens.replaceToken(")");
	}
	getJSXFuncInvocationCode(e) {
		return this.options.production ? e ? this.claimAutoImportedFuncInvocation("jsxs", "/jsx-runtime") : this.claimAutoImportedFuncInvocation("jsx", "/jsx-runtime") : this.claimAutoImportedFuncInvocation("jsxDEV", "/jsx-dev-runtime");
	}
	getCreateElementInvocationCode() {
		if (this.isAutomaticRuntime) return this.claimAutoImportedFuncInvocation("createElement", "");
		{
			let { jsxPragmaInfo: e } = this;
			return `${this.importProcessor && this.importProcessor.getIdentifierReplacement(e.base) || e.base}${e.suffix}(`;
		}
	}
	getFragmentCode() {
		if (this.isAutomaticRuntime) return this.claimAutoImportedName("Fragment", this.options.production ? "/jsx-runtime" : "/jsx-dev-runtime");
		{
			let { jsxPragmaInfo: e } = this;
			return (this.importProcessor && this.importProcessor.getIdentifierReplacement(e.fragmentBase) || e.fragmentBase) + e.fragmentSuffix;
		}
	}
	claimAutoImportedFuncInvocation(e, t) {
		let n = this.claimAutoImportedName(e, t);
		return this.importProcessor ? `${n}.call(void 0, ` : `${n}(`;
	}
	claimAutoImportedName(e, t) {
		if (this.importProcessor) {
			let n = this.jsxImportSource + t;
			return this.cjsAutomaticModuleNameResolutions[n] || (this.cjsAutomaticModuleNameResolutions[n] = this.importProcessor.getFreeIdentifierForPath(n)), `${this.cjsAutomaticModuleNameResolutions[n]}.${e}`;
		} else return this.esmAutomaticImportNameResolutions[e] || (this.esmAutomaticImportNameResolutions[e] = this.nameManager.claimFreeName(`_${e}`)), this.esmAutomaticImportNameResolutions[e];
	}
	processTagIntro() {
		let e = this.tokens.currentIndex() + 1;
		for (; this.tokens.tokens[e].isType || !this.tokens.matches2AtIndex(e - 1, u.jsxName, u.jsxName) && !this.tokens.matches2AtIndex(e - 1, u.greaterThan, u.jsxName) && !this.tokens.matches1AtIndex(e, u.braceL) && !this.tokens.matches1AtIndex(e, u.jsxTagEnd) && !this.tokens.matches2AtIndex(e, u.slash, u.jsxTagEnd);) e++;
		if (e === this.tokens.currentIndex() + 1) {
			let e = this.tokens.identifierName();
			ot(e) && this.tokens.replaceToken(`'${e}'`);
		}
		for (; this.tokens.currentIndex() < e;) this.rootTransformer.processToken();
	}
	processPropsObjectWithDevInfo(e) {
		let t = this.options.production ? "" : `__self: this, __source: ${this.getDevSource(e)}`;
		if (!this.tokens.matches1(u.jsxName) && !this.tokens.matches1(u.braceL)) {
			t ? this.tokens.appendCode(`, {${t}}`) : this.tokens.appendCode(", null");
			return;
		}
		this.tokens.appendCode(", {"), this.processProps(!1), t ? this.tokens.appendCode(` ${t}}`) : this.tokens.appendCode("}");
	}
	processProps(e) {
		let t = null;
		for (;;) {
			if (this.tokens.matches2(u.jsxName, u.eq)) {
				let n = this.tokens.identifierName();
				if (e && n === "key") {
					t !== null && this.tokens.appendCode(t.replace(/[^\n]/g, "")), this.tokens.removeToken(), this.tokens.removeToken();
					let e = this.tokens.snapshot();
					this.processPropValue(), t = this.tokens.dangerouslyGetAndRemoveCodeSinceSnapshot(e);
					continue;
				} else this.processPropName(n), this.tokens.replaceToken(": "), this.processPropValue();
			} else if (this.tokens.matches1(u.jsxName)) {
				let e = this.tokens.identifierName();
				this.processPropName(e), this.tokens.appendCode(": true");
			} else if (this.tokens.matches1(u.braceL)) this.tokens.replaceToken(""), this.rootTransformer.processBalancedCode(), this.tokens.replaceToken("");
			else break;
			this.tokens.appendCode(",");
		}
		return t;
	}
	processPropName(e) {
		e.includes("-") ? this.tokens.replaceToken(`'${e}'`) : this.tokens.copyToken();
	}
	processPropValue() {
		this.tokens.matches1(u.braceL) ? (this.tokens.replaceToken(""), this.rootTransformer.processBalancedCode(), this.tokens.replaceToken("")) : this.tokens.matches1(u.jsxTagStart) ? this.processJSXTag() : this.processStringPropValue();
	}
	processStringPropValue() {
		let e = this.tokens.currentToken(), t = this.tokens.code.slice(e.start + 1, e.end - 1), n = ct(t), r = lt(t);
		this.tokens.replaceToken(r + n);
	}
	processAutomaticChildrenAndEndProps(e) {
		e === M.StaticChildren ? (this.tokens.appendCode(" children: ["), this.processChildren(!1), this.tokens.appendCode("]}")) : (e === M.OneChild && this.tokens.appendCode(" children: "), this.processChildren(!1), this.tokens.appendCode("}"));
	}
	processChildren(e) {
		let t = e;
		for (;;) {
			if (this.tokens.matches2(u.jsxTagStart, u.slash)) return;
			let e = !1;
			if (this.tokens.matches1(u.braceL)) this.tokens.matches2(u.braceL, u.braceR) ? (this.tokens.replaceToken(""), this.tokens.replaceToken("")) : (this.tokens.replaceToken(t ? ", " : ""), this.rootTransformer.processBalancedCode(), this.tokens.replaceToken(""), e = !0);
			else if (this.tokens.matches1(u.jsxTagStart)) this.tokens.appendCode(t ? ", " : ""), this.processJSXTag(), e = !0;
			else if (this.tokens.matches1(u.jsxText) || this.tokens.matches1(u.jsxEmptyText)) e = this.processChildTextElement(t);
			else throw Error("Unexpected token when processing JSX children.");
			e && (t = !0);
		}
	}
	processChildTextElement(e) {
		let t = this.tokens.currentToken(), n = this.tokens.code.slice(t.start, t.end), r = ct(n), i = st(n);
		return i === "\"\"" ? (this.tokens.replaceToken(r), !1) : (this.tokens.replaceToken(`${e ? ", " : ""}${i}${r}`), !0);
	}
	getDevSource(e) {
		return `{fileName: ${this.getFilenameVarName()}, ${e}}`;
	}
	getFilenameVarName() {
		return this.filenameVarName ||= this.nameManager.claimFreeName("_jsxFileName"), this.filenameVarName;
	}
};
function ot(e) {
	let t = e.charCodeAt(0);
	return t >= h.lowercaseA && t <= h.lowercaseZ;
}
function st(e) {
	let t = "", n = "", r = !1, i = !1;
	for (let a = 0; a < e.length; a++) {
		let o = e[a];
		if (o === " " || o === "	" || o === "\r") r || (n += o);
		else if (o === "\n") n = "", r = !0;
		else {
			if (i && r && (t += " "), t += n, n = "", o === "&") {
				let { entity: n, newI: r } = ut(e, a + 1);
				a = r - 1, t += n;
			} else t += o;
			i = !0, r = !1;
		}
	}
	return r || (t += n), JSON.stringify(t);
}
function ct(e) {
	let t = 0, n = 0;
	for (let r of e) r === "\n" ? (t++, n = 0) : r === " " && n++;
	return "\n".repeat(t) + " ".repeat(n);
}
function lt(e) {
	let t = "";
	for (let n = 0; n < e.length; n++) {
		let r = e[n];
		if (r === "\n") if (/\s/.test(e[n + 1])) for (t += " "; n < e.length && /\s/.test(e[n + 1]);) n++;
		else t += "\n";
		else if (r === "&") {
			let { entity: r, newI: i } = ut(e, n + 1);
			t += r, n = i - 1;
		} else t += r;
	}
	return JSON.stringify(t);
}
function ut(e, t) {
	let n = "", r = 0, i, a = t;
	if (e[a] === "#") {
		let t = 10;
		a++;
		let n;
		if (e[a] === "x") for (t = 16, a++, n = a; a < e.length && ft(e.charCodeAt(a));) a++;
		else for (n = a; a < e.length && dt(e.charCodeAt(a));) a++;
		if (e[a] === ";") {
			let r = e.slice(n, a);
			r && (a++, i = String.fromCodePoint(parseInt(r, t)));
		}
	} else for (; a < e.length && r++ < 10;) {
		let t = e[a];
		if (a++, t === ";") {
			i = nt.get(n);
			break;
		}
		n += t;
	}
	return i ? {
		entity: i,
		newI: a
	} : {
		entity: "&",
		newI: t
	};
}
function dt(e) {
	return e >= h.digit0 && e <= h.digit9;
}
function ft(e) {
	return e >= h.digit0 && e <= h.digit9 || e >= h.lowercaseA && e <= h.lowercaseF || e >= h.uppercaseA && e <= h.uppercaseF;
}
function pt(e, t) {
	let n = rt(t), r = /* @__PURE__ */ new Set();
	for (let t = 0; t < e.tokens.length; t++) {
		let i = e.tokens[t];
		i.type === u.name && !i.isType && (i.identifierRole === j.Access || i.identifierRole === j.ObjectShorthand || i.identifierRole === j.ExportAccess) && !i.shadowsGlobal && r.add(e.identifierNameForToken(i)), i.type === u.jsxTagStart && r.add(n.base), i.type === u.jsxTagStart && t + 1 < e.tokens.length && e.tokens[t + 1].type === u.jsxTagEnd && (r.add(n.base), r.add(n.fragmentBase)), i.type === u.jsxName && i.identifierRole === j.Access && (!ot(e.identifierNameForToken(i)) || e.tokens[t + 1].type === u.dot) && r.add(e.identifierNameForToken(i));
	}
	return r;
}
var mt = class e {
	__init() {
		this.nonTypeIdentifiers = /* @__PURE__ */ new Set();
	}
	__init2() {
		this.importInfoByPath = /* @__PURE__ */ new Map();
	}
	__init3() {
		this.importsToReplace = /* @__PURE__ */ new Map();
	}
	__init4() {
		this.identifierReplacements = /* @__PURE__ */ new Map();
	}
	__init5() {
		this.exportBindingsByLocalName = /* @__PURE__ */ new Map();
	}
	constructor(t, n, r, i, a, o, s) {
		this.nameManager = t, this.tokens = n, this.enableLegacyTypeScriptModuleInterop = r, this.options = i, this.isTypeScriptTransformEnabled = a, this.keepUnusedImports = o, this.helperManager = s, e.prototype.__init.call(this), e.prototype.__init2.call(this), e.prototype.__init3.call(this), e.prototype.__init4.call(this), e.prototype.__init5.call(this);
	}
	preprocessTokens() {
		for (let e = 0; e < this.tokens.tokens.length; e++) this.tokens.matches1AtIndex(e, u._import) && !this.tokens.matches3AtIndex(e, u._import, u.name, u.eq) && this.preprocessImportAtIndex(e), this.tokens.matches1AtIndex(e, u._export) && !this.tokens.matches2AtIndex(e, u._export, u.eq) && this.preprocessExportAtIndex(e);
		this.generateImportReplacements();
	}
	pruneTypeOnlyImports() {
		this.nonTypeIdentifiers = pt(this.tokens, this.options);
		for (let [e, t] of this.importInfoByPath.entries()) t.hasBareImport || t.hasStarExport || t.exportStarNames.length > 0 || t.namedExports.length > 0 || [
			...t.defaultNames,
			...t.wildcardNames,
			...t.namedImports.map(({ localName: e }) => e)
		].every((e) => this.shouldAutomaticallyElideImportedName(e)) && this.importsToReplace.set(e, "");
	}
	shouldAutomaticallyElideImportedName(e) {
		return this.isTypeScriptTransformEnabled && !this.keepUnusedImports && !this.nonTypeIdentifiers.has(e);
	}
	generateImportReplacements() {
		for (let [e, t] of this.importInfoByPath.entries()) {
			let { defaultNames: n, wildcardNames: r, namedImports: i, namedExports: a, exportStarNames: o, hasStarExport: s } = t;
			if (n.length === 0 && r.length === 0 && i.length === 0 && a.length === 0 && o.length === 0 && !s) {
				this.importsToReplace.set(e, `require('${e}');`);
				continue;
			}
			let c = this.getFreeIdentifierForPath(e), l;
			l = this.enableLegacyTypeScriptModuleInterop ? c : r.length > 0 ? r[0] : this.getFreeIdentifierForPath(e);
			let u = `var ${c} = require('${e}');`;
			if (r.length > 0) for (let e of r) {
				let t = this.enableLegacyTypeScriptModuleInterop ? c : `${this.helperManager.getHelperName("interopRequireWildcard")}(${c})`;
				u += ` var ${e} = ${t};`;
			}
			else o.length > 0 && l !== c ? u += ` var ${l} = ${this.helperManager.getHelperName("interopRequireWildcard")}(${c});` : n.length > 0 && l !== c && (u += ` var ${l} = ${this.helperManager.getHelperName("interopRequireDefault")}(${c});`);
			for (let { importedName: e, localName: t } of a) u += ` ${this.helperManager.getHelperName("createNamedExportFrom")}(${c}, '${t}', '${e}');`;
			for (let e of o) u += ` exports.${e} = ${l};`;
			s && (u += ` ${this.helperManager.getHelperName("createStarExport")}(${c});`), this.importsToReplace.set(e, u);
			for (let e of n) this.identifierReplacements.set(e, `${l}.default`);
			for (let { importedName: e, localName: t } of i) this.identifierReplacements.set(t, `${c}.${e}`);
		}
	}
	getFreeIdentifierForPath(e) {
		let t = e.split("/"), n = t[t.length - 1].replace(/\W/g, "");
		return this.nameManager.claimFreeName(`_${n}`);
	}
	preprocessImportAtIndex(e) {
		let t = [], n = [], r = [];
		if (e++, (this.tokens.matchesContextualAtIndex(e, l._type) || this.tokens.matches1AtIndex(e, u._typeof)) && !this.tokens.matches1AtIndex(e + 1, u.comma) && !this.tokens.matchesContextualAtIndex(e + 1, l._from) || this.tokens.matches1AtIndex(e, u.parenL)) return;
		if (this.tokens.matches1AtIndex(e, u.name) && (t.push(this.tokens.identifierNameAtIndex(e)), e++, this.tokens.matches1AtIndex(e, u.comma) && e++), this.tokens.matches1AtIndex(e, u.star) && (e += 2, n.push(this.tokens.identifierNameAtIndex(e)), e++), this.tokens.matches1AtIndex(e, u.braceL)) {
			let n = this.getNamedImports(e + 1);
			e = n.newIndex;
			for (let e of n.namedImports) e.importedName === "default" ? t.push(e.localName) : r.push(e);
		}
		if (this.tokens.matchesContextualAtIndex(e, l._from) && e++, !this.tokens.matches1AtIndex(e, u.string)) throw Error("Expected string token at the end of import statement.");
		let i = this.tokens.stringValueAtIndex(e), a = this.getImportInfo(i);
		a.defaultNames.push(...t), a.wildcardNames.push(...n), a.namedImports.push(...r), t.length === 0 && n.length === 0 && r.length === 0 && (a.hasBareImport = !0);
	}
	preprocessExportAtIndex(e) {
		if (this.tokens.matches2AtIndex(e, u._export, u._var) || this.tokens.matches2AtIndex(e, u._export, u._let) || this.tokens.matches2AtIndex(e, u._export, u._const)) this.preprocessVarExportAtIndex(e);
		else if (this.tokens.matches2AtIndex(e, u._export, u._function) || this.tokens.matches2AtIndex(e, u._export, u._class)) {
			let t = this.tokens.identifierNameAtIndex(e + 2);
			this.addExportBinding(t, t);
		} else if (this.tokens.matches3AtIndex(e, u._export, u.name, u._function)) {
			let t = this.tokens.identifierNameAtIndex(e + 3);
			this.addExportBinding(t, t);
		} else this.tokens.matches2AtIndex(e, u._export, u.braceL) ? this.preprocessNamedExportAtIndex(e) : this.tokens.matches2AtIndex(e, u._export, u.star) && this.preprocessExportStarAtIndex(e);
	}
	preprocessVarExportAtIndex(e) {
		let t = 0;
		for (let n = e + 2;; n++) if (this.tokens.matches1AtIndex(n, u.braceL) || this.tokens.matches1AtIndex(n, u.dollarBraceL) || this.tokens.matches1AtIndex(n, u.bracketL)) t++;
		else if (this.tokens.matches1AtIndex(n, u.braceR) || this.tokens.matches1AtIndex(n, u.bracketR)) t--;
		else if (t === 0 && !this.tokens.matches1AtIndex(n, u.name)) break;
		else if (this.tokens.matches1AtIndex(1, u.eq)) {
			let e = this.tokens.currentToken().rhsEndIndex;
			if (e == null) throw Error("Expected = token with an end index.");
			n = e - 1;
		} else {
			let e = this.tokens.tokens[n];
			if (he(e)) {
				let e = this.tokens.identifierNameAtIndex(n);
				this.identifierReplacements.set(e, `exports.${e}`);
			}
		}
	}
	preprocessNamedExportAtIndex(e) {
		e += 2;
		let { newIndex: t, namedImports: n } = this.getNamedImports(e);
		if (e = t, this.tokens.matchesContextualAtIndex(e, l._from)) e++;
		else {
			for (let { importedName: e, localName: t } of n) this.addExportBinding(e, t);
			return;
		}
		if (!this.tokens.matches1AtIndex(e, u.string)) throw Error("Expected string token at the end of import statement.");
		let r = this.tokens.stringValueAtIndex(e);
		this.getImportInfo(r).namedExports.push(...n);
	}
	preprocessExportStarAtIndex(e) {
		let t = null;
		if (this.tokens.matches3AtIndex(e, u._export, u.star, u._as) ? (e += 3, t = this.tokens.identifierNameAtIndex(e), e += 2) : e += 3, !this.tokens.matches1AtIndex(e, u.string)) throw Error("Expected string token at the end of star export statement.");
		let n = this.tokens.stringValueAtIndex(e), r = this.getImportInfo(n);
		t === null ? r.hasStarExport = !0 : r.exportStarNames.push(t);
	}
	getNamedImports(e) {
		let t = [];
		for (;;) {
			if (this.tokens.matches1AtIndex(e, u.braceR)) {
				e++;
				break;
			}
			let n = et(this.tokens, e);
			if (e = n.endIndex, n.isType || t.push({
				importedName: n.leftName,
				localName: n.rightName
			}), this.tokens.matches2AtIndex(e, u.comma, u.braceR)) {
				e += 2;
				break;
			} else if (this.tokens.matches1AtIndex(e, u.braceR)) {
				e++;
				break;
			} else if (this.tokens.matches1AtIndex(e, u.comma)) e++;
			else throw Error(`Unexpected token: ${JSON.stringify(this.tokens.tokens[e])}`);
		}
		return {
			newIndex: e,
			namedImports: t
		};
	}
	getImportInfo(e) {
		let t = this.importInfoByPath.get(e);
		if (t) return t;
		let n = {
			defaultNames: [],
			wildcardNames: [],
			namedImports: [],
			namedExports: [],
			hasBareImport: !1,
			exportStarNames: [],
			hasStarExport: !1
		};
		return this.importInfoByPath.set(e, n), n;
	}
	addExportBinding(e, t) {
		this.exportBindingsByLocalName.has(e) || this.exportBindingsByLocalName.set(e, []), this.exportBindingsByLocalName.get(e).push(t);
	}
	claimImportCode(e) {
		let t = this.importsToReplace.get(e);
		return this.importsToReplace.set(e, ""), t || "";
	}
	getIdentifierReplacement(e) {
		return this.identifierReplacements.get(e) || null;
	}
	resolveExportBinding(e) {
		let t = this.exportBindingsByLocalName.get(e);
		return !t || t.length === 0 ? null : t.map((e) => `exports.${e}`).join(" = ");
	}
	getGlobalNames() {
		return new Set([...this.identifierReplacements.keys(), ...this.exportBindingsByLocalName.keys()]);
	}
}, ht = 44, gt = 59, _t = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/", vt = new Uint8Array(64), yt = new Uint8Array(128);
for (let e = 0; e < _t.length; e++) {
	let t = _t.charCodeAt(e);
	vt[e] = t, yt[t] = e;
}
function bt(e, t, n) {
	let r = t - n;
	r = r < 0 ? -r << 1 | 1 : r << 1;
	do {
		let t = r & 31;
		r >>>= 5, r > 0 && (t |= 32), e.write(vt[t]);
	} while (r > 0);
	return t;
}
var xt = 1024 * 16, St = typeof TextDecoder < "u" ? /* @__PURE__ */ new TextDecoder() : typeof Buffer < "u" ? { decode(e) {
	return Buffer.from(e.buffer, e.byteOffset, e.byteLength).toString();
} } : { decode(e) {
	let t = "";
	for (let n = 0; n < e.length; n++) t += String.fromCharCode(e[n]);
	return t;
} }, Ct = class {
	constructor() {
		this.pos = 0, this.out = "", this.buffer = new Uint8Array(xt);
	}
	write(e) {
		let { buffer: t } = this;
		t[this.pos++] = e, this.pos === xt && (this.out += St.decode(t), this.pos = 0);
	}
	flush() {
		let { buffer: e, out: t, pos: n } = this;
		return n > 0 ? t + St.decode(e.subarray(0, n)) : t;
	}
};
function wt(e) {
	let t = new Ct(), n = 0, r = 0, i = 0, a = 0;
	for (let o = 0; o < e.length; o++) {
		let s = e[o];
		if (o > 0 && t.write(gt), s.length === 0) continue;
		let c = 0;
		for (let e = 0; e < s.length; e++) {
			let o = s[e];
			e > 0 && t.write(ht), c = bt(t, o[0], c), o.length !== 1 && (n = bt(t, o[1], n), r = bt(t, o[2], r), i = bt(t, o[3], i), o.length !== 4 && (a = bt(t, o[4], a)));
		}
	}
	return t.flush();
}
(/* @__PURE__ */ o(((e, t) => {
	(function(n, r) {
		typeof e == "object" && t !== void 0 ? t.exports = r() : typeof define == "function" && define.amd ? define(r) : (n = typeof globalThis < "u" ? globalThis : n || self, n.resolveURI = r());
	})(e, (function() {
		let e = /^[\w+.-]+:\/\//, t = /^([\w+.-]+:)\/\/([^@/#?]*@)?([^:/#?]*)(:\d+)?(\/[^#?]*)?(\?[^#]*)?(#.*)?/, n = /^file:(?:\/\/((?![a-z]:)[^/#?]*)?)?(\/?[^#?]*)(\?[^#]*)?(#.*)?/i;
		function r(t) {
			return e.test(t);
		}
		function i(e) {
			return e.startsWith("//");
		}
		function a(e) {
			return e.startsWith("/");
		}
		function o(e) {
			return e.startsWith("file:");
		}
		function s(e) {
			return /^[.?#]/.test(e);
		}
		function c(e) {
			let n = t.exec(e);
			return u(n[1], n[2] || "", n[3], n[4] || "", n[5] || "/", n[6] || "", n[7] || "");
		}
		function l(e) {
			let t = n.exec(e), r = t[2];
			return u("file:", "", t[1] || "", "", a(r) ? r : "/" + r, t[3] || "", t[4] || "");
		}
		function u(e, t, n, r, i, a, o) {
			return {
				scheme: e,
				user: t,
				host: n,
				port: r,
				path: i,
				query: a,
				hash: o,
				type: 7
			};
		}
		function d(e) {
			if (i(e)) {
				let t = c("http:" + e);
				return t.scheme = "", t.type = 6, t;
			}
			if (a(e)) {
				let t = c("http://foo.com" + e);
				return t.scheme = "", t.host = "", t.type = 5, t;
			}
			if (o(e)) return l(e);
			if (r(e)) return c(e);
			let t = c("http://foo.com/" + e);
			return t.scheme = "", t.host = "", t.type = e ? e.startsWith("?") ? 3 : e.startsWith("#") ? 2 : 4 : 1, t;
		}
		function f(e) {
			if (e.endsWith("/..")) return e;
			let t = e.lastIndexOf("/");
			return e.slice(0, t + 1);
		}
		function p(e, t) {
			m(t, t.type), e.path === "/" ? e.path = t.path : e.path = f(t.path) + e.path;
		}
		function m(e, t) {
			let n = t <= 4, r = e.path.split("/"), i = 1, a = 0, o = !1;
			for (let e = 1; e < r.length; e++) {
				let t = r[e];
				if (!t) {
					o = !0;
					continue;
				}
				if (o = !1, t !== ".") {
					if (t === "..") {
						a ? (o = !0, a--, i--) : n && (r[i++] = t);
						continue;
					}
					r[i++] = t, a++;
				}
			}
			let s = "";
			for (let e = 1; e < i; e++) s += "/" + r[e];
			(!s || o && !s.endsWith("/..")) && (s += "/"), e.path = s;
		}
		function h(e, t) {
			if (!e && !t) return "";
			let n = d(e), r = n.type;
			if (t && r !== 7) {
				let e = d(t), i = e.type;
				switch (r) {
					case 1: n.hash = e.hash;
					case 2: n.query = e.query;
					case 3:
					case 4: p(n, e);
					case 5: n.user = e.user, n.host = e.host, n.port = e.port;
					case 6: n.scheme = e.scheme;
				}
				i > r && (r = i);
			}
			m(n, r);
			let i = n.query + n.hash;
			switch (r) {
				case 2:
				case 3: return i;
				case 4: {
					let r = n.path.slice(1);
					return r ? s(t || e) && !s(r) ? "./" + r + i : r + i : i || ".";
				}
				case 5: return n.path + i;
				default: return n.scheme + "//" + n.user + n.host + n.port + n.path + i;
			}
		}
		return h;
	}));
})))();
var Tt = class {
	constructor() {
		this._indexes = { __proto__: null }, this.array = [];
	}
};
function Et(e) {
	return e;
}
function Dt(e, t) {
	return Et(e)._indexes[t];
}
function Ot(e, t) {
	let n = Dt(e, t);
	if (n !== void 0) return n;
	let { array: r, _indexes: i } = Et(e);
	return i[t] = r.push(t) - 1;
}
var kt = 0, At = 1, jt = 2, Mt = 3, Nt = 4, Pt = -1, Ft = class {
	constructor({ file: e, sourceRoot: t } = {}) {
		this._names = new Tt(), this._sources = new Tt(), this._sourcesContent = [], this._mappings = [], this.file = e, this.sourceRoot = t, this._ignoreList = new Tt();
	}
};
function It(e) {
	return e;
}
var Lt = (e, t, n, r, i, a, o, s) => Bt(!0, e, t, n, r, i, a, o, s);
function Rt(e) {
	let { _mappings: t, _sources: n, _sourcesContent: r, _names: i, _ignoreList: a } = It(e);
	return Wt(t), {
		version: 3,
		file: e.file || void 0,
		names: i.array,
		sourceRoot: e.sourceRoot || void 0,
		sources: n.array,
		sourcesContent: r,
		mappings: t,
		ignoreList: a.array
	};
}
function zt(e) {
	let t = Rt(e);
	return Object.assign({}, t, { mappings: wt(t.mappings) });
}
function Bt(e, t, n, r, i, a, o, s, c) {
	let { _mappings: l, _sources: u, _sourcesContent: d, _names: f } = It(t), p = Vt(l, n), m = Ht(p, r);
	if (!i) return e && Gt(p, m) ? void 0 : Ut(p, m, [r]);
	let h = Ot(u, i), g = s ? Ot(f, s) : Pt;
	if (h === d.length && (d[h] = c ?? null), !(e && Kt(p, m, h, a, o, g))) return Ut(p, m, s ? [
		r,
		h,
		a,
		o,
		g
	] : [
		r,
		h,
		a,
		o
	]);
}
function Vt(e, t) {
	for (let n = e.length; n <= t; n++) e[n] = [];
	return e[t];
}
function Ht(e, t) {
	let n = e.length;
	for (let r = n - 1; r >= 0 && !(t >= e[r][kt]); n = r--);
	return n;
}
function Ut(e, t, n) {
	for (let n = e.length; n > t; n--) e[n] = e[n - 1];
	e[t] = n;
}
function Wt(e) {
	let { length: t } = e, n = t;
	for (let t = n - 1; t >= 0 && !(e[t].length > 0); n = t, t--);
	n < t && (e.length = n);
}
function Gt(e, t) {
	return t === 0 ? !0 : e[t - 1].length === 1;
}
function Kt(e, t, n, r, i, a) {
	if (t === 0) return !1;
	let o = e[t - 1];
	return o.length === 1 ? !1 : n === o[At] && r === o[jt] && i === o[Mt] && a === (o.length === 5 ? o[Nt] : Pt);
}
function qt({ code: e, mappings: t }, n, r, i, a) {
	let o = Jt(i, a), s = new Ft({ file: r.compiledFilename }), c = 0, l = t[0];
	for (; l === void 0 && c < t.length - 1;) c++, l = t[c];
	let u = 0, d = 0;
	l !== d && Lt(s, u, 0, n, u, 0);
	for (let r = 0; r < e.length; r++) {
		if (r === l) {
			let e = l - d, i = o[c];
			for (Lt(s, u, e, n, u, i); (l === r || l === void 0) && c < t.length - 1;) c++, l = t[c];
		}
		e.charCodeAt(r) === h.lineFeed && (u++, d = r + 1, l !== d && Lt(s, u, 0, n, u, 0));
	}
	let { sourceRoot: f, sourcesContent: p, ...m } = zt(s);
	return m;
}
function Jt(e, t) {
	let n = Array(t.length), r = 0, i = t[r].start, a = 0;
	for (let o = 0; o < e.length; o++) o === i && (n[r] = i - a, r++, i = t[r].start), e.charCodeAt(o) === h.lineFeed && (a = o + 1);
	return n;
}
var Yt = {
	require: "\n    import {createRequire as CREATE_REQUIRE_NAME} from \"module\";\n    const require = CREATE_REQUIRE_NAME(import.meta.url);\n  ",
	interopRequireWildcard: "\n    function interopRequireWildcard(obj) {\n      if (obj && obj.__esModule) {\n        return obj;\n      } else {\n        var newObj = {};\n        if (obj != null) {\n          for (var key in obj) {\n            if (Object.prototype.hasOwnProperty.call(obj, key)) {\n              newObj[key] = obj[key];\n            }\n          }\n        }\n        newObj.default = obj;\n        return newObj;\n      }\n    }\n  ",
	interopRequireDefault: "\n    function interopRequireDefault(obj) {\n      return obj && obj.__esModule ? obj : { default: obj };\n    }\n  ",
	createNamedExportFrom: "\n    function createNamedExportFrom(obj, localName, importedName) {\n      Object.defineProperty(exports, localName, {enumerable: true, configurable: true, get: () => obj[importedName]});\n    }\n  ",
	createStarExport: "\n    function createStarExport(obj) {\n      Object.keys(obj)\n        .filter((key) => key !== \"default\" && key !== \"__esModule\")\n        .forEach((key) => {\n          if (exports.hasOwnProperty(key)) {\n            return;\n          }\n          Object.defineProperty(exports, key, {enumerable: true, configurable: true, get: () => obj[key]});\n        });\n    }\n  ",
	nullishCoalesce: "\n    function nullishCoalesce(lhs, rhsFn) {\n      if (lhs != null) {\n        return lhs;\n      } else {\n        return rhsFn();\n      }\n    }\n  ",
	asyncNullishCoalesce: "\n    async function asyncNullishCoalesce(lhs, rhsFn) {\n      if (lhs != null) {\n        return lhs;\n      } else {\n        return await rhsFn();\n      }\n    }\n  ",
	optionalChain: "\n    function optionalChain(ops) {\n      let lastAccessLHS = undefined;\n      let value = ops[0];\n      let i = 1;\n      while (i < ops.length) {\n        const op = ops[i];\n        const fn = ops[i + 1];\n        i += 2;\n        if ((op === 'optionalAccess' || op === 'optionalCall') && value == null) {\n          return undefined;\n        }\n        if (op === 'access' || op === 'optionalAccess') {\n          lastAccessLHS = value;\n          value = fn(value);\n        } else if (op === 'call' || op === 'optionalCall') {\n          value = fn((...args) => value.call(lastAccessLHS, ...args));\n          lastAccessLHS = undefined;\n        }\n      }\n      return value;\n    }\n  ",
	asyncOptionalChain: "\n    async function asyncOptionalChain(ops) {\n      let lastAccessLHS = undefined;\n      let value = ops[0];\n      let i = 1;\n      while (i < ops.length) {\n        const op = ops[i];\n        const fn = ops[i + 1];\n        i += 2;\n        if ((op === 'optionalAccess' || op === 'optionalCall') && value == null) {\n          return undefined;\n        }\n        if (op === 'access' || op === 'optionalAccess') {\n          lastAccessLHS = value;\n          value = await fn(value);\n        } else if (op === 'call' || op === 'optionalCall') {\n          value = await fn((...args) => value.call(lastAccessLHS, ...args));\n          lastAccessLHS = undefined;\n        }\n      }\n      return value;\n    }\n  ",
	optionalChainDelete: "\n    function optionalChainDelete(ops) {\n      const result = OPTIONAL_CHAIN_NAME(ops);\n      return result == null ? true : result;\n    }\n  ",
	asyncOptionalChainDelete: "\n    async function asyncOptionalChainDelete(ops) {\n      const result = await ASYNC_OPTIONAL_CHAIN_NAME(ops);\n      return result == null ? true : result;\n    }\n  "
}, Xt = class e {
	__init() {
		this.helperNames = {};
	}
	__init2() {
		this.createRequireName = null;
	}
	constructor(t) {
		this.nameManager = t, e.prototype.__init.call(this), e.prototype.__init2.call(this);
	}
	getHelperName(e) {
		let t = this.helperNames[e];
		return t || (t = this.nameManager.claimFreeName(`_${e}`), this.helperNames[e] = t, t);
	}
	emitHelpers() {
		let e = "";
		this.helperNames.optionalChainDelete && this.getHelperName("optionalChain"), this.helperNames.asyncOptionalChainDelete && this.getHelperName("asyncOptionalChain");
		for (let [t, n] of Object.entries(Yt)) {
			let r = this.helperNames[t], i = n;
			t === "optionalChainDelete" ? i = i.replace("OPTIONAL_CHAIN_NAME", this.helperNames.optionalChain) : t === "asyncOptionalChainDelete" ? i = i.replace("ASYNC_OPTIONAL_CHAIN_NAME", this.helperNames.asyncOptionalChain) : t === "require" && (this.createRequireName === null && (this.createRequireName = this.nameManager.claimFreeName("_createRequire")), i = i.replace(/CREATE_REQUIRE_NAME/g, this.createRequireName)), r && (e += " ", e += i.replace(t, r).replace(/\s+/g, " ").trim());
		}
		return e;
	}
};
function Zt(e, t, n) {
	Qt(e, n) && $t(e, t, n);
}
function Qt(e, t) {
	for (let n of e.tokens) if (n.type === u.name && !n.isType && ge(n) && t.has(e.identifierNameForToken(n))) return !0;
	return !1;
}
function $t(e, t, n) {
	let r = [], i = t.length - 1;
	for (let a = e.tokens.length - 1;; a--) {
		for (; r.length > 0 && r[r.length - 1].startTokenIndex === a + 1;) r.pop();
		for (; i >= 0 && t[i].endTokenIndex === a + 1;) r.push(t[i]), i--;
		if (a < 0) break;
		let o = e.tokens[a], s = e.identifierNameForToken(o);
		if (r.length > 1 && !o.isType && o.type === u.name && n.has(s)) {
			if (ve(o)) en(r[r.length - 1], e, s);
			else if (ye(o)) {
				let t = r.length - 1;
				for (; t > 0 && !r[t].isFunctionScope;) t--;
				if (t < 0) throw Error("Did not find parent function scope.");
				en(r[t], e, s);
			}
		}
	}
	if (r.length > 0) throw Error("Expected empty scope stack after processing file.");
}
function en(e, t, n) {
	for (let r = e.startTokenIndex; r < e.endTokenIndex; r++) {
		let e = t.tokens[r];
		(e.type === u.name || e.type === u.jsxName) && t.identifierNameForToken(e) === n && (e.shadowsGlobal = !0);
	}
}
function tn(e, t) {
	let n = [];
	for (let r of t) r.type === u.name && n.push(e.slice(r.start, r.end));
	return n;
}
var nn = class e {
	__init() {
		this.usedNames = /* @__PURE__ */ new Set();
	}
	constructor(t, n) {
		e.prototype.__init.call(this), this.usedNames = new Set(tn(t, n));
	}
	claimFreeName(e) {
		let t = this.findFreeName(e);
		return this.usedNames.add(t), t;
	}
	findFreeName(e) {
		if (!this.usedNames.has(e)) return e;
		let t = 2;
		for (; this.usedNames.has(e + String(t));) t++;
		return e + String(t);
	}
}, rn = /* @__PURE__ */ o(((e) => {
	var t = e && e.__extends || (function() {
		var e = function(t, n) {
			return e = Object.setPrototypeOf || { __proto__: [] } instanceof Array && function(e, t) {
				e.__proto__ = t;
			} || function(e, t) {
				for (var n in t) t.hasOwnProperty(n) && (e[n] = t[n]);
			}, e(t, n);
		};
		return function(t, n) {
			e(t, n);
			function r() {
				this.constructor = t;
			}
			t.prototype = n === null ? Object.create(n) : (r.prototype = n.prototype, new r());
		};
	})();
	Object.defineProperty(e, "__esModule", { value: !0 }), e.DetailContext = e.NoopContext = e.VError = void 0;
	var n = function(e) {
		t(n, e);
		function n(t, r) {
			var i = e.call(this, r) || this;
			return i.path = t, Object.setPrototypeOf(i, n.prototype), i;
		}
		return n;
	}(Error);
	e.VError = n, e.NoopContext = function() {
		function e() {}
		return e.prototype.fail = function(e, t, n) {
			return !1;
		}, e.prototype.unionResolver = function() {
			return this;
		}, e.prototype.createContext = function() {
			return this;
		}, e.prototype.resolveUnion = function(e) {}, e;
	}();
	var r = function() {
		function e() {
			this._propNames = [""], this._messages = [null], this._score = 0;
		}
		return e.prototype.fail = function(e, t, n) {
			return this._propNames.push(e), this._messages.push(t), this._score += n, !1;
		}, e.prototype.unionResolver = function() {
			return new i();
		}, e.prototype.resolveUnion = function(e) {
			for (var t, n, r = e, i = null, a = 0, o = r.contexts; a < o.length; a++) {
				var s = o[a];
				(!i || s._score >= i._score) && (i = s);
			}
			i && i._score > 0 && ((t = this._propNames).push.apply(t, i._propNames), (n = this._messages).push.apply(n, i._messages));
		}, e.prototype.getError = function(e) {
			for (var t = [], r = this._propNames.length - 1; r >= 0; r--) {
				var i = this._propNames[r];
				e += typeof i == "number" ? "[" + i + "]" : i ? "." + i : "";
				var a = this._messages[r];
				a && t.push(e + " " + a);
			}
			return new n(e, t.join("; "));
		}, e.prototype.getErrorDetail = function(e) {
			for (var t = [], n = this._propNames.length - 1; n >= 0; n--) {
				var r = this._propNames[n];
				e += typeof r == "number" ? "[" + r + "]" : r ? "." + r : "";
				var i = this._messages[n];
				i && t.push({
					path: e,
					message: i
				});
			}
			for (var a = null, n = t.length - 1; n >= 0; n--) a && (t[n].nested = [a]), a = t[n];
			return a;
		}, e;
	}();
	e.DetailContext = r;
	var i = function() {
		function e() {
			this.contexts = [];
		}
		return e.prototype.createContext = function() {
			var e = new r();
			return this.contexts.push(e), e;
		}, e;
	}();
})), an = /* @__PURE__ */ o(((e) => {
	var t = e && e.__extends || (function() {
		var e = function(t, n) {
			return e = Object.setPrototypeOf || { __proto__: [] } instanceof Array && function(e, t) {
				e.__proto__ = t;
			} || function(e, t) {
				for (var n in t) t.hasOwnProperty(n) && (e[n] = t[n]);
			}, e(t, n);
		};
		return function(t, n) {
			e(t, n);
			function r() {
				this.constructor = t;
			}
			t.prototype = n === null ? Object.create(n) : (r.prototype = n.prototype, new r());
		};
	})();
	Object.defineProperty(e, "__esModule", { value: !0 }), e.basicTypes = e.BasicType = e.TParamList = e.TParam = e.param = e.TFunc = e.func = e.TProp = e.TOptional = e.opt = e.TIface = e.iface = e.TEnumLiteral = e.enumlit = e.TEnumType = e.enumtype = e.TIntersection = e.intersection = e.TUnion = e.union = e.TTuple = e.tuple = e.TArray = e.array = e.TLiteral = e.lit = e.TName = e.name = e.TType = void 0;
	var n = rn(), r = function() {
		function e() {}
		return e;
	}();
	e.TType = r;
	function i(e) {
		return typeof e == "string" ? o(e) : e;
	}
	function a(e, t) {
		var n = e[t];
		if (!n) throw Error("Unknown type " + t);
		return n;
	}
	function o(e) {
		return new s(e);
	}
	e.name = o;
	var s = function(e) {
		t(n, e);
		function n(t) {
			var n = e.call(this) || this;
			return n.name = t, n._failMsg = "is not a " + t, n;
		}
		return n.prototype.getChecker = function(e, t, r) {
			var i = this, o = a(e, this.name), s = o.getChecker(e, t, r);
			return o instanceof E || o instanceof n ? s : function(e, t) {
				return s(e, t) ? !0 : t.fail(null, i._failMsg, 0);
			};
		}, n;
	}(r);
	e.TName = s;
	function c(e) {
		return new l(e);
	}
	e.lit = c;
	var l = function(e) {
		t(n, e);
		function n(t) {
			var n = e.call(this) || this;
			return n.value = t, n.name = JSON.stringify(t), n._failMsg = "is not " + n.name, n;
		}
		return n.prototype.getChecker = function(e, t) {
			var n = this;
			return function(e, t) {
				return e === n.value ? !0 : t.fail(null, n._failMsg, -1);
			};
		}, n;
	}(r);
	e.TLiteral = l;
	function u(e) {
		return new d(i(e));
	}
	e.array = u;
	var d = function(e) {
		t(n, e);
		function n(t) {
			var n = e.call(this) || this;
			return n.ttype = t, n;
		}
		return n.prototype.getChecker = function(e, t) {
			var n = this.ttype.getChecker(e, t);
			return function(e, t) {
				if (!Array.isArray(e)) return t.fail(null, "is not an array", 0);
				for (var r = 0; r < e.length; r++) if (!n(e[r], t)) return t.fail(r, null, 1);
				return !0;
			};
		}, n;
	}(r);
	e.TArray = d;
	function f() {
		return new p([...arguments].map(function(e) {
			return i(e);
		}));
	}
	e.tuple = f;
	var p = function(e) {
		t(n, e);
		function n(t) {
			var n = e.call(this) || this;
			return n.ttypes = t, n;
		}
		return n.prototype.getChecker = function(e, t) {
			var n = this.ttypes.map(function(n) {
				return n.getChecker(e, t);
			}), r = function(e, t) {
				if (!Array.isArray(e)) return t.fail(null, "is not an array", 0);
				for (var r = 0; r < n.length; r++) if (!n[r](e[r], t)) return t.fail(r, null, 1);
				return !0;
			};
			return t ? function(e, t) {
				return r(e, t) ? e.length <= n.length ? !0 : t.fail(n.length, "is extraneous", 2) : !1;
			} : r;
		}, n;
	}(r);
	e.TTuple = p;
	function m() {
		return new h([...arguments].map(function(e) {
			return i(e);
		}));
	}
	e.union = m;
	var h = function(e) {
		t(n, e);
		function n(t) {
			var n = e.call(this) || this;
			n.ttypes = t;
			var r = t.map(function(e) {
				return e instanceof s || e instanceof l ? e.name : null;
			}).filter(function(e) {
				return e;
			}), i = t.length - r.length;
			return r.length ? (i > 0 && r.push(i + " more"), n._failMsg = "is none of " + r.join(", ")) : n._failMsg = "is none of " + i + " types", n;
		}
		return n.prototype.getChecker = function(e, t) {
			var n = this, r = this.ttypes.map(function(n) {
				return n.getChecker(e, t);
			});
			return function(e, t) {
				for (var i = t.unionResolver(), a = 0; a < r.length; a++) if (r[a](e, i.createContext())) return !0;
				return t.resolveUnion(i), t.fail(null, n._failMsg, 0);
			};
		}, n;
	}(r);
	e.TUnion = h;
	function g() {
		return new _([...arguments].map(function(e) {
			return i(e);
		}));
	}
	e.intersection = g;
	var _ = function(e) {
		t(n, e);
		function n(t) {
			var n = e.call(this) || this;
			return n.ttypes = t, n;
		}
		return n.prototype.getChecker = function(e, t) {
			var n = /* @__PURE__ */ new Set(), r = this.ttypes.map(function(r) {
				return r.getChecker(e, t, n);
			});
			return function(e, t) {
				return r.every(function(n) {
					return n(e, t);
				}) ? !0 : t.fail(null, null, 0);
			};
		}, n;
	}(r);
	e.TIntersection = _;
	function v(e) {
		return new y(e);
	}
	e.enumtype = v;
	var y = function(e) {
		t(n, e);
		function n(t) {
			var n = e.call(this) || this;
			return n.members = t, n.validValues = /* @__PURE__ */ new Set(), n._failMsg = "is not a valid enum value", n.validValues = new Set(Object.keys(t).map(function(e) {
				return t[e];
			})), n;
		}
		return n.prototype.getChecker = function(e, t) {
			var n = this;
			return function(e, t) {
				return n.validValues.has(e) ? !0 : t.fail(null, n._failMsg, 0);
			};
		}, n;
	}(r);
	e.TEnumType = y;
	function b(e, t) {
		return new ee(e, t);
	}
	e.enumlit = b;
	var ee = function(e) {
		t(n, e);
		function n(t, n) {
			var r = e.call(this) || this;
			return r.enumName = t, r.prop = n, r._failMsg = "is not " + t + "." + n, r;
		}
		return n.prototype.getChecker = function(e, t) {
			var n = this, r = a(e, this.enumName);
			if (!(r instanceof y)) throw Error("Type " + this.enumName + " used in enumlit is not an enum type");
			var i = r.members[this.prop];
			if (!r.members.hasOwnProperty(this.prop)) throw Error("Unknown value " + this.enumName + "." + this.prop + " used in enumlit");
			return function(e, t) {
				return e === i ? !0 : t.fail(null, n._failMsg, -1);
			};
		}, n;
	}(r);
	e.TEnumLiteral = ee;
	function te(e) {
		return Object.keys(e).map(function(t) {
			return ne(t, e[t]);
		});
	}
	function ne(e, t) {
		return t instanceof x ? new oe(e, t.ttype, !0) : new oe(e, i(t), !1);
	}
	function re(e, t) {
		return new ie(e, te(t));
	}
	e.iface = re;
	var ie = function(e) {
		t(r, e);
		function r(t, n) {
			var r = e.call(this) || this;
			return r.bases = t, r.props = n, r.propSet = new Set(n.map(function(e) {
				return e.name;
			})), r;
		}
		return r.prototype.getChecker = function(e, t, r) {
			var i = this, o = this.bases.map(function(n) {
				return a(e, n).getChecker(e, t);
			}), s = this.props.map(function(n) {
				return n.ttype.getChecker(e, t);
			}), c = new n.NoopContext(), l = this.props.map(function(e, t) {
				return !e.isOpt && !s[t](void 0, c);
			}), u = function(e, t) {
				if (typeof e != "object" || !e) return t.fail(null, "is not an object", 0);
				for (var n = 0; n < o.length; n++) if (!o[n](e, t)) return !1;
				for (var n = 0; n < s.length; n++) {
					var r = i.props[n].name, a = e[r];
					if (a === void 0) {
						if (l[n]) return t.fail(r, "is missing", 1);
					} else if (!s[n](a, t)) return t.fail(r, null, 1);
				}
				return !0;
			};
			if (!t) return u;
			var d = this.propSet;
			return r && (this.propSet.forEach(function(e) {
				return r.add(e);
			}), d = r), function(e, t) {
				if (!u(e, t)) return !1;
				for (var n in e) if (!d.has(n)) return t.fail(n, "is extraneous", 2);
				return !0;
			};
		}, r;
	}(r);
	e.TIface = ie;
	function ae(e) {
		return new x(i(e));
	}
	e.opt = ae;
	var x = function(e) {
		t(n, e);
		function n(t) {
			var n = e.call(this) || this;
			return n.ttype = t, n;
		}
		return n.prototype.getChecker = function(e, t) {
			var n = this.ttype.getChecker(e, t);
			return function(e, t) {
				return e === void 0 || n(e, t);
			};
		}, n;
	}(r);
	e.TOptional = x;
	var oe = function() {
		function e(e, t, n) {
			this.name = e, this.ttype = t, this.isOpt = n;
		}
		return e;
	}();
	e.TProp = oe;
	function S(e) {
		return new C(new se([...arguments].slice(1)), i(e));
	}
	e.func = S;
	var C = function(e) {
		t(n, e);
		function n(t, n) {
			var r = e.call(this) || this;
			return r.paramList = t, r.result = n, r;
		}
		return n.prototype.getChecker = function(e, t) {
			return function(e, t) {
				return typeof e == "function" ? !0 : t.fail(null, "is not a function", 0);
			};
		}, n;
	}(r);
	e.TFunc = C;
	function w(e, t, n) {
		return new T(e, i(t), !!n);
	}
	e.param = w;
	var T = function() {
		function e(e, t, n) {
			this.name = e, this.ttype = t, this.isOpt = n;
		}
		return e;
	}();
	e.TParam = T;
	var se = function(e) {
		t(r, e);
		function r(t) {
			var n = e.call(this) || this;
			return n.params = t, n;
		}
		return r.prototype.getChecker = function(e, t) {
			var r = this, i = this.params.map(function(n) {
				return n.ttype.getChecker(e, t);
			}), a = new n.NoopContext(), o = this.params.map(function(e, t) {
				return !e.isOpt && !i[t](void 0, a);
			}), s = function(e, t) {
				if (!Array.isArray(e)) return t.fail(null, "is not an array", 0);
				for (var n = 0; n < i.length; n++) {
					var a = r.params[n];
					if (e[n] === void 0) {
						if (o[n]) return t.fail(a.name, "is missing", 1);
					} else if (!i[n](e[n], t)) return t.fail(a.name, null, 1);
				}
				return !0;
			};
			return t ? function(e, t) {
				return s(e, t) ? e.length <= i.length ? !0 : t.fail(i.length, "is extraneous", 2) : !1;
			} : s;
		}, r;
	}(r);
	e.TParamList = se;
	var E = function(e) {
		t(n, e);
		function n(t, n) {
			var r = e.call(this) || this;
			return r.validator = t, r.message = n, r;
		}
		return n.prototype.getChecker = function(e, t) {
			var n = this;
			return function(e, t) {
				return n.validator(e) ? !0 : t.fail(null, n.message, 0);
			};
		}, n;
	}(r);
	e.BasicType = E, e.basicTypes = {
		any: new E(function(e) {
			return !0;
		}, "is invalid"),
		number: new E(function(e) {
			return typeof e == "number";
		}, "is not a number"),
		object: new E(function(e) {
			return typeof e == "object" && e;
		}, "is not an object"),
		boolean: new E(function(e) {
			return typeof e == "boolean";
		}, "is not a boolean"),
		string: new E(function(e) {
			return typeof e == "string";
		}, "is not a string"),
		symbol: new E(function(e) {
			return typeof e == "symbol";
		}, "is not a symbol"),
		void: new E(function(e) {
			return e == null;
		}, "is not void"),
		undefined: new E(function(e) {
			return e === void 0;
		}, "is not undefined"),
		null: new E(function(e) {
			return e === null;
		}, "is not null"),
		never: new E(function(e) {
			return !1;
		}, "is unexpected"),
		Date: new E(O("[object Date]"), "is not a Date"),
		RegExp: new E(O("[object RegExp]"), "is not a RegExp")
	};
	var D = Object.prototype.toString;
	function O(e) {
		return function(t) {
			return typeof t == "object" && t && D.call(t) === e;
		};
	}
	typeof Buffer < "u" && (e.basicTypes.Buffer = new E(function(e) {
		return Buffer.isBuffer(e);
	}, "is not a Buffer"));
	for (var k = function(t) {
		e.basicTypes[t.name] = new E(function(e) {
			return e instanceof t;
		}, "is not a " + t.name);
	}, ce = 0, le = [
		Int8Array,
		Uint8Array,
		Uint8ClampedArray,
		Int16Array,
		Uint16Array,
		Int32Array,
		Uint32Array,
		Float32Array,
		Float64Array,
		ArrayBuffer
	]; ce < le.length; ce++) {
		var ue = le[ce];
		k(ue);
	}
})), H = /* @__PURE__ */ c((/* @__PURE__ */ o(((e) => {
	var t = e && e.__spreadArrays || function() {
		for (var e = 0, t = 0, n = arguments.length; t < n; t++) e += arguments[t].length;
		for (var r = Array(e), i = 0, t = 0; t < n; t++) for (var a = arguments[t], o = 0, s = a.length; o < s; o++, i++) r[i] = a[o];
		return r;
	};
	Object.defineProperty(e, "__esModule", { value: !0 }), e.Checker = e.createCheckers = void 0;
	var n = an(), r = rn(), i = an();
	Object.defineProperty(e, "TArray", {
		enumerable: !0,
		get: function() {
			return i.TArray;
		}
	}), Object.defineProperty(e, "TEnumType", {
		enumerable: !0,
		get: function() {
			return i.TEnumType;
		}
	}), Object.defineProperty(e, "TEnumLiteral", {
		enumerable: !0,
		get: function() {
			return i.TEnumLiteral;
		}
	}), Object.defineProperty(e, "TFunc", {
		enumerable: !0,
		get: function() {
			return i.TFunc;
		}
	}), Object.defineProperty(e, "TIface", {
		enumerable: !0,
		get: function() {
			return i.TIface;
		}
	}), Object.defineProperty(e, "TLiteral", {
		enumerable: !0,
		get: function() {
			return i.TLiteral;
		}
	}), Object.defineProperty(e, "TName", {
		enumerable: !0,
		get: function() {
			return i.TName;
		}
	}), Object.defineProperty(e, "TOptional", {
		enumerable: !0,
		get: function() {
			return i.TOptional;
		}
	}), Object.defineProperty(e, "TParam", {
		enumerable: !0,
		get: function() {
			return i.TParam;
		}
	}), Object.defineProperty(e, "TParamList", {
		enumerable: !0,
		get: function() {
			return i.TParamList;
		}
	}), Object.defineProperty(e, "TProp", {
		enumerable: !0,
		get: function() {
			return i.TProp;
		}
	}), Object.defineProperty(e, "TTuple", {
		enumerable: !0,
		get: function() {
			return i.TTuple;
		}
	}), Object.defineProperty(e, "TType", {
		enumerable: !0,
		get: function() {
			return i.TType;
		}
	}), Object.defineProperty(e, "TUnion", {
		enumerable: !0,
		get: function() {
			return i.TUnion;
		}
	}), Object.defineProperty(e, "TIntersection", {
		enumerable: !0,
		get: function() {
			return i.TIntersection;
		}
	}), Object.defineProperty(e, "array", {
		enumerable: !0,
		get: function() {
			return i.array;
		}
	}), Object.defineProperty(e, "enumlit", {
		enumerable: !0,
		get: function() {
			return i.enumlit;
		}
	}), Object.defineProperty(e, "enumtype", {
		enumerable: !0,
		get: function() {
			return i.enumtype;
		}
	}), Object.defineProperty(e, "func", {
		enumerable: !0,
		get: function() {
			return i.func;
		}
	}), Object.defineProperty(e, "iface", {
		enumerable: !0,
		get: function() {
			return i.iface;
		}
	}), Object.defineProperty(e, "lit", {
		enumerable: !0,
		get: function() {
			return i.lit;
		}
	}), Object.defineProperty(e, "name", {
		enumerable: !0,
		get: function() {
			return i.name;
		}
	}), Object.defineProperty(e, "opt", {
		enumerable: !0,
		get: function() {
			return i.opt;
		}
	}), Object.defineProperty(e, "param", {
		enumerable: !0,
		get: function() {
			return i.param;
		}
	}), Object.defineProperty(e, "tuple", {
		enumerable: !0,
		get: function() {
			return i.tuple;
		}
	}), Object.defineProperty(e, "union", {
		enumerable: !0,
		get: function() {
			return i.union;
		}
	}), Object.defineProperty(e, "intersection", {
		enumerable: !0,
		get: function() {
			return i.intersection;
		}
	}), Object.defineProperty(e, "BasicType", {
		enumerable: !0,
		get: function() {
			return i.BasicType;
		}
	});
	var a = rn();
	Object.defineProperty(e, "VError", {
		enumerable: !0,
		get: function() {
			return a.VError;
		}
	});
	function o() {
		for (var e = [...arguments], r = Object.assign.apply(Object, t([{}, n.basicTypes], e)), i = {}, a = 0, o = e; a < o.length; a++) for (var c = o[a], l = 0, u = Object.keys(c); l < u.length; l++) {
			var d = u[l];
			i[d] = new s(r, c[d]);
		}
		return i;
	}
	e.createCheckers = o;
	var s = function() {
		function e(e, t, r) {
			if (r === void 0 && (r = "value"), this.suite = e, this.ttype = t, this._path = r, this.props = /* @__PURE__ */ new Map(), t instanceof n.TIface) for (var i = 0, a = t.props; i < a.length; i++) {
				var o = a[i];
				this.props.set(o.name, o.ttype);
			}
			this.checkerPlain = this.ttype.getChecker(e, !1), this.checkerStrict = this.ttype.getChecker(e, !0);
		}
		return e.prototype.setReportedPath = function(e) {
			this._path = e;
		}, e.prototype.check = function(e) {
			return this._doCheck(this.checkerPlain, e);
		}, e.prototype.test = function(e) {
			return this.checkerPlain(e, new r.NoopContext());
		}, e.prototype.validate = function(e) {
			return this._doValidate(this.checkerPlain, e);
		}, e.prototype.strictCheck = function(e) {
			return this._doCheck(this.checkerStrict, e);
		}, e.prototype.strictTest = function(e) {
			return this.checkerStrict(e, new r.NoopContext());
		}, e.prototype.strictValidate = function(e) {
			return this._doValidate(this.checkerStrict, e);
		}, e.prototype.getProp = function(t) {
			var n = this.props.get(t);
			if (!n) throw Error("Type has no property " + t);
			return new e(this.suite, n, this._path + "." + t);
		}, e.prototype.methodArgs = function(t) {
			var n = this._getMethod(t);
			return new e(this.suite, n.paramList);
		}, e.prototype.methodResult = function(t) {
			var n = this._getMethod(t);
			return new e(this.suite, n.result);
		}, e.prototype.getArgs = function() {
			if (!(this.ttype instanceof n.TFunc)) throw Error("getArgs() applied to non-function");
			return new e(this.suite, this.ttype.paramList);
		}, e.prototype.getResult = function() {
			if (!(this.ttype instanceof n.TFunc)) throw Error("getResult() applied to non-function");
			return new e(this.suite, this.ttype.result);
		}, e.prototype.getType = function() {
			return this.ttype;
		}, e.prototype._doCheck = function(e, t) {
			if (!e(t, new r.NoopContext())) {
				var n = new r.DetailContext();
				throw e(t, n), n.getError(this._path);
			}
		}, e.prototype._doValidate = function(e, t) {
			if (e(t, new r.NoopContext())) return null;
			var n = new r.DetailContext();
			return e(t, n), n.getErrorDetail(this._path);
		}, e.prototype._getMethod = function(e) {
			var t = this.props.get(e);
			if (!t) throw Error("Type has no property " + e);
			if (!(t instanceof n.TFunc)) throw Error("Property " + e + " is not a method");
			return t;
		}, e;
	}();
	e.Checker = s;
})))()), on = {
	Transform: H.union(H.lit("jsx"), H.lit("typescript"), H.lit("flow"), H.lit("imports"), H.lit("react-hot-loader"), H.lit("jest")),
	SourceMapOptions: H.iface([], { compiledFilename: "string" }),
	Options: H.iface([], {
		transforms: H.array("Transform"),
		disableESTransforms: H.opt("boolean"),
		jsxRuntime: H.opt(H.union(H.lit("classic"), H.lit("automatic"), H.lit("preserve"))),
		production: H.opt("boolean"),
		jsxImportSource: H.opt("string"),
		jsxPragma: H.opt("string"),
		jsxFragmentPragma: H.opt("string"),
		keepUnusedImports: H.opt("boolean"),
		preserveDynamicImport: H.opt("boolean"),
		injectCreateRequireForImportRequire: H.opt("boolean"),
		enableLegacyTypeScriptModuleInterop: H.opt("boolean"),
		enableLegacyBabel5ModuleInterop: H.opt("boolean"),
		sourceMapOptions: H.opt("SourceMapOptions"),
		filePath: H.opt("string")
	})
}, { Options: sn } = (0, H.createCheckers)(on);
function cn(e) {
	sn.strictCheck(e);
}
function ln() {
	N(), K(!1);
}
function un(e) {
	N(), mn(e);
}
function dn(e) {
	Y(), pn(e);
}
function fn() {
	Y(), y.tokens[y.tokens.length - 1].identifierRole = j.ImportDeclaration;
}
function pn(e) {
	let t;
	t = y.scopeDepth === 0 ? j.TopLevelDeclaration : e ? j.BlockScopedDeclaration : j.FunctionScopedDeclaration, y.tokens[y.tokens.length - 1].identifierRole = t;
}
function mn(e) {
	switch (y.type) {
		case u._this: {
			let e = P(0);
			N(), F(e);
			return;
		}
		case u._yield:
		case u.name:
			y.type = u.name, dn(e);
			return;
		case u.bracketL:
			N(), hn(u.bracketR, e, !0);
			return;
		case u.braceL:
			Zi(!0, e);
			return;
		default: k();
	}
}
function hn(e, t, n = !1, r = !1, i = 0) {
	let a = !0, o = !1, s = y.tokens.length;
	for (; !I(e) && !y.error;) if (a ? a = !1 : (O(u.comma), y.tokens[y.tokens.length - 1].contextId = i, !o && y.tokens[s].isType && (y.tokens[y.tokens.length - 1].isType = !0, o = !0)), !(n && L(u.comma))) {
		if (I(e)) break;
		if (L(u.ellipsis)) {
			un(t), _n(), I(u.comma), O(e);
			break;
		} else gn(r, t);
	}
}
function gn(e, t) {
	e && Sn([
		l._public,
		l._protected,
		l._private,
		l._readonly,
		l._override
	]), vn(t), _n(), vn(t, !0);
}
function _n() {
	v ? _o() : _ && oi();
}
function vn(e, t = !1) {
	if (t || mn(e), !I(u.eq)) return;
	let n = y.tokens.length - 1;
	K(), y.tokens[n].rhsEndIndex = y.tokens.length;
}
function yn() {
	return L(u.name);
}
function bn() {
	return L(u.name) || !!(y.type & u.IS_KEYWORD) || L(u.string) || L(u.num) || L(u.bigint) || L(u.decimal);
}
function xn() {
	let e = y.snapshot();
	return N(), (L(u.bracketL) || L(u.braceL) || L(u.star) || L(u.ellipsis) || L(u.hash) || bn()) && !T() ? !0 : (y.restoreFromSnapshot(e), !1);
}
function Sn(e) {
	for (; Cn(e) !== null;);
}
function Cn(e) {
	if (!L(u.name)) return null;
	let t = y.contextualKeyword;
	if (e.indexOf(t) !== -1 && xn()) {
		switch (t) {
			case l._readonly:
				y.tokens[y.tokens.length - 1].type = u._readonly;
				break;
			case l._abstract:
				y.tokens[y.tokens.length - 1].type = u._abstract;
				break;
			case l._static:
				y.tokens[y.tokens.length - 1].type = u._static;
				break;
			case l._public:
				y.tokens[y.tokens.length - 1].type = u._public;
				break;
			case l._private:
				y.tokens[y.tokens.length - 1].type = u._private;
				break;
			case l._protected:
				y.tokens[y.tokens.length - 1].type = u._protected;
				break;
			case l._override:
				y.tokens[y.tokens.length - 1].type = u._override;
				break;
			case l._declare:
				y.tokens[y.tokens.length - 1].type = u._declare;
				break;
			default: break;
		}
		return t;
	}
	return null;
}
function wn() {
	for (Y(); I(u.dot);) Y();
}
function Tn() {
	wn(), !T() && L(u.lessThan) && zr();
}
function En() {
	N(), mr();
}
function Dn() {
	N();
}
function On() {
	O(u._typeof), L(u._import) ? kn() : wn(), !T() && L(u.lessThan) && zr();
}
function kn() {
	O(u._import), O(u.parenL), O(u.string), O(u.parenR), I(u.dot) && wn(), L(u.lessThan) && zr();
}
function An() {
	I(u._const);
	let e = I(u._in), t = S(l._out);
	I(u._const), (e || t) && !L(u.name) ? y.tokens[y.tokens.length - 1].type = u.name : Y(), I(u._extends) && U(), I(u.eq) && U();
}
function jn() {
	L(u.lessThan) && Mn();
}
function Mn() {
	let e = P(0);
	for (L(u.lessThan) || L(u.typeParameterStart) ? N() : k(); !I(u.greaterThan) && !y.error;) An(), I(u.comma);
	F(e);
}
function Nn(e) {
	let t = e === u.arrow;
	jn(), O(u.parenL), y.scopeDepth++, Pn(!1), y.scopeDepth--, (t || L(e)) && lr(e);
}
function Pn(e) {
	hn(u.parenR, e);
}
function Fn() {
	I(u.comma) || D();
}
function In() {
	Nn(u.colon), Fn();
}
function Ln() {
	let e = y.snapshot();
	N();
	let t = I(u.name) && L(u.colon);
	return y.restoreFromSnapshot(e), t;
}
function Rn() {
	if (!(L(u.bracketL) && Ln())) return !1;
	let e = P(0);
	return O(u.bracketL), Y(), mr(), O(u.bracketR), dr(), Fn(), F(e), !0;
}
function zn(e) {
	I(u.question), !e && (L(u.parenL) || L(u.lessThan)) ? (Nn(u.colon), Fn()) : (dr(), Fn());
}
function Bn() {
	if (L(u.parenL) || L(u.lessThan)) {
		In();
		return;
	}
	if (L(u._new)) {
		N(), L(u.parenL) || L(u.lessThan) ? In() : zn(!1);
		return;
	}
	let e = !!Cn([l._readonly]);
	Rn() || ((x(l._get) || x(l._set)) && xn(), na(-1), zn(e));
}
function Vn() {
	Hn();
}
function Hn() {
	for (O(u.braceL); !I(u.braceR) && !y.error;) Bn();
}
function Un() {
	let e = y.snapshot(), t = Wn();
	return y.restoreFromSnapshot(e), t;
}
function Wn() {
	return N(), I(u.plus) || I(u.minus) ? x(l._readonly) : (x(l._readonly) && N(), !L(u.bracketL) || (N(), !yn()) ? !1 : (N(), L(u._in)));
}
function Gn() {
	Y(), O(u._in), U();
}
function Kn() {
	O(u.braceL), L(u.plus) || L(u.minus) ? (N(), C(l._readonly)) : S(l._readonly), O(u.bracketL), Gn(), S(l._as) && U(), O(u.bracketR), L(u.plus) || L(u.minus) ? (N(), O(u.question)) : I(u.question), fr(), D(), O(u.braceR);
}
function qn() {
	for (O(u.bracketL); !I(u.bracketR) && !y.error;) Jn(), I(u.comma);
}
function Jn() {
	I(u.ellipsis) ? U() : (U(), I(u.question)), I(u.colon) && U();
}
function Yn() {
	O(u.parenL), U(), O(u.parenR);
}
function Xn() {
	for (Se(), Se(); !L(u.backQuote) && !y.error;) O(u.dollarBraceL), U(), Se(), Se();
	N();
}
var Zn;
(function(e) {
	e[e.TSFunctionType = 0] = "TSFunctionType", e[e.TSConstructorType = 1] = "TSConstructorType", e[e.TSAbstractConstructorType = 2] = "TSAbstractConstructorType";
})(Zn ||= {});
function Qn(e) {
	e === Zn.TSAbstractConstructorType && C(l._abstract), (e === Zn.TSConstructorType || e === Zn.TSAbstractConstructorType) && O(u._new);
	let t = y.inDisallowConditionalTypesContext;
	y.inDisallowConditionalTypesContext = !1, Nn(u.arrow), y.inDisallowConditionalTypesContext = t;
}
function $n() {
	switch (y.type) {
		case u.name:
			Tn();
			return;
		case u._void:
		case u._null:
			N();
			return;
		case u.string:
		case u.num:
		case u.bigint:
		case u.decimal:
		case u._true:
		case u._false:
			Hi();
			return;
		case u.minus:
			N(), Hi();
			return;
		case u._this:
			Dn(), x(l._is) && !T() && En();
			return;
		case u._typeof:
			On();
			return;
		case u._import:
			kn();
			return;
		case u.braceL:
			Un() ? Kn() : Vn();
			return;
		case u.bracketL:
			qn();
			return;
		case u.parenL:
			Yn();
			return;
		case u.backQuote:
			Xn();
			return;
		default:
			if (y.type & u.IS_KEYWORD) {
				N(), y.tokens[y.tokens.length - 1].type = u.name;
				return;
			}
			break;
	}
	k();
}
function er() {
	for ($n(); !T() && I(u.bracketL);) I(u.bracketR) || (U(), O(u.bracketR));
}
function tr() {
	if (C(l._infer), Y(), L(u._extends)) {
		let e = y.snapshot();
		O(u._extends);
		let t = y.inDisallowConditionalTypesContext;
		y.inDisallowConditionalTypesContext = !0, U(), y.inDisallowConditionalTypesContext = t, (y.error || !y.inDisallowConditionalTypesContext && L(u.question)) && y.restoreFromSnapshot(e);
	}
}
function nr() {
	if (x(l._keyof) || x(l._unique) || x(l._readonly)) N(), nr();
	else if (x(l._infer)) tr();
	else {
		let e = y.inDisallowConditionalTypesContext;
		y.inDisallowConditionalTypesContext = !1, er(), y.inDisallowConditionalTypesContext = e;
	}
}
function rr() {
	if (I(u.bitwiseAND), nr(), L(u.bitwiseAND)) for (; I(u.bitwiseAND);) nr();
}
function ir() {
	if (I(u.bitwiseOR), rr(), L(u.bitwiseOR)) for (; I(u.bitwiseOR);) rr();
}
function ar() {
	return L(u.lessThan) ? !0 : L(u.parenL) && sr();
}
function or() {
	if (L(u.name) || L(u._this)) return N(), !0;
	if (L(u.braceL) || L(u.bracketL)) {
		let e = 1;
		for (N(); e > 0 && !y.error;) L(u.braceL) || L(u.bracketL) ? e++ : (L(u.braceR) || L(u.bracketR)) && e--, N();
		return !0;
	}
	return !1;
}
function sr() {
	let e = y.snapshot(), t = cr();
	return y.restoreFromSnapshot(e), t;
}
function cr() {
	return N(), !!(L(u.parenR) || L(u.ellipsis) || or() && (L(u.colon) || L(u.comma) || L(u.question) || L(u.eq) || L(u.parenR) && (N(), L(u.arrow))));
}
function lr(e) {
	let t = P(0);
	O(e), pr() || U(), F(t);
}
function ur() {
	L(u.colon) && lr(u.colon);
}
function dr() {
	L(u.colon) && mr();
}
function fr() {
	I(u.colon) && U();
}
function pr() {
	let e = y.snapshot();
	return x(l._asserts) ? (N(), S(l._is) ? (U(), !0) : yn() || L(u._this) ? (N(), S(l._is) && U(), !0) : (y.restoreFromSnapshot(e), !1)) : yn() || L(u._this) ? (N(), x(l._is) && !T() ? (N(), U(), !0) : (y.restoreFromSnapshot(e), !1)) : !1;
}
function mr() {
	let e = P(0);
	O(u.colon), U(), F(e);
}
function U() {
	if (gr(), y.inDisallowConditionalTypesContext || T() || !I(u._extends)) return;
	let e = y.inDisallowConditionalTypesContext;
	y.inDisallowConditionalTypesContext = !0, gr(), y.inDisallowConditionalTypesContext = e, O(u.question), U(), O(u.colon), U();
}
function hr() {
	return x(l._abstract) && R() === u._new;
}
function gr() {
	if (ar()) {
		Qn(Zn.TSFunctionType);
		return;
	}
	if (L(u._new)) {
		Qn(Zn.TSConstructorType);
		return;
	} else if (hr()) {
		Qn(Zn.TSAbstractConstructorType);
		return;
	}
	ir();
}
function _r() {
	let e = P(1);
	U(), O(u.greaterThan), F(e), ki();
}
function vr() {
	if (I(u.jsxTagStart)) {
		y.tokens[y.tokens.length - 1].type = u.typeParameterStart;
		let e = P(1);
		for (; !L(u.greaterThan) && !y.error;) U(), I(u.comma);
		W(), F(e);
	}
}
function yr() {
	for (; !L(u.braceL) && !y.error;) br(), I(u.comma);
}
function br() {
	wn(), L(u.lessThan) && zr();
}
function xr() {
	dn(!1), jn(), I(u._extends) && yr(), Hn();
}
function Sr() {
	dn(!1), jn(), O(u.eq), U(), D();
}
function Cr() {
	if (L(u.string) ? Hi() : Y(), I(u.eq)) {
		let e = y.tokens.length - 1;
		K(), y.tokens[e].rhsEndIndex = y.tokens.length;
	}
}
function wr() {
	for (dn(!1), O(u.braceL); !I(u.braceR) && !y.error;) Cr(), I(u.comma);
}
function Tr() {
	O(u.braceL), rs(u.braceR);
}
function Er() {
	dn(!1), I(u.dot) ? Er() : Tr();
}
function Dr() {
	x(l._global) ? Y() : L(u.string) ? J() : k(), L(u.braceL) ? Tr() : D();
}
function Or() {
	fn(), O(u.eq), Ar(), D();
}
function kr() {
	return x(l._require) && R() === u.parenL;
}
function Ar() {
	kr() ? jr() : wn();
}
function jr() {
	C(l._require), O(u.parenL), L(u.string) || k(), Hi(), O(u.parenR);
}
function Mr() {
	if (E()) return !1;
	switch (y.type) {
		case u._function: {
			let e = P(1);
			N();
			let t = y.start;
			return cs(t, !0), F(e), !0;
		}
		case u._class: {
			let e = P(1);
			return us(!0, !1), F(e), !0;
		}
		case u._const: if (L(u._const) && oe(l._enum)) {
			let e = P(1);
			return O(u._const), C(l._enum), y.tokens[y.tokens.length - 1].type = u._enum, wr(), F(e), !0;
		}
		case u._var:
		case u._let: {
			let e = P(1);
			return Zo(y.type !== u._var), F(e), !0;
		}
		case u.name: {
			let e = P(1), t = y.contextualKeyword, n = !1;
			return t === l._global ? (Dr(), n = !0) : n = Fr(t, !0), F(e), n;
		}
		default: return !1;
	}
}
function Nr() {
	return Fr(y.contextualKeyword, !0);
}
function Pr(e) {
	switch (e) {
		case l._declare: {
			let e = y.tokens.length - 1;
			if (Mr()) return y.tokens[e].type = u._declare, !0;
			break;
		}
		case l._global:
			if (L(u.braceL)) return Tr(), !0;
			break;
		default: return Fr(e, !1);
	}
	return !1;
}
function Fr(e, t) {
	switch (e) {
		case l._abstract:
			if (Ir(t) && L(u._class)) return y.tokens[y.tokens.length - 1].type = u._abstract, us(!0, !1), !0;
			break;
		case l._enum:
			if (Ir(t) && L(u.name)) return y.tokens[y.tokens.length - 1].type = u._enum, wr(), !0;
			break;
		case l._interface:
			if (Ir(t) && L(u.name)) {
				let e = P(t ? 2 : 1);
				return xr(), F(e), !0;
			}
			break;
		case l._module:
			if (Ir(t)) {
				if (L(u.string)) {
					let e = P(t ? 2 : 1);
					return Dr(), F(e), !0;
				} else if (L(u.name)) {
					let e = P(t ? 2 : 1);
					return Er(), F(e), !0;
				}
			}
			break;
		case l._namespace:
			if (Ir(t) && L(u.name)) {
				let e = P(t ? 2 : 1);
				return Er(), F(e), !0;
			}
			break;
		case l._type:
			if (Ir(t) && L(u.name)) {
				let e = P(t ? 2 : 1);
				return Sr(), F(e), !0;
			}
			break;
		default: break;
	}
	return !1;
}
function Ir(e) {
	return e ? (N(), !0) : !E();
}
function Lr() {
	let e = y.snapshot();
	return Mn(), ls(), ur(), O(u.arrow), y.error ? (y.restoreFromSnapshot(e), !1) : (oa(!0), !0);
}
function Rr() {
	y.type === u.bitShiftL && (--y.pos, z(u.lessThan)), zr();
}
function zr() {
	let e = P(0);
	for (O(u.lessThan); !L(u.greaterThan) && !y.error;) U(), I(u.comma);
	e ? (O(u.greaterThan), F(e)) : (F(e), Ue(), O(u.greaterThan), y.tokens[y.tokens.length - 1].isType = !0);
}
function Br() {
	if (L(u.name)) switch (y.contextualKeyword) {
		case l._abstract:
		case l._declare:
		case l._enum:
		case l._interface:
		case l._module:
		case l._namespace:
		case l._type: return !0;
		default: break;
	}
	return !1;
}
function Vr(e, t) {
	if (L(u.colon) && lr(u.colon), !L(u.braceL) && E()) {
		let t = y.tokens.length - 1;
		for (; t >= 0 && (y.tokens[t].start >= e || y.tokens[t].type === u._default || y.tokens[t].type === u._export);) y.tokens[t].isType = !0, t--;
		return;
	}
	oa(!1, t);
}
function Hr(e, t, n) {
	if (!T() && I(u.bang)) {
		y.tokens[y.tokens.length - 1].type = u.nonNullAssertion;
		return;
	}
	if (L(u.lessThan) || L(u.bitShiftL)) {
		let n = y.snapshot();
		if (!t && Fi() && Lr()) return;
		if (Rr(), !t && I(u.parenL) ? (y.tokens[y.tokens.length - 1].subscriptStartIndex = e, Ii()) : L(u.backQuote) ? Xi() : (y.type === u.greaterThan || y.type !== u.parenL && y.type & u.IS_EXPRESSION_START && !T()) && k(), y.error) y.restoreFromSnapshot(n);
		else return;
	} else !t && L(u.questionDot) && R() === u.lessThan && (N(), y.tokens[e].isOptionalChainStart = !0, y.tokens[y.tokens.length - 1].subscriptStartIndex = e, zr(), O(u.parenL), Ii());
	Pi(e, t, n);
}
function Ur() {
	if (I(u._import)) return x(l._type) && R() !== u.eq && C(l._type), Or(), !0;
	if (I(u.eq)) return G(), D(), !0;
	if (S(l._as)) return C(l._namespace), Y(), D(), !0;
	if (x(l._type)) {
		let e = R();
		(e === u.braceL || e === u.star) && N();
	}
	return !1;
}
function Wr() {
	if (Y(), L(u.comma) || L(u.braceR)) {
		y.tokens[y.tokens.length - 1].identifierRole = j.ImportDeclaration;
		return;
	}
	if (Y(), L(u.comma) || L(u.braceR)) {
		y.tokens[y.tokens.length - 1].identifierRole = j.ImportDeclaration, y.tokens[y.tokens.length - 2].isType = !0, y.tokens[y.tokens.length - 1].isType = !0;
		return;
	}
	if (Y(), L(u.comma) || L(u.braceR)) {
		y.tokens[y.tokens.length - 3].identifierRole = j.ImportAccess, y.tokens[y.tokens.length - 1].identifierRole = j.ImportDeclaration;
		return;
	}
	Y(), y.tokens[y.tokens.length - 3].identifierRole = j.ImportAccess, y.tokens[y.tokens.length - 1].identifierRole = j.ImportDeclaration, y.tokens[y.tokens.length - 4].isType = !0, y.tokens[y.tokens.length - 3].isType = !0, y.tokens[y.tokens.length - 2].isType = !0, y.tokens[y.tokens.length - 1].isType = !0;
}
function Gr() {
	if (Y(), L(u.comma) || L(u.braceR)) {
		y.tokens[y.tokens.length - 1].identifierRole = j.ExportAccess;
		return;
	}
	if (Y(), L(u.comma) || L(u.braceR)) {
		y.tokens[y.tokens.length - 1].identifierRole = j.ExportAccess, y.tokens[y.tokens.length - 2].isType = !0, y.tokens[y.tokens.length - 1].isType = !0;
		return;
	}
	if (Y(), L(u.comma) || L(u.braceR)) {
		y.tokens[y.tokens.length - 3].identifierRole = j.ExportAccess;
		return;
	}
	Y(), y.tokens[y.tokens.length - 3].identifierRole = j.ExportAccess, y.tokens[y.tokens.length - 4].isType = !0, y.tokens[y.tokens.length - 3].isType = !0, y.tokens[y.tokens.length - 2].isType = !0, y.tokens[y.tokens.length - 1].isType = !0;
}
function Kr() {
	if (x(l._abstract) && R() === u._class) return y.type = u._abstract, N(), us(!0, !0), !0;
	if (x(l._interface)) {
		let e = P(2);
		return Fr(l._interface, !0), F(e), !0;
	}
	return !1;
}
function qr() {
	if (y.type === u._const) {
		let e = Ee();
		if (e.type === u.name && e.contextualKeyword === l._enum) return O(u._const), C(l._enum), y.tokens[y.tokens.length - 1].type = u._enum, wr(), !0;
	}
	return !1;
}
function Jr(e) {
	let t = y.tokens.length;
	Sn([
		l._abstract,
		l._readonly,
		l._declare,
		l._static,
		l._override
	]);
	let n = y.tokens.length;
	if (Rn()) {
		let r = e ? t - 1 : t;
		for (let e = r; e < n; e++) y.tokens[e].isType = !0;
		return !0;
	}
	return !1;
}
function Yr(e) {
	Pr(e) || D();
}
function Xr() {
	let e = S(l._declare);
	e && (y.tokens[y.tokens.length - 1].type = u._declare);
	let t = !1;
	if (L(u.name)) if (e) {
		let e = P(2);
		t = Nr(), F(e);
	} else t = Nr();
	if (!t) if (e) {
		let e = P(2);
		$(!0), F(e);
	} else $(!0);
}
function Zr(e) {
	if (e && (L(u.lessThan) || L(u.bitShiftL)) && Rr(), S(l._implements)) {
		y.tokens[y.tokens.length - 1].type = u._implements;
		let e = P(1);
		yr(), F(e);
	}
}
function Qr() {
	jn();
}
function $r() {
	jn();
}
function ei() {
	let e = P(0);
	T() || I(u.bang), dr(), F(e);
}
function ti() {
	L(u.colon) && mr();
}
function ni(e, t) {
	return g ? ri(e, t) : ii(e, t);
}
function ri(e, t) {
	if (!L(u.lessThan)) return q(e, t);
	let n = y.snapshot(), r = q(e, t);
	if (y.error) y.restoreFromSnapshot(n);
	else return r;
	return y.type = u.typeParameterStart, Mn(), r = q(e, t), r || k(), r;
}
function ii(e, t) {
	if (!L(u.lessThan)) return q(e, t);
	let n = y.snapshot();
	Mn();
	let r = q(e, t);
	if (r || k(), y.error) y.restoreFromSnapshot(n);
	else return r;
	return q(e, t);
}
function ai() {
	if (L(u.colon)) {
		let e = y.snapshot();
		lr(u.colon), w() && k(), L(u.arrow) || k(), y.error && y.restoreFromSnapshot(e);
	}
	return I(u.arrow);
}
function oi() {
	let e = P(0);
	I(u.question), dr(), F(e);
}
function si() {
	(L(u.lessThan) || L(u.bitShiftL)) && Rr(), Lo();
}
function ci() {
	let e = !1, t = !1;
	for (;;) {
		if (y.pos >= b.length) {
			k("Unterminated JSX contents");
			return;
		}
		let n = b.charCodeAt(y.pos);
		if (n === h.lessThan || n === h.leftCurlyBrace) {
			if (y.pos === y.start) {
				if (n === h.lessThan) {
					y.pos++, z(u.jsxTagStart);
					return;
				}
				Ke(n);
				return;
			}
			z(e && !t ? u.jsxEmptyText : u.jsxText);
			return;
		}
		n === h.lineFeed ? e = !0 : n !== h.space && n !== h.carriageReturn && n !== h.tab && (t = !0), y.pos++;
	}
}
function li(e) {
	for (y.pos++;;) {
		if (y.pos >= b.length) {
			k("Unterminated string constant");
			return;
		}
		if (b.charCodeAt(y.pos) === e) {
			y.pos++;
			break;
		}
		y.pos++;
	}
	z(u.string);
}
function ui() {
	let e;
	do {
		if (y.pos > b.length) {
			k("Unexpectedly reached the end of input.");
			return;
		}
		e = b.charCodeAt(++y.pos);
	} while (A[e] || e === h.dash);
	z(u.jsxName);
}
function di() {
	W();
}
function fi(e) {
	if (di(), !I(u.colon)) {
		y.tokens[y.tokens.length - 1].identifierRole = e;
		return;
	}
	di();
}
function pi() {
	let e = y.tokens.length;
	fi(j.Access);
	let t = !1;
	for (; L(u.dot);) t = !0, W(), di();
	if (!t) {
		let t = y.tokens[e], n = b.charCodeAt(t.start);
		n >= h.lowercaseA && n <= h.lowercaseZ && (t.identifierRole = null);
	}
}
function mi() {
	switch (y.type) {
		case u.braceL:
			N(), G(), W();
			return;
		case u.jsxTagStart:
			yi(), W();
			return;
		case u.string:
			W();
			return;
		default: k("JSX value should be either an expression or a quoted JSX text");
	}
}
function hi() {
	O(u.ellipsis), G();
}
function gi(e) {
	if (L(u.jsxTagEnd)) return !1;
	pi(), _ && vr();
	let t = !1;
	for (; !L(u.slash) && !L(u.jsxTagEnd) && !y.error;) {
		if (I(u.braceL)) {
			t = !0, O(u.ellipsis), K(), W();
			continue;
		}
		t && y.end - y.start === 3 && b.charCodeAt(y.start) === h.lowercaseK && b.charCodeAt(y.start + 1) === h.lowercaseE && b.charCodeAt(y.start + 2) === h.lowercaseY && (y.tokens[e].jsxRole = M.KeyAfterPropSpread), fi(j.ObjectKey), L(u.eq) && (W(), mi());
	}
	let n = L(u.slash);
	return n && W(), n;
}
function _i() {
	L(u.jsxTagEnd) || pi();
}
function vi() {
	let e = y.tokens.length - 1;
	y.tokens[e].jsxRole = M.NoChildren;
	let t = 0;
	if (!gi(e)) for (bi();;) switch (y.type) {
		case u.jsxTagStart:
			if (W(), L(u.slash)) {
				W(), _i(), y.tokens[e].jsxRole !== M.KeyAfterPropSpread && (t === 1 ? y.tokens[e].jsxRole = M.OneChild : t > 1 && (y.tokens[e].jsxRole = M.StaticChildren));
				return;
			}
			t++, vi(), bi();
			break;
		case u.jsxText:
			t++, bi();
			break;
		case u.jsxEmptyText:
			bi();
			break;
		case u.braceL:
			N(), L(u.ellipsis) ? (hi(), bi(), t += 2) : (L(u.braceR) || (t++, G()), bi());
			break;
		// istanbul ignore next - should never happen
		default:
			k();
			return;
	}
}
function yi() {
	W(), vi();
}
function W() {
	y.tokens.push(new xe()), Pe(), y.start = y.pos;
	let e = b.charCodeAt(y.pos);
	if (fe[e]) ui();
	else if (e === h.quotationMark || e === h.apostrophe) li(e);
	else switch (++y.pos, e) {
		case h.greaterThan:
			z(u.jsxTagEnd);
			break;
		case h.lessThan:
			z(u.jsxTagStart);
			break;
		case h.slash:
			z(u.slash);
			break;
		case h.equalsTo:
			z(u.eq);
			break;
		case h.leftCurlyBrace:
			z(u.braceL);
			break;
		case h.dot:
			z(u.dot);
			break;
		case h.colon:
			z(u.colon);
			break;
		default: k();
	}
}
function bi() {
	y.tokens.push(new xe()), y.start = y.pos, ci();
}
function xi(e) {
	if (L(u.question)) {
		let e = R();
		if (e === u.colon || e === u.comma || e === u.parenR) return;
	}
	Ei(e);
}
function Si() {
	we(u.question), L(u.colon) && (_ ? mr() : v && eo());
}
var Ci = class {
	constructor(e) {
		this.stop = e;
	}
};
function G(e = !1) {
	if (K(e), L(u.comma)) for (; I(u.comma);) K(e);
}
function K(e = !1, t = !1) {
	return _ ? ni(e, t) : v ? Co(e, t) : q(e, t);
}
function q(e, t) {
	if (L(u._yield)) return ua(), !1;
	(L(u.parenL) || L(u.name) || L(u._yield)) && (y.potentialArrowAt = y.start);
	let n = wi(e);
	return t && qi(), y.type & u.IS_ASSIGN ? (N(), K(e), !1) : n;
}
function wi(e) {
	return Di(e) ? !0 : (Ti(e), !1);
}
function Ti(e) {
	_ || v ? xi(e) : Ei(e);
}
function Ei(e) {
	I(u.question) && (K(), O(u.colon), K(e));
}
function Di(e) {
	let t = y.tokens.length;
	return ki() ? !0 : (Oi(t, -1, e), !1);
}
function Oi(e, t, n) {
	if (_ && (u._in & u.PRECEDENCE_MASK) > t && !T() && (S(l._as) || S(l._satisfies))) {
		let r = P(1);
		U(), F(r), Ue(), Oi(e, t, n);
		return;
	}
	let r = y.type & u.PRECEDENCE_MASK;
	if (r > 0 && (!n || !L(u._in)) && r > t) {
		let i = y.type;
		N(), i === u.nullishCoalescing && (y.tokens[y.tokens.length - 1].nullishStartIndex = e);
		let a = y.tokens.length;
		ki(), Oi(a, i & u.IS_RIGHT_ASSOCIATIVE ? r - 1 : r, n), i === u.nullishCoalescing && (y.tokens[e].numNullishCoalesceStarts++, y.tokens[y.tokens.length - 1].numNullishCoalesceEnds++), Oi(e, t, n);
	}
}
function ki() {
	if (_ && !g && I(u.lessThan)) return _r(), !1;
	if (x(l._module) && ke() === h.leftCurlyBrace && !se()) return da(), !1;
	if (y.type & u.IS_PREFIX) return N(), ki(), !1;
	if (Ai()) return !0;
	for (; y.type & u.IS_POSTFIX && !w();) y.type === u.preIncDec && (y.type = u.postIncDec), N();
	return !1;
}
function Ai() {
	let e = y.tokens.length;
	return J() ? !0 : (ji(e), y.tokens.length > e && y.tokens[e].isOptionalChainStart && (y.tokens[y.tokens.length - 1].isOptionalChainEnd = !0), !1);
}
function ji(e, t = !1) {
	v ? To(e, t) : Mi(e, t);
}
function Mi(e, t = !1) {
	let n = new Ci(!1);
	do
		Ni(e, t, n);
	while (!n.stop && !y.error);
}
function Ni(e, t, n) {
	_ ? Hr(e, t, n) : v ? io(e, t, n) : Pi(e, t, n);
}
function Pi(e, t, n) {
	if (!t && I(u.doubleColon)) zi(), n.stop = !0, ji(e, t);
	else if (L(u.questionDot)) {
		if (y.tokens[e].isOptionalChainStart = !0, t && R() === u.parenL) {
			n.stop = !0;
			return;
		}
		N(), y.tokens[y.tokens.length - 1].subscriptStartIndex = e, I(u.bracketL) ? (G(), O(u.bracketR)) : I(u.parenL) ? Ii() : Bi();
	} else if (I(u.dot)) y.tokens[y.tokens.length - 1].subscriptStartIndex = e, Bi();
	else if (I(u.bracketL)) y.tokens[y.tokens.length - 1].subscriptStartIndex = e, G(), O(u.bracketR);
	else if (!t && L(u.parenL)) if (Fi()) {
		let t = y.snapshot(), r = y.tokens.length;
		N(), y.tokens[y.tokens.length - 1].subscriptStartIndex = e;
		let i = te();
		y.tokens[y.tokens.length - 1].contextId = i, Ii(), y.tokens[y.tokens.length - 1].contextId = i, Li() && (y.restoreFromSnapshot(t), n.stop = !0, y.scopeDepth++, ls(), Ri(r));
	} else {
		N(), y.tokens[y.tokens.length - 1].subscriptStartIndex = e;
		let t = te();
		y.tokens[y.tokens.length - 1].contextId = t, Ii(), y.tokens[y.tokens.length - 1].contextId = t;
	}
	else L(u.backQuote) ? Xi() : n.stop = !0;
}
function Fi() {
	return y.tokens[y.tokens.length - 1].contextualKeyword === l._async && !w();
}
function Ii() {
	let e = !0;
	for (; !I(u.parenR) && !y.error;) {
		if (e) e = !1;
		else if (O(u.comma), I(u.parenR)) break;
		ca(!1);
	}
}
function Li() {
	return L(u.colon) || L(u.arrow);
}
function Ri(e) {
	_ ? ti() : v && So(), O(u.arrow), ia(e);
}
function zi() {
	let e = y.tokens.length;
	J(), ji(e, !0);
}
function J() {
	if (I(u.modulo)) return Y(), !1;
	if (L(u.jsxText) || L(u.jsxEmptyText)) return Hi(), !1;
	if (L(u.lessThan) && g) return y.type = u.jsxTagStart, yi(), N(), !1;
	let e = y.potentialArrowAt === y.start;
	switch (y.type) {
		case u.slash:
		case u.assign: Ce();
		case u._super:
		case u._this:
		case u.regexp:
		case u.num:
		case u.bigint:
		case u.decimal:
		case u.string:
		case u._null:
		case u._true:
		case u._false: return N(), !1;
		case u._import: return N(), L(u.dot) && (y.tokens[y.tokens.length - 1].type = u.name, N(), Y()), !1;
		case u.name: {
			let t = y.tokens.length, n = y.start, r = y.contextualKeyword;
			return Y(), r === l._await ? (la(), !1) : r === l._async && L(u._function) && !w() ? (N(), cs(n, !1), !1) : e && r === l._async && !w() && L(u.name) ? (y.scopeDepth++, dn(!1), O(u.arrow), ia(t), !0) : L(u._do) && !w() ? (N(), ns(), !1) : e && !w() && L(u.arrow) ? (y.scopeDepth++, pn(!1), O(u.arrow), ia(t), !0) : (y.tokens[y.tokens.length - 1].identifierRole = j.Access, !1);
		}
		case u._do: return N(), ns(), !1;
		case u.parenL: return Wi(e);
		case u.bracketL: return N(), sa(u.bracketR, !0), !1;
		case u.braceL: return Zi(!1, !1), !1;
		case u._function: return Vi(), !1;
		case u.at: Po();
		case u._class: return us(!1), !1;
		case u._new: return Ji(), !1;
		case u.backQuote: return Xi(), !1;
		case u.doubleColon: return N(), zi(), !1;
		case u.hash: {
			let e = ke();
			return fe[e] || e === h.backslash ? Bi() : N(), !1;
		}
		default: return k(), !1;
	}
}
function Bi() {
	I(u.hash), Y();
}
function Vi() {
	let e = y.start;
	Y(), I(u.dot) && Y(), cs(e, !1);
}
function Hi() {
	N();
}
function Ui() {
	O(u.parenL), G(), O(u.parenR);
}
function Wi(e) {
	let t = y.snapshot(), n = y.tokens.length;
	O(u.parenL);
	let r = !0;
	for (; !L(u.parenR) && !y.error;) {
		if (r) r = !1;
		else if (O(u.comma), L(u.parenR)) break;
		if (L(u.ellipsis)) {
			un(!1), qi();
			break;
		} else K(!1, !0);
	}
	return O(u.parenR), e && Gi() && Ki() ? (y.restoreFromSnapshot(t), y.scopeDepth++, ls(), Ki(), ia(n), y.error ? (y.restoreFromSnapshot(t), Wi(!1), !1) : !0) : !1;
}
function Gi() {
	return L(u.colon) || !w();
}
function Ki() {
	return _ ? ai() : v ? wo() : I(u.arrow);
}
function qi() {
	(_ || v) && Si();
}
function Ji() {
	if (O(u._new), I(u.dot)) {
		Y();
		return;
	}
	Yi(), v && ao(), I(u.parenL) && sa(u.parenR);
}
function Yi() {
	zi(), I(u.questionDot);
}
function Xi() {
	for (Se(), Se(); !L(u.backQuote) && !y.error;) O(u.dollarBraceL), G(), Se(), Se();
	N();
}
function Zi(e, t) {
	let n = te(), r = !0;
	for (N(), y.tokens[y.tokens.length - 1].contextId = n; !I(u.braceR) && !y.error;) {
		if (r) r = !1;
		else if (O(u.comma), I(u.braceR)) break;
		let i = !1;
		if (L(u.ellipsis)) {
			let n = y.tokens.length;
			if (ln(), e && (y.tokens.length === n + 2 && pn(t), I(u.braceR))) break;
			continue;
		}
		e || (i = I(u.star)), !e && x(l._async) ? (i && k(), Y(), L(u.colon) || L(u.parenL) || L(u.braceR) || L(u.eq) || L(u.comma) || (L(u.star) && (N(), i = !0), na(n))) : na(n), ta(e, t, n);
	}
	y.tokens[y.tokens.length - 1].contextId = n;
}
function Qi(e) {
	return !e && (L(u.string) || L(u.num) || L(u.bracketL) || L(u.name) || !!(y.type & u.IS_KEYWORD));
}
function $i(e, t) {
	let n = y.start;
	return L(u.parenL) ? (e && k(), ra(n, !1), !0) : Qi(e) ? (na(t), ra(n, !1), !0) : !1;
}
function ea(e, t) {
	if (I(u.colon)) {
		e ? vn(t) : K(!1);
		return;
	}
	let n;
	n = e ? y.scopeDepth === 0 ? j.ObjectShorthandTopLevelDeclaration : t ? j.ObjectShorthandBlockScopedDeclaration : j.ObjectShorthandFunctionScopedDeclaration : j.ObjectShorthand, y.tokens[y.tokens.length - 1].identifierRole = n, vn(t, !0);
}
function ta(e, t, n) {
	_ ? Qr() : v && go(), $i(e, n) || ea(e, t);
}
function na(e) {
	v && no(), I(u.bracketL) ? (y.tokens[y.tokens.length - 1].contextId = e, K(), O(u.bracketR), y.tokens[y.tokens.length - 1].contextId = e) : (L(u.num) || L(u.string) || L(u.bigint) || L(u.decimal) ? J() : Bi(), y.tokens[y.tokens.length - 1].identifierRole = j.ObjectKey, y.tokens[y.tokens.length - 1].contextId = e);
}
function ra(e, t) {
	let n = te();
	y.scopeDepth++;
	let r = y.tokens.length;
	ls(t, n), aa(e, n);
	let i = y.tokens.length;
	y.scopes.push(new f(r, i, !0)), y.scopeDepth--;
}
function ia(e) {
	oa(!0);
	let t = y.tokens.length;
	y.scopes.push(new f(e, t, !0)), y.scopeDepth--;
}
function aa(e, t = 0) {
	_ ? Vr(e, t) : v ? ro(t) : oa(!1, t);
}
function oa(e, t = 0) {
	e && !L(u.braceL) ? K() : ns(!0, t);
}
function sa(e, t = !1) {
	let n = !0;
	for (; !I(e) && !y.error;) {
		if (n) n = !1;
		else if (O(u.comma), I(e)) break;
		ca(t);
	}
}
function ca(e) {
	e && L(u.comma) || (L(u.ellipsis) ? (ln(), qi()) : L(u.question) ? N() : K(!1, !0));
}
function Y() {
	N(), y.tokens[y.tokens.length - 1].type = u.name;
}
function la() {
	ki();
}
function ua() {
	N(), !L(u.semi) && !w() && (I(u.star), K());
}
function da() {
	C(l._module), O(u.braceL), rs(u.braceR);
}
function fa(e) {
	return (e.type === u.name || !!(e.type & u.IS_KEYWORD)) && e.contextualKeyword !== l._from;
}
function X(e) {
	let t = P(0);
	O(e || u.colon), Q(), F(t);
}
function pa() {
	O(u.modulo), C(l._checks), I(u.parenL) && (G(), O(u.parenR));
}
function ma() {
	let e = P(0);
	O(u.colon), L(u.modulo) ? pa() : (Q(), L(u.modulo) && pa()), F(e);
}
function ha() {
	N(), Ta(!0);
}
function ga() {
	N(), Y(), L(u.lessThan) && Z(), O(u.parenL), qa(), O(u.parenR), ma(), D();
}
function _a() {
	L(u._class) ? ha() : L(u._function) ? ga() : L(u._var) ? va() : S(l._module) ? I(u.dot) ? xa() : ya() : x(l._type) ? Sa() : x(l._opaque) ? Ca() : x(l._interface) ? wa() : L(u._export) ? ba() : k();
}
function va() {
	N(), to(), D();
}
function ya() {
	for (L(u.string) ? J() : Y(), O(u.braceL); !L(u.braceR) && !y.error;) L(u._import) ? (N(), Ls()) : k();
	O(u.braceR);
}
function ba() {
	O(u._export), I(u._default) ? L(u._function) || L(u._class) ? _a() : (Q(), D()) : L(u._var) || L(u._function) || L(u._class) || x(l._opaque) ? _a() : L(u.star) || L(u.braceL) || x(l._interface) || x(l._type) || x(l._opaque) ? Ss() : k();
}
function xa() {
	C(l._exports), eo(), D();
}
function Sa() {
	N(), ka();
}
function Ca() {
	N(), Aa(!0);
}
function wa() {
	N(), Ta();
}
function Ta(e = !1) {
	if (Oa(), L(u.lessThan) && Z(), I(u._extends)) do
		Ea();
	while (!e && I(u.comma));
	if (x(l._mixins)) {
		N();
		do
			Ea();
		while (I(u.comma));
	}
	if (x(l._implements)) {
		N();
		do
			Ea();
		while (I(u.comma));
	}
	za(e, !1, e);
}
function Ea() {
	Ha(!1), L(u.lessThan) && Ma();
}
function Da() {
	Ta();
}
function Oa() {
	Y();
}
function ka() {
	Oa(), L(u.lessThan) && Z(), X(u.eq), D();
}
function Aa(e) {
	C(l._type), Oa(), L(u.lessThan) && Z(), L(u.colon) && X(u.colon), e || X(u.eq), D();
}
function ja() {
	no(), to(), I(u.eq) && Q();
}
function Z() {
	let e = P(0);
	// istanbul ignore else: this condition is already checked at all call sites
	L(u.lessThan) || L(u.typeParameterStart) ? N() : k();
	do
		ja(), L(u.greaterThan) || O(u.comma);
	while (!L(u.greaterThan) && !y.error);
	O(u.greaterThan), F(e);
}
function Ma() {
	let e = P(0);
	for (O(u.lessThan); !L(u.greaterThan) && !y.error;) Q(), L(u.greaterThan) || O(u.comma);
	O(u.greaterThan), F(e);
}
function Na() {
	if (C(l._interface), I(u._extends)) do
		Ea();
	while (I(u.comma));
	za(!1, !1, !1);
}
function Pa() {
	L(u.num) || L(u.string) ? J() : Y();
}
function Fa() {
	R() === u.colon ? (Pa(), X()) : Q(), O(u.bracketR), X();
}
function Ia() {
	Pa(), O(u.bracketR), O(u.bracketR), L(u.lessThan) || L(u.parenL) ? La() : (I(u.question), X());
}
function La() {
	for (L(u.lessThan) && Z(), O(u.parenL); !L(u.parenR) && !L(u.ellipsis) && !y.error;) Ka(), L(u.parenR) || O(u.comma);
	I(u.ellipsis) && Ka(), O(u.parenR), X();
}
function Ra() {
	La();
}
function za(e, t, n) {
	let r;
	for (t && L(u.braceBarL) ? (O(u.braceBarL), r = u.braceBarR) : (O(u.braceL), r = u.braceR); !L(r) && !y.error;) {
		if (n && x(l._proto)) {
			let t = R();
			t !== u.colon && t !== u.question && (N(), e = !1);
		}
		if (e && x(l._static)) {
			let e = R();
			e !== u.colon && e !== u.question && N();
		}
		if (no(), I(u.bracketL)) I(u.bracketL) ? Ia() : Fa();
		else if (L(u.parenL) || L(u.lessThan)) Ra();
		else {
			if (x(l._get) || x(l._set)) {
				let e = R();
				(e === u.name || e === u.string || e === u.num) && N();
			}
			Ba();
		}
		Va();
	}
	O(r);
}
function Ba() {
	if (L(u.ellipsis)) {
		if (O(u.ellipsis), I(u.comma) || I(u.semi), L(u.braceR)) return;
		Q();
	} else Pa(), L(u.lessThan) || L(u.parenL) ? La() : (I(u.question), X());
}
function Va() {
	!I(u.semi) && !I(u.comma) && !L(u.braceR) && !L(u.braceBarR) && k();
}
function Ha(e) {
	for (e || Y(); I(u.dot);) Y();
}
function Ua() {
	Ha(!0), L(u.lessThan) && Ma();
}
function Wa() {
	O(u._typeof), Ja();
}
function Ga() {
	for (O(u.bracketL); y.pos < b.length && !L(u.bracketR) && (Q(), !L(u.bracketR));) O(u.comma);
	O(u.bracketR);
}
function Ka() {
	let e = R();
	e === u.colon || e === u.question ? (Y(), I(u.question), X()) : Q();
}
function qa() {
	for (; !L(u.parenR) && !L(u.ellipsis) && !y.error;) Ka(), L(u.parenR) || O(u.comma);
	I(u.ellipsis) && Ka();
}
function Ja() {
	let e = !1, t = y.noAnonFunctionType;
	switch (y.type) {
		case u.name:
			if (x(l._interface)) {
				Na();
				return;
			}
			Y(), Ua();
			return;
		case u.braceL:
			za(!1, !1, !1);
			return;
		case u.braceBarL:
			za(!1, !0, !1);
			return;
		case u.bracketL:
			Ga();
			return;
		case u.lessThan:
			Z(), O(u.parenL), qa(), O(u.parenR), O(u.arrow), Q();
			return;
		case u.parenL:
			if (N(), !L(u.parenR) && !L(u.ellipsis)) if (L(u.name)) {
				let t = R();
				e = t !== u.question && t !== u.colon;
			} else e = !0;
			if (e) if (y.noAnonFunctionType = !1, Q(), y.noAnonFunctionType = t, y.noAnonFunctionType || !(L(u.comma) || L(u.parenR) && R() === u.arrow)) {
				O(u.parenR);
				return;
			} else I(u.comma);
			qa(), O(u.parenR), O(u.arrow), Q();
			return;
		case u.minus:
			N(), Hi();
			return;
		case u.string:
		case u.num:
		case u._true:
		case u._false:
		case u._null:
		case u._this:
		case u._void:
		case u.star:
			N();
			return;
		default: if (y.type === u._typeof) {
			Wa();
			return;
		} else if (y.type & u.IS_KEYWORD) {
			N(), y.tokens[y.tokens.length - 1].type = u.name;
			return;
		}
	}
	k();
}
function Ya() {
	for (Ja(); !w() && (L(u.bracketL) || L(u.questionDot));) I(u.questionDot), O(u.bracketL), I(u.bracketR) || (Q(), O(u.bracketR));
}
function Xa() {
	I(u.question) ? Xa() : Ya();
}
function Za() {
	Xa(), !y.noAnonFunctionType && I(u.arrow) && Q();
}
function Qa() {
	for (I(u.bitwiseAND), Za(); I(u.bitwiseAND);) Za();
}
function $a() {
	for (I(u.bitwiseOR), Qa(); I(u.bitwiseOR);) Qa();
}
function Q() {
	$a();
}
function eo() {
	X();
}
function to() {
	Y(), L(u.colon) && eo();
}
function no() {
	(L(u.plus) || L(u.minus)) && (N(), y.tokens[y.tokens.length - 1].isType = !0);
}
function ro(e) {
	L(u.colon) && ma(), oa(!1, e);
}
function io(e, t, n) {
	if (L(u.questionDot) && R() === u.lessThan) {
		if (t) {
			n.stop = !0;
			return;
		}
		N(), Ma(), O(u.parenL), Ii();
		return;
	} else if (!t && L(u.lessThan)) {
		let e = y.snapshot();
		if (Ma(), O(u.parenL), Ii(), y.error) y.restoreFromSnapshot(e);
		else return;
	}
	Pi(e, t, n);
}
function ao() {
	if (L(u.lessThan)) {
		let e = y.snapshot();
		Ma(), y.error && y.restoreFromSnapshot(e);
	}
}
function oo() {
	if (L(u.name) && y.contextualKeyword === l._interface) {
		let e = P(0);
		return N(), Da(), F(e), !0;
	} else if (x(l._enum)) return Do(), !0;
	return !1;
}
function so() {
	return x(l._enum) ? (Do(), !0) : !1;
}
function co(e) {
	if (e === l._declare) {
		if (L(u._class) || L(u.name) || L(u._function) || L(u._var) || L(u._export)) {
			let e = P(1);
			_a(), F(e);
		}
	} else if (L(u.name)) {
		if (e === l._interface) {
			let e = P(1);
			Da(), F(e);
		} else if (e === l._type) {
			let e = P(1);
			ka(), F(e);
		} else if (e === l._opaque) {
			let e = P(1);
			Aa(!1), F(e);
		}
	}
	D();
}
function lo() {
	return x(l._type) || x(l._interface) || x(l._opaque) || x(l._enum);
}
function uo() {
	return L(u.name) && (y.contextualKeyword === l._type || y.contextualKeyword === l._interface || y.contextualKeyword === l._opaque || y.contextualKeyword === l._enum);
}
function fo() {
	if (x(l._type)) {
		let e = P(1);
		N(), L(u.braceL) ? (Ns(), Ds()) : ka(), F(e);
	} else if (x(l._opaque)) {
		let e = P(1);
		N(), Aa(!1), F(e);
	} else if (x(l._interface)) {
		let e = P(1);
		N(), Da(), F(e);
	} else $(!0);
}
function po() {
	return L(u.star) || x(l._type) && R() === u.star;
}
function mo() {
	if (S(l._type)) {
		let e = P(2);
		As(), F(e);
	} else As();
}
function ho(e) {
	if (e && L(u.lessThan) && Ma(), x(l._implements)) {
		let e = P(0);
		N(), y.tokens[y.tokens.length - 1].type = u._implements;
		do
			Oa(), L(u.lessThan) && Ma();
		while (I(u.comma));
		F(e);
	}
}
function go() {
	L(u.lessThan) && (Z(), L(u.parenL) || k());
}
function _o() {
	let e = P(0);
	I(u.question), L(u.colon) && eo(), F(e);
}
function vo() {
	if (L(u._typeof) || x(l._type)) {
		let e = Ee();
		(fa(e) || e.type === u.braceL || e.type === u.star) && N();
	}
}
function yo() {
	let e = y.contextualKeyword === l._type || y.type === u._typeof;
	e ? N() : Y(), x(l._as) && !oe(l._as) ? (Y(), e && !L(u.name) && !(y.type & u.IS_KEYWORD) || Y()) : (e && (L(u.name) || y.type & u.IS_KEYWORD) && Y(), S(l._as) && Y());
}
function bo() {
	if (L(u.lessThan)) {
		let e = P(0);
		Z(), F(e);
	}
}
function xo() {
	L(u.colon) && eo();
}
function So() {
	if (L(u.colon)) {
		let e = y.noAnonFunctionType;
		y.noAnonFunctionType = !0, eo(), y.noAnonFunctionType = e;
	}
}
function Co(e, t) {
	if (L(u.lessThan)) {
		let n = y.snapshot(), r = q(e, t);
		if (y.error) y.restoreFromSnapshot(n), y.type = u.typeParameterStart;
		else return r;
		let i = P(0);
		if (Z(), F(i), r = q(e, t), r) return !0;
		k();
	}
	return q(e, t);
}
function wo() {
	if (L(u.colon)) {
		let e = P(0), t = y.snapshot(), n = y.noAnonFunctionType;
		y.noAnonFunctionType = !0, ma(), y.noAnonFunctionType = n, w() && k(), L(u.arrow) || k(), y.error && y.restoreFromSnapshot(t), F(e);
	}
	return I(u.arrow);
}
function To(e, t = !1) {
	if (y.tokens[y.tokens.length - 1].contextualKeyword === l._async && L(u.lessThan)) {
		let e = y.snapshot();
		if (Eo() && !y.error) return;
		y.restoreFromSnapshot(e);
	}
	Mi(e, t);
}
function Eo() {
	y.scopeDepth++;
	let e = y.tokens.length;
	return ls(), Ki() ? (ia(e), !0) : !1;
}
function Do() {
	C(l._enum), y.tokens[y.tokens.length - 1].type = u._enum, Y(), Oo();
}
function Oo() {
	S(l._of) && N(), O(u.braceL), ko(), O(u.braceR);
}
function ko() {
	for (; !L(u.braceR) && !y.error && !I(u.ellipsis);) Ao(), L(u.braceR) || O(u.comma);
}
function Ao() {
	Y(), I(u.eq) && N();
}
function jo() {
	if (rs(u.eof), y.scopes.push(new f(0, y.tokens.length, !0)), y.scopeDepth !== 0) throw Error(`Invalid scope depth at end of file: ${y.scopeDepth}`);
	return new Ws(y.tokens, y.scopes);
}
function $(e) {
	v && oo() || (L(u.at) && Po(), Mo(e));
}
function Mo(e) {
	if (_ && qr()) return;
	let t = y.type;
	switch (t) {
		case u._break:
		case u._continue:
			Ro();
			return;
		case u._debugger:
			zo();
			return;
		case u._do:
			Bo();
			return;
		case u._for:
			Vo();
			return;
		case u._function:
			if (R() === u.dot) break;
			e || k(), Wo();
			return;
		case u._class:
			e || k(), us(!0);
			return;
		case u._if:
			Go();
			return;
		case u._return:
			Ko();
			return;
		case u._switch:
			qo();
			return;
		case u._throw:
			Jo();
			return;
		case u._try:
			Xo();
			return;
		case u._let:
		case u._const: e || k();
		case u._var:
			Zo(t !== u._var);
			return;
		case u._while:
			Qo();
			return;
		case u.braceL:
			ns();
			return;
		case u.semi:
			$o();
			return;
		case u._export:
		case u._import: {
			let e = R();
			if (e === u.parenL || e === u.dot) break;
			N(), t === u._import ? Ls() : Ss();
			return;
		}
		case u.name: if (y.contextualKeyword === l._async) {
			let e = y.start, t = y.snapshot();
			if (N(), L(u._function) && !w()) {
				O(u._function), cs(e, !0);
				return;
			} else y.restoreFromSnapshot(t);
		} else if (y.contextualKeyword === l._using && !se() && R() === u.name) {
			Zo(!0);
			return;
		} else if (No()) {
			C(l._await), Zo(!0);
			return;
		}
		default: break;
	}
	let n = y.tokens.length;
	G();
	let r = null;
	if (y.tokens.length === n + 1) {
		let e = y.tokens[y.tokens.length - 1];
		e.type === u.name && (r = e.contextualKeyword);
	}
	if (r == null) {
		D();
		return;
	}
	I(u.colon) ? es() : ts(r);
}
function No() {
	if (!x(l._await)) return !1;
	let e = y.snapshot();
	return N(), !x(l._using) || T() || (N(), !L(u.name) || T()) ? (y.restoreFromSnapshot(e), !1) : (y.restoreFromSnapshot(e), !0);
}
function Po() {
	for (; L(u.at);) Fo();
}
function Fo() {
	if (N(), I(u.parenL)) G(), O(u.parenR);
	else {
		for (Y(); I(u.dot);) Y();
		Io();
	}
}
function Io() {
	_ ? si() : Lo();
}
function Lo() {
	I(u.parenL) && Ii();
}
function Ro() {
	N(), E() || (Y(), D());
}
function zo() {
	N(), D();
}
function Bo() {
	N(), $(!1), O(u._while), Ui(), I(u.semi);
}
function Vo() {
	y.scopeDepth++;
	let e = y.tokens.length;
	Uo();
	let t = y.tokens.length;
	y.scopes.push(new f(e, t, !1)), y.scopeDepth--;
}
function Ho() {
	return !(!x(l._using) || oe(l._of));
}
function Uo() {
	N();
	let e = !1;
	if (x(l._await) && (e = !0, N()), O(u.parenL), L(u.semi)) {
		e && k(), is();
		return;
	}
	let t = No();
	if (t || L(u._var) || L(u._let) || L(u._const) || Ho()) {
		if (t && C(l._await), N(), os(!0, y.type !== u._var), L(u._in) || x(l._of)) {
			as(e);
			return;
		}
		is();
		return;
	}
	if (G(!0), L(u._in) || x(l._of)) {
		as(e);
		return;
	}
	e && k(), is();
}
function Wo() {
	let e = y.start;
	N(), cs(e, !0);
}
function Go() {
	N(), Ui(), $(!1), I(u._else) && $(!1);
}
function Ko() {
	N(), E() || (G(), D());
}
function qo() {
	N(), Ui(), y.scopeDepth++;
	let e = y.tokens.length;
	for (O(u.braceL); !L(u.braceR) && !y.error;) if (L(u._case) || L(u._default)) {
		let e = L(u._case);
		N(), e && G(), O(u.colon);
	} else $(!0);
	N();
	let t = y.tokens.length;
	y.scopes.push(new f(e, t, !1)), y.scopeDepth--;
}
function Jo() {
	N(), G(), D();
}
function Yo() {
	mn(!0), _ && dr();
}
function Xo() {
	if (N(), ns(), L(u._catch)) {
		N();
		let e = null;
		if (L(u.parenL) && (y.scopeDepth++, e = y.tokens.length, O(u.parenL), Yo(), O(u.parenR)), ns(), e != null) {
			let t = y.tokens.length;
			y.scopes.push(new f(e, t, !1)), y.scopeDepth--;
		}
	}
	I(u._finally) && ns();
}
function Zo(e) {
	N(), os(!1, e), D();
}
function Qo() {
	N(), Ui(), $(!1);
}
function $o() {
	N();
}
function es() {
	$(!0);
}
function ts(e) {
	_ ? Yr(e) : v ? co(e) : D();
}
function ns(e = !1, t = 0) {
	let n = y.tokens.length;
	y.scopeDepth++, O(u.braceL), t && (y.tokens[y.tokens.length - 1].contextId = t), rs(u.braceR), t && (y.tokens[y.tokens.length - 1].contextId = t);
	let r = y.tokens.length;
	y.scopes.push(new f(n, r, e)), y.scopeDepth--;
}
function rs(e) {
	for (; !I(e) && !y.error;) $(!0);
}
function is() {
	O(u.semi), L(u.semi) || G(), O(u.semi), L(u.parenR) || G(), O(u.parenR), $(!1);
}
function as(e) {
	e ? S(l._of) : N(), G(), O(u.parenR), $(!1);
}
function os(e, t) {
	for (;;) {
		if (ss(t), I(u.eq)) {
			let t = y.tokens.length - 1;
			K(e), y.tokens[t].rhsEndIndex = y.tokens.length;
		}
		if (!I(u.comma)) break;
	}
}
function ss(e) {
	mn(e), _ ? ei() : v && xo();
}
function cs(e, t, n = !1) {
	L(u.star) && N(), t && !n && !L(u.name) && !L(u._yield) && k();
	let r = null;
	L(u.name) && (t || (r = y.tokens.length, y.scopeDepth++), dn(!1));
	let i = y.tokens.length;
	y.scopeDepth++, ls(), aa(e);
	let a = y.tokens.length;
	y.scopes.push(new f(i, a, !0)), y.scopeDepth--, r !== null && (y.scopes.push(new f(r, a, !0)), y.scopeDepth--);
}
function ls(e = !1, t = 0) {
	_ ? $r() : v && bo(), O(u.parenL), t && (y.tokens[y.tokens.length - 1].contextId = t), hn(u.parenR, !1, !1, e, t), t && (y.tokens[y.tokens.length - 1].contextId = t);
}
function us(e, t = !1) {
	let n = te();
	N(), y.tokens[y.tokens.length - 1].contextId = n, y.tokens[y.tokens.length - 1].isExpression = !e;
	let r = null;
	e || (r = y.tokens.length, y.scopeDepth++), bs(e, t), xs();
	let i = y.tokens.length;
	if (ps(n), !y.error && (y.tokens[i].contextId = n, y.tokens[y.tokens.length - 1].contextId = n, r !== null)) {
		let e = y.tokens.length;
		y.scopes.push(new f(r, e, !1)), y.scopeDepth--;
	}
}
function ds() {
	return L(u.eq) || L(u.semi) || L(u.braceR) || L(u.bang) || L(u.colon);
}
function fs() {
	return L(u.parenL) || L(u.lessThan);
}
function ps(e) {
	for (O(u.braceL); !I(u.braceR) && !y.error;) {
		if (I(u.semi)) continue;
		if (L(u.at)) {
			Fo();
			continue;
		}
		let t = y.start;
		ms(t, e);
	}
}
function ms(e, t) {
	_ && Sn([
		l._declare,
		l._public,
		l._protected,
		l._private,
		l._override
	]);
	let n = !1;
	if (L(u.name) && y.contextualKeyword === l._static) {
		if (Y(), fs()) {
			gs(e, !1);
			return;
		} else if (ds()) {
			ys();
			return;
		}
		if (y.tokens[y.tokens.length - 1].type = u._static, n = !0, L(u.braceL)) {
			y.tokens[y.tokens.length - 1].contextId = t, ns();
			return;
		}
	}
	hs(e, n, t);
}
function hs(e, t, n) {
	if (_ && Jr(t)) return;
	if (I(u.star)) {
		_s(n), gs(e, !1);
		return;
	}
	_s(n);
	let r = !1, i = y.tokens[y.tokens.length - 1];
	i.contextualKeyword === l._constructor && (r = !0), vs(), fs() ? gs(e, r) : ds() ? ys() : i.contextualKeyword === l._async && !E() ? (y.tokens[y.tokens.length - 1].type = u._async, L(u.star) && N(), _s(n), vs(), gs(e, !1)) : (i.contextualKeyword === l._get || i.contextualKeyword === l._set) && !(E() && L(u.star)) ? (i.contextualKeyword === l._get ? y.tokens[y.tokens.length - 1].type = u._get : y.tokens[y.tokens.length - 1].type = u._set, _s(n), gs(e, !1)) : i.contextualKeyword === l._accessor && !E() ? (_s(n), ys()) : E() ? ys() : k();
}
function gs(e, t) {
	_ ? jn() : v && L(u.lessThan) && Z(), ra(e, t);
}
function _s(e) {
	na(e);
}
function vs() {
	if (_) {
		let e = P(0);
		I(u.question), F(e);
	}
}
function ys() {
	if (_ ? (we(u.bang), dr()) : v && L(u.colon) && eo(), L(u.eq)) {
		let e = y.tokens.length;
		N(), K(), y.tokens[e].rhsEndIndex = y.tokens.length;
	}
	D();
}
function bs(e, t = !1) {
	_ && (!e || t) && x(l._implements) || (L(u.name) && dn(!0), _ ? jn() : v && L(u.lessThan) && Z());
}
function xs() {
	let e = !1;
	I(u._extends) ? (Ai(), e = !0) : e = !1, _ ? Zr(e) : v && ho(e);
}
function Ss() {
	let e = y.tokens.length - 1;
	_ && Ur() || (Os() ? ks() : Ts() ? (Y(), L(u.comma) && R() === u.star ? (O(u.comma), O(u.star), C(l._as), Y()) : Es(), Ds()) : I(u._default) ? Cs() : Ms() ? ws() : (Ns(), Ds()), y.tokens[e].rhsEndIndex = y.tokens.length);
}
function Cs() {
	if (_ && Kr() || v && so()) return;
	let e = y.start;
	I(u._function) ? cs(e, !0, !0) : x(l._async) && R() === u._function ? (S(l._async), I(u._function), cs(e, !0, !0)) : L(u._class) ? us(!0, !0) : L(u.at) ? (Po(), us(!0, !0)) : (K(), D());
}
function ws() {
	_ ? Xr() : v ? fo() : $(!0);
}
function Ts() {
	if (_ && Br() || v && uo()) return !1;
	if (L(u.name)) return y.contextualKeyword !== l._async;
	if (!L(u._default)) return !1;
	let e = De(), t = Ee(), n = t.type === u.name && t.contextualKeyword === l._from;
	if (t.type === u.comma) return !0;
	if (n) {
		let t = b.charCodeAt(Oe(e + 4));
		return t === h.quotationMark || t === h.apostrophe;
	}
	return !1;
}
function Es() {
	I(u.comma) && Ns();
}
function Ds() {
	S(l._from) && (J(), Hs()), D();
}
function Os() {
	return v ? po() : L(u.star);
}
function ks() {
	v ? mo() : As();
}
function As() {
	O(u.star), x(l._as) ? js() : Ds();
}
function js() {
	N(), y.tokens[y.tokens.length - 1].type = u._as, Y(), Es(), Ds();
}
function Ms() {
	return _ && Br() || v && lo() || y.type === u._var || y.type === u._const || y.type === u._let || y.type === u._function || y.type === u._class || x(l._async) || L(u.at);
}
function Ns() {
	let e = !0;
	for (O(u.braceL); !I(u.braceR) && !y.error;) {
		if (e) e = !1;
		else if (O(u.comma), I(u.braceR)) break;
		Ps();
	}
}
function Ps() {
	if (_) {
		Gr();
		return;
	}
	Y(), y.tokens[y.tokens.length - 1].identifierRole = j.ExportAccess, S(l._as) && Y();
}
function Fs() {
	let e = y.snapshot();
	return C(l._module), S(l._from) ? x(l._from) ? (y.restoreFromSnapshot(e), !0) : (y.restoreFromSnapshot(e), !1) : L(u.comma) ? (y.restoreFromSnapshot(e), !1) : (y.restoreFromSnapshot(e), !0);
}
function Is() {
	x(l._module) && Fs() && N();
}
function Ls() {
	if (_ && L(u.name) && R() === u.eq) {
		Or();
		return;
	}
	if (_ && x(l._type)) {
		let e = Ee();
		if (e.type === u.name && e.contextualKeyword !== l._from) {
			if (C(l._type), R() === u.eq) {
				Or();
				return;
			}
		} else (e.type === u.star || e.type === u.braceL) && C(l._type);
	}
	L(u.string) ? J() : (Is(), Bs(), C(l._from), J()), Hs(), D();
}
function Rs() {
	return L(u.name);
}
function zs() {
	fn();
}
function Bs() {
	v && vo();
	let e = !0;
	if (!(Rs() && (zs(), !I(u.comma)))) {
		if (L(u.star)) {
			N(), C(l._as), zs();
			return;
		}
		for (O(u.braceL); !I(u.braceR) && !y.error;) {
			if (e) e = !1;
			else if (I(u.colon) && k("ES2015 named imports do not destructure. Use another statement for destructuring after the import."), O(u.comma), I(u.braceR)) break;
			Vs();
		}
	}
}
function Vs() {
	if (_) {
		Wr();
		return;
	}
	if (v) {
		yo();
		return;
	}
	fn(), x(l._as) && (y.tokens[y.tokens.length - 1].identifierRole = j.ImportAccess, N(), fn());
}
function Hs() {
	(L(u._with) || x(l._assert) && !T()) && (N(), Zi(!1, !1));
}
function Us() {
	return y.pos === 0 && b.charCodeAt(0) === h.numberSign && b.charCodeAt(1) === h.exclamationMark && Ne(2), Ae(), jo();
}
var Ws = class {
	constructor(e, t) {
		this.tokens = e, this.scopes = t;
	}
};
function Gs(e, t, n, r) {
	if (r && n) throw Error("Cannot combine flow and typescript plugins.");
	ae(e, t, n, r);
	let i = Us();
	if (y.error) throw ne(y.error);
	return i;
}
function Ks(e) {
	let t = e.currentIndex(), n = 0, r = e.currentToken();
	do {
		let i = e.tokens[t];
		if (i.isOptionalChainStart && n++, i.isOptionalChainEnd && n--, n += i.numNullishCoalesceStarts, n -= i.numNullishCoalesceEnds, i.contextualKeyword === l._await && i.identifierRole == null && i.scopeDepth === r.scopeDepth) return !0;
		t += 1;
	} while (n > 0 && t < e.tokens.length);
	return !1;
}
var qs = class e {
	__init() {
		this.resultCode = "";
	}
	__init2() {
		this.resultMappings = Array(this.tokens.length);
	}
	__init3() {
		this.tokenIndex = 0;
	}
	constructor(t, n, r, i, a) {
		this.code = t, this.tokens = n, this.isFlowEnabled = r, this.disableESTransforms = i, this.helperManager = a, e.prototype.__init.call(this), e.prototype.__init2.call(this), e.prototype.__init3.call(this);
	}
	snapshot() {
		return {
			resultCode: this.resultCode,
			tokenIndex: this.tokenIndex
		};
	}
	restoreToSnapshot(e) {
		this.resultCode = e.resultCode, this.tokenIndex = e.tokenIndex;
	}
	dangerouslyGetAndRemoveCodeSinceSnapshot(e) {
		let t = this.resultCode.slice(e.resultCode.length);
		return this.resultCode = e.resultCode, t;
	}
	reset() {
		this.resultCode = "", this.resultMappings = Array(this.tokens.length), this.tokenIndex = 0;
	}
	matchesContextualAtIndex(e, t) {
		return this.matches1AtIndex(e, u.name) && this.tokens[e].contextualKeyword === t;
	}
	identifierNameAtIndex(e) {
		return this.identifierNameForToken(this.tokens[e]);
	}
	identifierNameAtRelativeIndex(e) {
		return this.identifierNameForToken(this.tokenAtRelativeIndex(e));
	}
	identifierName() {
		return this.identifierNameForToken(this.currentToken());
	}
	identifierNameForToken(e) {
		return this.code.slice(e.start, e.end);
	}
	rawCodeForToken(e) {
		return this.code.slice(e.start, e.end);
	}
	stringValueAtIndex(e) {
		return this.stringValueForToken(this.tokens[e]);
	}
	stringValue() {
		return this.stringValueForToken(this.currentToken());
	}
	stringValueForToken(e) {
		return this.code.slice(e.start + 1, e.end - 1);
	}
	matches1AtIndex(e, t) {
		return this.tokens[e].type === t;
	}
	matches2AtIndex(e, t, n) {
		return this.tokens[e].type === t && this.tokens[e + 1].type === n;
	}
	matches3AtIndex(e, t, n, r) {
		return this.tokens[e].type === t && this.tokens[e + 1].type === n && this.tokens[e + 2].type === r;
	}
	matches1(e) {
		return this.tokens[this.tokenIndex].type === e;
	}
	matches2(e, t) {
		return this.tokens[this.tokenIndex].type === e && this.tokens[this.tokenIndex + 1].type === t;
	}
	matches3(e, t, n) {
		return this.tokens[this.tokenIndex].type === e && this.tokens[this.tokenIndex + 1].type === t && this.tokens[this.tokenIndex + 2].type === n;
	}
	matches4(e, t, n, r) {
		return this.tokens[this.tokenIndex].type === e && this.tokens[this.tokenIndex + 1].type === t && this.tokens[this.tokenIndex + 2].type === n && this.tokens[this.tokenIndex + 3].type === r;
	}
	matches5(e, t, n, r, i) {
		return this.tokens[this.tokenIndex].type === e && this.tokens[this.tokenIndex + 1].type === t && this.tokens[this.tokenIndex + 2].type === n && this.tokens[this.tokenIndex + 3].type === r && this.tokens[this.tokenIndex + 4].type === i;
	}
	matchesContextual(e) {
		return this.matchesContextualAtIndex(this.tokenIndex, e);
	}
	matchesContextIdAndLabel(e, t) {
		return this.matches1(e) && this.currentToken().contextId === t;
	}
	previousWhitespaceAndComments() {
		let e = this.code.slice(this.tokenIndex > 0 ? this.tokens[this.tokenIndex - 1].end : 0, this.tokenIndex < this.tokens.length ? this.tokens[this.tokenIndex].start : this.code.length);
		return this.isFlowEnabled && (e = e.replace(/@flow/g, "")), e;
	}
	replaceToken(e) {
		this.resultCode += this.previousWhitespaceAndComments(), this.appendTokenPrefix(), this.resultMappings[this.tokenIndex] = this.resultCode.length, this.resultCode += e, this.appendTokenSuffix(), this.tokenIndex++;
	}
	replaceTokenTrimmingLeftWhitespace(e) {
		this.resultCode += this.previousWhitespaceAndComments().replace(/[^\r\n]/g, ""), this.appendTokenPrefix(), this.resultMappings[this.tokenIndex] = this.resultCode.length, this.resultCode += e, this.appendTokenSuffix(), this.tokenIndex++;
	}
	removeInitialToken() {
		this.replaceToken("");
	}
	removeToken() {
		this.replaceTokenTrimmingLeftWhitespace("");
	}
	removeBalancedCode() {
		let e = 0;
		for (; !this.isAtEnd();) {
			if (this.matches1(u.braceL)) e++;
			else if (this.matches1(u.braceR)) {
				if (e === 0) return;
				e--;
			}
			this.removeToken();
		}
	}
	copyExpectedToken(e) {
		if (this.tokens[this.tokenIndex].type !== e) throw Error(`Expected token ${e}`);
		this.copyToken();
	}
	copyToken() {
		this.resultCode += this.previousWhitespaceAndComments(), this.appendTokenPrefix(), this.resultMappings[this.tokenIndex] = this.resultCode.length, this.resultCode += this.code.slice(this.tokens[this.tokenIndex].start, this.tokens[this.tokenIndex].end), this.appendTokenSuffix(), this.tokenIndex++;
	}
	copyTokenWithPrefix(e) {
		this.resultCode += this.previousWhitespaceAndComments(), this.appendTokenPrefix(), this.resultCode += e, this.resultMappings[this.tokenIndex] = this.resultCode.length, this.resultCode += this.code.slice(this.tokens[this.tokenIndex].start, this.tokens[this.tokenIndex].end), this.appendTokenSuffix(), this.tokenIndex++;
	}
	appendTokenPrefix() {
		let e = this.currentToken();
		if ((e.numNullishCoalesceStarts || e.isOptionalChainStart) && (e.isAsyncOperation = Ks(this)), !this.disableESTransforms) {
			if (e.numNullishCoalesceStarts) for (let t = 0; t < e.numNullishCoalesceStarts; t++) e.isAsyncOperation ? (this.resultCode += "await ", this.resultCode += this.helperManager.getHelperName("asyncNullishCoalesce")) : this.resultCode += this.helperManager.getHelperName("nullishCoalesce"), this.resultCode += "(";
			e.isOptionalChainStart && (e.isAsyncOperation && (this.resultCode += "await "), this.tokenIndex > 0 && this.tokenAtRelativeIndex(-1).type === u._delete ? e.isAsyncOperation ? this.resultCode += this.helperManager.getHelperName("asyncOptionalChainDelete") : this.resultCode += this.helperManager.getHelperName("optionalChainDelete") : e.isAsyncOperation ? this.resultCode += this.helperManager.getHelperName("asyncOptionalChain") : this.resultCode += this.helperManager.getHelperName("optionalChain"), this.resultCode += "([");
		}
	}
	appendTokenSuffix() {
		let e = this.currentToken();
		if (e.isOptionalChainEnd && !this.disableESTransforms && (this.resultCode += "])"), e.numNullishCoalesceEnds && !this.disableESTransforms) for (let t = 0; t < e.numNullishCoalesceEnds; t++) this.resultCode += "))";
	}
	appendCode(e) {
		this.resultCode += e;
	}
	currentToken() {
		return this.tokens[this.tokenIndex];
	}
	currentTokenCode() {
		let e = this.currentToken();
		return this.code.slice(e.start, e.end);
	}
	tokenAtRelativeIndex(e) {
		return this.tokens[this.tokenIndex + e];
	}
	currentIndex() {
		return this.tokenIndex;
	}
	nextToken() {
		if (this.tokenIndex === this.tokens.length) throw Error("Unexpectedly reached end of input.");
		this.tokenIndex++;
	}
	previousToken() {
		this.tokenIndex--;
	}
	finish() {
		if (this.tokenIndex !== this.tokens.length) throw Error("Tried to finish processing tokens before reaching the end.");
		return this.resultCode += this.previousWhitespaceAndComments(), {
			code: this.resultCode,
			mappings: this.resultMappings
		};
	}
	isAtEnd() {
		return this.tokenIndex === this.tokens.length;
	}
};
function Js(e, t, n, r) {
	let i = t.snapshot(), a = Xs(t), o = [], s = [], c = [], d = null, f = [], p = [], m = t.currentToken().contextId;
	if (m == null) throw Error("Expected non-null class context ID on class open-brace.");
	for (t.nextToken(); !t.matchesContextIdAndLabel(u.braceR, m);) if (t.matchesContextual(l._constructor) && !t.currentToken().isType) ({constructorInitializerStatements: o, constructorInsertPos: d} = Zs(t));
	else if (t.matches1(u.semi)) r || p.push({
		start: t.currentIndex(),
		end: t.currentIndex() + 1
	}), t.nextToken();
	else if (t.currentToken().isType) t.nextToken();
	else {
		let i = t.currentIndex(), a = !1, h = !1, g = !1;
		for (; Qs(t.currentToken());) t.matches1(u._static) && (a = !0), t.matches1(u.hash) && (h = !0), (t.matches1(u._declare) || t.matches1(u._abstract)) && (g = !0), t.nextToken();
		if (a && t.matches1(u.braceL)) {
			Ys(t, m);
			continue;
		}
		if (h) {
			Ys(t, m);
			continue;
		}
		if (t.matchesContextual(l._constructor) && !t.currentToken().isType) {
			({constructorInitializerStatements: o, constructorInsertPos: d} = Zs(t));
			continue;
		}
		let _ = t.currentIndex();
		if ($s(t), t.matches1(u.lessThan) || t.matches1(u.parenL)) {
			Ys(t, m);
			continue;
		}
		for (; t.currentToken().isType;) t.nextToken();
		if (t.matches1(u.eq)) {
			let r = t.currentIndex(), i = t.currentToken().rhsEndIndex;
			if (i == null) throw Error("Expected rhsEndIndex on class field assignment.");
			for (t.nextToken(); t.currentIndex() < i;) e.processToken();
			let o;
			a ? (o = n.claimFreeName("__initStatic"), c.push(o)) : (o = n.claimFreeName("__init"), s.push(o)), f.push({
				initializerName: o,
				equalsIndex: r,
				start: _,
				end: t.currentIndex()
			});
		} else (!r || g) && p.push({
			start: i,
			end: t.currentIndex()
		});
	}
	return t.restoreToSnapshot(i), r ? {
		headerInfo: a,
		constructorInitializerStatements: o,
		instanceInitializerNames: [],
		staticInitializerNames: [],
		constructorInsertPos: d,
		fields: [],
		rangesToRemove: p
	} : {
		headerInfo: a,
		constructorInitializerStatements: o,
		instanceInitializerNames: s,
		staticInitializerNames: c,
		constructorInsertPos: d,
		fields: f,
		rangesToRemove: p
	};
}
function Ys(e, t) {
	for (e.nextToken(); e.currentToken().contextId !== t;) e.nextToken();
	for (; Qs(e.tokenAtRelativeIndex(-1));) e.previousToken();
}
function Xs(e) {
	let t = e.currentToken(), n = t.contextId;
	if (n == null) throw Error("Expected context ID on class token.");
	let r = t.isExpression;
	if (r == null) throw Error("Expected isExpression on class token.");
	let i = null, a = !1;
	for (e.nextToken(), e.matches1(u.name) && (i = e.identifierName()); !e.matchesContextIdAndLabel(u.braceL, n);) e.matches1(u._extends) && !e.currentToken().isType && (a = !0), e.nextToken();
	return {
		isExpression: r,
		className: i,
		hasSuperclass: a
	};
}
function Zs(e) {
	let t = [];
	e.nextToken();
	let n = e.currentToken().contextId;
	if (n == null) throw Error("Expected context ID on open-paren starting constructor params.");
	for (; !e.matchesContextIdAndLabel(u.parenR, n);) if (e.currentToken().contextId === n) {
		if (e.nextToken(), Qs(e.currentToken())) {
			for (e.nextToken(); Qs(e.currentToken());) e.nextToken();
			let n = e.currentToken();
			if (n.type !== u.name) throw Error("Expected identifier after access modifiers in constructor arg.");
			let r = e.identifierNameForToken(n);
			t.push(`this.${r} = ${r}`);
		}
	} else e.nextToken();
	for (e.nextToken(); e.currentToken().isType;) e.nextToken();
	let r = e.currentIndex(), i = !1;
	for (; !e.matchesContextIdAndLabel(u.braceR, n);) {
		if (!i && e.matches2(u._super, u.parenL)) {
			e.nextToken();
			let t = e.currentToken().contextId;
			if (t == null) throw Error("Expected a context ID on the super call");
			for (; !e.matchesContextIdAndLabel(u.parenR, t);) e.nextToken();
			r = e.currentIndex(), i = !0;
		}
		e.nextToken();
	}
	return e.nextToken(), {
		constructorInitializerStatements: t,
		constructorInsertPos: r
	};
}
function Qs(e) {
	return [
		u._async,
		u._get,
		u._set,
		u.plus,
		u.minus,
		u._readonly,
		u._static,
		u._public,
		u._private,
		u._protected,
		u._override,
		u._abstract,
		u.star,
		u._declare,
		u.hash
	].includes(e.type);
}
function $s(e) {
	if (e.matches1(u.bracketL)) {
		let t = e.currentToken().contextId;
		if (t == null) throw Error("Expected class context ID on computed name open bracket.");
		for (; !e.matchesContextIdAndLabel(u.bracketR, t);) e.nextToken();
		e.nextToken();
	} else e.nextToken();
}
function ec(e) {
	if (e.removeInitialToken(), e.removeToken(), e.removeToken(), e.removeToken(), e.matches1(u.parenL)) e.removeToken(), e.removeToken(), e.removeToken();
	else for (; e.matches1(u.dot);) e.removeToken(), e.removeToken();
}
var tc = {
	typeDeclarations: /* @__PURE__ */ new Set(),
	valueDeclarations: /* @__PURE__ */ new Set()
};
function nc(e) {
	let t = /* @__PURE__ */ new Set(), n = /* @__PURE__ */ new Set();
	for (let r = 0; r < e.tokens.length; r++) {
		let i = e.tokens[r];
		i.type === u.name && _e(i) && (i.isType ? t.add(e.identifierNameForToken(i)) : n.add(e.identifierNameForToken(i)));
	}
	return {
		typeDeclarations: t,
		valueDeclarations: n
	};
}
function rc(e) {
	let t = e.currentIndex();
	for (; !e.matches1AtIndex(t, u.braceR);) t++;
	return e.matchesContextualAtIndex(t + 1, l._from) && e.matches1AtIndex(t + 2, u.string);
}
function ic(e) {
	(e.matches2(u._with, u.braceL) || e.matches2(u.name, u.braceL) && e.matchesContextual(l._assert)) && (e.removeToken(), e.removeToken(), e.removeBalancedCode(), e.removeToken());
}
function ac(e, t, n, r) {
	if (!e || t) return !1;
	let i = n.currentToken();
	if (i.rhsEndIndex == null) throw Error("Expected non-null rhsEndIndex on export token.");
	let a = i.rhsEndIndex - n.currentIndex();
	if (a !== 3 && !(a === 4 && n.matches1AtIndex(i.rhsEndIndex - 1, u.semi))) return !1;
	let o = n.tokenAtRelativeIndex(2);
	if (o.type !== u.name) return !1;
	let s = n.identifierNameForToken(o);
	return r.typeDeclarations.has(s) && !r.valueDeclarations.has(s);
}
var oc = class e extends V {
	__init() {
		this.hadExport = !1;
	}
	__init2() {
		this.hadNamedExport = !1;
	}
	__init3() {
		this.hadDefaultExport = !1;
	}
	constructor(t, n, r, i, a, o, s, c, l, u, d, f) {
		super(), this.rootTransformer = t, this.tokens = n, this.importProcessor = r, this.nameManager = i, this.helperManager = a, this.reactHotLoaderTransformer = o, this.enableLegacyBabel5ModuleInterop = s, this.enableLegacyTypeScriptModuleInterop = c, this.isTypeScriptTransformEnabled = l, this.isFlowTransformEnabled = u, this.preserveDynamicImport = d, this.keepUnusedImports = f, e.prototype.__init.call(this), e.prototype.__init2.call(this), e.prototype.__init3.call(this), this.declarationInfo = l ? nc(n) : tc;
	}
	getPrefixCode() {
		let e = "";
		return this.hadExport && (e += "Object.defineProperty(exports, \"__esModule\", {value: true});"), e;
	}
	getSuffixCode() {
		return this.enableLegacyBabel5ModuleInterop && this.hadDefaultExport && !this.hadNamedExport ? "\nmodule.exports = exports.default;\n" : "";
	}
	process() {
		return this.tokens.matches3(u._import, u.name, u.eq) ? this.processImportEquals() : this.tokens.matches1(u._import) ? (this.processImport(), !0) : this.tokens.matches2(u._export, u.eq) ? (this.tokens.replaceToken("module.exports"), !0) : this.tokens.matches1(u._export) && !this.tokens.currentToken().isType ? (this.hadExport = !0, this.processExport()) : this.tokens.matches2(u.name, u.postIncDec) && this.processPostIncDec() ? !0 : this.tokens.matches1(u.name) || this.tokens.matches1(u.jsxName) ? this.processIdentifier() : this.tokens.matches1(u.eq) ? this.processAssignment() : this.tokens.matches1(u.assign) ? this.processComplexAssignment() : this.tokens.matches1(u.preIncDec) ? this.processPreIncDec() : !1;
	}
	processImportEquals() {
		let e = this.tokens.identifierNameAtIndex(this.tokens.currentIndex() + 1);
		return this.importProcessor.shouldAutomaticallyElideImportedName(e) ? ec(this.tokens) : this.tokens.replaceToken("const"), !0;
	}
	processImport() {
		if (this.tokens.matches2(u._import, u.parenL)) {
			if (this.preserveDynamicImport) {
				this.tokens.copyToken();
				return;
			}
			let e = this.enableLegacyTypeScriptModuleInterop ? "" : `${this.helperManager.getHelperName("interopRequireWildcard")}(`;
			this.tokens.replaceToken(`Promise.resolve().then(() => ${e}require`);
			let t = this.tokens.currentToken().contextId;
			if (t == null) throw Error("Expected context ID on dynamic import invocation.");
			for (this.tokens.copyToken(); !this.tokens.matchesContextIdAndLabel(u.parenR, t);) this.rootTransformer.processToken();
			this.tokens.replaceToken(e ? ")))" : "))");
			return;
		}
		if (this.removeImportAndDetectIfShouldElide()) this.tokens.removeToken();
		else {
			let e = this.tokens.stringValue();
			this.tokens.replaceTokenTrimmingLeftWhitespace(this.importProcessor.claimImportCode(e)), this.tokens.appendCode(this.importProcessor.claimImportCode(e));
		}
		ic(this.tokens), this.tokens.matches1(u.semi) && this.tokens.removeToken();
	}
	removeImportAndDetectIfShouldElide() {
		if (this.tokens.removeInitialToken(), this.tokens.matchesContextual(l._type) && !this.tokens.matches1AtIndex(this.tokens.currentIndex() + 1, u.comma) && !this.tokens.matchesContextualAtIndex(this.tokens.currentIndex() + 1, l._from)) return this.removeRemainingImport(), !0;
		if (this.tokens.matches1(u.name) || this.tokens.matches1(u.star)) return this.removeRemainingImport(), !1;
		if (this.tokens.matches1(u.string)) return !1;
		let e = !1, t = !1;
		for (; !this.tokens.matches1(u.string);) (!e && this.tokens.matches1(u.braceL) || this.tokens.matches1(u.comma)) && (this.tokens.removeToken(), this.tokens.matches1(u.braceR) || (t = !0), (this.tokens.matches2(u.name, u.comma) || this.tokens.matches2(u.name, u.braceR) || this.tokens.matches4(u.name, u.name, u.name, u.comma) || this.tokens.matches4(u.name, u.name, u.name, u.braceR)) && (e = !0)), this.tokens.removeToken();
		return this.keepUnusedImports ? !1 : this.isTypeScriptTransformEnabled ? !e : this.isFlowTransformEnabled ? t && !e : !1;
	}
	removeRemainingImport() {
		for (; !this.tokens.matches1(u.string);) this.tokens.removeToken();
	}
	processIdentifier() {
		let e = this.tokens.currentToken();
		if (e.shadowsGlobal) return !1;
		if (e.identifierRole === j.ObjectShorthand) return this.processObjectShorthand();
		if (e.identifierRole !== j.Access) return !1;
		let t = this.importProcessor.getIdentifierReplacement(this.tokens.identifierNameForToken(e));
		if (!t) return !1;
		let n = this.tokens.currentIndex() + 1;
		for (; n < this.tokens.tokens.length && this.tokens.tokens[n].type === u.parenR;) n++;
		return this.tokens.tokens[n].type === u.parenL ? this.tokens.tokenAtRelativeIndex(1).type === u.parenL && this.tokens.tokenAtRelativeIndex(-1).type !== u._new ? (this.tokens.replaceToken(`${t}.call(void 0, `), this.tokens.removeToken(), this.rootTransformer.processBalancedCode(), this.tokens.copyExpectedToken(u.parenR)) : this.tokens.replaceToken(`(0, ${t})`) : this.tokens.replaceToken(t), !0;
	}
	processObjectShorthand() {
		let e = this.tokens.identifierName(), t = this.importProcessor.getIdentifierReplacement(e);
		return t ? (this.tokens.replaceToken(`${e}: ${t}`), !0) : !1;
	}
	processExport() {
		if (this.tokens.matches2(u._export, u._enum) || this.tokens.matches3(u._export, u._const, u._enum)) return this.hadNamedExport = !0, !1;
		if (this.tokens.matches2(u._export, u._default)) return this.tokens.matches3(u._export, u._default, u._enum) ? (this.hadDefaultExport = !0, !1) : (this.processExportDefault(), !0);
		if (this.tokens.matches2(u._export, u.braceL)) return this.processExportBindings(), !0;
		if (this.tokens.matches2(u._export, u.name) && this.tokens.matchesContextualAtIndex(this.tokens.currentIndex() + 1, l._type)) {
			if (this.tokens.removeInitialToken(), this.tokens.removeToken(), this.tokens.matches1(u.braceL)) {
				for (; !this.tokens.matches1(u.braceR);) this.tokens.removeToken();
				this.tokens.removeToken();
			} else this.tokens.removeToken(), this.tokens.matches1(u._as) && (this.tokens.removeToken(), this.tokens.removeToken());
			return this.tokens.matchesContextual(l._from) && this.tokens.matches1AtIndex(this.tokens.currentIndex() + 1, u.string) && (this.tokens.removeToken(), this.tokens.removeToken(), ic(this.tokens)), !0;
		}
		if (this.hadNamedExport = !0, this.tokens.matches2(u._export, u._var) || this.tokens.matches2(u._export, u._let) || this.tokens.matches2(u._export, u._const)) return this.processExportVar(), !0;
		if (this.tokens.matches2(u._export, u._function) || this.tokens.matches3(u._export, u.name, u._function)) return this.processExportFunction(), !0;
		if (this.tokens.matches2(u._export, u._class) || this.tokens.matches3(u._export, u._abstract, u._class) || this.tokens.matches2(u._export, u.at)) return this.processExportClass(), !0;
		if (this.tokens.matches2(u._export, u.star)) return this.processExportStar(), !0;
		throw Error("Unrecognized export syntax.");
	}
	processAssignment() {
		let e = this.tokens.currentIndex(), t = this.tokens.tokens[e - 1];
		if (t.isType || t.type !== u.name || t.shadowsGlobal || e >= 2 && this.tokens.matches1AtIndex(e - 2, u.dot) || e >= 2 && [
			u._var,
			u._let,
			u._const
		].includes(this.tokens.tokens[e - 2].type)) return !1;
		let n = this.importProcessor.resolveExportBinding(this.tokens.identifierNameForToken(t));
		return n ? (this.tokens.copyToken(), this.tokens.appendCode(` ${n} =`), !0) : !1;
	}
	processComplexAssignment() {
		let e = this.tokens.currentIndex(), t = this.tokens.tokens[e - 1];
		if (t.type !== u.name || t.shadowsGlobal || e >= 2 && this.tokens.matches1AtIndex(e - 2, u.dot)) return !1;
		let n = this.importProcessor.resolveExportBinding(this.tokens.identifierNameForToken(t));
		return n ? (this.tokens.appendCode(` = ${n}`), this.tokens.copyToken(), !0) : !1;
	}
	processPreIncDec() {
		let e = this.tokens.currentIndex(), t = this.tokens.tokens[e + 1];
		if (t.type !== u.name || t.shadowsGlobal || e + 2 < this.tokens.tokens.length && (this.tokens.matches1AtIndex(e + 2, u.dot) || this.tokens.matches1AtIndex(e + 2, u.bracketL) || this.tokens.matches1AtIndex(e + 2, u.parenL))) return !1;
		let n = this.tokens.identifierNameForToken(t), r = this.importProcessor.resolveExportBinding(n);
		return r ? (this.tokens.appendCode(`${r} = `), this.tokens.copyToken(), !0) : !1;
	}
	processPostIncDec() {
		let e = this.tokens.currentIndex(), t = this.tokens.tokens[e], n = this.tokens.tokens[e + 1];
		if (t.type !== u.name || t.shadowsGlobal || e >= 1 && this.tokens.matches1AtIndex(e - 1, u.dot)) return !1;
		let r = this.tokens.identifierNameForToken(t), i = this.importProcessor.resolveExportBinding(r);
		if (!i) return !1;
		let a = this.tokens.rawCodeForToken(n), o = this.importProcessor.getIdentifierReplacement(r) || r;
		if (a === "++") this.tokens.replaceToken(`(${o} = ${i} = ${o} + 1, ${o} - 1)`);
		else if (a === "--") this.tokens.replaceToken(`(${o} = ${i} = ${o} - 1, ${o} + 1)`);
		else throw Error(`Unexpected operator: ${a}`);
		return this.tokens.removeToken(), !0;
	}
	processExportDefault() {
		let e = !0;
		if (this.tokens.matches4(u._export, u._default, u._function, u.name) || this.tokens.matches5(u._export, u._default, u.name, u._function, u.name) && this.tokens.matchesContextualAtIndex(this.tokens.currentIndex() + 2, l._async)) {
			this.tokens.removeInitialToken(), this.tokens.removeToken();
			let e = this.processNamedFunction();
			this.tokens.appendCode(` exports.default = ${e};`);
		} else if (this.tokens.matches4(u._export, u._default, u._class, u.name) || this.tokens.matches5(u._export, u._default, u._abstract, u._class, u.name) || this.tokens.matches3(u._export, u._default, u.at)) {
			this.tokens.removeInitialToken(), this.tokens.removeToken(), this.copyDecorators(), this.tokens.matches1(u._abstract) && this.tokens.removeToken();
			let e = this.rootTransformer.processNamedClass();
			this.tokens.appendCode(` exports.default = ${e};`);
		} else if (ac(this.isTypeScriptTransformEnabled, this.keepUnusedImports, this.tokens, this.declarationInfo)) e = !1, this.tokens.removeInitialToken(), this.tokens.removeToken(), this.tokens.removeToken();
		else if (this.reactHotLoaderTransformer) {
			let e = this.nameManager.claimFreeName("_default");
			this.tokens.replaceToken(`let ${e}; exports.`), this.tokens.copyToken(), this.tokens.appendCode(` = ${e} =`), this.reactHotLoaderTransformer.setExtractedDefaultExportName(e);
		} else this.tokens.replaceToken("exports."), this.tokens.copyToken(), this.tokens.appendCode(" =");
		e && (this.hadDefaultExport = !0);
	}
	copyDecorators() {
		for (; this.tokens.matches1(u.at);) if (this.tokens.copyToken(), this.tokens.matches1(u.parenL)) this.tokens.copyExpectedToken(u.parenL), this.rootTransformer.processBalancedCode(), this.tokens.copyExpectedToken(u.parenR);
		else {
			for (this.tokens.copyExpectedToken(u.name); this.tokens.matches1(u.dot);) this.tokens.copyExpectedToken(u.dot), this.tokens.copyExpectedToken(u.name);
			this.tokens.matches1(u.parenL) && (this.tokens.copyExpectedToken(u.parenL), this.rootTransformer.processBalancedCode(), this.tokens.copyExpectedToken(u.parenR));
		}
	}
	processExportVar() {
		this.isSimpleExportVar() ? this.processSimpleExportVar() : this.processComplexExportVar();
	}
	isSimpleExportVar() {
		let e = this.tokens.currentIndex();
		if (e++, e++, !this.tokens.matches1AtIndex(e, u.name)) return !1;
		for (e++; e < this.tokens.tokens.length && this.tokens.tokens[e].isType;) e++;
		return !!this.tokens.matches1AtIndex(e, u.eq);
	}
	processSimpleExportVar() {
		this.tokens.removeInitialToken(), this.tokens.copyToken();
		let e = this.tokens.identifierName();
		for (; !this.tokens.matches1(u.eq);) this.rootTransformer.processToken();
		let t = this.tokens.currentToken().rhsEndIndex;
		if (t == null) throw Error("Expected = token with an end index.");
		for (; this.tokens.currentIndex() < t;) this.rootTransformer.processToken();
		this.tokens.appendCode(`; exports.${e} = ${e}`);
	}
	processComplexExportVar() {
		this.tokens.removeInitialToken(), this.tokens.removeToken();
		let e = this.tokens.matches1(u.braceL);
		e && this.tokens.appendCode("(");
		let t = 0;
		for (;;) if (this.tokens.matches1(u.braceL) || this.tokens.matches1(u.dollarBraceL) || this.tokens.matches1(u.bracketL)) t++, this.tokens.copyToken();
		else if (this.tokens.matches1(u.braceR) || this.tokens.matches1(u.bracketR)) t--, this.tokens.copyToken();
		else if (t === 0 && !this.tokens.matches1(u.name) && !this.tokens.currentToken().isType) break;
		else if (this.tokens.matches1(u.eq)) {
			let e = this.tokens.currentToken().rhsEndIndex;
			if (e == null) throw Error("Expected = token with an end index.");
			for (; this.tokens.currentIndex() < e;) this.rootTransformer.processToken();
		} else {
			let e = this.tokens.currentToken();
			if (he(e)) {
				let t = this.tokens.identifierName(), n = this.importProcessor.getIdentifierReplacement(t);
				if (n === null) throw Error(`Expected a replacement for ${t} in \`export var\` syntax.`);
				be(e) && (n = `${t}: ${n}`), this.tokens.replaceToken(n);
			} else this.rootTransformer.processToken();
		}
		if (e) {
			let e = this.tokens.currentToken().rhsEndIndex;
			if (e == null) throw Error("Expected = token with an end index.");
			for (; this.tokens.currentIndex() < e;) this.rootTransformer.processToken();
			this.tokens.appendCode(")");
		}
	}
	processExportFunction() {
		this.tokens.replaceToken("");
		let e = this.processNamedFunction();
		this.tokens.appendCode(` exports.${e} = ${e};`);
	}
	processNamedFunction() {
		if (this.tokens.matches1(u._function)) this.tokens.copyToken();
		else if (this.tokens.matches2(u.name, u._function)) {
			if (!this.tokens.matchesContextual(l._async)) throw Error("Expected async keyword in function export.");
			this.tokens.copyToken(), this.tokens.copyToken();
		}
		if (this.tokens.matches1(u.star) && this.tokens.copyToken(), !this.tokens.matches1(u.name)) throw Error("Expected identifier for exported function name.");
		let e = this.tokens.identifierName();
		if (this.tokens.copyToken(), this.tokens.currentToken().isType) for (this.tokens.removeInitialToken(); this.tokens.currentToken().isType;) this.tokens.removeToken();
		return this.tokens.copyExpectedToken(u.parenL), this.rootTransformer.processBalancedCode(), this.tokens.copyExpectedToken(u.parenR), this.rootTransformer.processPossibleTypeRange(), this.tokens.copyExpectedToken(u.braceL), this.rootTransformer.processBalancedCode(), this.tokens.copyExpectedToken(u.braceR), e;
	}
	processExportClass() {
		this.tokens.removeInitialToken(), this.copyDecorators(), this.tokens.matches1(u._abstract) && this.tokens.removeToken();
		let e = this.rootTransformer.processNamedClass();
		this.tokens.appendCode(` exports.${e} = ${e};`);
	}
	processExportBindings() {
		this.tokens.removeInitialToken(), this.tokens.removeToken();
		let e = rc(this.tokens), t = [];
		for (;;) {
			if (this.tokens.matches1(u.braceR)) {
				this.tokens.removeToken();
				break;
			}
			let n = et(this.tokens);
			for (; this.tokens.currentIndex() < n.endIndex;) this.tokens.removeToken();
			if (!(n.isType || !e && this.shouldElideExportedIdentifier(n.leftName))) {
				let e = n.rightName;
				e === "default" ? this.hadDefaultExport = !0 : this.hadNamedExport = !0;
				let r = n.leftName, i = this.importProcessor.getIdentifierReplacement(r);
				t.push(`exports.${e} = ${i || r};`);
			}
			if (this.tokens.matches1(u.braceR)) {
				this.tokens.removeToken();
				break;
			}
			if (this.tokens.matches2(u.comma, u.braceR)) {
				this.tokens.removeToken(), this.tokens.removeToken();
				break;
			} else if (this.tokens.matches1(u.comma)) this.tokens.removeToken();
			else throw Error(`Unexpected token: ${JSON.stringify(this.tokens.currentToken())}`);
		}
		if (this.tokens.matchesContextual(l._from)) {
			this.tokens.removeToken();
			let e = this.tokens.stringValue();
			this.tokens.replaceTokenTrimmingLeftWhitespace(this.importProcessor.claimImportCode(e)), ic(this.tokens);
		} else this.tokens.appendCode(t.join(" "));
		this.tokens.matches1(u.semi) && this.tokens.removeToken();
	}
	processExportStar() {
		for (this.tokens.removeInitialToken(); !this.tokens.matches1(u.string);) this.tokens.removeToken();
		let e = this.tokens.stringValue();
		this.tokens.replaceTokenTrimmingLeftWhitespace(this.importProcessor.claimImportCode(e)), ic(this.tokens), this.tokens.matches1(u.semi) && this.tokens.removeToken();
	}
	shouldElideExportedIdentifier(e) {
		return this.isTypeScriptTransformEnabled && !this.keepUnusedImports && !this.declarationInfo.valueDeclarations.has(e);
	}
}, sc = class extends V {
	constructor(e, t, n, r, i, a, o, s) {
		super(), this.tokens = e, this.nameManager = t, this.helperManager = n, this.reactHotLoaderTransformer = r, this.isTypeScriptTransformEnabled = i, this.isFlowTransformEnabled = a, this.keepUnusedImports = o, this.nonTypeIdentifiers = i && !o ? pt(e, s) : /* @__PURE__ */ new Set(), this.declarationInfo = i && !o ? nc(e) : tc, this.injectCreateRequireForImportRequire = !!s.injectCreateRequireForImportRequire;
	}
	process() {
		if (this.tokens.matches3(u._import, u.name, u.eq)) return this.processImportEquals();
		if (this.tokens.matches4(u._import, u.name, u.name, u.eq) && this.tokens.matchesContextualAtIndex(this.tokens.currentIndex() + 1, l._type)) {
			this.tokens.removeInitialToken();
			for (let e = 0; e < 7; e++) this.tokens.removeToken();
			return !0;
		}
		if (this.tokens.matches2(u._export, u.eq)) return this.tokens.replaceToken("module.exports"), !0;
		if (this.tokens.matches5(u._export, u._import, u.name, u.name, u.eq) && this.tokens.matchesContextualAtIndex(this.tokens.currentIndex() + 2, l._type)) {
			this.tokens.removeInitialToken();
			for (let e = 0; e < 8; e++) this.tokens.removeToken();
			return !0;
		}
		if (this.tokens.matches1(u._import)) return this.processImport();
		if (this.tokens.matches2(u._export, u._default)) return this.processExportDefault();
		if (this.tokens.matches2(u._export, u.braceL)) return this.processNamedExports();
		if (this.tokens.matches2(u._export, u.name) && this.tokens.matchesContextualAtIndex(this.tokens.currentIndex() + 1, l._type)) {
			if (this.tokens.removeInitialToken(), this.tokens.removeToken(), this.tokens.matches1(u.braceL)) {
				for (; !this.tokens.matches1(u.braceR);) this.tokens.removeToken();
				this.tokens.removeToken();
			} else this.tokens.removeToken(), this.tokens.matches1(u._as) && (this.tokens.removeToken(), this.tokens.removeToken());
			return this.tokens.matchesContextual(l._from) && this.tokens.matches1AtIndex(this.tokens.currentIndex() + 1, u.string) && (this.tokens.removeToken(), this.tokens.removeToken(), ic(this.tokens)), !0;
		}
		return !1;
	}
	processImportEquals() {
		let e = this.tokens.identifierNameAtIndex(this.tokens.currentIndex() + 1);
		return this.shouldAutomaticallyElideImportedName(e) ? ec(this.tokens) : this.injectCreateRequireForImportRequire ? (this.tokens.replaceToken("const"), this.tokens.copyToken(), this.tokens.copyToken(), this.tokens.replaceToken(this.helperManager.getHelperName("require"))) : this.tokens.replaceToken("const"), !0;
	}
	processImport() {
		if (this.tokens.matches2(u._import, u.parenL)) return !1;
		let e = this.tokens.snapshot();
		if (this.removeImportTypeBindings()) {
			for (this.tokens.restoreToSnapshot(e); !this.tokens.matches1(u.string);) this.tokens.removeToken();
			this.tokens.removeToken(), ic(this.tokens), this.tokens.matches1(u.semi) && this.tokens.removeToken();
		}
		return !0;
	}
	removeImportTypeBindings() {
		if (this.tokens.copyExpectedToken(u._import), this.tokens.matchesContextual(l._type) && !this.tokens.matches1AtIndex(this.tokens.currentIndex() + 1, u.comma) && !this.tokens.matchesContextualAtIndex(this.tokens.currentIndex() + 1, l._from)) return !0;
		if (this.tokens.matches1(u.string)) return this.tokens.copyToken(), !1;
		this.tokens.matchesContextual(l._module) && this.tokens.matchesContextualAtIndex(this.tokens.currentIndex() + 2, l._from) && this.tokens.copyToken();
		let e = !1, t = !1, n = !1;
		if (this.tokens.matches1(u.name) && (this.shouldAutomaticallyElideImportedName(this.tokens.identifierName()) ? (this.tokens.removeToken(), this.tokens.matches1(u.comma) && this.tokens.removeToken()) : (e = !0, this.tokens.copyToken(), this.tokens.matches1(u.comma) && (n = !0, this.tokens.removeToken()))), this.tokens.matches1(u.star)) this.shouldAutomaticallyElideImportedName(this.tokens.identifierNameAtRelativeIndex(2)) ? (this.tokens.removeToken(), this.tokens.removeToken(), this.tokens.removeToken()) : (n && this.tokens.appendCode(","), e = !0, this.tokens.copyExpectedToken(u.star), this.tokens.copyExpectedToken(u.name), this.tokens.copyExpectedToken(u.name));
		else if (this.tokens.matches1(u.braceL)) {
			for (n && this.tokens.appendCode(","), this.tokens.copyToken(); !this.tokens.matches1(u.braceR);) {
				t = !0;
				let n = et(this.tokens);
				if (n.isType || this.shouldAutomaticallyElideImportedName(n.rightName)) {
					for (; this.tokens.currentIndex() < n.endIndex;) this.tokens.removeToken();
					this.tokens.matches1(u.comma) && this.tokens.removeToken();
				} else {
					for (e = !0; this.tokens.currentIndex() < n.endIndex;) this.tokens.copyToken();
					this.tokens.matches1(u.comma) && this.tokens.copyToken();
				}
			}
			this.tokens.copyExpectedToken(u.braceR);
		}
		return this.keepUnusedImports ? !1 : this.isTypeScriptTransformEnabled ? !e : this.isFlowTransformEnabled ? t && !e : !1;
	}
	shouldAutomaticallyElideImportedName(e) {
		return this.isTypeScriptTransformEnabled && !this.keepUnusedImports && !this.nonTypeIdentifiers.has(e);
	}
	processExportDefault() {
		if (ac(this.isTypeScriptTransformEnabled, this.keepUnusedImports, this.tokens, this.declarationInfo)) return this.tokens.removeInitialToken(), this.tokens.removeToken(), this.tokens.removeToken(), !0;
		if (!(this.tokens.matches4(u._export, u._default, u._function, u.name) || this.tokens.matches5(u._export, u._default, u.name, u._function, u.name) && this.tokens.matchesContextualAtIndex(this.tokens.currentIndex() + 2, l._async) || this.tokens.matches4(u._export, u._default, u._class, u.name) || this.tokens.matches5(u._export, u._default, u._abstract, u._class, u.name)) && this.reactHotLoaderTransformer) {
			let e = this.nameManager.claimFreeName("_default");
			return this.tokens.replaceToken(`let ${e}; export`), this.tokens.copyToken(), this.tokens.appendCode(` ${e} =`), this.reactHotLoaderTransformer.setExtractedDefaultExportName(e), !0;
		}
		return !1;
	}
	processNamedExports() {
		if (!this.isTypeScriptTransformEnabled) return !1;
		this.tokens.copyExpectedToken(u._export), this.tokens.copyExpectedToken(u.braceL);
		let e = rc(this.tokens), t = !1;
		for (; !this.tokens.matches1(u.braceR);) {
			let n = et(this.tokens);
			if (n.isType || !e && this.shouldElideExportedName(n.leftName)) {
				for (; this.tokens.currentIndex() < n.endIndex;) this.tokens.removeToken();
				this.tokens.matches1(u.comma) && this.tokens.removeToken();
			} else {
				for (t = !0; this.tokens.currentIndex() < n.endIndex;) this.tokens.copyToken();
				this.tokens.matches1(u.comma) && this.tokens.copyToken();
			}
		}
		return this.tokens.copyExpectedToken(u.braceR), !this.keepUnusedImports && e && !t && (this.tokens.removeToken(), this.tokens.removeToken(), ic(this.tokens)), !0;
	}
	shouldElideExportedName(e) {
		return this.isTypeScriptTransformEnabled && !this.keepUnusedImports && this.declarationInfo.typeDeclarations.has(e) && !this.declarationInfo.valueDeclarations.has(e);
	}
}, cc = class extends V {
	constructor(e, t, n) {
		super(), this.rootTransformer = e, this.tokens = t, this.isImportsTransformEnabled = n;
	}
	process() {
		return this.rootTransformer.processPossibleArrowParamEnd() || this.rootTransformer.processPossibleAsyncArrowWithTypeParams() || this.rootTransformer.processPossibleTypeRange() ? !0 : this.tokens.matches1(u._enum) ? (this.processEnum(), !0) : this.tokens.matches2(u._export, u._enum) ? (this.processNamedExportEnum(), !0) : this.tokens.matches3(u._export, u._default, u._enum) ? (this.processDefaultExportEnum(), !0) : !1;
	}
	processNamedExportEnum() {
		if (this.isImportsTransformEnabled) {
			this.tokens.removeInitialToken();
			let e = this.tokens.identifierNameAtRelativeIndex(1);
			this.processEnum(), this.tokens.appendCode(` exports.${e} = ${e};`);
		} else this.tokens.copyToken(), this.processEnum();
	}
	processDefaultExportEnum() {
		this.tokens.removeInitialToken(), this.tokens.removeToken();
		let e = this.tokens.identifierNameAtRelativeIndex(1);
		this.processEnum(), this.isImportsTransformEnabled ? this.tokens.appendCode(` exports.default = ${e};`) : this.tokens.appendCode(` export default ${e};`);
	}
	processEnum() {
		this.tokens.replaceToken("const"), this.tokens.copyExpectedToken(u.name);
		let e = !1;
		this.tokens.matchesContextual(l._of) && (this.tokens.removeToken(), e = this.tokens.matchesContextual(l._symbol), this.tokens.removeToken());
		let t = this.tokens.matches3(u.braceL, u.name, u.eq);
		this.tokens.appendCode(" = require(\"flow-enums-runtime\")");
		let n = !e && !t;
		for (this.tokens.replaceTokenTrimmingLeftWhitespace(n ? ".Mirrored([" : "({"); !this.tokens.matches1(u.braceR);) {
			if (this.tokens.matches1(u.ellipsis)) {
				this.tokens.removeToken();
				break;
			}
			this.processEnumElement(e, t), this.tokens.matches1(u.comma) && this.tokens.copyToken();
		}
		this.tokens.replaceToken(n ? "]);" : "});");
	}
	processEnumElement(e, t) {
		if (e) {
			let e = this.tokens.identifierName();
			this.tokens.copyToken(), this.tokens.appendCode(`: Symbol("${e}")`);
		} else t ? (this.tokens.copyToken(), this.tokens.replaceTokenTrimmingLeftWhitespace(":"), this.tokens.copyToken()) : this.tokens.replaceToken(`"${this.tokens.identifierName()}"`);
	}
};
function lc(e) {
	let t, n = e[0], r = 1;
	for (; r < e.length;) {
		let i = e[r], a = e[r + 1];
		if (r += 2, (i === "optionalAccess" || i === "optionalCall") && n == null) return;
		i === "access" || i === "optionalAccess" ? (t = n, n = a(n)) : (i === "call" || i === "optionalCall") && (n = a((...e) => n.call(t, ...e)), t = void 0);
	}
	return n;
}
var uc = "jest", dc = [
	"mock",
	"unmock",
	"enableAutomock",
	"disableAutomock"
], fc = class e extends V {
	__init() {
		this.hoistedFunctionNames = [];
	}
	constructor(t, n, r, i) {
		super(), this.rootTransformer = t, this.tokens = n, this.nameManager = r, this.importProcessor = i, e.prototype.__init.call(this);
	}
	process() {
		return this.tokens.currentToken().scopeDepth === 0 && this.tokens.matches4(u.name, u.dot, u.name, u.parenL) && this.tokens.identifierName() === uc ? lc([
			this,
			"access",
			(e) => e.importProcessor,
			"optionalAccess",
			(e) => e.getGlobalNames,
			"call",
			(e) => e(),
			"optionalAccess",
			(e) => e.has,
			"call",
			(e) => e(uc)
		]) ? !1 : this.extractHoistedCalls() : !1;
	}
	getHoistedCode() {
		return this.hoistedFunctionNames.length > 0 ? this.hoistedFunctionNames.map((e) => `${e}();`).join("") : "";
	}
	extractHoistedCalls() {
		this.tokens.removeToken();
		let e = !1;
		for (; this.tokens.matches3(u.dot, u.name, u.parenL);) {
			let t = this.tokens.identifierNameAtIndex(this.tokens.currentIndex() + 1);
			if (dc.includes(t)) {
				let t = this.nameManager.claimFreeName("__jestHoist");
				this.hoistedFunctionNames.push(t), this.tokens.replaceToken(`function ${t}(){${uc}.`), this.tokens.copyToken(), this.tokens.copyToken(), this.rootTransformer.processBalancedCode(), this.tokens.copyExpectedToken(u.parenR), this.tokens.appendCode(";}"), e = !1;
			} else e ? this.tokens.copyToken() : this.tokens.replaceToken(`${uc}.`), this.tokens.copyToken(), this.tokens.copyToken(), this.rootTransformer.processBalancedCode(), this.tokens.copyExpectedToken(u.parenR), e = !0;
		}
		return !0;
	}
}, pc = class extends V {
	constructor(e) {
		super(), this.tokens = e;
	}
	process() {
		if (this.tokens.matches1(u.num)) {
			let e = this.tokens.currentTokenCode();
			if (e.includes("_")) return this.tokens.replaceToken(e.replace(/_/g, "")), !0;
		}
		return !1;
	}
}, mc = class extends V {
	constructor(e, t) {
		super(), this.tokens = e, this.nameManager = t;
	}
	process() {
		return this.tokens.matches2(u._catch, u.braceL) ? (this.tokens.copyToken(), this.tokens.appendCode(` (${this.nameManager.claimFreeName("e")})`), !0) : !1;
	}
}, hc = class extends V {
	constructor(e, t) {
		super(), this.tokens = e, this.nameManager = t;
	}
	process() {
		if (this.tokens.matches1(u.nullishCoalescing)) {
			let e = this.tokens.currentToken();
			return this.tokens.tokens[e.nullishStartIndex].isAsyncOperation ? this.tokens.replaceTokenTrimmingLeftWhitespace(", async () => (") : this.tokens.replaceTokenTrimmingLeftWhitespace(", () => ("), !0;
		}
		if (this.tokens.matches1(u._delete) && this.tokens.tokenAtRelativeIndex(1).isOptionalChainStart) return this.tokens.removeInitialToken(), !0;
		let e = this.tokens.currentToken().subscriptStartIndex;
		if (e != null && this.tokens.tokens[e].isOptionalChainStart && this.tokens.tokenAtRelativeIndex(-1).type !== u._super) {
			let t = this.nameManager.claimFreeName("_"), n;
			if (n = e > 0 && this.tokens.matches1AtIndex(e - 1, u._delete) && this.isLastSubscriptInChain() ? `${t} => delete ${t}` : `${t} => ${t}`, this.tokens.tokens[e].isAsyncOperation && (n = `async ${n}`), this.tokens.matches2(u.questionDot, u.parenL) || this.tokens.matches2(u.questionDot, u.lessThan)) this.justSkippedSuper() && this.tokens.appendCode(".bind(this)"), this.tokens.replaceTokenTrimmingLeftWhitespace(`, 'optionalCall', ${n}`);
			else if (this.tokens.matches2(u.questionDot, u.bracketL)) this.tokens.replaceTokenTrimmingLeftWhitespace(`, 'optionalAccess', ${n}`);
			else if (this.tokens.matches1(u.questionDot)) this.tokens.replaceTokenTrimmingLeftWhitespace(`, 'optionalAccess', ${n}.`);
			else if (this.tokens.matches1(u.dot)) this.tokens.replaceTokenTrimmingLeftWhitespace(`, 'access', ${n}.`);
			else if (this.tokens.matches1(u.bracketL)) this.tokens.replaceTokenTrimmingLeftWhitespace(`, 'access', ${n}[`);
			else if (this.tokens.matches1(u.parenL)) this.justSkippedSuper() && this.tokens.appendCode(".bind(this)"), this.tokens.replaceTokenTrimmingLeftWhitespace(`, 'call', ${n}(`);
			else throw Error("Unexpected subscript operator in optional chain.");
			return !0;
		}
		return !1;
	}
	isLastSubscriptInChain() {
		let e = 0;
		for (let t = this.tokens.currentIndex() + 1;; t++) {
			if (t >= this.tokens.tokens.length) throw Error("Reached the end of the code while finding the end of the access chain.");
			if (this.tokens.tokens[t].isOptionalChainStart ? e++ : this.tokens.tokens[t].isOptionalChainEnd && e--, e < 0) return !0;
			if (e === 0 && this.tokens.tokens[t].subscriptStartIndex != null) return !1;
		}
	}
	justSkippedSuper() {
		let e = 0, t = this.tokens.currentIndex() - 1;
		for (;;) {
			if (t < 0) throw Error("Reached the start of the code while finding the start of the access chain.");
			if (this.tokens.tokens[t].isOptionalChainStart ? e-- : this.tokens.tokens[t].isOptionalChainEnd && e++, e < 0) return !1;
			if (e === 0 && this.tokens.tokens[t].subscriptStartIndex != null) return this.tokens.tokens[t - 1].type === u._super;
			t--;
		}
	}
}, gc = class extends V {
	constructor(e, t, n, r) {
		super(), this.rootTransformer = e, this.tokens = t, this.importProcessor = n, this.options = r;
	}
	process() {
		let e = this.tokens.currentIndex();
		if (this.tokens.identifierName() === "createReactClass") {
			let t = this.importProcessor && this.importProcessor.getIdentifierReplacement("createReactClass");
			return t ? this.tokens.replaceToken(`(0, ${t})`) : this.tokens.copyToken(), this.tryProcessCreateClassCall(e), !0;
		}
		if (this.tokens.matches3(u.name, u.dot, u.name) && this.tokens.identifierName() === "React" && this.tokens.identifierNameAtIndex(this.tokens.currentIndex() + 2) === "createClass") {
			let t = this.importProcessor && this.importProcessor.getIdentifierReplacement("React") || "React";
			return t ? (this.tokens.replaceToken(t), this.tokens.copyToken(), this.tokens.copyToken()) : (this.tokens.copyToken(), this.tokens.copyToken(), this.tokens.copyToken()), this.tryProcessCreateClassCall(e), !0;
		}
		return !1;
	}
	tryProcessCreateClassCall(e) {
		let t = this.findDisplayName(e);
		t && this.classNeedsDisplayName() && (this.tokens.copyExpectedToken(u.parenL), this.tokens.copyExpectedToken(u.braceL), this.tokens.appendCode(`displayName: '${t}',`), this.rootTransformer.processBalancedCode(), this.tokens.copyExpectedToken(u.braceR), this.tokens.copyExpectedToken(u.parenR));
	}
	findDisplayName(e) {
		return e < 2 ? null : this.tokens.matches2AtIndex(e - 2, u.name, u.eq) || e >= 2 && this.tokens.tokens[e - 2].identifierRole === j.ObjectKey ? this.tokens.identifierNameAtIndex(e - 2) : this.tokens.matches2AtIndex(e - 2, u._export, u._default) ? this.getDisplayNameFromFilename() : null;
	}
	getDisplayNameFromFilename() {
		let e = (this.options.filePath || "unknown").split("/"), t = e[e.length - 1], n = t.lastIndexOf("."), r = n === -1 ? t : t.slice(0, n);
		return r === "index" && e[e.length - 2] ? e[e.length - 2] : r;
	}
	classNeedsDisplayName() {
		let e = this.tokens.currentIndex();
		if (!this.tokens.matches2(u.parenL, u.braceL)) return !1;
		let t = e + 1, n = this.tokens.tokens[t].contextId;
		if (n == null) throw Error("Expected non-null context ID on object open-brace.");
		for (; e < this.tokens.tokens.length; e++) {
			let t = this.tokens.tokens[e];
			if (t.type === u.braceR && t.contextId === n) {
				e++;
				break;
			}
			if (this.tokens.identifierNameAtIndex(e) === "displayName" && this.tokens.tokens[e].identifierRole === j.ObjectKey && t.contextId === n) return !1;
		}
		if (e === this.tokens.tokens.length) throw Error("Unexpected end of input when processing React class.");
		return this.tokens.matches1AtIndex(e, u.parenR) || this.tokens.matches2AtIndex(e, u.comma, u.parenR);
	}
}, _c = class e extends V {
	__init() {
		this.extractedDefaultExportName = null;
	}
	constructor(t, n) {
		super(), this.tokens = t, this.filePath = n, e.prototype.__init.call(this);
	}
	setExtractedDefaultExportName(e) {
		this.extractedDefaultExportName = e;
	}
	getPrefixCode() {
		return "\n      (function () {\n        var enterModule = require('react-hot-loader').enterModule;\n        enterModule && enterModule(module);\n      })();".replace(/\s+/g, " ").trim();
	}
	getSuffixCode() {
		let e = /* @__PURE__ */ new Set();
		for (let t of this.tokens.tokens) !t.isType && _e(t) && t.identifierRole !== j.ImportDeclaration && e.add(this.tokens.identifierNameForToken(t));
		let t = Array.from(e).map((e) => ({
			variableName: e,
			uniqueLocalName: e
		}));
		return this.extractedDefaultExportName && t.push({
			variableName: this.extractedDefaultExportName,
			uniqueLocalName: "default"
		}), `
;(function () {
  var reactHotLoader = require('react-hot-loader').default;
  var leaveModule = require('react-hot-loader').leaveModule;
  if (!reactHotLoader) {
    return;
  }
${t.map(({ variableName: e, uniqueLocalName: t }) => `  reactHotLoader.register(${e}, "${t}", ${JSON.stringify(this.filePath || "")});`).join("\n")}
  leaveModule(module);
})();`;
	}
	process() {
		return !1;
	}
}, vc = new Set(/* @__PURE__ */ "break.case.catch.class.const.continue.debugger.default.delete.do.else.export.extends.finally.for.function.if.import.in.instanceof.new.return.super.switch.this.throw.try.typeof.var.void.while.with.yield.enum.implements.interface.let.package.private.protected.public.static.await.false.null.true".split("."));
function yc(e) {
	if (e.length === 0 || !fe[e.charCodeAt(0)]) return !1;
	for (let t = 1; t < e.length; t++) if (!A[e.charCodeAt(t)]) return !1;
	return !vc.has(e);
}
var bc = class extends V {
	constructor(e, t, n) {
		super(), this.rootTransformer = e, this.tokens = t, this.isImportsTransformEnabled = n;
	}
	process() {
		return this.rootTransformer.processPossibleArrowParamEnd() || this.rootTransformer.processPossibleAsyncArrowWithTypeParams() || this.rootTransformer.processPossibleTypeRange() ? !0 : this.tokens.matches1(u._public) || this.tokens.matches1(u._protected) || this.tokens.matches1(u._private) || this.tokens.matches1(u._abstract) || this.tokens.matches1(u._readonly) || this.tokens.matches1(u._override) || this.tokens.matches1(u.nonNullAssertion) ? (this.tokens.removeInitialToken(), !0) : this.tokens.matches1(u._enum) || this.tokens.matches2(u._const, u._enum) ? (this.processEnum(), !0) : this.tokens.matches2(u._export, u._enum) || this.tokens.matches3(u._export, u._const, u._enum) ? (this.processEnum(!0), !0) : !1;
	}
	processEnum(e = !1) {
		for (this.tokens.removeInitialToken(); this.tokens.matches1(u._const) || this.tokens.matches1(u._enum);) this.tokens.removeToken();
		let t = this.tokens.identifierName();
		this.tokens.removeToken(), e && !this.isImportsTransformEnabled && this.tokens.appendCode("export "), this.tokens.appendCode(`var ${t}; (function (${t})`), this.tokens.copyExpectedToken(u.braceL), this.processEnumBody(t), this.tokens.copyExpectedToken(u.braceR), e && this.isImportsTransformEnabled ? this.tokens.appendCode(`)(${t} || (exports.${t} = ${t} = {}));`) : this.tokens.appendCode(`)(${t} || (${t} = {}));`);
	}
	processEnumBody(e) {
		let t = null;
		for (; !this.tokens.matches1(u.braceR);) {
			let { nameStringCode: n, variableName: r } = this.extractEnumKeyInfo(this.tokens.currentToken());
			this.tokens.removeInitialToken(), this.tokens.matches3(u.eq, u.string, u.comma) || this.tokens.matches3(u.eq, u.string, u.braceR) ? this.processStringLiteralEnumMember(e, n, r) : this.tokens.matches1(u.eq) ? this.processExplicitValueEnumMember(e, n, r) : this.processImplicitValueEnumMember(e, n, r, t), this.tokens.matches1(u.comma) && this.tokens.removeToken(), t = r ?? `${e}[${n}]`;
		}
	}
	extractEnumKeyInfo(e) {
		if (e.type === u.name) {
			let t = this.tokens.identifierNameForToken(e);
			return {
				nameStringCode: `"${t}"`,
				variableName: yc(t) ? t : null
			};
		} else if (e.type === u.string) {
			let t = this.tokens.stringValueForToken(e);
			return {
				nameStringCode: this.tokens.code.slice(e.start, e.end),
				variableName: yc(t) ? t : null
			};
		} else throw Error("Expected name or string at beginning of enum element.");
	}
	processStringLiteralEnumMember(e, t, n) {
		n == null ? (this.tokens.appendCode(`${e}[${t}]`), this.tokens.copyToken(), this.tokens.copyToken(), this.tokens.appendCode(";")) : (this.tokens.appendCode(`const ${n}`), this.tokens.copyToken(), this.tokens.copyToken(), this.tokens.appendCode(`; ${e}[${t}] = ${n};`));
	}
	processExplicitValueEnumMember(e, t, n) {
		let r = this.tokens.currentToken().rhsEndIndex;
		if (r == null) throw Error("Expected rhsEndIndex on enum assign.");
		if (n != null) {
			for (this.tokens.appendCode(`const ${n}`), this.tokens.copyToken(); this.tokens.currentIndex() < r;) this.rootTransformer.processToken();
			this.tokens.appendCode(`; ${e}[${e}[${t}] = ${n}] = ${t};`);
		} else {
			for (this.tokens.appendCode(`${e}[${e}[${t}]`), this.tokens.copyToken(); this.tokens.currentIndex() < r;) this.rootTransformer.processToken();
			this.tokens.appendCode(`] = ${t};`);
		}
	}
	processImplicitValueEnumMember(e, t, n, r) {
		let i = r == null ? "0" : `${r} + 1`;
		n != null && (this.tokens.appendCode(`const ${n} = ${i}; `), i = n), this.tokens.appendCode(`${e}[${e}[${t}] = ${i}] = ${t};`);
	}
}, xc = class e {
	__init() {
		this.transformers = [];
	}
	__init2() {
		this.generatedVariables = [];
	}
	constructor(t, n, r, i) {
		e.prototype.__init.call(this), e.prototype.__init2.call(this), this.nameManager = t.nameManager, this.helperManager = t.helperManager;
		let { tokenProcessor: a, importProcessor: o } = t;
		this.tokens = a, this.isImportsTransformEnabled = n.includes("imports"), this.isReactHotLoaderTransformEnabled = n.includes("react-hot-loader"), this.disableESTransforms = !!i.disableESTransforms, i.disableESTransforms || (this.transformers.push(new hc(a, this.nameManager)), this.transformers.push(new pc(a)), this.transformers.push(new mc(a, this.nameManager))), n.includes("jsx") && (i.jsxRuntime !== "preserve" && this.transformers.push(new at(this, a, o, this.nameManager, i)), this.transformers.push(new gc(this, a, o, i)));
		let s = null;
		if (n.includes("react-hot-loader")) {
			if (!i.filePath) throw Error("filePath is required when using the react-hot-loader transform.");
			s = new _c(a, i.filePath), this.transformers.push(s);
		}
		if (n.includes("imports")) {
			if (o === null) throw Error("Expected non-null importProcessor with imports transform enabled.");
			this.transformers.push(new oc(this, a, o, this.nameManager, this.helperManager, s, r, !!i.enableLegacyTypeScriptModuleInterop, n.includes("typescript"), n.includes("flow"), !!i.preserveDynamicImport, !!i.keepUnusedImports));
		} else this.transformers.push(new sc(a, this.nameManager, this.helperManager, s, n.includes("typescript"), n.includes("flow"), !!i.keepUnusedImports, i));
		n.includes("flow") && this.transformers.push(new cc(this, a, n.includes("imports"))), n.includes("typescript") && this.transformers.push(new bc(this, a, n.includes("imports"))), n.includes("jest") && this.transformers.push(new fc(this, a, this.nameManager, o));
	}
	transform() {
		this.tokens.reset(), this.processBalancedCode();
		let e = this.isImportsTransformEnabled ? "\"use strict\";" : "";
		for (let t of this.transformers) e += t.getPrefixCode();
		e += this.helperManager.emitHelpers(), e += this.generatedVariables.map((e) => ` var ${e};`).join("");
		for (let t of this.transformers) e += t.getHoistedCode();
		let t = "";
		for (let e of this.transformers) t += e.getSuffixCode();
		let n = this.tokens.finish(), { code: r } = n;
		if (r.startsWith("#!")) {
			let i = r.indexOf("\n");
			return i === -1 && (i = r.length, r += "\n"), {
				code: r.slice(0, i + 1) + e + r.slice(i + 1) + t,
				mappings: this.shiftMappings(n.mappings, e.length)
			};
		} else return {
			code: e + r + t,
			mappings: this.shiftMappings(n.mappings, e.length)
		};
	}
	processBalancedCode() {
		let e = 0, t = 0;
		for (; !this.tokens.isAtEnd();) {
			if (this.tokens.matches1(u.braceL) || this.tokens.matches1(u.dollarBraceL)) e++;
			else if (this.tokens.matches1(u.braceR)) {
				if (e === 0) return;
				e--;
			}
			if (this.tokens.matches1(u.parenL)) t++;
			else if (this.tokens.matches1(u.parenR)) {
				if (t === 0) return;
				t--;
			}
			this.processToken();
		}
	}
	processToken() {
		if (this.tokens.matches1(u._class)) {
			this.processClass();
			return;
		}
		for (let e of this.transformers) if (e.process()) return;
		this.tokens.copyToken();
	}
	processNamedClass() {
		if (!this.tokens.matches2(u._class, u.name)) throw Error("Expected identifier for exported class name.");
		let e = this.tokens.identifierNameAtIndex(this.tokens.currentIndex() + 1);
		return this.processClass(), e;
	}
	processClass() {
		let e = Js(this, this.tokens, this.nameManager, this.disableESTransforms), t = (e.headerInfo.isExpression || !e.headerInfo.className) && e.staticInitializerNames.length + e.instanceInitializerNames.length > 0, n = e.headerInfo.className;
		t && (n = this.nameManager.claimFreeName("_class"), this.generatedVariables.push(n), this.tokens.appendCode(` (${n} =`));
		let r = this.tokens.currentToken().contextId;
		if (r == null) throw Error("Expected class to have a context ID.");
		for (this.tokens.copyExpectedToken(u._class); !this.tokens.matchesContextIdAndLabel(u.braceL, r);) this.processToken();
		this.processClassBody(e, n);
		let i = e.staticInitializerNames.map((e) => `${n}.${e}()`);
		t ? this.tokens.appendCode(`, ${i.map((e) => `${e}, `).join("")}${n})`) : e.staticInitializerNames.length > 0 && this.tokens.appendCode(` ${i.map((e) => `${e};`).join(" ")}`);
	}
	processClassBody(e, t) {
		let { headerInfo: n, constructorInsertPos: r, constructorInitializerStatements: i, fields: a, instanceInitializerNames: o, rangesToRemove: s } = e, c = 0, l = 0, d = this.tokens.currentToken().contextId;
		if (d == null) throw Error("Expected non-null context ID on class.");
		this.tokens.copyExpectedToken(u.braceL), this.isReactHotLoaderTransformEnabled && this.tokens.appendCode("__reactstandin__regenerateByEval(key, code) {this[key] = eval(code);}");
		let f = i.length + o.length > 0;
		if (r === null && f) {
			let e = this.makeConstructorInitCode(i, o, t);
			if (n.hasSuperclass) {
				let t = this.nameManager.claimFreeName("args");
				this.tokens.appendCode(`constructor(...${t}) { super(...${t}); ${e}; }`);
			} else this.tokens.appendCode(`constructor() { ${e}; }`);
		}
		for (; !this.tokens.matchesContextIdAndLabel(u.braceR, d);) if (c < a.length && this.tokens.currentIndex() === a[c].start) {
			let e = !1;
			for (this.tokens.matches1(u.bracketL) ? this.tokens.copyTokenWithPrefix(`${a[c].initializerName}() {this`) : this.tokens.matches1(u.string) || this.tokens.matches1(u.num) ? (this.tokens.copyTokenWithPrefix(`${a[c].initializerName}() {this[`), e = !0) : this.tokens.copyTokenWithPrefix(`${a[c].initializerName}() {this.`); this.tokens.currentIndex() < a[c].end;) e && this.tokens.currentIndex() === a[c].equalsIndex && this.tokens.appendCode("]"), this.processToken();
			this.tokens.appendCode("}"), c++;
		} else if (l < s.length && this.tokens.currentIndex() >= s[l].start) {
			for (this.tokens.currentIndex() < s[l].end && this.tokens.removeInitialToken(); this.tokens.currentIndex() < s[l].end;) this.tokens.removeToken();
			l++;
		} else this.tokens.currentIndex() === r ? (this.tokens.copyToken(), f && this.tokens.appendCode(`;${this.makeConstructorInitCode(i, o, t)};`), this.processToken()) : this.processToken();
		this.tokens.copyExpectedToken(u.braceR);
	}
	makeConstructorInitCode(e, t, n) {
		return [...e, ...t.map((e) => `${n}.prototype.${e}.call(this)`)].join(";");
	}
	processPossibleArrowParamEnd() {
		if (this.tokens.matches2(u.parenR, u.colon) && this.tokens.tokenAtRelativeIndex(1).isType) {
			let e = this.tokens.currentIndex() + 1;
			for (; this.tokens.tokens[e].isType;) e++;
			if (this.tokens.matches1AtIndex(e, u.arrow)) {
				for (this.tokens.removeInitialToken(); this.tokens.currentIndex() < e;) this.tokens.removeToken();
				return this.tokens.replaceTokenTrimmingLeftWhitespace(") =>"), !0;
			}
		}
		return !1;
	}
	processPossibleAsyncArrowWithTypeParams() {
		if (!this.tokens.matchesContextual(l._async) && !this.tokens.matches1(u._async)) return !1;
		let e = this.tokens.tokenAtRelativeIndex(1);
		if (e.type !== u.lessThan || !e.isType) return !1;
		let t = this.tokens.currentIndex() + 1;
		for (; this.tokens.tokens[t].isType;) t++;
		if (this.tokens.matches1AtIndex(t, u.parenL)) {
			for (this.tokens.replaceToken("async ("), this.tokens.removeInitialToken(); this.tokens.currentIndex() < t;) this.tokens.removeToken();
			return this.tokens.removeToken(), this.processBalancedCode(), this.processToken(), !0;
		}
		return !1;
	}
	processPossibleTypeRange() {
		if (this.tokens.currentToken().isType) {
			for (this.tokens.removeInitialToken(); this.tokens.currentToken().isType;) this.tokens.removeToken();
			return !0;
		}
		return !1;
	}
	shiftMappings(e, t) {
		for (let n = 0; n < e.length; n++) {
			let r = e[n];
			r !== void 0 && (e[n] = r + t);
		}
		return e;
	}
};
(/* @__PURE__ */ o(((e) => {
	e.__esModule = !0, e.LinesAndColumns = void 0;
	var t = "\n", n = "\r";
	e.LinesAndColumns = function() {
		function e(e) {
			this.string = e;
			for (var r = [0], i = 0; i < e.length;) switch (e[i]) {
				case t:
					i += t.length, r.push(i);
					break;
				case n:
					i += n.length, e[i] === t && (i += t.length), r.push(i);
					break;
				default:
					i++;
					break;
			}
			this.offsets = r;
		}
		return e.prototype.locationForIndex = function(e) {
			if (e < 0 || e > this.string.length) return null;
			for (var t = 0, n = this.offsets; n[t + 1] <= e;) t++;
			var r = e - n[t];
			return {
				line: t,
				column: r
			};
		}, e.prototype.indexForLocation = function(e) {
			var t = e.line, n = e.column;
			return t < 0 || t >= this.offsets.length || n < 0 || n > this.lengthOfLine(t) ? null : this.offsets[t] + n;
		}, e.prototype.lengthOfLine = function(e) {
			var t = this.offsets[e];
			return (e === this.offsets.length - 1 ? this.string.length : this.offsets[e + 1]) - t;
		}, e;
	}();
})))();
function Sc(e) {
	let t = /* @__PURE__ */ new Set();
	for (let n = 0; n < e.tokens.length; n++) e.matches1AtIndex(n, u._import) && !e.matches3AtIndex(n, u._import, u.name, u.eq) && Cc(e, n, t);
	return t;
}
function Cc(e, t, n) {
	t++, !e.matches1AtIndex(t, u.parenL) && (e.matches1AtIndex(t, u.name) && (n.add(e.identifierNameAtIndex(t)), t++, e.matches1AtIndex(t, u.comma) && t++), e.matches1AtIndex(t, u.star) && (t += 2, n.add(e.identifierNameAtIndex(t)), t++), e.matches1AtIndex(t, u.braceL) && (t++, wc(e, t, n)));
}
function wc(e, t, n) {
	for (;;) {
		if (e.matches1AtIndex(t, u.braceR)) return;
		let r = et(e, t);
		if (t = r.endIndex, r.isType || n.add(r.rightName), e.matches2AtIndex(t, u.comma, u.braceR) || e.matches1AtIndex(t, u.braceR)) return;
		if (e.matches1AtIndex(t, u.comma)) t++;
		else throw Error(`Unexpected token: ${JSON.stringify(e.tokens[t])}`);
	}
}
function Tc(e, t) {
	cn(t);
	try {
		let n = Ec(e, t), r = new xc(n, t.transforms, !!t.enableLegacyBabel5ModuleInterop, t).transform(), i = { code: r.code };
		if (t.sourceMapOptions) {
			if (!t.filePath) throw Error("filePath must be specified when generating a source map.");
			i = {
				...i,
				sourceMap: qt(r, t.filePath, t.sourceMapOptions, e, n.tokenProcessor.tokens)
			};
		}
		return i;
	} catch (e) {
		throw t.filePath && (e.message = `Error transforming ${t.filePath}: ${e.message}`), e;
	}
}
function Ec(e, t) {
	let n = t.transforms.includes("jsx"), r = t.transforms.includes("typescript"), i = t.transforms.includes("flow"), a = t.disableESTransforms === !0, o = Gs(e, n, r, i), s = o.tokens, c = o.scopes, l = new nn(e, s), u = new Xt(l), d = new qs(e, s, i, a, u), f = !!t.enableLegacyTypeScriptModuleInterop, p = null;
	return t.transforms.includes("imports") ? (p = new mt(l, d, f, t, t.transforms.includes("typescript"), !!t.keepUnusedImports, u), p.preprocessTokens(), Zt(d, c, p.getGlobalNames()), t.transforms.includes("typescript") && !t.keepUnusedImports && p.pruneTypeOnlyImports()) : t.transforms.includes("typescript") && !t.keepUnusedImports && Zt(d, c, Sc(d)), {
		tokenProcessor: d,
		scopes: c,
		nameManager: l,
		importProcessor: p,
		helperManager: u
	};
}
var Dc = "41e1b1cefc721fa2889683134f896f1bafa9907d9057800343b2b7376f7d36a1", Oc = Object.freeze({
	path: "tailwindcss-catalog.css",
	role: "shadow_style",
	media_type: "text/css; charset=utf-8",
	sha256: "77c009cb4826b765d416513e3d9c83093482ecb69de9e361e4c25f5441240b36"
}), kc = Object.freeze({
	name: "@1flowbase/tailwindcss-catalog",
	contract: "block-preset-v1",
	tailwind_version: "4.3.3"
}), Ac = Object.freeze({
	package: "tailwindcss",
	version: "4.3.3",
	mode: "block-preset"
}), jc = Object.freeze({
	package: "tailwindcss",
	version: "4.3.3",
	mode: "block-preset",
	sha256: Dc
}), Mc = Object.freeze({
	name: "@1flowbase/tailwindcss-catalog/compiler",
	version: "4.3.3",
	contract: "executable-compiler-v1",
	preset: "default-utilities-standard-variants-v1",
	preset_asset_sha256: Oc.sha256,
	stylesheet_sha256: Dc,
	tsx_validation: "sucrase@3.35.1/dependency-lock-imports-v2"
}), Nc = "2005c459882fcaeb283ff36706b327efebf8783414ecaa111f92f628c4ba0af8", Pc = new Set([
	"react",
	"react/jsx-runtime",
	"antd"
]), Fc = Object.freeze({
	compiler_identity: kc,
	toolchain_lock: Ac,
	stylesheet_identity: jc,
	artifact_identity: Mc,
	artifact_sha256: Nc,
	async compile(e, t) {
		let n = Ic(e, t);
		return n.length > 0 ? {
			generatedCss: "",
			diagnostics: n
		} : {
			generatedCss: "",
			diagnostics: []
		};
	}
});
Object.freeze([Fc]);
function Ic(e, t) {
	let n = new Set(Pc);
	for (let e of t) n.add(e.module_source);
	let r = Lc(e).find((e) => !n.has(e));
	if (r) return [Bc("illegal_import", "source_code.imports", `Import '${r}' is not allowed by the executable compiler contract.`)];
	if (/\bimport\s*\(/u.test(e)) return [Bc("illegal_import", "source_code.imports", "Dynamic import is not allowed by the executable compiler contract.")];
	try {
		return Tc(e, {
			transforms: ["typescript", "jsx"],
			jsxRuntime: "automatic",
			jsxImportSource: "react",
			production: !0,
			filePath: "native-react-block.tsx"
		}), [];
	} catch (e) {
		let t = Rc(e);
		return [{
			...Bc("tsx_transform_failed", "source_code", `TSX validation failed: ${zc(e)}`),
			...t ? { source_location: t } : {}
		}];
	}
}
function Lc(e) {
	let t = [];
	for (let n of e.matchAll(/(?:\bimport\s+(?:[^'";]+?\s+from\s+)?|\bexport\s+[^'";]+?\s+from\s+)['"]([^'"]+)['"]/gu)) t.push(n[1]);
	return t;
}
function Rc(e) {
	if (typeof e != "object" || !e) return;
	let t = e.loc;
	if (typeof t != "object" || !t) return;
	let n = t.line, r = t.column;
	return typeof n == "number" && typeof r == "number" ? {
		line: n,
		column: r + 1
	} : void 0;
}
function zc(e) {
	return e instanceof Error && e.message ? e.message : "Unknown transform error.";
}
function Bc(e, t, n) {
	return {
		phase: "validation",
		code: e,
		path: t,
		message: n
	};
}
var Vc = Object.freeze({
	name: "tailwindcss",
	version: "4.3.3",
	mode: "block-preset",
	compiler: Object.freeze({
		name: "@1flowbase/tailwindcss-catalog",
		contract: "block-preset-v1",
		tailwind_version: "4.3.3"
	})
});
export { Vc as default };
