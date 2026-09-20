pub(super) struct Job<'a> {
    pub(super) source: &'a str,
    pub(super) root: &'a str,
    pub(super) output: &'a str,
}

impl super::Session {
    pub(super) fn compile_module(
        &self,
        mut mounts: std::vec::Vec<crate::sandbox::Mount>,
        search_path: &str,
        job: Job<'_>,
    ) -> Result<crate::sandbox::Transcript, super::super::output::Failure> {
        mounts.push(super::super::artifacts::read_mount(
            &self.workspace.checker,
            "/checker",
        ));
        self.sandbox.run(
            &mounts,
            search_path,
            &std::vec![
                "-R".into(),
                "/checker".into(),
                "-j".into(),
                "2".into(),
                "--run".into(),
                "/checker/NobleCompile.lean".into(),
                job.source.into(),
                job.root.into(),
                job.output.into(),
            ],
            self.deadline,
        )
    }

    pub(super) fn compile_library(
        &self,
        library: &super::super::rules::Library,
    ) -> Result<(), super::super::output::Failure> {
        let mut built = std::vec![false; library.modules.len()];
        let mut built_count = 0;
        while built_count < library.modules.len() {
            let mut has_progress = false;
            attempt!(library
                .modules
                .iter()
                .enumerate()
                .try_for_each(|(index, module)| {
                    if built[index] || !can_compile(library, module, &built) {
                        return Ok(());
                    }
                    attempt!(self.compile_rule(module));
                    built[index] = true;
                    built_count += 1;
                    has_progress = true;
                    Ok::<(), super::super::output::Failure>(())
                }));
            if !has_progress {
                return Err(super::super::output::Failure::unsupported(
                    "library-import-cycle",
                    "rule-library local imports cannot be ordered".into(),
                ));
            }
        }
        Ok(())
    }

    fn compile_rule(
        &self,
        module: &super::super::rules::Module,
    ) -> Result<(), super::super::output::Failure> {
        let output = module.relative.with_extension("olean");
        let writable = attempt!(super::super::artifacts::output_slots(
            &self.workspace.library,
            &output
        ));
        let mounts = std::vec![
            super::super::artifacts::read_mount(&self.workspace.sources, "/inputs"),
            super::super::artifacts::read_mount(&self.workspace.library, "/out"),
        ];
        let mounts = attempt!(super::super::artifacts::output_mounts(
            mounts,
            &self.workspace.library,
            &writable,
            "/out"
        ));
        let source = attempt!(super::super::artifacts::guest_path(
            "/inputs",
            &module.relative
        ));
        let destination = attempt!(super::super::artifacts::guest_path("/out", &output));
        let transcript = attempt!(self.compile_module(
            mounts,
            "/out",
            Job {
                source: &source,
                root: "/inputs",
                output: &destination,
            }
        ));
        attempt!(super::require_success(
            transcript,
            "rule-library-build",
            "consumer rule-library compilation failed",
        ));
        super::super::artifacts::finish_outputs(&self.workspace.library, &output, &writable)
    }

    pub(super) fn compile_obligation(
        &self,
        obligation: &str,
    ) -> Result<(), super::super::output::Failure> {
        let expected = &self.workspace.obligation;
        attempt!(super::super::artifacts::write_source(
            &expected.join("MC1Obligation.lean"),
            obligation.as_bytes()
        ));
        let output = std::path::Path::new("MC1Obligation.olean");
        let writable = attempt!(super::super::artifacts::output_slots(expected, output));
        let mounts = std::vec![
            super::super::artifacts::read_mount(&self.workspace.library, "/library"),
            super::super::artifacts::read_mount(expected, "/obligation"),
        ];
        let mounts = attempt!(super::super::artifacts::output_mounts(
            mounts,
            expected,
            &writable,
            "/obligation"
        ));
        let transcript = attempt!(self.compile_module(
            mounts,
            "/library:/obligation",
            Job {
                source: "/obligation/MC1Obligation.lean",
                root: "/obligation",
                output: "/obligation/MC1Obligation.olean",
            }
        ));
        attempt!(super::require_success(
            transcript,
            "obligation-build",
            "consumer-generated obligation was not accepted by Lean",
        ));
        super::super::artifacts::finish_outputs(expected, output, &writable)
    }

    pub(super) fn compile_submission(
        &self,
        proof: &str,
    ) -> Result<(), super::super::output::Failure> {
        let producer = &self.workspace.producer;
        attempt!(super::super::artifacts::write_source(
            &self.workspace.submission.join("MC1Proof.lean"),
            proof.as_bytes()
        ));
        let output = std::path::Path::new("MC1Proof.olean");
        let writable = attempt!(super::super::artifacts::output_slots(producer, output));
        let mounts = std::vec![
            super::super::artifacts::read_mount(&self.workspace.library, "/library"),
            super::super::artifacts::read_mount(&self.workspace.obligation, "/obligation"),
            super::super::artifacts::read_mount(&self.workspace.submission, "/inputs"),
            super::super::artifacts::read_mount(producer, "/out"),
        ];
        let mounts = attempt!(super::super::artifacts::output_mounts(
            mounts, producer, &writable, "/out"
        ));
        let transcript = attempt!(self.compile_module(
            mounts,
            "/library:/obligation",
            Job {
                source: "/inputs/MC1Proof.lean",
                root: "/inputs",
                output: "/out/MC1Proof.olean",
            }
        ));
        attempt!(super::require_success(
            transcript,
            "proof-rejected",
            "proof elaboration failed; this is not a disproof",
        ));
        super::super::artifacts::finish_outputs(producer, output, &writable)
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; can_compile traverses runtime Vec imports with Iterator::all/position and String equality to query the built dependency set. Those trait operations are not const on the pinned compiler; reassess when their const APIs are available."
)]
fn can_compile(
    library: &super::super::rules::Library,
    module: &super::super::rules::Module,
    built: &[bool],
) -> bool {
    module.imports.iter().all(|name| {
        library
            .modules
            .iter()
            .position(|dependency| dependency.name == *name)
            .is_none_or(|index| built[index])
    })
}
