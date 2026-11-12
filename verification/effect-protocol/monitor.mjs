// The RegionProtocol-v1 trace monitor (behavioral-effect-protocol design, "Monitor").
//
// It replays the ordered event trace of one `noble-bounded-region/v2` report against the
// design's transition table and returns the violations it observes. A verdict is evidence about
// that one finite execution, never a protocol proof: a truncated trace is judged only up to its
// bound, and a call the runtime refused before it reached the host leaves no event to judge.

const RELEASES = new Set(['region.release', 'region.drop', 'host.retire']);
const READ_REFUSALS = new Set(['bounds-reject', 'invalid-handle']);

// Verdicts, in the design's vocabulary where it has one.
export const VERDICTS = Object.freeze([
  'omitted-release', // the guest returned normally and the host retired the owner (P1, F1)
  'double-release', // a release request after a release committed
  'post-release-use', // a read request after a release committed (P2)
  'unsolicited-response', // a response with no unanswered request (P4)
  'unanswered-request', // a request while another is unanswered (P4)
  'unsolicited-commit', // a protected commit outside a read request (P3)
  'uncommitted-read', // a read answered with bytes and no protected commit (P3)
  'refused-after-commit', // a read refused after its protected commit (P3)
  // an ok of another length than its request, a refusal outside the result schema, or an
  // outside-model exit (A3)
  'outside-model',
  'release-count', // a complete trace without exactly one release commit (P1)
  'truncated', // the trace hit its bound; only the prefix is judged
]);

// A JSON integer's decimal text, whether parsed as a number or kept exact as raw JSON.
const decimal = value => (JSON.isRawJSON(value) ? value.rawJSON : String(value));

export function monitor(report) {
  const verdicts = [];
  const flag = (verdict, index) => verdicts.push({ verdict, event: index });
  let pending = null;
  let committed = false;
  let released = null;
  let releaseCommits = 0;
  report.events.forEach((event, index) => {
    if (event.kind === 'request') {
      if (pending !== null) flag('unanswered-request', index);
      if (released !== null && event.operation === 'region.read') flag('post-release-use', index);
      if (released !== null && RELEASES.has(event.operation)) flag('double-release', index);
      pending = event;
      committed = false;
    } else if (event.kind === 'commit') {
      // P3: every protected commit is immediately preceded by its read
      // request, so one read request authorises exactly one commit.
      if (pending?.operation !== 'region.read' || committed) flag('unsolicited-commit', index);
      committed = true;
    } else if (event.kind === 'response') {
      if (pending === null) {
        flag('unsolicited-response', index);
      } else if (pending.operation === 'region.read') {
        if (event.result === 'value') {
          if (!committed) flag('uncommitted-read', index);
          if (decimal(event.bytes) !== decimal(pending.length)) flag('outside-model', index);
        } else {
          if (committed) flag('refused-after-commit', index);
          if (!READ_REFUSALS.has(event.reason)) flag('outside-model', index);
        }
      } else if (RELEASES.has(pending.operation) && event.result === 'value') {
        released = pending.operation;
        releaseCommits += 1;
      }
      pending = null;
      committed = false;
    } else {
      throw Error(`unknown event kind ${event.kind}`);
    }
  });
  const end = report.events.length;
  if (report.events_truncated) flag('truncated', end);
  if (released === 'host.retire' && ['normal', 'rejected', 'outside-model'].includes(report.exit_class)) {
    flag('omitted-release', end);
  }
  if (report.exit_class === 'outside-model') flag('outside-model', end);
  if (!report.events_truncated && releaseCommits !== 1) flag('release-count', end);
  return {
    verdicts,
    codes: [...new Set(verdicts.map(row => row.verdict))].sort(),
    judged_events: end,
    release_commits: releaseCommits,
    released_by: released,
  };
}
