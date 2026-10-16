//! Private evidence-build observer. No report fields or domain-core effects.
//! The default build compiles away every hook, including environment access.

#[cfg(feature = "safety-observe")]
mod enabled {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    use std::sync::Mutex;

    struct Output {
        file: std::fs::File,
        sequence: u64,
        guest: u64,
        protected: u64,
    }

    // Composition-root evidence build only. One process scope, including host
    // callbacks on any thread; never a domain-core global or library counter.
    static OUTPUT: Mutex<Option<Output>> = Mutex::new(None);

    pub(crate) struct Scope;

    pub(crate) fn begin() -> Scope {
        if let Some(path) = std::env::var_os("NOBLE_SAFETY_OBSERVATION") {
            let file = std::fs::OpenOptions::new()
                .create_new(true)
                .append(true)
                .mode(0o600)
                .open(path)
                .expect("create fresh safety observation");
            let mut output = OUTPUT.lock().expect("safety observation lock");
            assert!(output.is_none(), "one safety observation scope per process");
            *output = Some(Output {
                file, sequence: 0, guest: 0, protected: 0,
            });
            drop(output);
            event("begin");
        }
        Scope
    }

    pub(crate) fn record(value: &serde_json::Value) {
        let mut output = OUTPUT.lock().expect("safety observation lock");
        if let Some(output) = output.as_mut() {
            let mut value = value.clone();
            value["pid"] = serde_json::json!(std::process::id());
            value["sequence"] = serde_json::json!(output.sequence);
            output.sequence += 1;
            match value["event"].as_str() {
                Some("guest-request") => output.guest += 1,
                Some("protected-operation") => output.protected += 1,
                Some("end") => {
                    value["guest_requests"] = serde_json::json!(output.guest);
                    value["protected_operations"] = serde_json::json!(output.protected);
                }
                _ => {}
            }
            let mut bytes = serde_json::to_vec(&value).expect("encode safety observation");
            bytes.push(b'\n');
            output.file.write_all(&bytes).expect("write safety observation");
            output.file.flush().expect("flush safety observation");
        }
    }

    pub(crate) fn event(kind: &str) {
        record(&serde_json::json!({"event":kind,"pid":std::process::id()}));
    }

    impl Drop for Scope {
        fn drop(&mut self) {
            event("end");
            *OUTPUT.lock().expect("safety observation lock") = None;
        }
    }

    pub(crate) struct Nodes(pub(crate) u64);
    impl Nodes {
        pub(crate) const fn new() -> Self { Self(0) }
        pub(crate) fn counter(&mut self) -> Option<&mut u64> { Some(&mut self.0) }
    }
    impl Drop for Nodes {
        fn drop(&mut self) {
            record(&serde_json::json!({"event":"nodes","evaluated_nodes":self.0}));
        }
    }

    pub(crate) fn child(command: &mut std::process::Command) {
        if let Some(path) = std::env::var_os("NOBLE_SAFETY_OBSERVATION") {
            command.env("NOBLE_SAFETY_OBSERVATION", path);
            event("child-launch");
        }
    }
}

#[cfg(feature = "safety-observe")]
pub(crate) use enabled::*;

#[cfg(not(feature = "safety-observe"))]
mod disabled {
    pub(crate) struct Scope;
    pub(crate) const fn begin() -> Scope { Scope }
    pub(crate) const fn event(_: &str) {}
    pub(crate) struct Nodes;
    impl Nodes {
        pub(crate) const fn new() -> Self { Self }
        pub(crate) const fn counter(&mut self) -> Option<&mut u64> { None }
    }
    pub(crate) const fn child(_: &mut std::process::Command) {}
}

#[cfg(not(feature = "safety-observe"))]
pub(crate) use disabled::*;

#[cfg(all(test, feature = "safety-observe"))]
mod tests {
    use noble_ngraph::format::{data, vectors};
    use noble_ngraph::Value;

    #[test]
    fn registered_format_values() {
        let _scope = super::begin();
        let variants = [
            ("noble-ngraph-vectors/1 schema line", "noble-ngraph-vectors/1\ntypes f32\nv 1\n", "E-FORMAT"),
            ("missing schema line", "types f32\nv 1\n", "E-FORMAT"),
            ("f32 value 0.1", "noble-ngraph-vectors/0\ntypes f32\nv 0.1\n", "E-LITERAL-INEXACT"),
            ("f32 value 0x7fc00001", "noble-ngraph-vectors/0\ntypes f32\nv 0x7fc00001\n", "0x7fc00001"),
            ("f32 value -0", "noble-ngraph-vectors/0\ntypes f32\nv -0\n", "0x80000000"),
            ("i64 value 9223372036854775808", "noble-ngraph-vectors/0\ntypes i64\nv 9223372036854775808\n", "E-FORMAT"),
            ("row with one value too few", "noble-ngraph-vectors/0\ntypes f32 f32\nv 1\n", "E-FORMAT"),
        ];
        for (variant, input, expected) in variants {
            let observed = match vectors::parse(input) {
                Err(d) => d.code_string(),
                Ok(v) => match v.rows.as_slice() {
                    [row] => match row.as_slice() {
                        [Value::F32(bits)] => format!("0x{bits:08x}"),
                        other => panic!("unexpected values: {other:?}"),
                    },
                    other => panic!("unexpected rows: {other:?}"),
                },
            };
            assert_eq!(observed, expected, "{variant}");
            super::record(&serde_json::json!({"event":"format-result", "variant":variant,
                "input":input,"observed":observed}));
        }
        let input = "noble-ngraph-data/0\ntypes f32 f32\npoint 1 ; 2\n";
        let observed = data::parse(input).unwrap_err().code_string();
        assert_eq!(observed, "E-FORMAT");
        super::record(&serde_json::json!({"event":"format-result",
            "variant":"data file types line without ';'","input":input,"observed":observed}));
    }
}
