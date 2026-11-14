use super::*;

#[path = "run/baselines.rs"]
mod baselines;
use baselines::*;
#[path = "run/learned.rs"]
mod learned;
use learned::*;
#[path = "run/shuffled.rs"]
mod shuffled;
use shuffled::*;

struct ScorerStats {
    scored: Vec<ScoredCandidate>,
    score_calls: u32,
    scorer_elapsed_ns: u64,
    score_evaluated_nodes: u64,
    score_scratch_peak_bytes: u64,
    model_load_ns: u64,
}

struct ModeOutcome {
    response: chunks::Response,
    attempts: Vec<chunks::CandidateOutcome>,
    disposition: Option<chunks::RankingDisposition>,
    invalid_scores: u32,
    ranked_candidates: Vec<chunks::CandidateId>,
    candidate_row_count: usize,
    ranking_identity: Option<chunks::RankingIdentity>,
    scorer: ScorerStats,
}

impl ModeOutcome {
    fn fixed(response: chunks::Response, attempts: Vec<chunks::CandidateOutcome>,
        candidate_row_count: usize) -> Self {
        let ranked_candidates = attempts.iter().map(|attempt| attempt.candidate).collect();
        Self {
            response, attempts, disposition: None, invalid_scores: 0,
            ranked_candidates, candidate_row_count, ranking_identity: None,
            scorer: ScorerStats { scored: Vec::new(), score_calls: 0, scorer_elapsed_ns: 0,
                score_evaluated_nodes: 0, score_scratch_peak_bytes: 0, model_load_ns: 0 },
        }
    }

    fn ranked(ranked: chunks::RankedResponse, attempts: Vec<chunks::CandidateOutcome>,
        scorer: ScorerStats) -> Self {
        debug_assert!(ranked.invalid_scores <= u32::try_from(ranked.ranked_candidates.len()).expect("bounded queue"));
        debug_assert!(ranked.ranked_candidates.len() <= usize::try_from(chunks::MAX_CANDIDATES).expect("fixed profile"));
        let candidate_row_count = ranked.ranked_candidates.len();
        Self {
            response: ranked.response,
            attempts,
            disposition: Some(ranked.disposition),
            invalid_scores: ranked.invalid_scores,
            ranked_candidates: ranked.ranked_candidates,
            candidate_row_count,
            ranking_identity: ranked.identity,
            scorer,
        }
    }
}

pub(crate) fn run_completion_mode(run: EvaluationRun<'_>) -> Result<Value, String> {
    debug_assert!(run.metadata.spec.family < FAMILY_NAMES.len());
    debug_assert!(run.metadata.library.entries().len()
        <= usize::try_from(chunks::MAX_LIBRARY_ENTRIES).expect("fixed profile"));
    let started = std::time::Instant::now();
    if matches!(run.metadata.mode, EvaluationMode::PrimitiveOnly) {
        return run_primitive(&run, started);
    }
    let result = match run.metadata.mode {
        EvaluationMode::PrimitiveOnly => unreachable!("primitive mode returns above"),
        EvaluationMode::FixedChunkShortlex => run_fixed(&run),
        EvaluationMode::LearnedRanking => run_learned(&run)?,
        EvaluationMode::ShuffledControl => run_shuffled(&run)?,
    };
    let ModeOutcome { response, attempts, disposition, invalid_scores,
        ranked_candidates, candidate_row_count, ranking_identity, scorer } = result;
    let ScorerStats { scored, score_calls, scorer_elapsed_ns, score_evaluated_nodes,
        score_scratch_peak_bytes, model_load_ns } = scorer;
    evaluation_row(EvaluationRecord {
        metadata: run.metadata, response, attempts, disposition, invalid_scores,
        ranked_candidates, candidate_row_count, ranking_identity, scored, score_calls,
        scorer_elapsed_ns, score_evaluated_nodes, score_scratch_peak_bytes, model_load_ns,
        model_payload_bytes: if matches!(run.metadata.mode, EvaluationMode::LearnedRanking) {
            run.model_payload_bytes
        } else { 0 },
        started,
    })
}
