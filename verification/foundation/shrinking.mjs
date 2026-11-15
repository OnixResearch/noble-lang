/** Deterministic candidates shared by the foundation and interaction lanes. */
export function* shrinkCandidates(c) {
  for (let i = 0; i < c.prefix.length; i++) {
    yield { ...c, prefix: c.prefix.filter((_, j) => j !== i) };
  }
  const listShrinks = function* (key) {
    const list = c[key];
    if (!list) return;
    for (let i = 0; i < list.length; i++) {
      yield { ...c, [key]: list.filter((_, j) => j !== i) };
    }
    for (let i = 0; i < list.length; i++) {
      if (typeof list[i] !== 'object') continue;
      const value = list[i].push;
      for (const simpler of [0n, 1n, -1n, value / 2n]) {
        if (simpler !== value) {
          yield { ...c, [key]: list.map((op, j) => j === i ? { push: simpler } : op) };
        }
      }
    }
  };
  yield* listShrinks('ops');
  yield* listShrinks('ins');
  if (c.program?.compose) {
    yield { ...c, program: c.program.compose[0] };
    yield { ...c, program: c.program.compose[1] };
  }
  if (c.program?.arith) {
    const body = c.program.arith;
    for (let i = 0; i < body.length; i++) {
      yield { ...c, program: { arith: body.filter((_, j) => j !== i) } };
    }
  }
}
