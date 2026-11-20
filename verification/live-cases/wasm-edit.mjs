// Canonical unsigned-LEB128 section editor for the LIVE-04 mutated-byte
// variants. Every rewrite re-encodes the touched section/body lengths
// canonically and keeps the module's existing type and function indices in
// place; the one variant that adds a function import shifts only the
// function-index references (exports and element entries) by exactly one.
import assert from 'node:assert/strict';

export function lebEncode(value) {
  assert.ok(Number.isSafeInteger(value) && value >= 0, 'LEB128 value');
  const bytes = [];
  let rest = value;
  do {
    let byte = rest % 128;
    rest = Math.floor(rest / 128);
    if (rest > 0) byte |= 0x80;
    bytes.push(byte);
  } while (rest > 0);
  return Buffer.from(bytes);
}

export function lebDecode(bytes, at) {
  let value = 0;
  let shift = 0;
  let index = at;
  for (let n = 0; n < 5; n += 1, shift += 7) {
    assert.ok(index < bytes.length, 'truncated LEB128');
    const byte = bytes[index];
    index += 1;
    value += (byte & 0x7f) * 2 ** shift;
    if (!(byte & 0x80)) {
      assert.ok(!(n > 0 && byte === 0), 'noncanonical LEB128');
      return { value, next: index };
    }
  }
  throw new Error('invalid LEB128');
}

function textAt(bytes, at) {
  const length = lebDecode(bytes, at);
  const end = length.next + length.value;
  assert.ok(end <= bytes.length, 'truncated Wasm name');
  return { value: bytes.subarray(length.next, end), next: end };
}

export function sectionsOf(bytes) {
  assert.ok(Buffer.isBuffer(bytes) && bytes.length >= 8, 'bounded Wasm module');
  assert.equal(bytes.readUInt32LE(0), 0x6d736100);
  assert.equal(bytes.readUInt32LE(4), 1);
  const sections = [];
  let at = 8;
  while (at < bytes.length) {
    const id = bytes[at];
    at += 1;
    const size = lebDecode(bytes, at);
    at = size.next;
    const end = at + size.value;
    assert.ok(end <= bytes.length, 'truncated Wasm section');
    sections.push({ id, body: Buffer.from(bytes.subarray(at, end)) });
    at = end;
  }
  return sections;
}

export function moduleOf(sections) {
  return Buffer.concat([
    Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]),
    ...sections.map(section => Buffer.concat([
      Buffer.from([section.id]), lebEncode(section.body.length), section.body,
    ])),
  ]);
}

function find(sections, id) {
  const index = sections.findIndex(section => section.id === id);
  assert.ok(index >= 0, `Wasm section ${id} present`);
  return index;
}

function vectorCount(body) {
  const count = lebDecode(body, 0);
  return { value: count.value, prefix: count.next };
}

function appendVector(body, entry) {
  const count = vectorCount(body);
  return Buffer.concat([lebEncode(count.value + 1), body.subarray(count.prefix), entry]);
}

function importFunctionCount(body) {
  const count = vectorCount(body);
  let at = count.prefix;
  let functions = 0;
  for (let index = 0; index < count.value; index += 1) {
    const module = textAt(body, at);
    const name = textAt(body, module.next);
    at = name.next;
    const kind = body[at];
    at += 1;
    if (kind === 0) {
      functions += 1;
      at = lebDecode(body, at).next;
    } else if (kind === 1) {
      at += 1; // reftype
      const flags = body[at];
      at += 1;
      at = lebDecode(body, at).next;
      if (flags & 1) at = lebDecode(body, at).next;
    } else if (kind === 2) {
      const flags = body[at];
      at += 1;
      at = lebDecode(body, at).next;
      if (flags & 1) at = lebDecode(body, at).next;
    } else if (kind === 3) {
      at += 2; // value type and mutability
    } else {
      throw new Error('unsupported Wasm import kind');
    }
    assert.ok(at <= body.length, 'truncated Wasm import');
  }
  assert.equal(at, body.length, 'Wasm import vector is fully consumed');
  return functions;
}

function functionCount(body) {
  return vectorCount(body).value;
}

function shiftExports(body, delta) {
  const count = vectorCount(body);
  let at = count.prefix;
  const parts = [lebEncode(count.value)];
  for (let index = 0; index < count.value; index += 1) {
    const name = textAt(body, at);
    const kind = body[name.next];
    const value = lebDecode(body, name.next + 1);
    assert.ok(value.next <= body.length, 'truncated Wasm export');
    parts.push(body.subarray(at, name.next + 1));
    parts.push(lebEncode(kind === 0 ? value.value + delta : value.value));
    at = value.next;
  }
  assert.equal(at, body.length, 'Wasm export vector is fully consumed');
  return Buffer.concat(parts);
}

function shiftElementFunctions(body, delta) {
  const count = vectorCount(body);
  let at = count.prefix;
  const parts = [lebEncode(count.value)];
  for (let index = 0; index < count.value; index += 1) {
    const flags = lebDecode(body, at);
    at = flags.next;
    parts.push(lebEncode(flags.value));
    if (flags.value === 0 || flags.value === 2) {
      if (flags.value === 2) {
        parts.push(body.subarray(at, at + 1));
        at += 1;
      }
      // Active offset expressions in the baseline are `i32.const 0; end`.
      const offset = body.subarray(at, at + 3);
      assert.deepEqual(offset, Buffer.from([0x41, 0x00, 0x0b]), 'offset expression');
      parts.push(offset);
      at += 3;
    }
    if (flags.value !== 0) {
      parts.push(body.subarray(at, at + 1)); // elemkind
      at += 1;
    }
    const functions = lebDecode(body, at);
    at = functions.next;
    parts.push(lebEncode(functions.value));
    for (let entry = 0; entry < functions.value; entry += 1) {
      const functionIndex = lebDecode(body, at);
      at = functionIndex.next;
      parts.push(lebEncode(functionIndex.value + delta));
    }
  }
  assert.equal(at, body.length, 'Wasm element vector is fully consumed');
  return Buffer.concat(parts);
}

function signedLeb(bytes, at) {
  let value = 0;
  let shift = 0;
  let index = at;
  let byte;
  do {
    assert.ok(index < bytes.length, 'truncated signed LEB128');
    byte = bytes[index];
    index += 1;
    value |= (byte & 0x7f) << shift;
    shift += 7;
  } while (byte & 0x80);
  if (shift < 32 && byte & 0x40) value |= ~0 << shift;
  return { value, next: index };
}

// Rewrite function-index immediates in one expression. The observed module
// uses the Core MVP subset plus the bulk-memory prefixes; any opcode outside
// the table is an error rather than a guessed walk.
function shiftInstructions(bytes, at, end, delta, output) {
  while (at < end) {
    const opcode = bytes[at];
    at += 1;
    output.push(opcode);
    if (opcode === 0x10 || opcode === 0xd2) {
      const value = lebDecode(bytes, at);
      at = value.next;
      output.push(...lebEncode(value.value + delta));
    } else if ([0x0c, 0x0d, 0x20, 0x21, 0x22, 0x23, 0x24].includes(opcode)) {
      const value = lebDecode(bytes, at);
      at = value.next;
      output.push(...lebEncode(value.value));
    } else if ([0x02, 0x03, 0x04].includes(opcode)) {
      const blockType = bytes[at];
      if (blockType === 0x40 || [0x7f, 0x7e, 0x7d, 0x7c, 0x7b, 0x70, 0x6f].includes(blockType)) {
        output.push(blockType);
        at += 1;
      } else {
        throw new Error('type-index block types are unsupported by this editor');
      }
    } else if (opcode === 0x11) {
      const type = lebDecode(bytes, at);
      at = type.next;
      const table = lebDecode(bytes, at);
      at = table.next;
      output.push(...lebEncode(type.value), ...lebEncode(table.value));
    } else if (opcode === 0x41 || opcode === 0x42) {
      const start = at;
      const value = signedLeb(bytes, at);
      at = value.next;
      output.push(...bytes.subarray(start, at));
    } else if (opcode === 0x1c) {
      const count = lebDecode(bytes, at);
      at = count.next;
      output.push(...lebEncode(count.value), ...bytes.subarray(at, at + count.value));
      at += count.value;
    } else if (opcode >= 0x28 && opcode <= 0x3e) {
      const align = lebDecode(bytes, at);
      at = align.next;
      const offset = lebDecode(bytes, at);
      at = offset.next;
      output.push(...lebEncode(align.value), ...lebEncode(offset.value));
    } else if (opcode === 0x3f || opcode === 0x40) {
      output.push(bytes[at]);
      at += 1;
    } else if (opcode === 0xfc) {
      const sub = lebDecode(bytes, at);
      at = sub.next;
      output.push(...lebEncode(sub.value));
      const immediateCounts = new Map([[8, 2], [9, 1], [10, 2], [11, 1], [12, 2],
        [13, 1], [14, 2], [15, 1], [16, 1], [17, 1]]);
      const count = immediateCounts.get(sub.value);
      assert.ok(count !== undefined, `unsupported 0xfc subopcode ${sub.value}`);
      for (let index = 0; index < count; index += 1) {
        const value = lebDecode(bytes, at);
        at = value.next;
        output.push(...lebEncode(value.value));
      }
    } else {
      const structural = [0x00, 0x01, 0x05, 0x0b, 0x0f, 0x1a, 0x1b].includes(opcode);
      const numeric = opcode >= 0x45 && opcode <= 0xc4;
      assert.ok(structural || numeric, `unsupported opcode ${opcode.toString(16)}`);
    }
  }
}

function shiftCodeFunctions(body, delta) {
  const count = vectorCount(body);
  let at = count.prefix;
  const parts = [lebEncode(count.value)];
  for (let index = 0; index < count.value; index += 1) {
    const size = lebDecode(body, at);
    at = size.next;
    const end = at + size.value;
    const locals = lebDecode(body, at);
    let cursor = locals.next;
    const localsParts = [lebEncode(locals.value)];
    for (let local = 0; local < locals.value; local += 1) {
      const declared = lebDecode(body, cursor);
      cursor = declared.next;
      const type = body[cursor];
      cursor += 1;
      localsParts.push(lebEncode(declared.value), Buffer.from([type]));
    }
    const instructions = [];
    shiftInstructions(body, cursor, end, delta, instructions);
    const content = Buffer.concat([...localsParts, Buffer.from(instructions)]);
    parts.push(lebEncode(content.length), content);
    at = end;
  }
  assert.equal(at, body.length, 'Wasm code vector is fully consumed');
  return Buffer.concat(parts);
}

function appendVoidFunction(sections, instructions) {
  const types = sections[find(sections, 1)];
  const typeCount = vectorCount(types.body).value;
  types.body = appendVector(types.body, Buffer.from([0x60, 0x00, 0x00]));
  const functions = sections[find(sections, 3)];
  functions.body = appendVector(functions.body, lebEncode(typeCount));
  const code = sections[find(sections, 10)];
  const body = Buffer.concat([
    Buffer.from([0x00]), instructions, Buffer.from([0x0b]),
  ]);
  code.body = appendVector(code.body, Buffer.concat([lebEncode(body.length), body]));
  return importFunctionCount(sections[find(sections, 2)].body) + functionCount(functions.body) - 1;
}

function insertSection(sections, id, body, before) {
  const index = sections.findIndex(section => section.id === before);
  assert.ok(index >= 0, `insertion point section ${before}`);
  sections.splice(index, 0, { id, body });
}

function customPadding(baseline, target) {
  const header = 8;
  const overhead = 1; // custom section id
  let lengthBytes = 1;
  for (;;) {
    const pad = target - baseline.length - overhead - lengthBytes;
    if (pad < 0) throw new Error('padding target below baseline size');
    if (lebEncode(pad).length === lengthBytes) {
      return Buffer.concat([Buffer.from([0x00]), lebEncode(pad), Buffer.alloc(pad)]);
    }
    lengthBytes = lebEncode(pad).length;
    assert.ok(header > 0);
  }
}

// The ten variants the LIVE-04 case names. `different` is the independently
// emitted, source-valid module from `2 2 +`; the editor never runs on it.
export function live04Variants(baseline, different) {
  const variants = new Map();
  const copy = () => sectionsOf(baseline);

  const control = copy();
  appendVoidFunction(control, Buffer.from([0x6a]));
  variants.set('invalid-wasm-control-stack', moduleOf(control));

  const unknown = copy();
  appendVoidFunction(unknown, Buffer.from([0xff]));
  variants.set('unknown-opcode', moduleOf(unknown));

  const version = Buffer.from(baseline);
  version.writeUInt32LE(2, 4);
  variants.set('wrong-bytecode-version', version);

  const start = copy();
  const startIndex = appendVoidFunction(start, Buffer.alloc(0));
  insertSection(start, 8, lebEncode(startIndex), 9);
  variants.set('start-section', moduleOf(start));

  const data = copy();
  const dataSection = data[find(data, 11)];
  dataSection.body = appendVector(dataSection.body, Buffer.concat([
    lebEncode(0), Buffer.from([0x41, 0x00, 0x0b]), lebEncode(1), Buffer.from([0x41]),
  ]));
  const dataCount = data[find(data, 12)];
  dataCount.body = lebEncode(vectorCount(dataCount.body).value + 1);
  variants.set('active-imported-memory-initializer', moduleOf(data));

  const element = copy();
  const elementSection = element[find(element, 9)];
  elementSection.body = appendVector(elementSection.body, Buffer.concat([
    lebEncode(0), Buffer.from([0x41, 0x00, 0x0b]), lebEncode(1), lebEncode(0),
  ]));
  variants.set('active-imported-table-element-initializer', moduleOf(element));

  const imported = copy();
  const types = imported[find(imported, 1)];
  const typeCount = vectorCount(types.body).value;
  types.body = appendVector(types.body, Buffer.from([0x60, 0x01, 0x7f, 0x01, 0x7f]));
  const imports = imported[find(imported, 2)];
  imports.body = appendVector(imports.body, Buffer.concat([
    lebEncode(3), Buffer.from('env'), lebEncode(4), Buffer.from('open'),
    Buffer.from([0x00]), lebEncode(typeCount),
  ]));
  imported[find(imported, 7)].body = shiftExports(imported[find(imported, 7)].body, 1);
  imported[find(imported, 9)].body =
    shiftElementFunctions(imported[find(imported, 9)].body, 1);
  imported[find(imported, 10)].body = shiftCodeFunctions(imported[find(imported, 10)].body, 1);
  variants.set('unauthorized-import', moduleOf(imported));

  variants.set('forged-source-digest', Buffer.from(different));
  variants.set('valid-wasm-from-different-source', Buffer.from(different));
  variants.set('excess-module-bytes', Buffer.concat([baseline, customPadding(baseline, 4_194_305)]));
  return variants;
}

export function editorSelfCheck(baseline) {
  const sections = sectionsOf(baseline);
  assert.deepEqual(moduleOf(sections), Buffer.from(baseline), 'section editor identity');
  for (const [name, bytes] of live04Variants(baseline, baseline)) {
    assert.ok(Buffer.isBuffer(bytes) && bytes.length >= 8, `${name} is a bounded module`);
    if (name !== 'excess-module-bytes' && name !== 'wrong-bytecode-version') sectionsOf(bytes);
  }
}
