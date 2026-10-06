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
  const reportError = error => config.slot_mode === true
    ? { ...slotReply('internal-failure'), diagnostic: `${error.name}: ${error.message}`,
      request_trace: engine ? engine.trace.slice(engine.lastTraceStart) : [],
      session_state: 'terminated-after-internal-failure' }
    : ({ schema: 'noble-core-report/v1', profile: 'Core-Bootstrap',
    backend: 'managed-linear-memory', stage: 'wasm', outcome: 'internal-failure',
    diagnostic: `${error.name}: ${error.message}`,
    request_trace: engine ? engine.trace.slice(engine.lastTraceStart) : [],
    guest_requests: engine ? engine.trace.length - engine.lastTraceStart : 0,
    protected_operations: 0, candidate_prepare_requests: 0,
    session_state: engine?.proposalInspectionFailed
      ? 'terminated-after-proposal-inspection-failure' : 'terminated-after-internal-failure' });
  try {
    engine = config.slot_mode === true
      ? new SlotEngine(JSON.parse(config.selection), JSON.parse(config.abi), config)
      : new CoreEngine(JSON.parse(config.selection), JSON.parse(config.abi), config);
    emit('ready', { outcome: 'ready', tools: engine.tools });
    let pending = Buffer.alloc(0), command = null;
    for await (const chunk of process.stdin) {
      if (pending.length + chunk.length > 2 * MAX_FRAME + 65536 + 4096) fail('engine input frame limit exceeded');
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
          if (config.slot_mode === true) {
            const slot = /^slot ([0-9]+) ([0-9]+)$/.exec(header);
            if (!slot) fail('only bounded slot commands are accepted by slot worker');
            command = { kind: 'slot',
              json: integer(Number(slot[1]), MAX_FRAME, 'slot request'),
              binary: integer(Number(slot[2]), MAX_FRAME, 'checked slot binary') };
            if (command.json === 0) fail('empty slot command');
            continue;
          }
          if (engine.staged && header !== 'publish-bin' && header !== 'discard-bin') {
            fail('staged live reload requires publish or discard');
          }
          if (header === 'publish-bin' || header === 'discard-bin') {
            try {
              const report = header === 'publish-bin' ? engine.publishBinary() : engine.discardBinary();
              emit(report.outcome, report);
            } catch (error) { emit('internal-failure', reportError(error)); return; }
            continue;
          }
          if (header === 'take-live-proposal') {
            try {
              const report = engine.takeLiveProposal();
              emit(report.outcome, report);
            } catch (error) { emit('internal-failure', reportError(error)); return; }
            continue;
          }
          const grant = /^live-grant ([0-9]+) ([0-9]+) ([0-9]+)$/.exec(header);
          if (grant) {
            const decimal = value => {
              if (!/^(0|[1-9][0-9]{0,19})$/.test(value)
                || BigInt(value) > 0xffffffffffffffffn) fail('invalid live grant integer');
              return BigInt(value);
            };
            command = { kind: 'live-grant', bytes: integer(Number(grant[1]), 256, 'grant name'),
              owner: decimal(grant[2]), sourceGeneration: decimal(grant[3]) };
          } else if (header === 'park' || header === 'restore') {
            const report = header === 'park' ? engine.park() : engine.restore();
            emit(report.outcome, report);
            continue;
          } else if (header === 'execute') {
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
            const admission = /^admit ([0-9]+) ([0-9]+) ([0-9]+) ([0-9]+) ([0-9]+)$/.exec(header);
            if (admission) {
              command = { kind: 'admit',
                artifact: integer(Number(admission[1]), MAX_FRAME, 'artifact frame'),
                wat: integer(Number(admission[2]), MAX_FRAME, 'WAT frame'),
                source: integer(Number(admission[3]), 65536, 'source frame'),
                claims: integer(Number(admission[4]), 1024, 'claim frame'),
                allowed: integer(Number(admission[5]), 1024, 'host-policy frame') };
            } else {
              const match = /^(compile|compile-bin|stage-bin) ([0-9]+) ([0-9]+) ([0-9]+)$/.exec(header);
              if (!match) fail('invalid engine command');
              command = { kind: match[1],
                bytes: integer(Number(match[2]), MAX_FRAME, 'module frame'),
                source: integer(Number(match[3]), 65536, 'source frame'),
                submission: integer(Number(match[4]), Number.MAX_SAFE_INTEGER, 'submission') };
              if (command.kind !== 'compile' && command.bytes < 8) fail('invalid Wasm frame size');
            }
          }
        }
        if (command.kind === 'slot') {
          if (pending.length < command.json + command.binary) break;
          const json = pending.subarray(0, command.json);
          const binary = pending.subarray(command.json, command.json + command.binary);
          pending = pending.subarray(command.json + command.binary);
          command = null;
          try {
            const report = engine.request(JSON.parse(utf8.decode(json)), binary);
            emit(report.outcome, { ...report, retire_code_spans: engine.retireCodeSpans() });
            if (engine.poisoned) return;
          } catch (error) { emit('internal-failure', reportError(error)); return; }
          continue;
        }
        if (command.kind === 'live-grant') {
          if (pending.length < command.bytes) break;
          const name = utf8.decode(pending.subarray(0, command.bytes));
          pending = pending.subarray(command.bytes);
          const report = engine.setLiveGrant(name, command.owner, command.sourceGeneration);
          command = null;
          emit(report.outcome, report);
          continue;
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
        if (command.kind === 'admit') {
          const total = command.artifact + command.wat + command.source + command.claims + command.allowed;
          if (pending.length < total) break;
          const artifact = pending.subarray(0, command.artifact);
          const wat = pending.subarray(command.artifact, command.artifact + command.wat);
          const source = pending.subarray(command.artifact + command.wat,
            command.artifact + command.wat + command.source);
          const claims = pending.subarray(command.artifact + command.wat + command.source,
            command.artifact + command.wat + command.source + command.claims);
          const allowed = pending.subarray(command.artifact + command.wat + command.source + command.claims, total);
          pending = pending.subarray(total);
          command = null;
          try {
            const report = engine.admit(artifact, wat, source,
              JSON.parse(utf8.decode(claims)), JSON.parse(utf8.decode(allowed)), source.length > 0);
            emit(report.outcome, report);
          } catch (error) { emit('internal-failure', reportError(error)); return; }
          continue;
        }
        if (pending.length < command.bytes + command.source) break;
        const bytes = pending.subarray(0, command.bytes);
        const source = pending.subarray(command.bytes, command.bytes + command.source);
        pending = pending.subarray(command.bytes + command.source);
        try {
          const report = command.kind === 'compile' ? engine.prepare(bytes, source, command.submission)
            : command.kind === 'compile-bin' ? engine.compileBinary(bytes, source, command.submission)
              : engine.stageBinary(bytes, source, command.submission);
          emit(report.outcome, report);
        }
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
