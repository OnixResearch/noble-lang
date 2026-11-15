// Keep host filesystem effects outside noble-ngraph's pure library test unit.
import { spawnSync } from "node:child_process";
import { writeFileSync } from "node:fs";

const root = new URL("../../", import.meta.url);
const path = new URL(
  "proofs/foundation/NobleFoundation/NetI64PlanCorpus.lean",
  root,
);
const cargo = process.env.PLAN_CORPUS_CARGO ?? "cargo";
const target = "net::tests::emission::regenerate_lean_plan_corpus";
const base = ["test", "-p", "noble-ngraph", "--lib", target, "--offline", "--locked", "--"];
const run = (args) =>
  spawnSync(cargo, args, {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 4 * 1024 * 1024,
  });

const producer = run([...base, "--ignored", "--exact", "--nocapture"]);
if (producer.error) throw producer.error;
if (producer.status !== 0) {
  throw new Error(`plan corpus producer failed:\n${producer.stderr}\n${producer.stdout}`);
}
const begin = "NOBLE_NET_PLAN_CORPUS_BEGIN\n";
const end = "\nNOBLE_NET_PLAN_CORPUS_END";
const transcript = producer.stdout + producer.stderr;
const first = transcript.indexOf(begin);
const last = transcript.indexOf(end, first + begin.length);
if (
  first < 0 ||
  last < 0 ||
  transcript.indexOf(begin, first + begin.length) >= 0 ||
  transcript.indexOf(end, last + end.length) >= 0
) {
  throw new Error("expected one complete Rust-emitted plan corpus payload");
}
const source = transcript.slice(first + begin.length, last);
if (!source.startsWith("import NobleFoundation.NetI64Plans\n") ||
    !source.endsWith("end NobleFoundation.NetI64PlanCorpus\n")) {
  throw new Error("Rust-emitted plan corpus payload is incomplete");
}
writeFileSync(path, source, { encoding: "utf8" });
const fresh = run([
  ...base.slice(0, 4),
  "net::tests::emission::generated_lean_plan_corpus_is_fresh",
  ...base.slice(5),
  "--exact",
]);
if (fresh.error) throw fresh.error;
if (fresh.status !== 0) {
  throw new Error(`regenerated corpus is not fresh:\n${fresh.stderr}\n${fresh.stdout}`);
}
console.log(`Regenerated and checked ${path.pathname}`);
