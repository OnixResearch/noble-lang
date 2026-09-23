function rejectStartSection(bytes) {
  if (bytes.length < 8 || bytes.readUInt32LE(0) !== 0x6d736100 || bytes.readUInt32LE(4) !== 1) fail('invalid Wasm header');
  let at = 8;
  while (at < bytes.length) {
    const id = bytes[at++];
    let size = 0, multiplier = 1, count = 0, byte;
    do {
      if (at >= bytes.length || count++ >= 5) fail('invalid Wasm section length');
      byte = bytes[at++]; size += (byte & 127) * multiplier; multiplier *= 128;
    } while (byte & 128);
    if (id === 8) fail('source module must not have a start function');
    if (!Number.isSafeInteger(size) || at + size > bytes.length) fail('truncated Wasm section');
    at += size;
  }
}

async function protocol(config) {
  let engine;
  const emit = (outcome, value) => {
    const json = Buffer.from(JSON.stringify(value));
    if (json.length > MAX_FRAME) fail('engine output limit exceeded');
    process.stdout.write(`${outcome} ${json.length}\n`);
    process.stdout.write(json);
  };
  const reportError = error => ({ schema: 'noble-core-report/v1', profile: 'Core-Bootstrap',
    backend: 'managed-linear-memory', stage: 'wasm', outcome: 'internal-failure',
    diagnostic: `${error.name}: ${error.message}`,
    request_trace: engine ? engine.trace.slice(engine.lastTraceStart) : [],
    guest_requests: engine ? engine.trace.length - engine.lastTraceStart : 0,
    protected_operations: 0, candidate_prepare_requests: 0,
    session_state: 'terminated-after-internal-failure' });
  try {
    engine = new CoreEngine(JSON.parse(config.selection), JSON.parse(config.abi), config);
    emit('ready', { outcome: 'ready', tools: engine.tools });
    let pending = Buffer.alloc(0), command = null;
    for await (const chunk of process.stdin) {
      if (pending.length + chunk.length > MAX_FRAME + 65536 + 256) fail('engine input frame limit exceeded');
      pending = Buffer.concat([pending, chunk]);
      while (pending.length) {
        if (command === null) {
          const newline = pending.indexOf(10);
          if (newline < 0) {
            if (pending.length > 128) fail('engine header limit exceeded');
            break;
          }
          const header = utf8.decode(pending.subarray(0, newline));
          pending = pending.subarray(newline + 1);
          if (header === 'shutdown') {
            emit('closed', { outcome: 'closed' });
            return;
          }
          if (header === 'park' || header === 'restore') {
            const report = header === 'park' ? engine.park() : engine.restore();
            emit(report.outcome, report);
            continue;
          }
          if (header === 'execute') {
            command = { kind: 'execute', inputs: 0 };
          } else if (/^execute [0-9]+$/.test(header)) {
            const count = Number(header.slice('execute '.length));
            command = { kind: 'execute', inputs: integer(count, MAX_FRAME, 'injection frame') };
          } else if (/^observe [0-9]+$/.test(header)) {
            try { emit('observed', engine.observe(Number(header.slice('observe '.length)))); }
            catch (error) { emit('internal-failure', reportError(error)); return; }
            continue;
          } else if (/^push [0-9]+$/.test(header)) {
            const count = Number(header.slice('push '.length));
            command = { kind: 'push', inputs: integer(count, MAX_FRAME, 'injection frame') };
          } else if (/^project [0-9]+$/.test(header)) {
            try { emit('projected', engine.project(Number(header.slice('project '.length)))); }
            catch (error) { emit('internal-failure', reportError(error)); return; }
            continue;
          } else {
            const match = /^compile ([0-9]+) ([0-9]+) ([0-9]+)$/.exec(header);
            if (!match) fail('invalid engine command');
            command = { kind: 'compile', wat: integer(Number(match[1]), MAX_FRAME, 'WAT frame'),
              source: integer(Number(match[2]), 65536, 'source frame'),
              submission: integer(Number(match[3]), Number.MAX_SAFE_INTEGER, 'submission') };
          }
        }
        if (command.kind === 'execute') {
          if (pending.length < command.inputs) break;
          const bytes = pending.subarray(0, command.inputs);
          pending = pending.subarray(command.inputs);
          command = null;
          let inputs = [];
          if (bytes.length) {
            try { inputs = JSON.parse(utf8.decode(bytes)); }
            catch (error) { emit('internal-failure', reportError(error)); return; }
          }
          try {
            const report = engine.execute({ inputs });
            emit(report.outcome, report);
          } catch (error) { emit('internal-failure', reportError(error)); return; }
          if (engine.poisoned) return;
          continue;
        }
        if (command.kind === 'push') {
          if (pending.length < command.inputs) break;
          const bytes = pending.subarray(0, command.inputs);
          pending = pending.subarray(command.inputs);
          command = null;
          let inputs;
          try { inputs = JSON.parse(utf8.decode(bytes)); }
          catch (error) { emit('internal-failure', reportError(error)); return; }
          try { emit('pushed', engine.push(inputs)); }
          catch (error) { emit('internal-failure', reportError(error)); return; }
          continue;
        }
        if (pending.length < command.wat + command.source) break;
        const wat = pending.subarray(0, command.wat), source = pending.subarray(command.wat, command.wat + command.source);
        pending = pending.subarray(command.wat + command.source);
        try { emit('ready', engine.prepare(wat, source, command.submission)); }
        catch (error) { emit('internal-failure', reportError(error)); return; }
        command = null;
      }
    }
    if (pending.length || command) fail('truncated engine request');
  } catch (error) { emit('internal-failure', reportError(error)); }
  finally { if (engine) engine.close(); }
}

if (process.argv.includes('--protocol')) {
  const index = process.argv.indexOf('--protocol');
  await protocol(JSON.parse(process.argv[index + 1]));
}
