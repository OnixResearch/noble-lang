use crate::differential::TestIds;
use noble_ngraph::cert::{self, ChainedAction, ChainedDerivation, Rule};
use noble_ngraph::ir::{Digest, Graph};

fn digest_term(digest: &Digest) -> String {
    let bytes = digest
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{bytes}]")
}

fn rule_term(rule: Rule) -> u8 {
    match rule {
        Rule::FoldAddConstants => 0,
        Rule::NeutralAddLeft => 1,
        Rule::NeutralMulRight => 2,
    }
}

fn action_term(action: &ChainedAction) -> String {
    match action {
        ChainedAction::Rewrite(step) => format!(
            ".rewrite ⟨{}, {}, {}, {}, 0x{:016X}, 0x{:016X}, 0x{:016X}⟩",
            rule_term(step.rule),
            step.node.0,
            step.operands[0].0,
            step.operands[1].0,
            step.first_bits,
            step.second_bits,
            step.claimed
        ),
        ChainedAction::PruneUnreachable => ".prune".to_owned(),
    }
}

fn transition_term(transition: &cert::ChainedStep) -> String {
    format!(
        "⟨{}, {}, {}⟩",
        action_term(&transition.action),
        digest_term(&transition.before),
        digest_term(&transition.after)
    )
}

fn fixture_term(name: &str, graph: Graph, proposer: cert::Proposer) -> String {
    noble_ngraph::check::check(&TestIds, graph.clone())
        .expect("the Rust fixture source is admitted");
    let (derivation, result): (ChainedDerivation, Graph) =
        cert::propose_chained(&TestIds, proposer, &graph).expect("bounded real Rust proposal");
    assert_eq!(derivation.revision, cert::CHAINED_REVISION);
    assert_eq!(
        cert::check_chained(&TestIds, &graph, &result, &derivation),
        Ok(()),
        "the exported chain is accepted by the Rust checker"
    );
    noble_ngraph::check::check(&TestIds, result.clone())
        .expect("the Rust candidate passes unchanged graph admission");
    let wire = cert::encode_chained(&derivation);
    let decoded = cert::decode_chained(&wire).expect("the actual emitted wire bytes decode");
    assert_eq!(
        decoded, derivation,
        "wire transport preserves every certificate field"
    );

    let mut tokens = wire.split(' ');
    let revision = tokens.next().expect("revision token");
    let model = tokens.next().expect("model token");
    let declared_steps = tokens.next().expect("step count token");
    let body = tokens
        .map(|token| format!("\"{token}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let transitions = decoded
        .steps
        .iter()
        .map(transition_term)
        .collect::<Vec<_>>()
        .join(",\n    ");
    format!(
        concat!(
            "def {name}WireBody : List String := [{body}]\n",
            "def {name}Certificate : ChainedDerivation where\n",
            "  revision := \"{revision}\"\n",
            "  model := \"{model}\"\n",
            "  left := {left}\n",
            "  right := {right}\n",
            "  steps := [{transitions}]\n",
            "theorem {name}_wire_decodes :\n",
            "    decodeChained \"{revision}\" \"{model}\" {declared_steps} {name}WireBody =\n",
            "      some {name}Certificate := by\n",
            "  decide\n\n"
        ),
        name = name,
        body = body,
        revision = revision,
        model = model,
        declared_steps = declared_steps,
        left = digest_term(&decoded.left),
        right = digest_term(&decoded.right),
        transitions = transitions
    )
}

fn render() -> String {
    let mut output = String::from(
        "import NobleFoundation.CertificateChained\n\nset_option maxRecDepth 100000\n\n/-!\n# Real Rust chained certificates parsed by the Lean model (issue #64 follow-up)\n\nGenerated from certificates proposed, encoded, decoded and accepted by the Rust checker. The exact ngraph-cert/2 body tokens are retained below; the Rust freshness test regenerates this file from the source fixtures. `*_wire_decodes` kernel-checks that the model parser returns the listed transitions and exact endpoint digests.\n\nNo correspondence theorem binds these bytes or the model parser to Rust `cert.rs`, the Rust graph evaluator, or the implementation of pruning. The independent Rust checker, Rust tests, Lean model and this finite corpus are separate evidence.\n-/\n\nnamespace NobleFoundation.CertificateChainedCorpus\n\nopen NobleFoundation.CertificateChained\nopen NobleFoundation.Certificate\n\n"
    );
    output.push_str(&fixture_term(
        "rustChainedFixedPoint",
        super::chain_graph(),
        cert::Proposer::FixedPoint,
    ));
    output.push_str(&fixture_term(
        "rustOrderedFirstEligible",
        super::ordered_graph(),
        cert::Proposer::FirstEligible,
    ));
    output.push_str(&fixture_term(
        "rustIdentity",
        super::differential::symbolic_sum(),
        cert::Proposer::Identity,
    ));
    output.push_str("end NobleFoundation.CertificateChainedCorpus\n");
    output
}

pub(super) fn lean_file() -> String {
    render()
}
