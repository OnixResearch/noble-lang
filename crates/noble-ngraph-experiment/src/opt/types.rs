//! Shared wire, refusal and transition types for `ngraph-opt/1`.

/// Domain-separated 32-byte identity used by frozen optimizer artifacts.
pub type Digest = [u8; 32];

/// Numeric contract used by the issue #58 reference transition.
pub const NUMERIC_SEMANTICS: &str =
    "Binary32/RNE/two-roundings/no-FMA/no-reassociation/gradual-underflow/nonfinite-refused/v1";

/// A candidate/refinement refusal class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefusalClass {
    /// The envelope or transition descriptor does not have the published shape.
    Malformed,
    /// The candidate asks for an operation, shape, effect or value outside the profile.
    Unsupported,
    /// A measured resource exceeds the fixed profile limit.
    Exhausted,
    /// The checker failed independently of candidate-controlled input.
    Internal,
}

impl RefusalClass {
    /// Stable receipt spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Malformed => "malformed",
            Self::Unsupported => "unsupported",
            Self::Exhausted => "exhausted",
            Self::Internal => "internal",
        }
    }
}

/// First refusal with an exact JSON-pointer location and observed value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// Published refusal class.
    pub class: RefusalClass,
    /// JSON pointer to the first offending field.
    pub location: String,
    /// Exact observed value or a bounded parse/error description.
    pub observed: String,
}

impl Refusal {
    /// Constructs a refusal without replacing the class or source location.
    #[must_use]
    pub fn new(
        class: RefusalClass,
        location: impl Into<String>,
        observed: impl Into<String>,
    ) -> Self {
        Self {
            class,
            location: location.into(),
            observed: observed.into(),
        }
    }
}

/// Classification of a nonfinite binary32 bit pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NonFinite {
    /// Exponent is all ones and the fraction is zero.
    Infinity,
    /// Exponent and fraction are nonzero/all-ones respectively.
    NaN,
}

/// Classifies infinity and NaN from the exact bits; signed zeros and every subnormal are finite.
#[must_use]
pub const fn classify_bits(bits: u32) -> Option<NonFinite> {
    if bits & 0x7f80_0000 != 0x7f80_0000 {
        None
    } else if bits & 0x007f_ffff == 0 {
        Some(NonFinite::Infinity)
    } else {
        Some(NonFinite::NaN)
    }
}

/// The fixed nine-byte host metadata input. It is not readable by an opcode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepDescriptor {
    /// Descriptor version (`1` only).
    pub version: u8,
    /// Zero-based update index.
    pub step_index: u32,
    /// Zero-based selected data-row index.
    pub batch_index: u32,
}

impl StepDescriptor {
    /// Constructs descriptor bytes after validating the only supported version.
    pub fn new(version: u8, step_index: u32, batch_index: u32) -> Result<Self, Refusal> {
        if version != 1 {
            return Err(Refusal::new(
                RefusalClass::Malformed,
                "$.inputs.step.version",
                version.to_string(),
            ));
        }
        Ok(Self {
            version,
            step_index,
            batch_index,
        })
    }

    /// Exact little-endian serialization used in snapshot provenance.
    #[must_use]
    pub const fn to_bytes(self) -> [u8; 9] {
        let step = self.step_index.to_le_bytes();
        let batch = self.batch_index.to_le_bytes();
        [
            self.version,
            step[0],
            step[1],
            step[2],
            step[3],
            batch[0],
            batch[1],
            batch[2],
            batch[3],
        ]
    }
}

/// Runtime inputs supplied by the host, all represented by exact binary32 bits.
pub struct TransitionInput<'a> {
    /// Parameters in serialized lane order.
    pub parameters: &'a [u32],
    /// Gradients in serialized lane order.
    pub gradients: &'a [u32],
    /// Learning-rate scalar bits.
    pub eta: u32,
    /// Host-only step metadata.
    pub step: StepDescriptor,
    /// Serialized `State0`; must be empty.
    pub state: &'a [u8],
}

/// One finite candidate transition result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionOutput {
    /// Result parameter bits in lane order.
    pub parameters: Vec<u32>,
    /// Serialized `State0`, always empty for this envelope.
    pub state: Vec<u8>,
    /// Measured scalar mul/sub lane invocations.
    pub work: u64,
}
