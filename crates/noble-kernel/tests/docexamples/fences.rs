//! Iterative source discovery and fenced-block scanning for documentation examples.

/// One fenced block found by the scan.
pub(super) struct Block {
    pub(super) file: std::path::PathBuf,
    pub(super) line: usize,
    pub(super) info: String,
    pub(super) body: String,
}

/// Strip one Rust doc-comment marker so fences scan like Markdown.
fn strip_doc(line: &str) -> &str {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("///") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("//!") {
        rest
    } else {
        trimmed
    }
}

/// Scan one file, retaining the original fence and line-number rules.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; scan_text accepts arbitrary source text, ignores unmatched fences and reports block-reservation failures through Result; assertions would impose a different scanner contract."
)]
fn scan_text(file: &std::path::Path, text: &str) -> Result<Vec<Block>, String> {
    let mut blocks = Vec::new();
    let mut open: Option<Block> = None;
    for (index, line) in text.lines().enumerate() {
        let stripped = strip_doc(line);
        if let Some(info) = stripped.strip_prefix("```") {
            match open.take() {
                Some(block) => {
                    #[expect(
                        clippy::needless_late_init,
                        reason = "Owner: noble-maintainers; pinned Octet recognizes fallible reservation assignments but skips let initializers and ? expansions; reassess when its finder supports both."
                    )]
                    let reservation;
                    reservation = blocks.try_reserve(1);
                    reservation.map_err(|problem| {
                        format!("cannot reserve a block for {}: {problem}", file.display())
                    })?;
                    blocks.push(block);
                }
                None => {
                    #[expect(
                        tigerstyle::raw_arithmetic_overflow,
                        reason = "Owner: noble-maintainers; each enumerated line consumes a source byte, so index is below text.len(), which is at most isize::MAX; the two-line offset fits usize."
                    )]
                    #[expect(
                        tigerstyle::allocation_in_loop,
                        reason = "Owner: noble-maintainers; every retained Block owns its independent body; String::new allocates no buffer, and one reusable string cannot back simultaneously retained blocks."
                    )]
                    let block = Block {
                        file: file.to_path_buf(),
                        line: index + 2,
                        info: info.trim().to_string(),
                        body: String::new(),
                    };
                    open = Some(block);
                }
            }
        } else if let Some(block) = open.as_mut() {
            block.body.push_str(stripped);
            block.body.push('\n');
        }
    }
    Ok(blocks)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; directory_entries propagates directory-read and per-entry reservation errors while preserving sorted paths; filesystem failures are Result errors, not assertion failures."
)]
fn directory_entries(directory: &std::path::Path) -> Result<Vec<std::path::PathBuf>, String> {
    let entries = std::fs::read_dir(directory)
        .map_err(|problem| format!("cannot scan {}: {problem}", directory.display()))?;
    let mut sorted = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|problem| {
            format!("cannot read entry in {}: {problem}", directory.display())
        })?;
        #[expect(
            clippy::needless_late_init,
            reason = "Owner: noble-maintainers; pinned Octet recognizes fallible reservation assignments but skips let initializers and ? expansions; reassess when its finder supports both."
        )]
        let reservation;
        reservation = sorted.try_reserve(1);
        reservation.map_err(|problem| {
            format!(
                "cannot reserve an entry for {}: {problem}",
                directory.display()
            )
        })?;
        sorted.push(entry.path());
    }
    sorted.sort();
    Ok(sorted)
}

/// Visit source paths in sorted depth-first order without recursive calls.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; collect_rust iteratively visits sorted paths and propagates discovery/reservation errors; the caller checks example coverage rather than asserting filesystem contents here."
)]
fn collect_rust(directory: &std::path::Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut pending = vec![directory.to_path_buf()];
    let mut files = Vec::new();
    while let Some(path) = pending.pop() {
        if path.is_dir() {
            let entries = directory_entries(&path)?;
            pending
                .try_reserve(entries.len())
                .map_err(|problem| format!("cannot reserve source traversal: {problem}"))?;
            pending.extend(entries.into_iter().rev());
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            #[expect(
                clippy::needless_late_init,
                reason = "Owner: noble-maintainers; pinned Octet recognizes fallible reservation assignments but skips let initializers and ? expansions; reassess when its finder supports both."
            )]
            let reservation;
            reservation = files.try_reserve(1);
            reservation.map_err(|problem| format!("cannot reserve a source path: {problem}"))?;
            files.push(path);
        }
    }
    Ok(files)
}

/// Scan sources, both fragment definitions, and the v1 executed examples.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; scan_all propagates source-read, fence-scan and fallible reservation errors; executed-example and word-coverage contracts are checked by the consuming tests."
)]
pub(super) fn scan_all() -> Result<Vec<Block>, String> {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = collect_rust(&manifest.join("src"))?;
    files
        .try_reserve(3)
        .map_err(|problem| format!("cannot reserve documentation paths: {problem}"))?;
    files.push(manifest.join("../../verification/m2-fragment.md"));
    files.push(manifest.join("../../verification/m3-fragment.md"));
    files.push(manifest.join("../../verification/m3-docexamples.md"));
    let mut blocks = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file)
            .map_err(|problem| format!("cannot read {}: {problem}", file.display()))?;
        let found = scan_text(&file, &text)?;
        #[expect(
            clippy::needless_late_init,
            reason = "Owner: noble-maintainers; pinned Octet recognizes fallible reservation assignments but skips let initializers and ? expansions; reassess when its finder supports both."
        )]
        let reservation;
        reservation = blocks.try_reserve(found.len());
        reservation.map_err(|problem| format!("cannot reserve documentation blocks: {problem}"))?;
        blocks.extend(found);
    }
    Ok(blocks)
}
