// Independent CALC-12 reference calculator: lexer, CALC-PARSE-01 parser,
// whole-expression validation, left-to-right exact evaluation, lexical
// session and exact display.
//
// Independence: written from `.cairn/specs/calculator/spec.md` (CALC-PARSE-01,
// CALC-PARSE-02, CALC-STATE-01/02, CALC-DISPLAY-01) and, for the definition
// command spelling only, the README's `let NAME = EXPR` / `fn NAME(P, ...)=EXPR`
// (the spec leaves that spelling an open CLI design gate). It imports only
// `./rational.mjs`; no guest module, Rust host code or #19 runner code is read
// or executed.
//
// Every outcome carries `notes`: points where the spec does not determine the
// observable result (`unspecified:*`) or where two readings of the spec give
// different results (`ambiguous:*`). Spans carry a `basis`; only spans whose
// rule the spec's CALC-02 examples fix are judged by `compare.mjs`.
import * as Q from './rational.mjs';

const encoder = new TextEncoder();

// Spans that the CALC-02 examples determine: an unexpected or stray token,
// the first trailing token, and an unknown identifier.
export const JUDGED_SPAN_BASES = new Set(['token', 'trailing-token', 'identifier']);

class Diagnostic extends Error {
  constructor(category, span, basis, notes = []) {
    super(`${category} [${span[0]},${span[1]})`);
    this.category = category;
    this.span = span;
    this.basis = basis;
    this.notes = notes;
  }
}

const isDigit = byte => byte >= 0x30 && byte <= 0x39;
const isLetter = byte => (byte >= 0x41 && byte <= 0x5a) || (byte >= 0x61 && byte <= 0x7a);
const isIdentStart = byte => isLetter(byte) || byte === 0x5f;
const isIdentPart = byte => isIdentStart(byte) || isDigit(byte);
const isSpace = byte => byte === 0x20 || byte === 0x09 || byte === 0x0d || byte === 0x0a;
const PUNCT = new Map([...'+-*/^(),='].map(ch => [ch.charCodeAt(0), ch]));

// Maximal-munch tokens over UTF-8 bytes in [start, end). Offsets are absolute
// byte offsets in the submitted line. Bytes outside the grammar become
// `invalid` tokens; the caller decides when they are reported.
const tokenize = (bytes, start, end) => {
  const tokens = [];
  let i = start;
  while (i < end) {
    const byte = bytes[i];
    if (isSpace(byte)) { i += 1; continue; }
    if (isDigit(byte)) {
      let j = i;
      while (j < end && isDigit(bytes[j])) j += 1;
      let whole = String.fromCharCode(...bytes.subarray(i, j));
      let fraction = null;
      if (j < end && bytes[j] === 0x2e && j + 1 < end && isDigit(bytes[j + 1])) {
        let k = j + 1;
        while (k < end && isDigit(bytes[k])) k += 1;
        fraction = String.fromCharCode(...bytes.subarray(j + 1, k));
        j = k;
      }
      tokens.push({ type: 'number', whole, fraction, span: [i, j] });
      i = j;
      continue;
    }
    if (isIdentStart(byte)) {
      let j = i;
      while (j < end && isIdentPart(bytes[j])) j += 1;
      tokens.push({ type: 'ident', text: String.fromCharCode(...bytes.subarray(i, j)), span: [i, j] });
      i = j;
      continue;
    }
    if (PUNCT.has(byte)) {
      tokens.push({ type: PUNCT.get(byte), span: [i, i + 1] });
      i += 1;
      continue;
    }
    // One invalid byte, or one whole UTF-8 sequence when it is well formed.
    let width = 1;
    if (byte >= 0xc0) width = byte >= 0xf0 ? 4 : byte >= 0xe0 ? 3 : 2;
    tokens.push({ type: 'invalid', span: [i, Math.min(end, i + width)] });
    i += width;
  }
  return tokens;
};

// Recursive descent for CALC-PARSE-01. `tokens` may contain `invalid` tokens;
// the parser reports one as SyntaxError when it reaches it.
const parseExpression = (tokens, endOffset, semantics) => {
  let position = 0;
  let depth = 0;
  const peek = () => tokens[position];
  const endSpan = [endOffset, endOffset];
  const unexpected = () => {
    const token = peek();
    if (token === undefined) {
      throw new Diagnostic('SyntaxError', endSpan, 'end-of-input', ['unspecified:end-of-input-span']);
    }
    throw new Diagnostic('SyntaxError', token.span, 'token');
  };
  const accept = type => {
    if (peek()?.type === type) { position += 1; return tokens[position - 1]; }
    return null;
  };
  const expect = type => accept(type) ?? unexpected();

  const expression = () => {
    depth += 1;
    const node = sum();
    depth -= 1;
    return node;
  };
  const sum = () => {
    let left = product();
    for (;;) {
      const op = accept('+') ?? accept('-');
      if (op === null) return left;
      left = { kind: 'binary', op: op.type, opSpan: op.span, left, right: product() };
    }
  };
  const product = () => {
    let left = unary();
    for (;;) {
      const op = accept('*') ?? accept('/');
      if (op === null) return left;
      left = { kind: 'binary', op: op.type, opSpan: op.span, left, right: unary() };
    }
  };
  const unary = () => {
    const sign = accept('+') ?? accept('-');
    if (sign !== null) return { kind: 'unary', op: sign.type, opSpan: sign.span, operand: unary() };
    return power();
  };
  // power := primary [ "^" unary ]  -- right associative, binds tighter than
  // a leading sign, signed exponents allowed.
  const power = () => {
    const base = primary();
    const caret = accept('^');
    if (caret === null) return base;
    if (semantics.power === 'right') {
      return { kind: 'binary', op: '^', opSpan: caret.span, left: base, right: unary() };
    }
    // Seeded-fault semantics only: left-associated power chains.
    let node = { kind: 'binary', op: '^', opSpan: caret.span, left: base, right: signedPrimary() };
    for (let next = accept('^'); next !== null; next = accept('^')) {
      node = { kind: 'binary', op: '^', opSpan: next.span, left: node, right: signedPrimary() };
    }
    return node;
  };
  const signedPrimary = () => {
    const sign = accept('+') ?? accept('-');
    if (sign !== null) return { kind: 'unary', op: sign.type, opSpan: sign.span, operand: signedPrimary() };
    return primary();
  };
  const primary = () => {
    const token = peek();
    if (token?.type === 'number') {
      position += 1;
      return { kind: 'number', value: semantics.arith.decimal(token.whole, token.fraction ?? ''), span: token.span };
    }
    if (token?.type === 'ident') {
      position += 1;
      if (accept('(') === null) return { kind: 'name', name: token.text, span: token.span };
      const args = [];
      if (accept(')') === null) {
        args.push(expression());
        while (accept(',') !== null) args.push(expression());
        expect(')');
      }
      const close = tokens[position - 1];
      return { kind: 'call', name: token.text, nameSpan: token.span, args, span: [token.span[0], close.span[1]] };
    }
    if (accept('(') !== null) {
      const inner = expression();
      expect(')');
      return inner;
    }
    return unexpected();
  };

  const tree = expression();
  if (position < tokens.length) {
    const trailing = tokens[position];
    if (trailing.type === 'invalid') throw new Diagnostic('SyntaxError', trailing.span, 'token');
    throw new Diagnostic('TrailingInput', trailing.span, 'trailing-token');
  }
  return tree;
};

// Lexical errors: the spec does not say whether a stray byte is reported
// before a parse error that occurs earlier in the line. The oracle reports
// the first invalid byte (tokens first), and notes when the lazy reading
// (parse until the stray byte is reached) would report something else.
const parseChecked = (bytes, start, end, semantics) => {
  const tokens = tokenize(bytes, start, end);
  const firstInvalid = tokens.find(token => token.type === 'invalid');
  if (firstInvalid === undefined) return parseExpression(tokens, end, semantics);
  let lazy;
  try {
    parseExpression(tokens, end, semantics);
    lazy = null;
  } catch (error) {
    if (!(error instanceof Diagnostic)) throw error;
    lazy = error;
  }
  const notes = [];
  if (lazy === null || lazy.category !== 'SyntaxError' || lazy.span[0] !== firstInvalid.span[0]) {
    notes.push('ambiguous:lexical-error-order');
  }
  throw new Diagnostic('SyntaxError', firstInvalid.span, 'token', notes);
};

// Whole-expression validation before evaluation (CALC-PARSE-02), in source
// order. `scope` resolves parameter names first, then captured globals.
const validate = (node, resolve, context) => {
  switch (node.kind) {
  case 'number':
    return;
  case 'unary':
    validate(node.operand, resolve, context);
    return;
  case 'binary':
    validate(node.left, resolve, context);
    validate(node.right, resolve, context);
    return;
  case 'name': {
    if (context.selfName === node.name && !context.parameters.has(node.name)) {
      throw new Diagnostic('DefinitionError', node.span, 'self-reference', ['unspecified:definition-error-span']);
    }
    const binding = resolve(node.name);
    if (binding === undefined) throw new Diagnostic('UnknownName', node.span, 'identifier');
    if (binding.kind === 'function') {
      throw new Diagnostic('UnspecifiedFunctionAsValue', node.span, 'identifier',
        ['unspecified:function-name-used-as-value']);
    }
    return;
  }
  case 'call': {
    if (context.selfName === node.name && !context.parameters.has(node.name)) {
      throw new Diagnostic('DefinitionError', node.nameSpan, 'self-reference', ['unspecified:definition-error-span']);
    }
    const binding = context.parameters.has(node.name) ? context.parameters.get(node.name) : resolve(node.name);
    if (binding === undefined) throw new Diagnostic('UnknownName', node.nameSpan, 'identifier');
    if (binding.kind !== 'function') {
      throw new Diagnostic('UnspecifiedCallOfValue', node.nameSpan, 'identifier',
        ['unspecified:call-of-non-function']);
    }
    if (binding.parameters.length !== node.args.length) {
      // Whether arguments are validated before arity is unspecified.
      const notes = ['unspecified:wrong-arity-span'];
      for (const argument of node.args) {
        try { validate(argument, resolve, context); } catch (error) {
          if (!(error instanceof Diagnostic)) throw error;
          notes.push('ambiguous:arity-versus-argument-order');
          break;
        }
      }
      throw new Diagnostic('WrongArity', node.span, 'call', notes);
    }
    for (const argument of node.args) validate(argument, resolve, context);
    return;
  }
  default:
    throw new Error(`unknown node ${node.kind}`);
  }
};

const numericError = (error, span) => {
  if (error instanceof Q.OracleError) {
    return new Diagnostic(error.category, span, 'operator', ['unspecified:numeric-error-span']);
  }
  return error;
};

// Left-to-right exact evaluation; an error stops the remaining operands.
const evaluate = (node, lookup, arith) => {
  switch (node.kind) {
  case 'number':
    return node.value;
  case 'unary': {
    const value = evaluate(node.operand, lookup, arith);
    return node.op === '-' ? arith.neg(value) : value;
  }
  case 'binary': {
    const left = evaluate(node.left, lookup, arith);
    const right = evaluate(node.right, lookup, arith);
    try {
      switch (node.op) {
      case '+': return arith.add(left, right);
      case '-': return arith.sub(left, right);
      case '*': return arith.mul(left, right);
      case '/': return arith.div(left, right);
      case '^': return arith.pow(left, right);
      default: throw new Error(`operator ${node.op}`);
      }
    } catch (error) { throw numericError(error, node.opSpan); }
  }
  case 'name':
    return lookup(node.name).value;
  case 'call': {
    const fn = lookup(node.name);
    const args = node.args.map(argument => evaluate(argument, lookup, arith));
    const frame = new Map(fn.parameters.map((name, index) => [name, args[index]]));
    return evaluate(fn.body, name => (frame.has(name)
      ? { kind: 'value', value: frame.get(name) } : fn.captured.get(name)), arith);
  }
  default:
    throw new Error(`unknown node ${node.kind}`);
  }
};

const readIdent = (bytes, i, end) => {
  if (i >= end || !isIdentStart(bytes[i])) return null;
  let j = i;
  while (j < end && isIdentPart(bytes[j])) j += 1;
  return { text: String.fromCharCode(...bytes.subarray(i, j)), span: [i, j] };
};
const skip = (bytes, i, end) => { while (i < end && isSpace(bytes[i])) i += 1; return i; };
const headerError = (i, end) => new Diagnostic('SyntaxError', [i, Math.min(i + 1, end)], 'definition-header',
  ['unspecified:definition-header-span']);

// README spelling: `let NAME = EXPR` / `fn NAME(P, ...)=EXPR`. Returns null
// for an expression line.
const definitionHeader = (bytes, end) => {
  const start = skip(bytes, 0, end);
  const keyword = readIdent(bytes, start, end);
  if (keyword === null || (keyword.text !== 'let' && keyword.text !== 'fn')) return null;
  let i = skip(bytes, keyword.span[1], end);
  if (i === keyword.span[1] && keyword.text === 'let') return null;
  const name = readIdent(bytes, i, end);
  if (name === null) throw headerError(i, end);
  i = skip(bytes, name.span[1], end);
  const parameters = [];
  if (keyword.text === 'fn') {
    if (bytes[i] !== 0x28) throw headerError(i, end);
    i = skip(bytes, i + 1, end);
    if (bytes[i] !== 0x29) {
      for (;;) {
        const parameter = readIdent(bytes, i, end);
        if (parameter === null) throw headerError(i, end);
        parameters.push(parameter);
        i = skip(bytes, parameter.span[1], end);
        if (bytes[i] === 0x2c) { i = skip(bytes, i + 1, end); continue; }
        if (bytes[i] === 0x29) break;
        throw headerError(i, end);
      }
    }
    i = skip(bytes, i + 1, end);
  }
  if (bytes[i] !== 0x3d) throw headerError(i, end);
  return { kind: keyword.text, name, parameters, bodyStart: i + 1 };
};

const decodeStrict = bytes => {
  try {
    new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    return null;
  } catch {
    let i = 0;
    while (i < bytes.length && bytes[i] < 0x80) i += 1;
    return new Diagnostic('SyntaxError', [i, i + 1], 'token', ['unspecified:invalid-utf8-span']);
  }
};

export const display = (value, mode) =>
  (mode === 'finite-decimal' ? Q.displayFiniteDecimal(value) : Q.displayDefault(value));

// A session is an immutable Map from name to binding. `submit` returns the
// outcome and the session to use next; a failure returns the prior session.
export const emptySession = () => new Map();

// The exact semantics. `faults.mjs` substitutes seeded incorrect variants.
export const EXACT = Object.freeze({ arith: Q, power: 'right' });

const outcomeOf = diagnostic => ({
  kind: 'diagnostic', category: diagnostic.category, span: diagnostic.span, basis: diagnostic.basis,
  notes: diagnostic.notes,
});

export const submit = (session, line, { mode = 'default', semantics = EXACT } = {}) => {
  const bytes = typeof line === 'string' ? encoder.encode(line) : line;
  const end = bytes.length;
  try {
    const invalid = decodeStrict(bytes);
    if (invalid !== null) throw invalid;
    const header = definitionHeader(bytes, end);
    if (header === null) {
      const tree = parseChecked(bytes, 0, end, semantics);
      validate(tree, name => session.get(name), { selfName: null, parameters: new Map() });
      const value = evaluate(tree, name => session.get(name), semantics.arith);
      return { outcome: { kind: 'value', value, display: display(value, mode), notes: [] }, session };
    }
    const notes = [];
    const seen = new Set();
    for (const parameter of header.parameters) {
      if (seen.has(parameter.text)) {
        throw new Diagnostic('DefinitionError', parameter.span, 'parameter', ['unspecified:definition-error-span']);
      }
      seen.add(parameter.text);
    }
    const tree = parseChecked(bytes, header.bodyStart, end, semantics);
    const parameterBindings = new Map(header.parameters.map(p => [p.text, { kind: 'value', parameter: true }]));
    const resolve = name => (parameterBindings.has(name) ? parameterBindings.get(name) : session.get(name));
    if (header.kind === 'let') {
      validate(tree, resolve, { selfName: null, parameters: new Map() });
      const value = evaluate(tree, name => session.get(name), semantics.arith);
      if (session.get(header.name.text)?.kind === 'function') notes.push('unspecified:variable-replaces-function');
      const next = new Map(session);
      next.set(header.name.text, { kind: 'value', value });
      // CALC-STATE-02: an accepted submission yields "an accepted value and
      // new session state"; a variable definition reports its value. A
      // function definition has no value and is silent.
      return { outcome: { kind: 'value', value, display: display(value, mode), notes }, session: next };
    }
    if (session.has(header.name.text)) {
      notes.push('ambiguous:self-reference-versus-earlier-binding');
    }
    validate(tree, resolve, { selfName: header.name.text, parameters: parameterBindings });
    const next = new Map(session);
    next.set(header.name.text, {
      kind: 'function', parameters: header.parameters.map(p => p.text), body: tree, captured: new Map(session),
    });
    return { outcome: { kind: 'silent', notes }, session: next };
  } catch (error) {
    if (!(error instanceof Diagnostic)) throw error;
    return { outcome: outcomeOf(error), session };
  }
};

// Runs every line of one process in order (one session per process, as the
// CLI does) and returns per-line outcomes plus the expected process streams.
export const run = (lines, options = {}) => {
  let session = emptySession();
  const outcomes = [];
  for (const line of lines) {
    const result = submit(session, line, options);
    session = result.session;
    outcomes.push(result.outcome);
  }
  return {
    outcomes,
    stdout: outcomes.filter(o => o.kind === 'value').map(o => o.display),
    stderr: outcomes.filter(o => o.kind === 'diagnostic'),
    status: outcomes.some(o => o.kind === 'diagnostic') ? 1 : 0,
  };
};
