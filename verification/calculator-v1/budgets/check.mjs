// Plan expectation check shared by run.mjs and summarize.mjs.
export const expectations = (session, result) => {
  // Each plan line names its expected stdout value, stderr line prefix, or silence.
  const outLines = result.stdout.split('\n').filter(Boolean);
  const errLines = result.stderr.split('\n').filter(Boolean);
  const wantOut = session.expect.filter(item => 'stdout' in item).map(item => item.stdout);
  const wantErr = session.expect.filter(item => 'stderr' in item).map(item => item.stderr);
  const outOk = outLines.length === wantOut.length && outLines.every((line, index) => line === wantOut[index]);
  const errOk = errLines.length === wantErr.length && errLines.every((line, index) => line.startsWith(wantErr[index]));
  const statusOk = session.expect_status === undefined || session.expect_status === result.status;
  const signalOk = session.expect_signal === undefined || session.expect_signal === result.signal;
  const timeoutOk = (session.expect_timeout ?? false) === result.timed_out;
  return { matched: outOk && errOk && statusOk && signalOk && timeoutOk, stdout_lines: outLines, stderr_lines: errLines,
    expected_stdout: wantOut, expected_stderr_prefixes: wantErr,
    resource_limit_lines: errLines.filter(line => line.startsWith('ResourceLimit ')) };
};
