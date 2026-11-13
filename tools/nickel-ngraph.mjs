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
  })}\n`);
}

try {
  main(process.argv.slice(2));
} catch (error) {
  process.stderr.write(`${error.stack ?? error}\n`);
  process.exitCode = 1;
}
