import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";

const criteriaUrl = new URL("./criteria-v1.json", import.meta.url);
const criteria = JSON.parse(readFileSync(criteriaUrl, "utf8"));
const corpusUrl = new URL("./corpus-v1.json", import.meta.url);
const corpusBytes = readFileSync(corpusUrl);
const corpus = JSON.parse(corpusBytes.toString("utf8"));
const corpusSha256 = createHash("sha256").update(corpusBytes).digest("hex");
const failures = [];
const requireValue = (condition, message) => {
  if (!condition) failures.push(message);
};

requireValue(criteria.schema === "tchunk-ranking-criteria/1", "criteria schema must be tchunk-ranking-criteria/1");
requireValue(criteria.corpus?.schema === "tchunk-ranking-corpus/1", "corpus schema must be tchunk-ranking-corpus/1");
requireValue(criteria.corpus?.task_count === 480, "corpus criteria must require exactly 480 tasks");
requireValue(criteria.corpus?.families?.length === 5, "corpus criteria must require five families");
requireValue(criteria.corpus.families.every((family) => family.tasks === 96 && family.blocks === 12 && family.tasks_per_block === 8), "each family must require 96 tasks in twelve blocks of eight");
requireValue(criteria.corpus.families.reduce((sum, family) => sum + family.tasks, 0) === criteria.corpus.task_count, "family counts must cover the required corpus");
requireValue(criteria.corpus.split?.train_tasks === 280 && criteria.corpus.split.validation_tasks === 80 && criteria.corpus.split.test_tasks === 120, "frozen split must be 280/80/120");
requireValue(criteria.corpus.split?.split_unit === "family-provenance-block" && criteria.corpus.split.row_randomization === false, "split must be by provenance block, never randomized rows");
requireValue(criteria.features?.lanes === 64 && criteria.features.revision === "tchunk-features/1", "feature criteria must specify exactly 64 lanes at the frozen revision");
requireValue(criteria.scorer?.input_slots === 64 && criteria.scorer.trainable_param_slots === 65, "scorer criteria must bind the fixed 64-input/65-parameter shape");
requireValue(criteria.scorer?.optimizer?.steps === 256 && criteria.scorer.optimizer.batch_size === 32, "training steps and batch size must match the preregistration");
requireValue(criteria.scorer?.optimizer?.training_seeds?.length === 3, "exactly three training seeds must be frozen");
requireValue(criteria.modes?.join(",") === "primitive-only,fixed-chunk-shortlex,learned-ranking,splitmix64-shuffled-control", "all four matched modes must remain frozen and ordered");
requireValue(criteria.evaluation?.test_tasks === 120 && criteria.evaluation.paired_repetitions === 10 && criteria.evaluation.seeds.length === 10, "evaluation criteria must include all 120 test tasks and ten paired seeds");
requireValue(criteria.decision?.default === "NO-GO" && criteria.decision.go_requires_all === true, "GO must require every frozen decision condition");
requireValue(criteria.measurements?.total_charged_work_definition === "Report.checking_work shared-budget units; other budget dimensions are reported separately and are not summed because their units differ and would double-count work", "total charged work must use one shared-budget counter without mixing resource units");
requireValue(criteria.ranking?.authority === "queue-permutation-only", "the scorer must remain a queue-permutation-only signal");
requireValue(criteria.ranking?.exhaustion === "fail-closed-exhausted-no-fallback-work-no-partial-publication", "exhaustion must remain fail-closed");
requireValue(criteria.system_one?.consumer_owns_observation === true, "System One observations must be consumer-owned");
requireValue(criteria.system_one?.procedure_identity === "exact-body-and-dependency-identity-never-signature-only", "procedure identity must include exact body and dependencies");
requireValue(criteria.scope_limit?.includes("no-branch-skipping-claim"), "compiled-select scope limit must remain explicit");

requireValue(corpus.schema === criteria.corpus.schema, "materialized corpus schema must match criteria");
requireValue(corpus.task_count === criteria.corpus.task_count && corpus.tasks?.length === criteria.corpus.task_count, "materialized corpus must contain exactly 480 tasks");
requireValue(corpus.family_count === criteria.corpus.families.length && corpus.libraries?.length === criteria.corpus.families.length, "materialized corpus must bind all five family libraries");
requireValue(corpus.tasks_per_family === 96 && corpus.provenance_blocks_per_family === 12 && corpus.tasks_per_block === 8, "materialized corpus block dimensions must match criteria");
requireValue(corpus.split?.train === criteria.corpus.split.train_tasks && corpus.split?.validation === criteria.corpus.split.validation_tasks && corpus.split?.test === criteria.corpus.split.test_tasks, "materialized corpus split counts must match criteria");
requireValue(corpus.expected_values_owner === "independent-consumer-fixture" && corpus.expected_values_are_features === false, "expected-value ownership and feature exclusion must remain explicit");
const taskIds = new Set();
const canonicalSources = new Set();
const familyCounts = new Map();
const splitCounts = new Map();
for (const task of corpus.tasks ?? []) {
  requireValue(typeof task.id === "string" && !taskIds.has(task.id), `task ID missing or duplicated: ${task.id}`);
  taskIds.add(task.id);
  const source = task.primitive_reference_source;
  const canonicalSource = typeof source === "string" ? source.trim().replace(/\s+/gu, " ") : null;
  requireValue(canonicalSource !== null && canonicalSource.length > 0 && !canonicalSources.has(canonicalSource), `task ${task.id} has a missing or duplicated canonical reference program`);
  if (canonicalSource !== null) canonicalSources.add(canonicalSource);
  familyCounts.set(task.family, (familyCounts.get(task.family) ?? 0) + 1);
  const block = task.provenance?.block;
  const expectedSplit = block <= 6 ? "train" : block <= 8 ? "validation" : "test";
  requireValue(task.provenance?.split === expectedSplit, `task ${task.id} has a split inconsistent with its provenance block`);
  requireValue(task.provenance?.generator === "tchunk-ranking-corpus/1", `task ${task.id} lacks its frozen generator provenance`);
  splitCounts.set(expectedSplit, (splitCounts.get(expectedSplit) ?? 0) + 1);
  requireValue(task.independent_fixtures?.length === 4, `task ${task.id} must retain four independent fixtures`);
  requireValue(typeof task.chunk_library_identity === "string" && /^[0-9a-f]{64}$/.test(task.chunk_library_identity), `task ${task.id} lacks a full library identity`);
  requireValue((corpus.libraries ?? []).some((library) => library.family === task.family && library.identity === task.chunk_library_identity), `task ${task.id} references a different library identity`);
}
for (const family of criteria.corpus.families) {
  requireValue(familyCounts.get(family.id) === family.tasks, `family ${family.id} task count differs from criteria`);
  requireValue((corpus.libraries ?? []).some((library) => library.family === family.id && /^[0-9a-f]{64}$/.test(library.identity)), `family ${family.id} lacks an exact library identity`);
}
requireValue(splitCounts.get("train") === criteria.corpus.split.train_tasks, "train task count differs from criteria");
requireValue(splitCounts.get("validation") === criteria.corpus.split.validation_tasks, "validation task count differs from criteria");
requireValue(splitCounts.get("test") === criteria.corpus.split.test_tasks, "test task count differs from criteria");

if (failures.length > 0) {
  console.error(JSON.stringify({ schema: "tchunk-ranking-criteria-check/1", status: "invalid", failures }, null, 2));
  process.exitCode = 1;
} else {
  console.log(JSON.stringify({ schema: "tchunk-ranking-criteria-check/1", status: "criteria-valid", corpus_manifest_materialized: true, corpus_sha256: corpusSha256, required_task_count: criteria.corpus.task_count, family_counts: Object.fromEntries(familyCounts), split_counts: Object.fromEntries(splitCounts), modes: criteria.modes, test_tasks_per_seed: criteria.evaluation.test_tasks, paired_repetitions: criteria.evaluation.paired_repetitions, measurement_performed: false }, null, 2));
}
