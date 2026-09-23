// Bounded inert observations of the engine-owned memory; no guest execution.
function readText(engine, offset, count) {
    integer(offset, engine.memory.buffer.byteLength, 'text offset');
    integer(count, engine.memory.buffer.byteLength - offset, 'text length');
    return utf8.decode(new Uint8Array(engine.memory.buffer, offset, count));
  }

function readSignature(engine, runtime, identity) {
    integer(identity, 16384, 'signature identity');
    return readText(engine, runtime.signature_address(identity), runtime.signature_length(identity));
  }

function readStack(engine, runtime) {
    const count = integer(runtime.stack_length(), 128, 'stack height');
    engine.observationWork = 0;
    // Every slot also reports its live cell handle (null for immediate
    // values), so a consumer can project a Program or pass a companion cell to
    // a later push without re-deriving the handle from the value's shape.
    return Array.from({ length: count }, (_, at) => {
      const kind = runtime.stack_kind(at);
      const raw = runtime.stack_value(at);
      return {
        handle: kind < 4 ? null : Number(BigInt.asUintN(64, raw)),
        ...readValue(engine, runtime, kind, raw, 0),
      };
    });
  }

function chargeObservation(engine, depth) {
    if (depth > 512 || ++engine.observationWork > MAX_OBSERVATION_WORK) fail('observation limit exhausted');
  }

function readBoxed(engine, runtime, handle, depth) {
    integer(handle, engine.abi.limits_maximum.session_cells, 'cell handle');
    if (handle === 0 || handle > engine.shared.heap_cursor.value) fail('invalid live cell handle');
    const kind = runtime.cell_kind(handle);
    // A boxed value also reports its own live handle, so a consumer can pass the
    // same cell to a later push without re-deriving the handle from its shape.
    return { handle, ...readValue(engine, runtime, kind, kind < 4 ? runtime.cell_payload(handle) : BigInt(handle), depth) };
  }

function readValue(engine, runtime, kind, value, depth) {
    chargeObservation(engine, depth);
    if (kind === 1) return { type: 'I64', value: String(value) };
    if (kind === 2) {
      if (value !== 0n && value !== 1n) fail('invalid Bool value');
      return { type: 'Bool', value: value === 1n };
    }
    if (kind === 3) return { type: 'Unit', value: null };
    const handle = integer(Number(value), engine.shared.heap_cursor.value, 'value handle');
    if (!handle || runtime.cell_kind(handle) !== kind) fail('value tag disagrees with cell');
    if (kind === 4) return { type: 'Program', interface: {
      stack_in: readSignature(engine, runtime, runtime.cell_y(handle)),
      stack_out: readSignature(engine, runtime, runtime.cell_z(handle)),
      effects: [0, 1].filter(bit => Number(runtime.cell_payload(handle)) & (1 << bit))
        .map(bit => bit === 0 ? 'test.emit' : 'test.abort'),
    }, ...readRecipe(engine, runtime, runtime.cell_c(handle), depth + 1) };
    if (kind === 5) return { type: 'Pair', value: [readBoxed(engine, runtime, runtime.cell_a(handle), depth + 1), readBoxed(engine, runtime, runtime.cell_b(handle), depth + 1)] };
    if (kind === 6 || kind === 7) {
      const items = [];
      let tail = handle;
      while (runtime.cell_kind(tail) === 6) {
        chargeObservation(engine, depth);
        items.push(readBoxed(engine, runtime, runtime.cell_a(tail), depth + 1));
        const next = runtime.cell_b(tail);
        if (next <= 0 || next >= tail) fail('cyclic/non-backward list');
        tail = next;
      }
      if (runtime.cell_kind(tail) !== 7) fail('invalid list tail');
      return { type: 'List', value: items };
    }
    if (kind === 10) return { type: 'Syntax', ...readRecipe(engine, runtime, runtime.cell_a(handle), depth + 1) };
    if (kind === 11) return { type: 'Text', value: readText(engine, runtime.cell_x(handle), runtime.cell_y(handle)) };
    if (kind === 12 || kind === 13) return { type: 'Sum', variant: kind === 12 ? 'left' : 'right', value: readBoxed(engine, runtime, runtime.cell_a(handle), depth + 1) };
    if (kind === 14) return { type: 'Contract', statement: String(BigInt.asUintN(64, runtime.cell_payload(handle))),
      index: runtime.cell_x(handle), claim_kind: runtime.cell_y(handle), revision: runtime.cell_z(handle) };
    if (kind === 15) return { type: 'Evidence', index: Number(runtime.cell_payload(handle)),
      class: runtime.cell_x(handle), ruleset: runtime.cell_y(handle),
      contract: readBoxed(engine, runtime, runtime.cell_a(handle), depth + 1) };
    if (kind === 16) return { type: 'Certified',
      identity: String(BigInt.asUintN(64, runtime.cell_payload(handle))),
      subject: readBoxed(engine, runtime, runtime.cell_a(handle), depth + 1),
      contract: readBoxed(engine, runtime, runtime.cell_b(handle), depth + 1),
      evidence: readBoxed(engine, runtime, runtime.cell_c(handle), depth + 1) };
    fail(`unsupported observable value kind: ${kind}`);
  }

  // Read the bounded inspection events one `observe` call produced. These are
  // inert descriptions: reading them never fetches a proof file or starts a
  // prover.
function readReflection(engine, runtime) {
    const count = integer(runtime.recipe_length(), 2048, 'reflection event count');
    const events = [];
    for (let index = 0; index < count; index += 1) {
      events.push({ kind: runtime.recipe_kind(index), value: String(BigInt.asUintN(64, runtime.recipe_value(index))) });
    }
    return events;
  }

function readRecipe(engine, runtime, root, depth) {
    chargeObservation(engine, depth);
    const result = [];
    const witnesses = [];
    const pending = root ? [root] : [];
    while (pending.length) {
      chargeObservation(engine, depth);
      const handle = pending.pop();
      integer(handle, engine.shared.heap_cursor.value, 'recipe handle');
      const kind = runtime.cell_kind(handle);
      if (kind === 9) {
        const left = runtime.cell_a(handle), right = runtime.cell_b(handle);
        if (left <= 0 || right <= 0 || left >= handle || right >= handle) fail('non-backward recipe graph');
        pending.push(right, left);
        continue;
      }
      if (kind !== 8) fail('invalid recipe node');
      const atom = runtime.cell_x(handle), value = runtime.cell_payload(handle), child = runtime.cell_a(handle);
      if (atom === 1) result.push({ literal: { type: 'I64', value: String(value) } });
      else if (atom === 4) result.push({ literal: { type: 'Bool', value: value !== 0n } });
      else if (atom === 5) result.push({ literal: { type: 'Unit', value: null } });
      else if (atom === 2) {
        const id = integer(Number(value), 23, 'builtin identity');
        result.push({ invoke: `${id < 22 ? 'builtin' : 'host'}:${names[id]}` });
      } else if (atom === 15) result.push({ invoke: `definition:${BigInt.asUintN(64, value)}` });
      else if (atom === 3) {
        const program = readBoxed(engine, runtime, child, depth + 1);
        if (program.type !== 'Program') fail('quotation recipe has no program');
        result.push({ quotation: program.recipe, interface: program.interface, witnesses: program.witnesses });
      } else if (atom === 8) result.push({ literal: {
        ...readBoxed(engine, runtime, child, depth + 1),
        schema: readSignature(engine, runtime, runtime.cell_y(handle)),
      } });
      else fail(`unsupported recipe atom: ${atom}`);
      if (atom === 2 || atom === 15) {
        witnesses.push({
          node: result.length - 1,
          stack_in: readSignature(engine, runtime, runtime.cell_y(handle)),
          stack_out: readSignature(engine, runtime, runtime.cell_z(handle)),
          effects: [0, 1].filter(bit => runtime.cell_w(handle) & (1 << bit))
            .map(bit => bit === 0 ? 'test.emit' : 'test.abort'),
        });
      }
    }
    return { recipe: result, witnesses };
  }

