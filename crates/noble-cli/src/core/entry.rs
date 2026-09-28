mod compilation;

pub fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let parsed = super::arguments::parse(arguments);
    let is_declared = parsed
        .as_ref()
        .is_ok_and(|options| options.declared_modules);
    let result = parsed.and_then(execute);
    match result {
        Ok(exit) => std::process::ExitCode::from(exit),
        Err(error) => {
            let report = super::output::Report::failure(&error, is_declared);
            if let Err(write_error) = super::output::print(&report) {
                eprintln!("noble: {}", write_error.message);
            }
            std::process::ExitCode::from(error.exit())
        }
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; execute preserves source refusals, terminal engine outcomes and artifact/stdout failures as reports and exit statuses. External input and I/O failures must not become assertion panics."
)]
fn execute(options: super::arguments::Options) -> Result<u8, super::output::Failure> {
    if options.compile_only {
        return compilation::compile(&options);
    }
    if let Some(directory) = &options.emit {
        attempt!(std::fs::create_dir(directory).map_err(super::framing::io_error));
    }
    let mut session = attempt!(super::Session::new(&options));
    if let Some(path) = &options.source {
        let source = attempt!(super::framing::read_file(path, options.limits.bytes));
        let report = attempt!(session.submit(&source, &options));
        attempt!(retain(&options, session.submissions, &source, &report));
        attempt!(super::output::print(&report));
        return Ok(report.exit());
    }
    let mut input = std::io::BufReader::new(std::io::stdin().lock());
    let mut exit = 0;
    while let Some(source) = attempt!(super::framing::read_submission(
        &mut input,
        options.framed,
        options.limits.bytes,
    )) {
        let report = attempt!(session.submit(&source, &options));
        attempt!(retain(&options, session.submissions, &source, &report));
        attempt!(super::output::print(&report));
        exit = exit.max(report.exit());
        if report.is_terminal() {
            break;
        }
    }
    Ok(exit)
}

fn retain(
    options: &super::arguments::Options,
    number: u64,
    source: &[u8],
    report: &super::output::Report,
) -> Result<(), super::output::Failure> {
    if let Some(directory) = &options.emit {
        attempt!(super::framing::write_new(
            &directory.join(std::format!("{number}.noble")),
            source
        ));
        attempt!(super::framing::write_new(
            &directory.join(std::format!("{number}.json")),
            report.json.as_bytes(),
        ));
    }
    Ok(())
}
