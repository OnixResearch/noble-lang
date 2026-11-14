import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const runDir = resolve(process.argv[2] ?? "");
if (process.argv.length !== 3 || !runDir) {
  throw new Error("usage: node analyze.mjs <external-run-directory>");
}
const root = process.cwd();
const git = process.env.NOBLE_GIT ?? "git";
const criteriaPath = join(root, "verification/typed-chunk-ranking/criteria-v1.json");
const corpusPath = join(root, "verification/typed-chunk-ranking/corpus-v1.json");
const criteriaBytes = readFileSync(criteriaPath);
const corpusBytes = readFileSync(corpusPath);
const criteria = JSON.parse(criteriaBytes.toString("utf8"));
const corpus = JSON.parse(corpusBytes.toString("utf8"));
const receipt = JSON.parse(readFileSync(join(runDir, "receipt.json"), "utf8"));
const model = JSON.parse(readFileSync(join(runDir, "model.json"), "utf8"));
const selection = JSON.parse(readFileSync(join(runDir, "selection.json"), "utf8"));
const readJsonl = (name) => readFileSync(join(runDir, name), "utf8")
  .split("\n")
  .filter((line) => line.length > 0)
  .map((line) => JSON.parse(line));
const training = readJsonl("training.jsonl");
const rows = readJsonl("results.jsonl");
const failures = [];
const requireValue = (condition, message) => {
  if (!condition) failures.push(message);
};
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const gitText = (...args) => execFileSync(git, args, { cwd: root, encoding: "utf8" }).trim();
const sourceCommit = gitText("rev-parse", "HEAD");
const sourceStatus = gitText("status", "--porcelain");

requireValue(criteria.schema === "tchunk-ranking-criteria/1", "criteria schema changed");
requireValue(receipt.schema === "tchunk-ranking-run/1", "run receipt schema is invalid");
requireValue(receipt.measurement_performed === true, "receipt does not declare a completed measurement");
requireValue(receipt.source_tree_clean === true, "producer did not require a clean source tree");
requireValue(receipt.source_commit === sourceCommit, "receipt source commit differs from current HEAD");
requireValue(sourceStatus === "", `analysis requires the same clean source commit; status=${sourceStatus}`);
requireValue(receipt.criteria_blake3 === model.criteria_blake3, "receipt/model criteria binding differs");
requireValue(receipt.corpus_blake3 === model.corpus_blake3, "receipt/model corpus binding differs");
requireValue(receipt.criteria_blake3.length === 64 && receipt.corpus_blake3.length === 64, "model source bindings must be full 32-byte digests");
requireValue(model.source_commit === sourceCommit, "model source commit differs from current HEAD");
requireValue(model.identity === receipt.model_identity, "model identity differs between artifact and run receipt");
requireValue(receipt.task_count === 480 && receipt.train_tasks === 280 && receipt.validation_tasks === 80 && receipt.test_tasks === 120, "receipt corpus split differs from frozen criteria");
requireValue(receipt.paired_repetitions === 10 && receipt.test_rows === 4800, "receipt must contain all 120 tasks x 10 seeds x 4 modes");
requireValue(criteria.measurements.total_charged_work_definition === "Report.checking_work shared-budget units; other budget dimensions are reported separately and are not summed because their units differ and would double-count work", "frozen work definition is absent or changed");
requireValue(receipt.modes.join(",") === criteria.modes.join(","), "mode order differs from frozen criteria");
requireValue(model.graph_shape.input_nodes === 64 && model.graph_shape.param_nodes === 65, "model graph input/parameter counts differ");
requireValue(model.graph_shape.f32_multiply_nodes === 64 && model.graph_shape.ordered_f32_add_nodes === 64 && model.graph_shape.output_count === 1, "model graph operation shape differs");
requireValue(model.graph_shape.forbidden_operations.length === 0, "model graph contains forbidden operations");
requireValue(model.parameter_snapshot.parameter_byte_count === 260, "model parameter bytes differ from 65 F32 parameters");
requireValue(model.training_provenance.optimizer === "adam/0" && model.training_provenance.steps === 256, "training optimizer or step count differs from criteria");
requireValue(selection.training_task_count === 280 && selection.validation_task_count === 80, "training/validation task counts differ");
requireValue(selection.training_point_count === 1120 && selection.validation_point_count === 320, "training/validation point counts differ");
requireValue(selection.training_memory?.actual_peak_retained_bytes === null, "unmeasured training retained memory must not be reported as measured");
requireValue(Number.isSafeInteger(selection.training_memory?.scratch_peak_bytes), "training scratch peak is missing");
requireValue(selection.training_cost_amortization_count === 0, "offline training cost must not be amortized into inference");
requireValue(training.length === 1440 && receipt.training_rows === training.length, "training artifact must contain 1,120 train and 320 validation rows");

const testTasks = corpus.tasks.filter((task) => task.provenance.split === "test");
const expectedTaskIds = testTasks.map((task) => task.id);
const expectedSeeds = criteria.evaluation.seeds;
const modeOrder = criteria.modes;
requireValue(testTasks.length === 120 && new Set(expectedTaskIds).size === 120, "frozen corpus test-task roster is not exactly 120 unique tasks");
requireValue(rows.length === 4800, `results contain ${rows.length} rows, expected 4,800`);
const testIds = new Set(expectedTaskIds);
const trainIds = new Set(corpus.tasks.filter((task) => task.provenance.split !== "test").map((task) => task.id));
const trainingRowKeys = new Set();
const equalFeatures = new Map();
const equalLabels = new Map();
for (const row of training) {
  const expectedSplit = corpus.tasks.find((task) => task.id === row.task_id)?.provenance.split;
  requireValue(expectedSplit === row.split && row.split !== "test", `training row contains invalid or held-out split ${row.task_id}`);
  requireValue(trainIds.has(row.task_id), `held-out task entered training artifacts: ${row.task_id}`);
  requireValue(!Object.hasOwn(row, "expected") && !Object.hasOwn(row, "outputs"), `expected behavior value leaked into feature row ${row.task_id}`);
  requireValue(typeof row.accepted === "boolean", `training label lost accepted/refused outcome for ${row.task_id}/${row.ordinal}`);
  const expectedProposerSource = row.sequence?.length === 0
    ? "empty-shortlex-candidate/no-library-entry"
    : "immutable-library-entry/exact-body-and-dependency-identity";
  requireValue(row.proposer_provenance?.source === expectedProposerSource, `exact proposer provenance missing for ${row.task_id}/${row.ordinal}`);
  requireValue(row.proposer_provenance?.candidate_id?.ordinal === row.label_candidate_id?.ordinal && row.proposer_provenance?.candidate_id?.digest === row.label_candidate_id?.digest, `proposer identity differs from label candidate for ${row.task_id}/${row.ordinal}`);
  requireValue(Number.isSafeInteger(row.proposer_provenance?.re_admission_ns) && row.proposer_provenance.re_admission_ns >= 0, `proposer re-admission timing missing for ${row.task_id}/${row.ordinal}`);
  requireValue(row.features_f32_bits?.length === 64, `row ${row.task_id}/${row.ordinal} has wrong feature width`);
  requireValue(row.target_f32_bits === 0x00000000 || row.target_f32_bits === 0x3f800000, `row ${row.task_id}/${row.ordinal} has a non-binary target`);
  const features = new DataView(new ArrayBuffer(4));
  for (const bits of row.features_f32_bits ?? []) {
    features.setUint32(0, bits, true);
    requireValue(Number.isFinite(features.getFloat32(0, true)), `nonfinite feature in ${row.task_id}/${row.ordinal}`);
  }
  const key = `${row.split}:${row.task_id}:${row.ordinal}`;
  requireValue(!trainingRowKeys.has(key), `duplicate training row ${key}`);
  trainingRowKeys.add(key);
  if (row.family === "equal-signature-contrast" && row.split === "train" && row.ordinal > 0) {
    const featureKey = String(row.ordinal);
    const bits = JSON.stringify(row.features_f32_bits);
    if (equalFeatures.has(featureKey)) requireValue(equalFeatures.get(featureKey) === bits, `equal-signature features leak behavior at ordinal ${row.ordinal}`);
    else equalFeatures.set(featureKey, bits);
    if (!equalLabels.has(featureKey)) equalLabels.set(featureKey, new Set());
    equalLabels.get(featureKey).add(row.target_f32_bits);
  }
}
requireValue(training.filter((row) => row.split === "train").length === 1120, "training split row count differs from 280 tasks x four queue entries");
requireValue(training.filter((row) => row.split === "validation").length === 320, "training split row count differs from 80 tasks x four queue entries");
requireValue(training.some((row) => row.accepted === false), "training data contains no refused attempts");
requireValue(training.filter((row) => row.proposer_provenance?.source === "immutable-library-entry/exact-body-and-dependency-identity").length === 1080, "chunk-backed proposer provenance count differs from nonempty train/validation queue rows");
requireValue(receipt.training_labeling?.teacher_assisted_training_labels === true && receipt.training_labeling?.inference_student_only === true, "teacher-assisted training labels and student-only inference must be reported separately");
requireValue(receipt.training_labeling?.teacher_calls === 1080, "teacher label call count differs from the frozen train/validation candidate roster");
requireValue(receipt.training_labeling?.teacher_calls_in_test_inference === 0, "held-out inference performed teacher calls");
requireValue(receipt.training_labeling?.student_only_learned_inference_rows === 1200 && receipt.training_labeling?.teacher_assisted_completion_rows === 0, "student-only and teacher-assisted completion rows are not separated");
requireValue(Number.isSafeInteger(receipt.training_labeling?.verification_cost), "teacher verification cost is missing");
for (const ordinal of [1, 2, 3]) requireValue(equalLabels.get(String(ordinal))?.size === 2, `equal-signature ordinal ${ordinal} lacks both refused/accepted behavior labels`);

const taskById = new Map(testTasks.map((task) => [task.id, task]));
const seen = new Set();
const modeRows = new Map(modeOrder.map((mode) => [mode, []]));
const modeIndex = new Map(modeOrder.map((mode, index) => [mode, index]));
const expectedOrder = [];
for (let repetition = 0; repetition < expectedSeeds.length; repetition += 1) {
  for (const taskId of expectedTaskIds) {
    for (const mode of modeOrder) expectedOrder.push(`${repetition}|${taskId}|${mode}`);
  }
}
for (let index = 0; index < rows.length; index += 1) {
  const row = rows[index];
  const key = `${row.repetition}|${row.task_id}|${row.mode}`;
  requireValue(key === expectedOrder[index], `row order or no-cherry-picking roster differs at position ${index}`);
  requireValue(!seen.has(key), `duplicate held-out evaluation row ${key}`);
  seen.add(key);
  requireValue(testIds.has(row.task_id), `non-held-out task entered evaluation ${row.task_id}`);
  requireValue(row.split === "test", `evaluation row not marked held out: ${key}`);
  requireValue(row.seed === expectedSeeds[row.repetition], `paired seed mismatch for ${key}`);
  requireValue(modeIndex.has(row.mode), `unknown evaluation mode ${row.mode}`);
  const task = taskById.get(row.task_id);
  requireValue(task?.family === row.family, `family mismatch for ${key}`);
  requireValue(typeof row.structural_acceptance === "boolean" && typeof row.finite_fixture_success === "boolean" && typeof row.independent_replay === "boolean", `missing distinct behavioral outcomes in ${key}`);
  requireValue(Number.isSafeInteger(row.latency_ns) && row.latency_ns >= 0, `invalid end-to-end latency in ${key}`);
  requireValue(row.candidate_rows === (row.mode === "primitive-only" ? 0 : 4), `candidate row count differs from the frozen mode queue in ${key}`);
  if (row.mode === "primitive-only") {
    requireValue(row.report?.maximum_expanded_program_nodes === null, `primitive expansion size must remain explicitly unmeasured in ${key}`);
  } else {
    requireValue(Number.isSafeInteger(row.report?.maximum_expanded_program_nodes) && row.report.maximum_expanded_program_nodes >= 0, `maximum expanded-program node count is missing in ${key}`);
    requireValue(row.work?.maximum_expanded_program_nodes === row.report.maximum_expanded_program_nodes, `expanded-program size differs between work and report in ${key}`);
  }
  requireValue(row.work.total_charged_work === null || (Number.isSafeInteger(row.work.total_charged_work) && row.work.total_charged_work >= 0), `invalid total charged work in ${key}`);
  if (row.mode !== "primitive-only") requireValue(Number.isSafeInteger(row.work.total_charged_work), `missing shared charged-work counter in ${key}`);
  if (row.mode === "learned-ranking" || row.mode === "splitmix64-shuffled-control") {
    const identity = row.ranking_identity;
    const context = model.contexts.find((candidate) => candidate.family === row.family);
    requireValue(identity !== null && typeof identity === "object", `consumer ranking identity missing for ${key}`);
    if (identity && context) {
      requireValue(identity.feature_revision === criteria.features.revision && identity.semantic_revision === 0, `feature/semantic identity mismatch for ${key}`);
      requireValue(identity.library_revision === context.library_revision && identity.candidate_set === context.candidate_set && identity.policy === context.policy && identity.editor_request === context.editor_request, `stale library, queue, policy, or editor identity for ${key}`);
      requireValue(context.task_interfaces.includes(identity.task_interface), `unknown task interface identity for ${key}`);
      requireValue(identity.independent_goal !== "0".repeat(64), `independent consumer goal identity missing for ${key}`);
      if (row.mode === "learned-ranking") requireValue(identity.model_identity === model.identity, `learned scorer model identity mismatch for ${key}`);
      const allowed = new Set(context.allowed_candidates.map((candidate) => `${candidate.ordinal}:${candidate.digest}`));
      for (const candidate of row.ranked_candidates ?? []) requireValue(allowed.has(`${candidate.ordinal}:${candidate.digest}`), `unknown ranked candidate identity for ${key}`);
      for (const candidate of row.scored_candidates ?? []) requireValue(allowed.has(`${candidate.candidate_id.ordinal}:${candidate.candidate_id.digest}`), `unknown scored candidate identity for ${key}`);
      for (const attempt of row.attempts ?? []) requireValue(attempt.request_identity === identity.request, `observer request identity mismatch for ${key}`);
    }
  }
  if (!modeRows.has(row.mode)) modeRows.set(row.mode, []);
  modeRows.get(row.mode).push(row);
}
requireValue(seen.size === 4800, "held-out evaluation roster is incomplete");
for (const mode of modeOrder) requireValue(modeRows.get(mode)?.length === 1200, `${mode} does not contain 120 tasks x 10 paired repetitions`);

const requiredModes = ["fixed-chunk-shortlex", "learned-ranking", "splitmix64-shuffled-control"];
const byPair = new Map();
for (const mode of requiredModes) {
  for (const row of modeRows.get(mode)) byPair.set(`${row.repetition}|${row.task_id}|${mode}`, row);
}
const reductionRows = [];
for (let repetition = 0; repetition < expectedSeeds.length; repetition += 1) {
  for (const taskId of expectedTaskIds) {
    const fixed = byPair.get(`${repetition}|${taskId}|fixed-chunk-shortlex`);
    const learned = byPair.get(`${repetition}|${taskId}|learned-ranking`);
    const shuffled = byPair.get(`${repetition}|${taskId}|splitmix64-shuffled-control`);
    const reduction = (baseline, candidate, field) => baseline === 0 ? null : 100 * (baseline - candidate) / baseline;
    const fixedWork = fixed.work.total_charged_work;
    const learnedWork = learned.work.total_charged_work;
    const shuffledWork = shuffled.work.total_charged_work;
    reductionRows.push({
      task_id: taskId,
      family: fixed.family,
      repetition,
      seed: fixed.seed,
      fixed_work: fixedWork,
      learned_work: learnedWork,
      shuffled_work: shuffledWork,
      learned_work_reduction_percent: reduction(fixedWork, learnedWork, "work"),
      shuffled_work_reduction_percent: reduction(fixedWork, shuffledWork, "work"),
      fixed_latency_ns: fixed.latency_ns,
      learned_latency_ns: learned.latency_ns,
      shuffled_latency_ns: shuffled.latency_ns,
      learned_latency_reduction_percent: reduction(fixed.latency_ns, learned.latency_ns, "latency"),
      shuffled_latency_reduction_percent: reduction(fixed.latency_ns, shuffled.latency_ns, "latency"),
      fixed_finite_success: fixed.finite_fixture_success,
      learned_finite_success: learned.finite_fixture_success,
      shuffled_finite_success: shuffled.finite_fixture_success,
      fixed_structural_acceptance: fixed.structural_acceptance,
      learned_structural_acceptance: learned.structural_acceptance,
      fixed_independent_replay: fixed.independent_replay,
      learned_independent_replay: learned.independent_replay
    });
  }
}
const taskMeans = (field) => {
  const grouped = new Map();
  for (const row of reductionRows) {
    if (!grouped.has(row.task_id)) grouped.set(row.task_id, { family: row.family, values: [] });
    const value = row[field];
    if (value !== null && Number.isFinite(value)) grouped.get(row.task_id).values.push(value);
  }
  return [...grouped].map(([task_id, item]) => ({
    task_id,
    family: item.family,
    mean: item.values.length ? item.values.reduce((sum, value) => sum + value, 0) / item.values.length : null
  }));
};
const median = (values) => {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
};
const quantile = (values, probability) => {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(probability * (sorted.length - 1))];
};
let bootstrapState = 0x7420b501;
const nextRandom = () => {
  bootstrapState ^= bootstrapState << 13;
  bootstrapState ^= bootstrapState >>> 17;
  bootstrapState ^= bootstrapState << 5;
  return bootstrapState >>> 0;
};
const bootstrapMedian = (values) => {
  if (values.length !== 120 || values.some((value) => !Number.isFinite(value))) return { lower: null, upper: null, samples: 0 };
  const samples = [];
  for (let replicate = 0; replicate < 10_000; replicate += 1) {
    const draw = new Array(values.length);
    for (let index = 0; index < draw.length; index += 1) draw[index] = values[nextRandom() % values.length];
    samples.push(median(draw));
  }
  samples.sort((a, b) => a - b);
  return { lower: samples[249], upper: samples[9749], samples: samples.length };
};
const workTaskMeans = taskMeans("learned_work_reduction_percent");
const latencyTaskMeans = taskMeans("learned_latency_reduction_percent");
const shuffleWorkTaskMeans = taskMeans("shuffled_work_reduction_percent");
const shuffleLatencyTaskMeans = taskMeans("shuffled_latency_reduction_percent");
const primary = {
  learned_work_reduction_percent: {
    median: median(workTaskMeans.map((row) => row.mean).filter(Number.isFinite)),
    bootstrap_95: bootstrapMedian(workTaskMeans.map((row) => row.mean))
  },
  learned_latency_reduction_percent: {
    median: median(latencyTaskMeans.map((row) => row.mean).filter(Number.isFinite)),
    bootstrap_95: bootstrapMedian(latencyTaskMeans.map((row) => row.mean))
  },
  shuffled_work_reduction_percent: {
    median: median(shuffleWorkTaskMeans.map((row) => row.mean).filter(Number.isFinite)),
    bootstrap_95: bootstrapMedian(shuffleWorkTaskMeans.map((row) => row.mean))
  },
  shuffled_latency_reduction_percent: {
    median: median(shuffleLatencyTaskMeans.map((row) => row.mean).filter(Number.isFinite)),
    bootstrap_95: bootstrapMedian(shuffleLatencyTaskMeans.map((row) => row.mean))
  },
  bootstrap: {
    resampling_unit: "120 task-level means; ten paired repetitions averaged within task",
    replicates: 10_000,
    deterministic_xorshift32_seed: "0x7420b501",
    interval: "percentile 95%; lower and upper order statistics 250/9750 of 10,000"
  }
};
const rate = (items, predicate) => items.length ? 100 * items.filter(predicate).length / items.length : null;
const successRegression = (mode, field, subset) => rate(subset.filter((row) => row.mode === "fixed-chunk-shortlex"), (row) => row[field])
  - rate(subset.filter((row) => row.mode === mode), (row) => row[field]);
const testRows = modeOrder.flatMap((mode) => modeRows.get(mode));
const learnedRows = modeRows.get("learned-ranking");
const fixedRows = modeRows.get("fixed-chunk-shortlex");
const learnedFallbackRows = learnedRows.reduce((sum, row) => sum + (row.invalid_score_rows ?? 0) + (row.fallback_candidate_rows ?? 0), 0);
const learnedCandidateRows = learnedRows.reduce((sum, row) => sum + row.candidate_rows, 0);
const fallbackPercent = learnedCandidateRows ? 100 * learnedFallbackRows / learnedCandidateRows : null;
const fixedRetainedProxy = Math.max(...fixedRows.map((row) => row.memory.retained_bytes_proxy));
const learnedRetainedProxy = Math.max(...learnedRows.map((row) => row.memory.retained_bytes_proxy));
const retainedProxyRatio = fixedRetainedProxy ? learnedRetainedProxy / fixedRetainedProxy : null;
const actualRetainedMeasured = learnedRows.every((row) => Number.isSafeInteger(row.memory.actual_peak_retained_bytes));
const familySummaries = criteria.corpus.families.map((family) => {
  const subset = testRows.filter((row) => row.family === family.id);
  const output = { family: family.id, task_count: 24, repetitions: 10, modes: {} };
  for (const mode of modeOrder) {
    const modeSubset = subset.filter((row) => row.mode === mode);
    output.modes[mode] = {
      rows: modeSubset.length,
      structural_acceptance_percent: rate(modeSubset, (row) => row.structural_acceptance),
      finite_fixture_success_percent: rate(modeSubset, (row) => row.finite_fixture_success),
      independent_replay_percent: rate(modeSubset, (row) => row.independent_replay),
      median_work_units: median(modeSubset.map((row) => row.work.total_charged_work).filter(Number.isFinite)),
      median_latency_ns: median(modeSubset.map((row) => row.latency_ns)),
      invalid_score_rows: modeSubset.reduce((sum, row) => sum + (row.invalid_score_rows ?? 0), 0),
      fallback_candidate_rows: modeSubset.reduce((sum, row) => sum + (row.fallback_candidate_rows ?? 0), 0)
    };
  }
  return output;
});
const learnedSuccessRegressionOverall = successRegression("learned-ranking", "finite_fixture_success", testRows);
const learnedStructuralRegression = successRegression("learned-ranking", "structural_acceptance", testRows);
const learnedReplayRegression = successRegression("learned-ranking", "independent_replay", testRows);
const perFamilySuccessRegression = familySummaries.map((family) => ({
  family: family.family,
  percentage_points: successRegression("learned-ranking", "finite_fixture_success", testRows.filter((row) => row.family === family.family))
}));
const maximumFamilySuccessRegression = Math.max(...perFamilySuccessRegression.map((row) => row.percentage_points));
const workPass = primary.learned_work_reduction_percent.median >= criteria.decision.work_reduction_median_percent_min
  && primary.learned_work_reduction_percent.bootstrap_95.lower > criteria.decision.work_reduction_paired_bootstrap_95_lower_bound_percent_min_exclusive;
const latencyPass = primary.learned_latency_reduction_percent.median >= criteria.decision.warm_latency_reduction_median_percent_min
  && primary.learned_latency_reduction_percent.bootstrap_95.lower > criteria.decision.warm_latency_paired_bootstrap_95_lower_bound_percent_min_exclusive;
const successPass = learnedSuccessRegressionOverall <= criteria.decision.finite_fixture_success_regression_overall_percentage_points_max
  && maximumFamilySuccessRegression <= criteria.decision.finite_fixture_success_regression_per_family_percentage_points_max;
const structuralPass = learnedStructuralRegression <= criteria.decision.structural_acceptance_regression_percentage_points_max;
const replayPass = learnedReplayRegression <= criteria.decision.independent_replay_regression_percentage_points_max;
const memoryPass = actualRetainedMeasured && retainedProxyRatio !== null && retainedProxyRatio <= criteria.decision.retained_bytes_ratio_max;
const fallbackPass = fallbackPercent !== null && fallbackPercent <= criteria.decision.invalid_or_fallback_candidate_rows_percent_max;
const shufflePass = primary.learned_work_reduction_percent.median > primary.shuffled_work_reduction_percent.median
  && primary.learned_latency_reduction_percent.median > primary.shuffled_latency_reduction_percent.median;
const conditions = {
  work_reduction: workPass,
  warm_latency_reduction: latencyPass,
  finite_fixture_success: successPass,
  structural_acceptance: structuralPass,
  independent_replay: replayPass,
  retained_memory_measured_and_within_ratio: memoryPass,
  invalid_or_fallback_rows: fallbackPass,
  beats_shuffled_control_on_both_primary_reductions: shufflePass
};
const decision = Object.values(conditions).every(Boolean) ? "GO" : "NO-GO";
const modeSummary = Object.fromEntries(modeOrder.map((mode) => {
  const subset = modeRows.get(mode);
  return [mode, {
    rows: subset.length,
    structural_acceptance_percent: rate(subset, (row) => row.structural_acceptance),
    finite_fixture_success_percent: rate(subset, (row) => row.finite_fixture_success),
    independent_replay_percent: rate(subset, (row) => row.independent_replay),
    median_latency_ns: median(subset.map((row) => row.latency_ns)),
    p95_latency_ns: quantile(subset.map((row) => row.latency_ns), 0.95),
    median_total_charged_work: median(subset.map((row) => row.work.total_charged_work).filter(Number.isFinite)),
    invalid_score_rows: subset.reduce((sum, row) => sum + (row.invalid_score_rows ?? 0), 0),
    fallback_candidate_rows: subset.reduce((sum, row) => sum + (row.fallback_candidate_rows ?? 0), 0),
    no_result_rows: subset.filter((row) => row.response_outcome === "no-result").length,
    exhausted_rows: subset.filter((row) => row.response_outcome === "exhausted").length,
    invalid_rows: subset.filter((row) => row.response_outcome === "invalid").length,
    unsupported_rows: subset.filter((row) => row.response_outcome === "unsupported").length
  }];
}));
const pairedTaskSummary = workTaskMeans.map((row, index) => ({
  task_id: row.task_id,
  family: row.family,
  learned_work_reduction_percent: row.mean,
  learned_latency_reduction_percent: latencyTaskMeans[index]?.mean ?? null,
  shuffled_work_reduction_percent: shuffleWorkTaskMeans[index]?.mean ?? null,
  shuffled_latency_reduction_percent: shuffleLatencyTaskMeans[index]?.mean ?? null
}));
const sourceFiles = [
  "Cargo.lock",
  "crates/noble-contracts/src/source/chunks.rs",
  "crates/noble-contracts/tests/editor/chunks-completion.rs",
  "crates/noble-contracts/tests/editor/chunks-completion/ranking.rs",
  "crates/noble-contracts/tests/editor/chunks-completion/ranking/budgets.rs",
  "crates/noble-contracts/tests/editor/chunks-completion/ranking/durability.rs",
  "crates/noble-contracts/tests/editor/chunks-completion/ranking/fallbacks.rs",
  "crates/noble-contracts/tests/editor/chunks-completion/ranking/goals.rs",
  "crates/noble-contracts/tests/editor/chunks-completion/ranking/lanes.rs",
  "crates/noble-contracts/tests/editor/chunks-completion/ranking/observations.rs",
  "crates/noble-contracts/tests/editor/chunks-completion/ranking/ordering.rs",
  "crates/noble-ngraph/src/train.rs",
  "crates/noble-ngraph/tests/train_admission.rs",
  "crates/noble-ngraph/tests/train_admission/batches.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/execution.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/execution/json.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/execution/row.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/execution/run.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/execution/run/baselines.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/execution/run/learned.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/execution/run/shuffled.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/fixtures.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/fixtures/libraries.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/fixtures/manifest.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/fixtures/tasks.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/labels.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/labels/collect.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/labels/oracle.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/measure.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/measure/artifacts.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/measure/evaluate.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/measure/experiment.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/measure/receipt.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/measure/tools.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/samples.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/samples/collect.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/samples/rows.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/scoring.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/scoring/fit.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/scoring/model.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/scoring/rankers.rs",
  "crates/noble-ngraph-experiment/tests/b5_training/ranking/scoring/shuffle.rs",
  "verification/typed-chunk-ranking/criteria-v1.json",
  "verification/typed-chunk-ranking/corpus-v1.json",
  "verification/typed-chunk-ranking/runner.mjs",
  "verification/typed-chunk-ranking/analyze.mjs"
];
const sourceHashes = Object.fromEntries(sourceFiles.map((name) => [name, sha256(readFileSync(join(root, name)))]));
const hashes = Object.fromEntries(["receipt.json", "model.json", "selection.json", "training.jsonl", "results.jsonl"].map((name) => [name, sha256(readFileSync(join(runDir, name)))]));
if (failures.length > 0) {
  console.error(JSON.stringify({ schema: "tchunk-ranking-analysis/1", status: "invalid", failures }, null, 2));
  process.exitCode = 1;
} else {
  const analysis = {
    schema: "tchunk-ranking-analysis/1",
    source_commit: sourceCommit,
    source_tree_clean_at_analysis: true,
    source_files_sha256: sourceHashes,
    artifact_sha256: hashes,
    criteria_sha256: sha256(criteriaBytes),
    corpus_sha256: sha256(corpusBytes),
    model_identity: model.identity,
    toolchain: receipt.toolchain,
    test_roster: {
      corpus_test_task_ids: expectedTaskIds,
      family_task_counts: Object.fromEntries(criteria.corpus.families.map((family) => [family.id, 24])),
      paired_seeds: expectedSeeds,
      modes: modeOrder,
      expected_rows: 4800,
      observed_rows: rows.length,
      no_cherry_picking_order_verified: true
    },
    training_roster: {
      training_rows: training.filter((row) => row.split === "train").length,
      validation_rows: training.filter((row) => row.split === "validation").length,
      held_out_rows_in_training_artifact: training.filter((row) => row.split === "test").length,
      refused_attempt_rows: training.filter((row) => row.accepted === false).length,
      exact_proposer_provenance_rows: training.filter((row) => row.proposer_provenance?.source === "immutable-library-entry/exact-body-and-dependency-identity").length,
      empty_candidate_rows: training.filter((row) => row.proposer_provenance?.source === "empty-shortlex-candidate/no-library-entry").length,
      equal_signature_feature_invariance_checked: true,
      expected_fixture_fields_in_training_rows: false
    },
    training_cost: receipt.training_summary,
    teacher_reporting: receipt.training_labeling,
    model_load: receipt.model_load,
    mode_summary: modeSummary,
    family_summary: familySummaries,
    primary_reductions: primary,
    paired_task_mean_reductions: pairedTaskSummary,
    decision_metrics: {
      finite_fixture_success_regression_overall_percentage_points: learnedSuccessRegressionOverall,
      finite_fixture_success_regression_per_family_percentage_points: perFamilySuccessRegression,
      maximum_family_success_regression_percentage_points: maximumFamilySuccessRegression,
      structural_acceptance_regression_percentage_points: learnedStructuralRegression,
      independent_replay_regression_percentage_points: learnedReplayRegression,
      invalid_or_fallback_candidate_rows_percent: fallbackPercent,
      retained_bytes_proxy_ratio_learned_to_fixed: retainedProxyRatio,
      actual_peak_retained_bytes_measured: actualRetainedMeasured,
      conditions,
      decision
    },
    caveats: [
      "Report.checking_work is the pre-registered scalar total charged work; independent resource dimensions remain separate.",
      "Session::prepare does not expose host parser work counters; those work fields are null, while source bytes and end-to-end latency are retained.",
      "Actual process peak retained bytes are not instrumented; the serialized-model and budget-retained value is a lower-bound proxy, so the memory GO condition fails closed.",
      "Training reports parameter, optimizer-state and scratch-counter bytes separately; actual process or allocator retained-memory peaks and offline-cost amortization are not claimed.",
      "Finite fixture agreement and independent finite replay are behavioral observations only, not proof, completeness, semantic equivalence, or generalization.",
      "All modes use the same fixed library; library-growth gains were not measured.",
      "The compiled select/step evaluator schedules a fixed DAG and selects already-computed values; no skipped-computation or sparse-compute claim is made."
    ]
  };
  writeFileSync(join(runDir, "paired.jsonl"), reductionRows.map((row) => JSON.stringify(row)).join("\n") + "\n");
  writeFileSync(join(runDir, "task-summary.json"), JSON.stringify(pairedTaskSummary, null, 2) + "\n");
  writeFileSync(join(runDir, "family-summary.json"), JSON.stringify(familySummaries, null, 2) + "\n");
  writeFileSync(join(runDir, "analysis.json"), JSON.stringify(analysis, null, 2) + "\n");
  writeFileSync(join(runDir, "no-cherry-picking.json"), JSON.stringify(analysis.test_roster, null, 2) + "\n");
  console.log(JSON.stringify({ schema: analysis.schema, status: "analyzed", decision, conditions, primary_reductions: primary, overall_finite_success_regression_pp: learnedSuccessRegressionOverall, retained_proxy_ratio: retainedProxyRatio, actual_peak_retained_bytes_measured: actualRetainedMeasured, output: runDir }, null, 2));
}
