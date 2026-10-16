// Included only by the explicit safety-observe evidence build.
import safetyFs from 'node:fs';
const safetyPath = process.env.NOBLE_SAFETY_OBSERVATION;
delete process.env.NOBLE_SAFETY_OBSERVATION;
if (safetyPath) {
  const safetyFd = safetyFs.openSync(safetyPath, 'a');
  let sequence = 0, guest = 0, protectedCount = 0, closed = false;
  globalThis.nobleSafetyObserve = event => {
    if (closed) throw Error('semantic operation after safety observation closed');
    if (event === 'guest-request') guest += 1;
    if (event === 'protected-operation') protectedCount += 1;
    const row = { event, pid: process.pid, sequence: sequence++ };
    if (event === 'child-end') Object.assign(row,
      { guest_requests: guest, protected_operations: protectedCount });
    const bytes = Buffer.from(JSON.stringify(row) + '\n');
    if (safetyFs.writeSync(safetyFd, bytes) !== bytes.length) throw Error('short safety observation write');
  };
  globalThis.nobleSafetyObserve('child-begin');
  globalThis.nobleSafetyFinish = () => {
    if (closed) return;
    globalThis.nobleSafetyObserve('child-end');
    closed = true;
    safetyFs.closeSync(safetyFd);
  };
  process.on('exit', globalThis.nobleSafetyFinish);
}
