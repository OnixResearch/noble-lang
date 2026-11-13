use super::programs::{self, Program};
use crate::common;
use noble_ngraph::check::{self, CheckedGraph};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::ir::Graph;
use noble_ngraph::sem::Value;
use noble_ngraph::snapshot::ParamSnapshot;

pub struct CandidateChecker {
    program: Program,
    checked: CheckedGraph,
    compiled: Compiled,
    weights: ParamSnapshot,
}

impl CandidateChecker {
    /// Creates an emitter-independent checker from a candidate graph and its weights.
    pub fn new(program: Program, graph: Graph, weight_values: &[f32]) -> Result<Self, String> {
        let checked =
            check::check(&common::IDS, graph).map_err(|error| format!("graph: {error:?}"))?;
        let weights = ParamSnapshot::new(&common::IDS, &checked, weight_values)
            .map_err(|error| format!("weights: {error:?}"))?;
        let compiled = Compiled::new(&common::IDS, &checked);
        Ok(Self {
            program,
            checked,
            compiled,
            weights,
        })
    }

    pub fn identities(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        (
            self.checked.id().to_vec(),
            self.weights.id().to_vec(),
            self.weights.bytes().to_vec(),
        )
    }

    pub fn one_step(&self, state: &[u64], input: &[u64]) -> Result<Vec<u64>, String> {
        let source = programs::source_transition(self.program, state, input)
            .ok_or_else(|| "state/input shape refused".to_owned())?;
        let words = programs::inputs_for(self.program, state, input)
            .ok_or_else(|| "state/input shape refused".to_owned())?;
        let encoded: Vec<Value> = match self.program {
            Program::Accumulator => encode_word(words[0])
                .into_iter()
                .zip(encode_word(words[1]))
                .flat_map(|(left, right)| [left, right])
                .collect(),
            Program::StackSwap => words.iter().flat_map(|word| encode_word(*word)).collect(),
        };
        let mut compiled_control = Control::default();
        let mut reference_control = Control::default();
        let compiled_obs = self
            .compiled
            .evaluate(&encoded, Some(&self.weights), &mut compiled_control)
            .obs;
        let reference_obs = eval::reference(
            &self.checked,
            &encoded,
            Some(&self.weights),
            &mut reference_control,
        )
        .obs;
        let (Obs::Normal(compiled_values), Obs::Normal(reference_values)) =
            (compiled_obs, reference_obs)
        else {
            return Err("candidate or reference refused a valid encoded input".to_owned());
        };
        if compiled_values != reference_values {
            return Err("compiled/reference observation mismatch".to_owned());
        }
        let decoded = decode_words(&compiled_values)?;
        if decoded != source {
            return Err(format!(
                "source transition mismatch: expected {source:?}, got {decoded:?}"
            ));
        }
        Ok(decoded)
    }
}

fn encode_word(word: u64) -> [Value; 64] {
    core::array::from_fn(|bit| Value::f32(((word >> bit) & 1) as f32))
}

fn decode_words(values: &[Value]) -> Result<Vec<u64>, String> {
    if values.len() % 64 != 0 {
        return Err(format!(
            "decoder length {} is not a multiple of 64",
            values.len()
        ));
    }
    values.chunks_exact(64).map(decode_word).collect()
}

fn decode_word(values: &[Value]) -> Result<u64, String> {
    values.iter().enumerate().try_fold(0, |word, (bit, value)| {
        let Value::F32(bits) = value else {
            return Err(format!("decoder received non-F32 at bit {bit}"));
        };
        match f32::from_bits(*bits) {
            0.0 => Ok(word),
            1.0 => Ok(word | (1u64 << bit)),
            value => Err(format!("decoder received non-bit {value:?} at bit {bit}")),
        }
    })
}
