impl super::Session {
    pub(super) fn export_proof(
        &self,
        is_refutation: bool,
    ) -> Result<std::string::String, super::super::output::Failure> {
        let sources = self.workspace.temporary.path.join("export-source");
        let artifacts = self.workspace.temporary.path.join("export-artifacts");
        attempt!(super::super::artifacts::create_directory(&sources));
        attempt!(super::super::artifacts::create_directory(&artifacts));
        attempt!(super::super::artifacts::write_source(
            &sources.join("NobleProducer.lean"),
            include_bytes!("../../producer.lean"),
        ));
        let selected = if is_refutation {
            "MC1Proof.refutation"
        } else {
            "MC1Proof.proof"
        };
        let source =
            std::format!("import NobleProducer\nimport MC1Proof\n\n#mc1_export {selected}\n");
        attempt!(super::super::artifacts::write_source(
            &sources.join("ExportProof.lean"),
            source.as_bytes()
        ));
        attempt!(self.compile_exporter(&sources, &artifacts));
        let wire = artifacts.join("MC1Proof.json");
        attempt!(super::super::artifacts::write_source(&wire, &[]));
        let mounts = std::vec![
            super::super::artifacts::read_mount(&sources, "/inputs"),
            super::super::artifacts::read_mount(&artifacts, "/out"),
            super::super::artifacts::read_mount(&self.workspace.library, "/library"),
            super::super::artifacts::read_mount(&self.workspace.obligation, "/obligation"),
            super::super::artifacts::read_mount(&self.workspace.producer, "/producer"),
            crate::sandbox::Mount {
                host: wire.clone(),
                guest: "/out/MC1Proof.json".into(),
                writable: true
            },
        ];
        let transcript = attempt!(self.sandbox.run(
            &mounts,
            "/out:/library:/obligation:/producer",
            &std::vec![
                "-R".into(),
                "/inputs".into(),
                "-j".into(),
                "2".into(),
                "/inputs/ExportProof.lean".into()
            ],
            self.deadline,
        ));
        attempt!(super::require_success(
            transcript,
            "proof-export-rejected",
            "proof could not be exported as bounded declarations",
        ));
        std::string::String::from_utf8(attempt!(super::super::artifacts::read_bounded(
            &wire,
            super::super::PROOF_WIRE_LIMIT,
            "proof-wire"
        )))
        .map_err(|_| {
            super::super::output::Failure::error(
                "proof-wire-encoding",
                "declarations must be UTF-8 JSON".into(),
            )
        })
    }

    fn compile_exporter(
        &self,
        sources: &std::path::Path,
        artifacts: &std::path::Path,
    ) -> Result<(), super::super::output::Failure> {
        let object = std::path::Path::new("NobleProducer.olean");
        let writable = attempt!(super::super::artifacts::output_slots(artifacts, object));
        let mounts = std::vec![
            super::super::artifacts::read_mount(sources, "/inputs"),
            super::super::artifacts::read_mount(artifacts, "/out")
        ];
        let mounts = attempt!(super::super::artifacts::output_mounts(
            mounts, artifacts, &writable, "/out"
        ));
        let transcript = attempt!(self.compile_module(
            mounts,
            "/out",
            super::compilation::Job {
                source: "/inputs/NobleProducer.lean",
                root: "/inputs",
                output: "/out/NobleProducer.olean",
            }
        ));
        attempt!(super::require_success(
            transcript,
            "proof-export-build",
            "declaration exporter compilation failed",
        ));
        super::super::artifacts::finish_outputs(artifacts, object, &writable)
    }

    pub(super) fn replay(
        &self,
        wire: &str,
        is_refutation: bool,
    ) -> Result<std::vec::Vec<std::string::String>, super::super::output::Failure> {
        attempt!(super::super::artifacts::write_source(
            &self.workspace.consumer.join("MC1Proof.json"),
            wire.as_bytes()
        ));
        // Only the consumer's fresh bounded JSON file is mounted at /producer.
        // No producer or exporter object directory appears in this process.
        let mounts = std::vec![
            super::super::artifacts::read_mount(&self.workspace.library, "/library"),
            super::super::artifacts::read_mount(&self.workspace.obligation, "/obligation"),
            super::super::artifacts::read_mount(&self.workspace.consumer, "/producer"),
            super::super::artifacts::read_mount(&self.workspace.checker, "/checker"),
        ];
        let transcript = attempt!(self.sandbox.run(
            &mounts,
            "/library:/obligation:/producer",
            &std::vec![
                "-R".into(),
                "/checker".into(),
                "-j".into(),
                "2".into(),
                "--run".into(),
                "/checker/NobleConsumer.lean".into(),
                if is_refutation {
                    "refutation".into()
                } else {
                    "proof".into()
                },
                "/producer/MC1Proof.json".into(),
            ],
            self.deadline,
        ));
        let code = rejection_code(&transcript.stderr);
        let transcript = attempt!(super::require_success(
            transcript,
            code,
            "independent kernel replay, theorem type, or axiom policy rejected the evidence",
        ));
        acceptance_output(&transcript.stdout)
    }
}

fn rejection_code(stderr: &str) -> &'static str {
    [
        ("MC1_THEOREM_MISSING", "theorem-missing"),
        ("MC1_TYPE_MISMATCH", "theorem-type-mismatch"),
        ("MC1_AXIOM_REJECTED", "forbidden-axiom"),
        ("MC1_DEPENDENCY_LIMIT", "dependency-limit"),
    ]
    .iter()
    .find(|(marker, _)| stderr.contains(*marker))
    .map_or("independent-recheck-rejected", |(_, code)| *code)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; acceptance_output validates the checker acceptance marker and at most three distinct approved axiom lines. Missing markers, unexpected lines and duplicate or forbidden axioms return consumer-protocol Failure instead of panicking on subprocess output."
)]
fn acceptance_output(
    output: &str,
) -> Result<std::vec::Vec<std::string::String>, super::super::output::Failure> {
    let mut lines = output.lines();
    if lines.next() != Some("NOBLE-MC1-ACCEPT") {
        return Err(super::super::output::Failure::error(
            "consumer-protocol",
            "trusted checker did not report acceptance".into(),
        ));
    }
    let mut axioms = std::vec::Vec::with_capacity(3);
    attempt!(lines.try_for_each(|line| {
        let axiom = attempt!(line.strip_prefix("axiom:").ok_or_else(|| {
            super::super::output::Failure::error(
                "consumer-protocol",
                "unexpected trusted-checker output".into(),
            )
        }));
        if !["propext", "Classical.choice", "Quot.sound"].contains(&axiom) || axioms.len() >= 3 {
            return Err(super::super::output::Failure::error(
                "consumer-protocol",
                "unapproved or duplicate axiom output".into(),
            ));
        }
        if axioms.iter().any(|existing| existing == axiom) {
            return Err(super::super::output::Failure::error(
                "consumer-protocol",
                "duplicate axiom output".into(),
            ));
        }
        axioms.push(axiom.into());
        Ok::<(), super::super::output::Failure>(())
    }));
    Ok(axioms)
}
