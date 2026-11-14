import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const criteriaUrl = new URL("./criteria-v1.json", import.meta.url);
const criteria = JSON.parse(readFileSync(criteriaUrl, "utf8"));
const failures = [];
const requireValue = (condition, message) => {
  if (!condition) failures.push(message);
};

requireValue(criteria.schema === "tchunk-ranking-criteria/1", "criteria schema must be tchunk-ranking-criteria/1");
requireValue(criteria.corpus?.schema === "tchunk-ranking-corpus/1", "corpus schema must be tchunk-ranking-corpus/1");
requireValue(criteria.corpus?.task_count === 480, "corpus must contain exactly 480 tasks");
requireValue(criteria.corpus?.families?.length === 5, "corpus must have five frozen families");
requireValue(criteria.corpus.families.every((family) => family.tasks === 96 && family.blocks === 12 && family.tasks_per_block === 8), "each family must have 96 tasks in twelve blocks of eight");
requireValue(criteria.corpus.families.reduce((sum, family) => sum + family.tasks, 0) === criteria.corpus.task_count, "family counts must cover the corpus");
requireValue(criteria.corpus.split?.train_tasks === 280 && criteria.corpus.split.validation_tasks === 80 && criteria.corpus.split.test_tasks === 120, "frozen split must be 280/80/120");
requireValue(criteria.corpus.split?.split_unit === "family-provenance-block" && criteria.corpus.split.row_randomization === false, "split must be by provenance block, never randomized rows");
requireValue(criteria.features?.lanes === 64 && criteria.features.revision === "tchunk-features/1", "feature schema must contain exactly 64 lanes at the frozen revision");
requireValue(criteria.scorer?.input_slots === 64 && criteria.scorer.trainable_param_slots === 65, "scorer must bind the fixed 64-input/65-parameter shape");
requireValue(criteria.scorer?.optimizer?.steps === 256 && criteria.scorer.optimizer.batch_size === 32, "training steps and batch size must match the preregistration");
requireValue(criteria.scorer?.optimizer?.training_seeds?.length === 3, "exactly three training seeds must be frozen");
requireValue(criteria.modes?.join(",") === "primitive-only,fixed-chunk-shortlex,learned-ranking,splitmix64-shuffled-control", "all four matched modes must remain frozen and ordered");
requireValue(criteria.evaluation?.test_tasks === 120 && criteria.evaluation.paired_repetitions === 10 && criteria.evaluation.seeds.length === 10, "evaluation must include all 120 test tasks and ten paired seeds");
requireValue(criteria.decision?.default === "NO-GO" && criteria.decision.go_requires_all === true, "GO must require every frozen decision condition");
requireValue(criteria.ranking?.authority === "queue-permutation-only", "the scorer must remain a queue-permutation-only signal");
requireValue(criteria.ranking?.exhaustion === "fail-closed-exhausted-no-fallback-work-no-partial-publication", "exhaustion must remain fail-closed");
requireValue(criteria.system_one?.consumer_owns_observation === true, "System One observations must be consumer-owned");
requireValue(criteria.system_one?.procedure_identity === "exact-body-and-dependency-identity-never-signature-only", "procedure identity must include exact body and dependencies");
requireValue(criteria.scope_limit?.includes("no-branch-skipping-claim"), "compiled-select scope limit must remain explicit");

if (failures.length > 0) {
  console.error(JSON.stringify({ schema: "tchunk-ranking-criteria-check/1", status: "invalid", failures }, null, 2));
  process.exitCode = 1;
} else {
  console.log(JSON.stringify({ schema: "tchunk-ranking-criteria-check/1", status: "valid", task_count: criteria.corpus.task_count, modes: criteria.modes, test_tasks_per_seed: criteria.evaluation.test_tasks, paired_repetitions: criteria.evaluation.paired_repetitions, measurement_performed: false }, null, 2));
}

export const sourcePath = fileURLToPath(criteriaUrl);
