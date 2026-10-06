// The separate opt-in slot worker does not load CoreEngine's sixteen-page
// policy. Check every imported arena object and every executable ABI before
// V8 can instantiate against its persistent twenty-page memory/table.
const slotImports = Object.freeze({
  live_select: 'i32,i32->i32', live_bind_validate: 'i32,i32->i32',
  live_frame_enter: 'i32,i32,i32->i32', live_frame_leave: 'i32->i32',
  resource_validate: 'i32,i32->i32',
  resource_bind_validate: 'i32,i32,i32,i32,i32,i32->i32',
  test_emit: 'i32,i32->i32', test_abort: '->i32',
});
const slotExports = Object.freeze({
  begin: 'i32,i32,i32,i32,i32,i32->i32', submit: '->i32',
  push_i64: 'i64->i32', push_bool: 'i32->i32', push_unit: '->i32',
  push_program: 'i32->i32', push_contract: 'i32,i64,i32,i32->i32',
  push_evidence: 'i32,i32,i32,i32->i32',
  push_certified: 'i32,i32,i32,i64->i32', certified_subject: 'i32->i32',
  stack_length: '->i32', stack_kind: 'i32->i32', stack_value: 'i32->i64',
  cell_kind: 'i32->i32', cell_payload: 'i32->i64',
  cell_a: 'i32->i32', cell_b: 'i32->i32', cell_c: 'i32->i32',
  cell_x: 'i32->i32', cell_y: 'i32->i32', cell_z: 'i32->i32',
  cell_w: 'i32->i32', cell_n: 'i32->i32',
  observe: 'i32->i32', reflect: '->i32', recipe_length: '->i32',
  recipe_kind: 'i32->i32', recipe_value: 'i32->i64',
  metric: 'i32->i64', signature_address: 'i32->i32',
  signature_length: 'i32->i32',
  push_nominal_resource: 'i32,i32,i32,i32,i32,i32->i32',
  begin_live: 'i32,i32,i32,i32,i32,i32->i32',
  bind_live_ref: 'i32,i32->i32', retain_target: 'i32->i32',
  release_target: 'i32->i32', collect_unowned: '->i32',
  module_program_count: '->i32', module_program_handle: 'i32->i32',
  install_target: '->i32', invoke_live: '->i32',
  save_stack_program: 'i32->i32', release_saved_program: 'i32->i32',
  clear_stack: '->i32', code_owners: 'i32,i32->i32',
  release_module_roots: '->i32',
});

function checkSlotBytes(bytes, abi) {
  if (!Buffer.isBuffer(bytes) || bytes.length < 8 || bytes.length > MAX_FRAME
    || bytes.readUInt32LE(0) !== 0x6d736100 || bytes.readUInt32LE(4) !== 1) {
    fail('invalid bounded slot Wasm module');
  }
  let at = 8, end = bytes.length, lastSection = 0;
  const byte = () => {
    if (at >= end) fail('truncated slot Wasm section');
    return bytes[at++];
  };
  const uint = () => {
    let value = 0, shift = 0;
    for (let n = 0; n < 5; n += 1, shift += 7) {
      const digit = byte();
      if (n === 4 && digit > 15) fail('invalid slot Wasm integer');
      value += (digit & 127) * 2 ** shift;
      if (!(digit & 128)) {
        if (n && digit === 0) fail('noncanonical slot Wasm integer');
        return value;
      }
    }
    fail('invalid slot Wasm integer');
  };
  const text = () => {
    const length = uint();
    if (length > end - at) fail('truncated slot Wasm name');
    const value = utf8.decode(bytes.subarray(at, at + length));
    at += length;
    return value;
  };
  const count = () => integer(uint(), MAX_FRAME, 'slot Wasm vector');
  const signature = () => {
    if (byte() !== 0x60) fail('unsupported slot Wasm function type');
    const params = [], results = [];
    const valueType = () => {
      const type = byte();
      if (type === 0x7f) return 'i32';
      if (type === 0x7e) return 'i64';
      fail('unsupported slot Wasm value type');
    };
    const p = count();
    if (p > 16) fail('oversized slot Wasm function signature');
    for (let i = 0; i < p; i++) params.push(valueType());
    const r = count();
    if (r > 2) fail('oversized slot Wasm function result');
    for (let i = 0; i < r; i++) results.push(valueType());
    return `${params.join(',')}->${results.join(',')}`;
  };
  const limits = (initial, maximum) => {
    if (byte() !== 1 || uint() !== initial || uint() !== maximum) fail('incompatible slot Wasm arena limits');
  };
  const expectedGlobals = new Map(abi.globals.map(global => [global.name, global.type]));
  const types = [], functionTypes = [], imports = [], exports = [];
  const seenImports = new Set(), seenExports = new Set();
  let memoryImports = 0, tableImports = 0, globalImports = 0;
  while (at < bytes.length) {
    end = bytes.length;
    const id = byte(), size = uint();
    if (size > bytes.length - at) fail('truncated slot Wasm section');
    end = at + size;
    if (id !== 0) {
      const order = id === 12 ? 9.5 : id;
      if (id > 12 || order <= lastSection || id === 8) fail('invalid or start slot Wasm section');
      lastSection = order;
    }
    if (id === 1) {
      const n = count();
      if (n > 16384) fail('too many slot Wasm types');
      for (let i = 0; i < n; i++) types.push(signature());
    } else if (id === 2) {
      const n = count();
      if (n > 256) fail('too many slot Wasm imports');
      for (let i = 0; i < n; i++) {
        const module = text(), name = text(), kind = byte();
        if (module !== abi.module || seenImports.has(name)) fail('unexpected or duplicate slot Wasm import');
        seenImports.add(name);
        if (kind === 0) {
          const type = types[uint()];
          if (type !== slotImports[name]) fail('undeclared or mistyped slot host effect');
          functionTypes.push(type);
          imports.push({ module, name, kind: 'function' });
        } else if (kind === 1) {
          if (name !== abi.table.name || byte() !== 0x70) fail('incompatible slot Wasm table import');
          limits(abi.table.initial, abi.table.maximum);
          tableImports++;
          imports.push({ module, name, kind: 'table' });
        } else if (kind === 2) {
          if (name !== abi.memory.name) fail('incompatible slot Wasm memory import');
          limits(abi.memory.initial, abi.memory.maximum);
          memoryImports++;
          imports.push({ module, name, kind: 'memory' });
        } else if (kind === 3) {
          const type = byte(), mutable = byte();
          if (mutable !== 1 || expectedGlobals.get(name) !==
            (type === 0x7f ? 'i32' : type === 0x7e ? 'i64' : null)) {
            fail('incompatible slot Wasm global import');
          }
          globalImports++;
          imports.push({ module, name, kind: 'global' });
        } else fail('unsupported slot Wasm import kind');
      }
    } else if (id === 3) {
      const n = count();
      if (n > 16384) fail('too many slot Wasm functions');
      for (let i = 0; i < n; i++) {
        const type = types[uint()];
        if (type === undefined) fail('invalid slot Wasm function type');
        functionTypes.push(type);
      }
    } else if (id === 7) {
      const n = count();
      if (n !== Object.keys(slotExports).length + 1
        || Object.keys(abi.exports).length !== n - 1) fail('incompatible slot Wasm export count');
      for (let i = 0; i < n; i++) {
        const name = text(), kind = byte(), index = uint();
        if (seenExports.has(name)) fail('duplicate slot Wasm export');
        seenExports.add(name);
        if (name === 'memory') {
          if (kind !== 2 || index !== 0) fail('incompatible slot Wasm memory export');
        } else if (!Object.hasOwn(abi.exports, name) || kind !== 0
          || functionTypes[index] !== slotExports[name]) fail('incompatible slot Wasm function export');
        exports.push({ name, kind: kind === 0 ? 'function' : 'memory' });
      }
    } else if (id === 4 || id === 5) {
      fail('slot module must import its only table and memory');
    } else if (id === 9) {
      const n = count();
      if (n > 16384) fail('too many slot Wasm element segments');
      for (let i = 0; i < n; i++) {
        const flags = uint();
        if ((flags !== 1 && flags !== 3) || byte() !== 0) fail('active or unsupported slot Wasm element segment');
        const length = count();
        if (length > 16384) fail('oversized slot Wasm element segment');
        for (let j = 0; j < length; j++) uint();
      }
    } else if (id === 11) {
      const n = count();
      for (let i = 0; i < n; i++) {
        if (uint() !== 1) fail('active or unsupported slot Wasm data segment');
        const length = uint();
        if (length > end - at) fail('truncated slot Wasm data segment');
        at += length;
      }
    } else at = end;
    if (at !== end) fail('invalid slot Wasm section length');
  }
  if (memoryImports !== 1 || tableImports !== 1 || globalImports !== expectedGlobals.size
    || seenExports.size !== Object.keys(slotExports).length + 1) fail('incomplete slot Wasm arena ABI');
  return { imports, exports };
}
