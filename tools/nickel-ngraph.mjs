#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), ".."));
const TOOLS = JSON.parse(readFileSync(path.join(ROOT, "policy/tool-selection.json"), "utf8")).tool_paths;
const NICKEL = path.join(TOOLS.nickel.output, "bin/nickel");
const PINNED_NODE = path.join(TOOLS.node.output, "bin/node");
const SOURCE = "nickel/ngraph/fixtures/arithmetic.ncl";
const COMPOSITION_SOURCE = "nickel/ngraph/fixtures/composition.ncl";
const DELAYED_SOURCE = "nickel/ngraph/fixtures/delayed-contract-failure.ncl";
const LIBRARY = "nickel/ngraph/library.ncl";
const CONTRACT = "nickel/ngraph/contract.ncl";
const LOWERER_SOURCE = "crates/noble-cli/src/ngraph/nickel.rs";
const LOWERER_REVISION = "noble-ngraph-nickel-lowerer/1";
const DECLARED_CLOSURE = [SOURCE, LIBRARY, CONTRACT];
const COMPOSITION_CLOSURE = [COMPOSITION_SOURCE, LIBRARY, CONTRACT];
const I64_MIN = -(1n << 63n);
const I64_MAX = (1n << 63n) - 1n;
const I64_WIDE = 9007199254740993n;

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function stable(value) {
  if (Array.isArray(value)) return `[${value.map(stable).join(",")}]`;
  if (value !== null && typeof value === "object") {
    const members = Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${stable(value[key])}`);
    return `{${members.join(",")}}`;
  }
  return JSON.stringify(value);
}

function run(command, args) {
  try {
    return { status: 0, stdout: execFileSync(command, args, {
      cwd: ROOT,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      maxBuffer: 32 * 1024 * 1024,
    }), stderr: "" };
  } catch (error) {
    return {
      status: Number.isInteger(error.status) ? error.status : 1,
      stdout: error.stdout?.toString() ?? "",
      stderr: error.stderr?.toString() ?? error.message,
    };
  }
}

function importsFrom(source, label) {
  const tokens = [...source.matchAll(/\bimport\b/g)];
  const imports = [...source.matchAll(/\bimport\s+"([^"]+)"/g)];
  if (tokens.length !== imports.length) {
    throw new Error(`${label}: only literal, relative imports are admitted`);
  }
  return imports.map((match) => match[1]);
}

function observeClosure(entry, declared) {
  const declaredSet = new Set(declared);
  const visited = new Set();
  const files = [];
  function visit(relative) {
    const absolute = path.resolve(ROOT, relative);
    if (!absolute.startsWith(`${ROOT}${path.sep}`)) {
      throw new Error(`import escapes the repository root: ${relative}`);
    }
    const canonical = realpathSync(absolute);
    const resolved = path.relative(ROOT, canonical).split(path.sep).join("/");
    if (!declaredSet.has(resolved)) {
      throw new Error(`undeclared import in constructor closure: ${resolved}`);
    }
    if (visited.has(resolved)) return;
    visited.add(resolved);
    const bytes = readFileSync(canonical);
    files.push({ path: resolved, sha256: sha256(bytes), bytes: bytes.length });
    for (const specifier of importsFrom(bytes.toString("utf8"), resolved)) {
      if (path.isAbsolute(specifier)) throw new Error(`absolute Nickel import refused: ${specifier}`);
      const target = path.relative(ROOT, path.resolve(path.dirname(canonical), specifier));
      visit(target.split(path.sep).join("/"));
    }
  }
  visit(entry);
  if (files.length !== declared.length || files.some((file, index) => file.path !== declared[index])) {
    throw new Error(`import closure differs from declared ordered list: ${files.map((file) => file.path).join(", ")}`);
  }
  return files;
}

function evaluatorIdentity() {
  const node = realpathSync(process.execPath);
  if (node !== realpathSync(PINNED_NODE)) throw new Error(`Node evaluator is not pinned: ${node}`);
  const nickelVersion = run(NICKEL, ["--version"]);
  if (nickelVersion.status !== 0 || !nickelVersion.stdout.includes("1.17.0")) {
    throw new Error(`pinned Nickel 1.17.0 identity check failed: ${nickelVersion.stderr || nickelVersion.stdout}`);
  }
  return {
    node: { executable: node, version: process.version, sha256: sha256(readFileSync(node)) },
    nickel: {
      executable: realpathSync(NICKEL),
      version: nickelVersion.stdout.trim(),
      sha256: sha256(readFileSync(NICKEL)),
    },
  };
}

function exportNickel(source) {
  const result = run(NICKEL, ["export", "--format", "json", path.join(ROOT, source)]);
  if (result.status !== 0) throw new Error(`Nickel export failed: ${result.stderr}`);
  return Buffer.from(result.stdout, "utf8");
}

function closureRefusal() {
  try {
    observeClosure(SOURCE, [SOURCE, CONTRACT]);
  } catch (error) {
    if (!String(error.message).includes("undeclared import in constructor closure")) throw error;
    return { refused: true, code: "E-IMPORT-CLOSURE", declared: [SOURCE, CONTRACT] };
  }
  throw new Error("import-closure control unexpectedly accepted an undeclared import");
}

function delayedContractRefusal(graphPath) {
  const result = run(NICKEL, ["export", "--format", "json", path.join(ROOT, DELAYED_SOURCE)]);
  if (result.status === 0 || !result.stderr.includes("contract broken by a value")) {
    throw new Error(`delayed-contract control did not fail at its demanded contract: ${result.stderr}`);
  }
  if (result.stdout.length !== 0 || existsSync(graphPath)) {
    throw new Error("delayed-contract control produced export bytes or an admitted graph");
  }
  return {
    fixture: DELAYED_SOURCE,
    evaluator_exit: result.status,
    contract_failure: "contract broken by a value",
    exported_bytes: 0,
    lowerer_invoked: false,
    admitted_graph_created: false,
  };
}

function jsonLines(text, label) {
  return text.split(/\r?\n/).filter(Boolean).map((line) => {
    try {
      return JSON.parse(line);
    } catch (error) {
      throw new Error(`${label}: invalid JSON report line: ${error.message}`);
    }
  });
}

function nobleReports(noble, args, label) {
  const result = run(noble, args);
  if (result.status !== 0) throw new Error(`${label} failed (${result.status}): ${result.stderr}\n${result.stdout}`);
  const reports = jsonLines(result.stdout, label);
  if (reports.length === 0 || reports.some((report) => report.outcome !== "normal")) {
    throw new Error(`${label} returned a refusal: ${result.stdout}`);
  }
  return reports;
}

function wrap64(value) {
  return BigInt.asIntN(64, value);
}

function expectedFor(input) {
  const x = BigInt(input);
  const first = wrap64(x + I64_MAX);
  const firstScaled = wrap64(first * I64_WIDE);
  const firstDifference = wrap64(first - x);
  const second = wrap64(first + I64_MIN);
  const secondScaled = wrap64(second * -2n);
  const secondDifference = wrap64(second - first);
  return [firstScaled, firstDifference, secondScaled, secondDifference, first, second, firstScaled]
    .map((value) => value.toString());
}

function evaluate(noble, graph, vectors, reference) {
  const args = ["ngraph", "eval", graph, vectors];
  if (reference) args.push("--reference");
  const reports = nobleReports(noble, args, `${reference ? "reference" : "packed"} evaluator`);
  return reports.filter((report) => Array.isArray(report.outputs)).map((report) =>
    report.outputs.map((value) => {
      if (typeof value !== "string" || !value.startsWith("i64:")) {
        throw new Error(`unexpected I64 evaluator output: ${JSON.stringify(value)}`);
      }
      return value.slice("i64:".length);
    }),
  );
}

function admitConstructor(noble, bundle, options, out, label) {
  const versions = {
    schema: bundle.schema,
    library: bundle.library_version,
    contract: bundle.contract_version,
  };
  const bundleSha = sha256(Buffer.from(stable({
    source_sha256: options.sourceSha,
    import_closure: options.importClosure,
    parameters: bundle.parameters,
    versions,
    evaluator: options.evaluator,
    declared_import_closure: options.declaredClosure,
    exported_sha256: options.exportSha,
    case: label,
    bundle,
  })));
  const bound = { ...bundle, bundle_sha256: bundleSha };
  const boundBytes = Buffer.from(`${JSON.stringify(bound, null, 2)}\n`);
  const boundPath = path.join(out, `${label}.constructor.json`);
  const graphPath = path.join(out, `${label}.ngraph`);
  writeFileSync(boundPath, boundBytes);
  const checked = nobleReports(noble, [
    "ngraph", "check", boundPath,
    "--graph-out", graphPath,
    "--budget-work", "4096",
    "--budget-expansion", "1024",
    "--budget-inputs", "32",
    "--budget-outputs", "64",
    "--budget-layers", "128",
    "--budget-live-nodes", "1024",
    "--budget-graph-bytes", "1048576",
  ], `native admission for ${label}`)[0];
  if (checked.profile !== "ngraph-i64/0" || !checked.subject.includes("authored-nickel")) {
    throw new Error(`${label}: native checker admitted an unexpected graph profile or producer identity`);
  }
  if (checked.lowerer_revision !== LOWERER_REVISION) {
    throw new Error(`${label}: native checker reported an unexpected lowerer revision`);
  }
  const graphBytes = readFileSync(graphPath);
  return {
    bound,
    boundBytes,
    boundPath,
    graphPath,
    graphBytes,
    checked,
    graphSha: sha256(graphBytes),
  };
}

function vectorFile(rows) {
  const arity = rows[0].length;
  return [
    "noble-ngraph-vectors/0",
    `types ${Array.from({ length: arity }, () => "i64").join(" ")}`,
    ...rows.map((row) => `v ${row.join(" ")}`),
    "",
  ].join("\n");
}

function compositionExpected(name, row) {
  const x = BigInt(row[0]);
  if (name === "identity") return [x.toString()];
  if (name === "scalar_copy") return [x.toString(), x.toString()];
  if (name === "product") {
    const y = BigInt(row[1]);
    return [wrap64(x + 5n).toString(), wrap64(y - 2n).toString()];
  }
  if (name === "permutation") return [row[1], row[0]];
  if (name === "reassociation_left" || name === "reassociation_right") {
    return [wrap64(wrap64(wrap64(x + 5n) * 2n) - 3n).toString()];
  }
  throw new Error(`unknown composition case ${name}`);
}

function expectedGraphSkeleton(name) {
  const input = ["ngraph-ir/0 ngraph-i64/0", "slot input i64", "%0 = input 0"];
  if (name === "identity") return [...input, "out %0"];
  if (name === "scalar_copy") return [...input, "out %0", "out %0"];
  if (name === "product") return [
    "ngraph-ir/0 ngraph-i64/0",
    "slot input i64",
    "slot input i64",
    "%0 = input 0",
    "%1 = input 1",
    "%2 = const i64 5",
    "%3 = i64.add %0 %2",
    "%4 = const i64 2",
    "%5 = i64.sub %1 %4",
    "out %3",
    "out %5",
  ];
  if (name === "permutation") return [
    "ngraph-ir/0 ngraph-i64/0",
    "slot input i64",
    "slot input i64",
    "%0 = input 0",
    "%1 = input 1",
    "out %1",
    "out %0",
  ];
  if (name === "reassociation_left" || name === "reassociation_right") return [
    ...input,
    "%1 = const i64 5",
    "%2 = i64.add %0 %1",
    "%3 = const i64 2",
    "%4 = i64.mul %2 %3",
    "%5 = const i64 3",
    "%6 = i64.sub %4 %5",
    "out %6",
  ];
  throw new Error(`unknown composition graph ${name}`);
}

function runCompositionLaws(noble, evaluator, out) {
  const closure = observeClosure(COMPOSITION_SOURCE, COMPOSITION_CLOSURE);
  const source = readFileSync(path.join(ROOT, COMPOSITION_SOURCE));
  const exportedBytes = exportNickel(COMPOSITION_SOURCE);
  const exported = JSON.parse(exportedBytes.toString("utf8"));
  const names = ["identity", "scalar_copy", "product", "permutation", "reassociation_left", "reassociation_right"];
  if (stable(Object.keys(exported).sort()) !== stable([...names].sort())) {
    throw new Error("composition fixture export does not have the exact expected law matrix");
  }
  writeFileSync(path.join(out, "composition-exported-constructor.json"), exportedBytes);
  const scalarRows = ["0", "1", "-1", I64_MIN.toString(), I64_MAX.toString(), I64_WIDE.toString(), "-9007199254740995"]
    .map((value) => [value]);
  const pairRows = [
    ["0", "1"], ["1", "-1"], ["-1", I64_MIN.toString()],
    [I64_MIN.toString(), I64_MAX.toString()], [I64_MAX.toString(), I64_WIDE.toString()],
    [I64_WIDE.toString(), "-9007199254740995"], ["-9007199254740995", "0"],
  ];
  const cases = {};
  const admitted = {};
  for (const name of names) {
    const rows = name === "product" || name === "permutation" ? pairRows : scalarRows;
    const constructor = admitConstructor(noble, exported[name], {
      sourceSha: sha256(source),
      importClosure: closure,
      evaluator,
      declaredClosure: COMPOSITION_CLOSURE,
      exportSha: sha256(exportedBytes),
    }, out, `composition-${name}`);
    admitted[name] = constructor;
    const graphLines = constructor.graphBytes.toString("utf8").trimEnd().split(/\r?\n/);
    if (!graphLines[1]?.startsWith("subject authored-nickel ")) {
      throw new Error(`${name}: admitted graph lacks the distinct Nickel subject`);
    }
    const observedGraph = [graphLines[0], ...graphLines.slice(2)];
    const expectedGraph = expectedGraphSkeleton(name);
    if (stable(observedGraph) !== stable(expectedGraph)) {
      throw new Error(`${name}: admitted graph differs from independent expected graph skeleton`);
    }
    const vectorsPath = path.join(out, `composition-${name}.vectors`);
    writeFileSync(vectorsPath, vectorFile(rows));
    const expected = rows.map((row) => compositionExpected(name, row));
    const reference = evaluate(noble, constructor.graphPath, vectorsPath, true);
    const packed = evaluate(noble, constructor.graphPath, vectorsPath, false);
    if (stable(reference) !== stable(expected)) throw new Error(`${name}: reference result differs from independent model`);
    if (stable(packed) !== stable(expected)) throw new Error(`${name}: packed result differs from independent model`);
    cases[name] = {
      graph_identity: constructor.checked.graph,
      graph_sha256: constructor.graphSha,
      expected_graph_skeleton: expectedGraph,
      observed_graph_skeleton: observedGraph,
      graph_structure_matches: true,
      construction_parameters: constructor.bound.parameters,
      input_rows: rows,
      expected,
      reference,
      packed,
    };
  }
  const reassociationMatches = stable(cases.reassociation_left.reference) === stable(cases.reassociation_right.reference)
    && stable(cases.reassociation_left.packed) === stable(cases.reassociation_right.packed);
  if (!reassociationMatches) throw new Error("reassociation pair differs under the ordered-output observer");
  const result = {
    schema: "noble-ngraph-nickel-composition-result/v1",
    source: { path: COMPOSITION_SOURCE, sha256: sha256(source), bytes: source.length },
    import_closure: { declared: COMPOSITION_CLOSURE, observed: closure },
    exported: { sha256: sha256(exportedBytes), bytes: exportedBytes.length },
    observer: "ordered outputs under exact wrapping-I64 evaluation",
    law_results: cases,
    reassociation_equal: reassociationMatches,
    theorem_status: "No theorem binds these Nickel fixture bytes, lowerer, or evaluators to the observed laws.",
    gaps: [
      "No source-to-implementation correspondence theorem is established.",
      "No effects, resource permissions, or arbitrary output reordering are modeled or claimed.",
    ],
  };
  writeFileSync(path.join(out, "composition-result.json"), `${JSON.stringify(result, null, 2)}\n`);
  return { result, exportedBytes, exported, source, closure, admitted };
}

function fileRecord(relative) {
  const bytes = readFileSync(path.join(ROOT, relative));
  return { path: relative, sha256: sha256(bytes), bytes: bytes.length };
}

function receiptDigest(payload) {
  return sha256(Buffer.from(stable(payload)));
}

function consumerBindings(payload) {
  return {
    source_commit: payload.source_commit,
    source_bytes_sha256: sha256(Buffer.from(stable(payload.source_bytes))),
    import_bytes_sha256: sha256(Buffer.from(stable(payload.import_bytes))),
    import_closures_sha256: sha256(Buffer.from(stable(payload.import_closures))),
    construction_parameters_sha256: sha256(Buffer.from(stable(payload.construction_parameters))),
    versions_sha256: sha256(Buffer.from(stable(payload.versions))),
    evaluator_identity_sha256: sha256(Buffer.from(stable(payload.evaluator_identity))),
    exported_constructor_bytes_sha256: sha256(Buffer.from(stable(payload.exported_constructor_bytes))),
    lowerer_revision: payload.lowerer.revision,
    lowerer_source_sha256: payload.lowerer.sha256,
    profile: payload.profile,
    graphs: payload.graphs.map((graph) => ({
      label: graph.label,
      sha256: graph.sha256,
      checked_identity: graph.checked_identity,
      profile: graph.profile,
    })),
  };
}

function consumerDigest(record) {
  return sha256(Buffer.from(stable({
    schema: record.schema,
    receipt_sha256: record.receipt_sha256,
    bindings: record.bindings,
  })));
}

function verifyReceipt(receipt) {
  return receipt.schema === "noble-ngraph-nickel-receipt/v1"
    && receipt.sha256 === receiptDigest(receipt.payload);
}

function verifyConsumer(consumer, receipt) {
  return verifyReceipt(receipt)
    && consumer.schema === "noble-ngraph-nickel-consumer/v1"
    && consumer.receipt_sha256 === receipt.sha256
    && stable(consumer.bindings) === stable(consumerBindings(receipt.payload))
    && consumer.sha256 === consumerDigest(consumer);
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function alterAt(value, keys) {
  let parent = value;
  for (const key of keys.slice(0, -1)) parent = parent[key];
  const key = keys.at(-1);
  const original = parent[key];
  if (typeof original === "string") parent[key] = `${original}!stale`;
  else if (typeof original === "number") parent[key] = original + 1;
  else if (typeof original === "boolean") parent[key] = !original;
  else throw new Error(`cannot mutate non-scalar binding ${keys.join(".")}`);
}

function staleBindingControls(receipt, consumer) {
  const mutations = [
    { field: "source bytes", receipt: ["source_bytes", 0, "sha256"], consumer: ["bindings", "source_bytes_sha256"] },
    { field: "library import bytes", receipt: ["import_bytes", 0, "sha256"], consumer: ["bindings", "import_bytes_sha256"] },
    { field: "contract import bytes", receipt: ["import_bytes", 1, "sha256"], consumer: ["bindings", "import_bytes_sha256"] },
    { field: "construction parameters", receipt: ["construction_parameters", 0, "parameters", 0, "value"], consumer: ["bindings", "construction_parameters_sha256"] },
    { field: "library version", receipt: ["versions", "library"], consumer: ["bindings", "versions_sha256"] },
    { field: "contract version", receipt: ["versions", "contract"], consumer: ["bindings", "versions_sha256"] },
    { field: "evaluator identity", receipt: ["evaluator_identity", "node", "sha256"], consumer: ["bindings", "evaluator_identity_sha256"] },
    { field: "Nickel evaluator identity", receipt: ["evaluator_identity", "nickel", "sha256"], consumer: ["bindings", "evaluator_identity_sha256"] },
    { field: "import closure", receipt: ["import_closures", 0, "declared", 1], consumer: ["bindings", "import_closures_sha256"] },
    { field: "observed import closure", receipt: ["import_closures", 0, "observed", 1, "sha256"], consumer: ["bindings", "import_closures_sha256"] },
    { field: "exported constructor bytes", receipt: ["exported_constructor_bytes", 0, "sha256"], consumer: ["bindings", "exported_constructor_bytes_sha256"] },
    { field: "lowerer revision", receipt: ["lowerer", "revision"], consumer: ["bindings", "lowerer_revision"] },
    { field: "lowerer source bytes", receipt: ["lowerer", "sha256"], consumer: ["bindings", "lowerer_source_sha256"] },
    { field: "profile", receipt: ["profile"], consumer: ["bindings", "profile"] },
    { field: "graph bytes", receipt: ["graphs", 0, "sha256"], consumer: ["bindings", "graphs", 0, "sha256"] },
    { field: "checked identity", receipt: ["graphs", 0, "checked_identity"], consumer: ["bindings", "graphs", 0, "checked_identity"] },
  ];
  const results = mutations.map((mutation) => {
    const staleReceipt = clone(receipt);
    alterAt(staleReceipt.payload, mutation.receipt);
    const receiptRefused = !verifyReceipt(staleReceipt);

    const changedValidReceipt = clone(receipt);
    alterAt(changedValidReceipt.payload, mutation.receipt);
    changedValidReceipt.sha256 = receiptDigest(changedValidReceipt.payload);
    const staleConsumerRefused = !verifyConsumer(consumer, changedValidReceipt);

    const changedConsumer = clone(consumer);
    alterAt(changedConsumer, mutation.consumer);
    const consumerRecordRefused = !verifyConsumer(changedConsumer, receipt);
    if (!receiptRefused || !staleConsumerRefused || !consumerRecordRefused) {
      throw new Error(`stale binding control accepted a changed ${mutation.field}`);
    }
    return {
      field: mutation.field,
      stale_receipt_refused: receiptRefused,
      stale_consumer_refused: staleConsumerRefused,
      changed_consumer_record_refused: consumerRecordRefused,
    };
  });
  return {
    schema: "noble-ngraph-nickel-stale-controls/v1",
    current_receipt_accepted: verifyReceipt(receipt),
    current_consumer_accepted: verifyConsumer(consumer, receipt),
    mutation_count: results.length,
    mutations: results,
  };
}

function retainReceipt(out, sourceCommit, arithmetic, composition, evaluator, closures, exported, checked) {
  const arithmeticParameters = JSON.parse(readFileSync(path.join(out, "bound-constructor.json"), "utf8")).parameters;
  const compositionParameters = Object.entries(composition.result.law_results).map(([name, law]) => ({
    source: COMPOSITION_SOURCE,
    case: name,
    parameters: law.construction_parameters,
  }));
  const lowererBytes = readFileSync(path.join(ROOT, LOWERER_SOURCE));
  const payload = {
    source_commit: sourceCommit,
    source_bytes: [SOURCE, COMPOSITION_SOURCE, DELAYED_SOURCE].map(fileRecord),
    import_bytes: [LIBRARY, CONTRACT].map(fileRecord),
    construction_parameters: [
      { source: SOURCE, case: "arithmetic", parameters: arithmeticParameters },
      ...compositionParameters,
    ],
    versions: {
      schema: exported.schema,
      library: exported.library_version,
      contract: exported.contract_version,
    },
    evaluator_identity: evaluator,
    import_closures: closures,
    exported_constructor_bytes: [
      { source: SOURCE, sha256: sha256(arithmetic.rawExport), bytes: arithmetic.rawExport.length },
      { source: COMPOSITION_SOURCE, sha256: sha256(composition.exportedBytes), bytes: composition.exportedBytes.length },
    ],
    lowerer: {
      revision: LOWERER_REVISION,
      source: LOWERER_SOURCE,
      sha256: sha256(lowererBytes),
      bytes: lowererBytes.length,
    },
    profile: "ngraph-i64/0",
    graphs: [
      {
        label: "arithmetic",
        sha256: checked.graphSha,
        bytes: checked.graphBytes.length,
        checked_identity: checked.checked.graph,
        profile: checked.checked.profile,
      },
      ...Object.entries(composition.admitted).map(([label, graph]) => ({
        label: `composition-${label}`,
        sha256: graph.graphSha,
        bytes: graph.graphBytes.length,
        checked_identity: graph.checked.graph,
        profile: graph.checked.profile,
      })),
    ],
  };
  const receipt = {
    schema: "noble-ngraph-nickel-receipt/v1",
    payload,
    sha256: receiptDigest(payload),
  };
  const consumer = {
    schema: "noble-ngraph-nickel-consumer/v1",
    receipt_sha256: receipt.sha256,
    bindings: consumerBindings(payload),
  };
  consumer.sha256 = consumerDigest(consumer);
  if (!verifyReceipt(receipt) || !verifyConsumer(consumer, receipt)) {
    throw new Error("fresh Nickel provenance receipt or consumer record failed verification");
  }
  const stale = staleBindingControls(receipt, consumer);
  writeFileSync(path.join(out, "receipt.json"), `${JSON.stringify(receipt, null, 2)}\n`);
  writeFileSync(path.join(out, "consumer.json"), `${JSON.stringify(consumer, null, 2)}\n`);
  writeFileSync(path.join(out, "stale-controls.json"), `${JSON.stringify(stale, null, 2)}\n`);
  return { receipt, consumer, stale };
}

function cleanSourceCommit() {
  const status = run("git", ["status", "--porcelain"]);
  if (status.status !== 0) throw new Error(`git status failed: ${status.stderr}`);
  if (status.stdout.trim()) throw new Error("replay requires a clean committed source tree");
  const commit = run("git", ["rev-parse", "HEAD"]);
  if (commit.status !== 0) throw new Error(`git rev-parse failed: ${commit.stderr}`);
  return commit.stdout.trim();
}

function replay(noblePath, outPath) {
  const out = path.resolve(outPath);
  if (!path.isAbsolute(outPath) || out === ROOT || out.startsWith(`${ROOT}${path.sep}`)) {
    throw new Error("--out must be a new absolute directory outside the repository");
  }
  if (existsSync(out) && readdirSync(out).length !== 0) throw new Error("--out must be new or empty");
  mkdirSync(out, { recursive: true });
  const sourceCommit = cleanSourceCommit();
  const noble = realpathSync(noblePath);
  const evaluator = evaluatorIdentity();
  const importClosure = observeClosure(SOURCE, DECLARED_CLOSURE);
  const sourceBytes = readFileSync(path.join(ROOT, SOURCE));
  const rawExport = exportNickel(SOURCE);
  const closureControl = closureRefusal();
  const delayedGraphPath = path.join(out, "delayed-contract-admission.ngraph");
  const delayedControl = delayedContractRefusal(delayedGraphPath);
  const delayedClosure = observeClosure(DELAYED_SOURCE, [DELAYED_SOURCE, SOURCE, LIBRARY, CONTRACT]);
  writeFileSync(path.join(out, "import-closure-control.json"), `${JSON.stringify(closureControl, null, 2)}\n`);
  writeFileSync(path.join(out, "delayed-contract-control.json"), `${JSON.stringify({ ...delayedControl, import_closure: delayedClosure }, null, 2)}\n`);
  const exported = JSON.parse(rawExport.toString("utf8"));
  const parameters = exported.parameters;
  const versions = {
    schema: exported.schema,
    library: exported.library_version,
    contract: exported.contract_version,
  };
  const exportSha = sha256(rawExport);
  const bundleSha = sha256(Buffer.from(stable({
    source_sha256: sha256(sourceBytes),
    import_closure: importClosure,
    parameters,
    versions,
    evaluator,
    declared_import_closure: DECLARED_CLOSURE,
    exported_sha256: exportSha,
  })));
  exported.bundle_sha256 = bundleSha;
  const boundBytes = Buffer.from(`${JSON.stringify(exported, null, 2)}\n`);
  const boundPath = path.join(out, "bound-constructor.json");
  const graphPath = path.join(out, "admitted.ngraph");
  const vectorsPath = path.join(out, "arithmetic.vectors");
  writeFileSync(path.join(out, "exported-constructor.json"), rawExport);
  writeFileSync(boundPath, boundBytes);

  const checked = nobleReports(noble, [
    "ngraph", "check", boundPath,
    "--graph-out", graphPath,
    "--budget-work", "4096",
    "--budget-expansion", "1024",
    "--budget-inputs", "32",
    "--budget-outputs", "64",
    "--budget-layers", "128",
    "--budget-live-nodes", "1024",
    "--budget-graph-bytes", "1048576",
  ], "native check_with admission")[0];
  if (checked.profile !== "ngraph-i64/0" || !checked.subject.includes("authored-nickel")) {
    throw new Error("native checker admitted an unexpected graph profile or producer identity");
  }
  if (checked.lowerer_revision !== LOWERER_REVISION) {
    throw new Error(`native checker reported an unexpected lowerer revision: ${checked.lowerer_revision}`);
  }
  const graphBytes = readFileSync(graphPath);
  const inputs = ["0", "1", "-1", I64_MIN.toString(), I64_MAX.toString(), I64_WIDE.toString(), "-9007199254740995"];
  writeFileSync(vectorsPath, [
    "noble-ngraph-vectors/0",
    "types i64",
    ...inputs.map((input) => `v ${input}`),
    "",
  ].join("\n"));
  const expected = inputs.map(expectedFor);
  const reference = evaluate(noble, graphPath, vectorsPath, true);
  const packed = evaluate(noble, graphPath, vectorsPath, false);
  if (stable(reference) !== stable(expected)) throw new Error(`reference mismatch: ${stable(reference)}`);
  if (stable(packed) !== stable(expected)) throw new Error(`packed mismatch: ${stable(packed)}`);
  const composition = runCompositionLaws(noble, evaluator, out);
  const closures = [
    { entry: SOURCE, declared: DECLARED_CLOSURE, observed: importClosure },
    { entry: COMPOSITION_SOURCE, declared: COMPOSITION_CLOSURE, observed: composition.closure },
    {
      entry: DELAYED_SOURCE,
      declared: [DELAYED_SOURCE, SOURCE, LIBRARY, CONTRACT],
      observed: delayedClosure,
    },
  ];
  const provenance = retainReceipt(
    out,
    sourceCommit,
    { rawExport },
    composition,
    evaluator,
    closures,
    exported,
    { checked, graphBytes, graphSha: sha256(graphBytes) },
  );

  const result = {
    schema: "noble-ngraph-nickel-fixture-result/v1",
    source_commit: sourceCommit,
    source: { path: SOURCE, sha256: sha256(sourceBytes), bytes: sourceBytes.length },
    import_closure: { policy: "exact ordered relative paths", declared: DECLARED_CLOSURE, observed: importClosure },
    evaluator,
    import_closure_control: closureControl,
    delayed_contract_control: delayedControl,
    exported: { sha256: exportSha, bytes: rawExport.length },
    constructor: { sha256: sha256(boundBytes), bundle_sha256: bundleSha, bytes: boundBytes.length },
    admission: {
      graph_identity: checked.graph,
      graph_sha256: sha256(graphBytes),
      graph_bytes: graphBytes.length,
      subject: checked.subject,
      profile: checked.profile,
      live_nodes: checked.live_nodes,
      layers: checked.layers,
    },
    observer: "ordered outputs under exact wrapping-I64 evaluation",
    independent_expected: {
      input_rows: inputs,
      outputs: expected,
      method: "Independent BigInt arithmetic with BigInt.asIntN(64); not evaluated by Nickel or Noble.",
    },
    reference,
    packed,
    composition: composition.result,
    provenance: {
      receipt_sha256: provenance.receipt.sha256,
      consumer_sha256: provenance.consumer.sha256,
      stale_controls: provenance.stale.mutation_count,
    },
    non_claims: [
      "Finite implementation observations do not prove source-to-IR correspondence.",
      "No effect independence, resource permission, or arbitrary output reordering is claimed.",
    ],
  };
  writeFileSync(path.join(out, "fixture-result.json"), `${JSON.stringify(result, null, 2)}\n`);
  writeFileSync(path.join(out, "expected-results.json"), `${JSON.stringify(result.independent_expected, null, 2)}\n`);
  return { ...result, compositionRun: composition };
}

function main(args) {
  if (args.length !== 5 || args[0] !== "replay" || args[1] !== "--noble" || args[3] !== "--out") {
    throw new Error("usage: node tools/nickel-ngraph.mjs replay --noble <absolute noble CLI> --out <new absolute directory>");
  }
  const result = replay(args[2], args[4]);
  process.stdout.write(`${JSON.stringify({
    status: "PASS",
    source_commit: result.source_commit,
    graph_identity: result.admission.graph_identity,
    evaluators_match: true,
    composition_laws_match: result.composition.reassociation_equal,
    provenance_bindings_match: result.provenance.stale_controls === 16,
  })}\n`);
}

try {
  main(process.argv.slice(2));
} catch (error) {
  process.stderr.write(`${error.stack ?? error}\n`);
  process.exitCode = 1;
}
