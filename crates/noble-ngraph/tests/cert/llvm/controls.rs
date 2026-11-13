use super::*;

pub(super) struct ControlMutation {
    artifact: &'static str,
    class: &'static str,
    text: String,
    refusal: Refusal,
    /// Whether the changed text is syntactically in the modelled instruction subset.
    model: bool,
}

/// Replaces the first occurrence of `from`; the mutation must name a real site.
fn replaced(text: &str, from: &str, to: &str) -> String {
    assert!(
        text.contains(from),
        "the control names an occurrence of {from:?}"
    );
    text.replacen(from, to, 1)
}

fn operand_changes(all: &[Artifact]) -> Vec<ControlMutation> {
    let subtractive = artifact_named(all, "subtractive");
    let negative = artifact_named(all, "negative");
    let shared = artifact_named(all, "shared");
    vec![
        ControlMutation {
            artifact: "subtractive",
            class: "operand-swapped",
            text: replaced(&subtractive.text, "sub i64 %a0, %a1", "sub i64 %a1, %a0"),
            refusal: Refusal::Operands,
            model: true,
        },
        ControlMutation {
            artifact: "negative",
            class: "constant-changed",
            text: replaced(&negative.text, ", -3", ", -2"),
            refusal: Refusal::Operands,
            model: true,
        },
        ControlMutation {
            artifact: "shared",
            class: "negative-zero",
            text: replaced(&shared.text, ", -1", ", -0"),
            refusal: Refusal::Operand,
            model: false,
        },
        ControlMutation {
            artifact: "shared",
            class: "return-changed",
            text: replaced(
                &shared.text,
                &format!("  ret i64 {}", operand_text(shared.module.output)),
                "  ret i64 %v3",
            ),
            refusal: Refusal::Output,
            model: true,
        },
    ]
}

fn instruction_changes(all: &[Artifact]) -> Vec<ControlMutation> {
    let wrapping = artifact_named(all, "wrapping");
    let shared = artifact_named(all, "shared");
    vec![
        ControlMutation {
            artifact: "wrapping",
            class: "flag-nsw",
            text: replaced(&wrapping.text, " = add i64 ", " = add nsw i64 "),
            refusal: Refusal::Flags,
            model: false,
        },
        ControlMutation {
            artifact: "wrapping",
            class: "flag-nuw",
            text: replaced(&wrapping.text, " = mul i64 ", " = mul nuw i64 "),
            refusal: Refusal::Flags,
            model: false,
        },
        ControlMutation {
            artifact: "shared",
            class: "opcode-unsupported",
            text: replaced(&shared.text, " = add i64 ", " = xor i64 "),
            refusal: Refusal::Opcode,
            model: false,
        },
        ControlMutation {
            artifact: "shared",
            class: "instruction-call",
            text: replaced(
                &shared.text,
                "  ret i64",
                "  call void @llvm.trap()\n  ret i64",
            ),
            refusal: Refusal::Instruction,
            model: false,
        },
    ]
}

fn interface_changes(all: &[Artifact]) -> Vec<ControlMutation> {
    let wrapping = artifact_named(all, "wrapping");
    vec![
        ControlMutation {
            artifact: "wrapping",
            class: "attribute-dropped",
            text: replaced(&wrapping.text, "i64 noundef %a1", "i64 %a1"),
            refusal: Refusal::Interface,
            model: false,
        },
        ControlMutation {
            artifact: "wrapping",
            class: "convention-named",
            text: replaced(
                &wrapping.text,
                "define i64 @noble_i64",
                "define ccc i64 @noble_i64",
            ),
            refusal: Refusal::Interface,
            model: false,
        },
        ControlMutation {
            artifact: "wrapping",
            class: "triple-changed",
            text: replaced(
                &wrapping.text,
                llvm::TARGET_TRIPLE,
                "aarch64-unknown-linux-gnu",
            ),
            refusal: Refusal::Target,
            model: false,
        },
        ControlMutation {
            artifact: "wrapping",
            class: "datalayout-changed",
            text: replaced(&wrapping.text, "n8:16:32:64-S128", "n8:16:32:64-S64"),
            refusal: Refusal::Target,
            model: false,
        },
    ]
}

fn structure_changes(all: &[Artifact]) -> Vec<ControlMutation> {
    let rewritten = artifact_named(all, "foldable-rewritten");
    let shared = artifact_named(all, "shared");
    let wrapping = artifact_named(all, "wrapping");
    vec![
        ControlMutation {
            artifact: "foldable",
            class: "artifact-substituted",
            text: rewritten.text.clone(),
            refusal: Refusal::NodeMap,
            model: true,
        },
        ControlMutation {
            artifact: "shared",
            class: "return-duplicated",
            text: replaced(&shared.text, "  ret i64", "  ret i64 %v3\n  ret i64"),
            refusal: Refusal::Return,
            model: false,
        },
        ControlMutation {
            artifact: "wrapping",
            class: "text-after-brace",
            text: format!("{}\n; trailing\n", wrapping.text.trim_end()),
            refusal: Refusal::Return,
            model: false,
        },
        ControlMutation {
            artifact: "shared",
            class: "instruction-missing",
            text: replaced(&shared.text, "  %v2 = mul i64 %a0, %a0\n", ""),
            refusal: Refusal::NodeMap,
            model: true,
        },
        ControlMutation {
            artifact: "shared",
            class: "destination-changed",
            text: replaced(&shared.text, "  %v3 = add i64", "  %v4 = add i64"),
            refusal: Refusal::NodeMap,
            model: true,
        },
    ]
}

pub(super) fn all_changes(all: &[Artifact]) -> Vec<ControlMutation> {
    let mut changes = operand_changes(all);
    changes.extend(instruction_changes(all));
    changes.extend(interface_changes(all));
    changes.extend(structure_changes(all));
    changes
}

/// What the lane observed for one control: the refusal it raised, where it stopped, and whether the
/// Lean model can state the same refusal.
pub(super) struct ControlRecord {
    pub(super) artifact: &'static str,
    pub(super) class: &'static str,
    pub(super) refusal: &'static str,
    pub(super) stage: &'static str,
    pub(super) model: bool,
    pub(super) model_refusal: &'static str,
    pub(super) mutated: Option<Module>,
    pub(super) text: String,
}

/// A deterministic test-only identity that binds retained text and its relative path.
pub(super) fn retained_identity(domain: &str, path: &str, text: &str) -> String {
    let mut hasher = TestIds.start();
    hasher.update(domain.as_bytes());
    hasher.update(&[0]);
    for part in [path.as_bytes(), text.as_bytes()] {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    noble_ngraph::ir::hex(&hasher.finish())
}

pub(super) fn control_path(artifact: &str, class: &str) -> String {
    format!("controls/{artifact}-{class}.ll")
}

pub(super) fn control_records(all: &[Artifact]) -> Vec<ControlRecord> {
    all_changes(all)
        .into_iter()
        .map(|control| {
            let held = artifact_named(all, control.artifact);
            let parsed = llvm::parse(&control.text);
            let stage = if parsed.is_ok() { "verify" } else { "parse" };
            let mutated = parsed.ok();
            assert_eq!(
                mutated.is_some(),
                control.model,
                "control {} has the parser/model boundary it declares",
                control.class
            );
            let observed =
                llvm::validate(&held.checked, &control.text).expect_err("the control is refused");
            assert_eq!(
                observed, control.refusal,
                "control {} on {} raises its own refusal",
                control.class, control.artifact
            );
            assert_eq!(stage, if control.model { "verify" } else { "parse" });
            ControlRecord {
                artifact: control.artifact,
                class: control.class,
                refusal: observed.name(),
                stage,
                model: control.model,
                model_refusal: observed.name(),
                mutated,
                text: control.text,
            }
        })
        .collect()
}

pub(super) fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

pub(super) fn json_list(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|item| json_string(item)).collect();
    format!("[{}]", quoted.join(", "))
}
