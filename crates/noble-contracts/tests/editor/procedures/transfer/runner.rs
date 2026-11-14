use super::*;

pub(super) fn run_arm_seed(
    arm: &str,
    seed_index: usize,
    control: &ControlSelection,
) -> Result<(String, ArmMetrics, bool), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(super::revision(7));
    let mut sources = Vec::new();
    let mut build_ns = 0u64;
    let mut sweeps = Vec::new();
    match arm {
        "no_library" => {
            sweeps.push(super::sweep::evaluate(SweepInput {
                namespace: &namespace,
                arm,
                seed_index,
                library: &library,
                sources: &sources,
                label: "empty",
                control,
            })?);
        }
        "frozen_hand_library" => {
            let records = vec![
                super::producer::task_record(SourceTask::Commuted, Producer::Chunk)?,
                super::producer::task_record(SourceTask::Subtract, Producer::Chunk)?,
            ];
            let started = Instant::now();
            let ids = library
                .admit_batch(&namespace, None, records)
                .map_err(|error| format!("hand-library acquisition: {error:?}"))?;
            build_ns = super::producer::elapsed_nanoseconds(started)?;
            sources.push(RetainedSource {
                task: SourceTask::Commuted,
                id: ids[0],
            });
            sources.push(RetainedSource {
                task: SourceTask::Subtract,
                id: ids[1],
            });
            sweeps.push(super::sweep::evaluate(SweepInput {
                namespace: &namespace,
                arm,
                seed_index,
                library: &library,
                sources: &sources,
                label: "final",
                control,
            })?);
        }
        "accumulated_learned_library" => {
            sweeps.push(super::sweep::evaluate(SweepInput {
                namespace: &namespace,
                arm,
                seed_index,
                library: &library,
                sources: &sources,
                label: "zero",
                control,
            })?);
            sources.push(super::producer::admit_source(
                &mut library,
                &namespace,
                SourceTask::Commuted,
                Producer::Synthesis,
                &mut build_ns,
            )?);
            sweeps.push(super::sweep::evaluate(SweepInput {
                namespace: &namespace,
                arm,
                seed_index,
                library: &library,
                sources: &sources,
                label: "one",
                control,
            })?);
            sources.push(super::producer::admit_source(
                &mut library,
                &namespace,
                SourceTask::Subtract,
                Producer::Synthesis,
                &mut build_ns,
            )?);
            sweeps.push(super::sweep::evaluate(SweepInput {
                namespace: &namespace,
                arm,
                seed_index,
                library: &library,
                sources: &sources,
                label: "two",
                control,
            })?);
        }
        "size_matched_nontransfer" => {
            sweeps.push(super::sweep::evaluate(SweepInput {
                namespace: &namespace,
                arm,
                seed_index,
                library: &library,
                sources: &sources,
                label: "zero",
                control,
            })?);
            for task in control.tasks {
                sources.push(super::producer::admit_source(
                    &mut library,
                    &namespace,
                    task,
                    Producer::Synthesis,
                    &mut build_ns,
                )?);
                let label = if sources.len() == 1 { "one" } else { "two" };
                sweeps.push(super::sweep::evaluate(SweepInput {
                    namespace: &namespace,
                    arm,
                    seed_index,
                    library: &library,
                    sources: &sources,
                    label,
                    control,
                })?);
            }
        }
        _ => return Err(format!("unknown transfer arm {arm}")),
    }
    super::report::build_row(ReportInput {
        namespace: &namespace,
        arm,
        seed: SEEDS[seed_index],
        library: &library,
        sources: &sources,
        build_ns,
        sweeps: &sweeps,
        control,
    })
}
