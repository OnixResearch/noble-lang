//! PAA-1 envelope refusals that are decided before any proof check, rebuild or
//! instantiation: a changed law byte, a substituted contract-mode law, and a
//! source outside the MC2 pure quotation profile. Every refusal reports zero
//! guest requests and protected operations.

const LAW_V1: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../verification/owner-law/laws/owner-law-v1.json"
));
const LAW_V1_SHA256: &str = "9a8eda12c74356ca0d312be840411d4445c6f377c16ee7b96a680581b2e91eac";
const INCREMENT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../verification/mc2/contracts/increment.contract"
));
const Q_ONE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../verification/mc2/contracts/q-one.noble"
));
const PROOF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../verification/mc2/contracts/increment.proof.lean"
));

/// SHA-256 encodes the message length in bits.
const BITS_PER_BYTE: u64 = 8;

fn sha256(bytes: &[u8]) -> String {
    // A local SHA-256: the test target cannot import the binary crate's digest.
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = bytes.to_vec();
    let bit_length = (bytes.len() as u64).wrapping_mul(BITS_PER_BYTE);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_be_bytes());
    for block in message.chunks_exact(64) {
        let mut words = [0u32; 64];
        for (index, word) in block.chunks_exact(4).enumerate() {
            words[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
    let mut text = String::with_capacity(64);
    for word in state {
        text.push_str(&format!("{word:08x}"));
    }
    text
}

struct Fixture {
    root: std::path::PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Result<Self, std::io::Error> {
        let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("noble-paa1-{}-{name}", std::process::id()));
        match std::fs::remove_dir_all(&root) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn file(&self, name: &str, bytes: &[u8]) -> Result<std::path::PathBuf, std::io::Error> {
        let path = self.root.join(name);
        std::fs::write(&path, bytes)?;
        Ok(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn admit(args: &[&std::ffi::OsStr]) -> Result<std::process::Output, std::io::Error> {
    std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("admit-proof-artifact")
        .args(args)
        .output()
}

fn report(output: &std::process::Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).expect("one JSON envelope per run")
}

fn admission(
    fixture: &Fixture,
    law: &[u8],
    digest: &str,
    contract: &[u8],
) -> Vec<std::ffi::OsString> {
    let candidate = fixture
        .file("candidate.wasm", b"not read before the law")
        .expect("candidate");
    let call = fixture
        .file("call.wasm", b"not read before the law")
        .expect("call");
    let law = fixture.file("law", law).expect("law");
    let contract = fixture.file("contract", contract).expect("contract");
    let source = fixture.file("source.noble", Q_ONE).expect("source");
    let proof = fixture.file("proof.lean", PROOF).expect("proof");
    let pins = fixture.file("pins.json", b"{}").expect("pins");
    let mut args: Vec<std::ffi::OsString> = Vec::new();
    args.push(candidate.into_os_string());
    let operands: [(&str, &std::path::Path); 6] = [
        ("--law", &law),
        ("--source", &source),
        ("--contract", &contract),
        ("--proof", &proof),
        ("--pins", &pins),
        ("--call", &call),
    ];
    for (name, value) in operands {
        args.push(name.into());
        args.push(value.into());
    }
    // The law digest and the argument are not paths; append them explicitly.
    args.push("--law-digest".into());
    args.push(digest.into());
    args.push("--argument".into());
    args.push("41".into());
    args
}

fn refusal(output: &std::process::Output, code: &str, exit: i32) {
    assert_eq!(
        output.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = report(output);
    assert_eq!(report["schema"], "noble-proof-artifact-admission/v1");
    assert_eq!(report["admission"], "refused");
    assert_eq!(report["outcome"], code, "{report}");
    assert_eq!(report["refusal"]["code"], code, "{report}");
    assert_eq!(report["guest_requests"], 0);
    assert_eq!(report["protected_operations"], 0);
    assert!(report["refusal"]["message"]
        .as_str()
        .is_some_and(|message| !message.is_empty()));
}

#[test]
fn the_recorded_law_digest_is_the_frozen_record() {
    assert_eq!(sha256(LAW_V1), LAW_V1_SHA256);
}

#[test]
fn a_changed_law_byte_refuses_before_any_proof_check() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("law-digest")?;
    let mut changed = LAW_V1.to_vec();
    changed.push(b'\n');
    let args = admission(&fixture, &changed, LAW_V1_SHA256, INCREMENT);
    let borrowed: Vec<&std::ffi::OsStr> = args.iter().map(std::ffi::OsString::as_os_str).collect();
    let output = admit(&borrowed)?;
    refusal(&output, "law-mismatch", 1);
    Ok(())
}

#[test]
fn a_substituted_contract_mode_law_refuses() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("law-contract")?;
    let law = b"(contract 1 increment\n  (input (x I64))\n  (output (y I64))\n  (program [ 1 + ])\n  (requires true)\n  (ensures (eq (out y) (add (in x) 1))))\n";
    let substituted = b"(contract 1 increment\n  (input (x I64))\n  (output (y I64))\n  (program [ 2 + ])\n  (requires true)\n  (ensures (eq (out y) (add (in x) 1))))\n";
    let args = admission(&fixture, law, &sha256(law), substituted);
    let borrowed: Vec<&std::ffi::OsStr> = args.iter().map(std::ffi::OsString::as_os_str).collect();
    let output = admit(&borrowed)?;
    refusal(&output, "law-mismatch", 1);
    Ok(())
}

#[test]
fn a_definition_source_is_outside_the_pure_quotation_profile(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("definition")?;
    let args = admission(&fixture, LAW_V1, LAW_V1_SHA256, INCREMENT);
    // Replace the source operand's file with a definition.
    let source = fixture.file("source.noble", b"def twice [ dup compose ]\n")?;
    let mut args = args;
    let at = args
        .iter()
        .position(|value| value == "--source")
        .expect("the source operand");
    args[at + 1] = source.into_os_string();
    let borrowed: Vec<&std::ffi::OsStr> = args.iter().map(std::ffi::OsString::as_os_str).collect();
    let output = admit(&borrowed)?;
    refusal(&output, "unsupported-profile", 4);
    Ok(())
}

#[test]
fn unreadable_submission_files_refuse_as_invalid_envelopes(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("unreadable")?;
    let missing = fixture.root.join("missing.wasm");
    let mut args = admission(&fixture, LAW_V1, LAW_V1_SHA256, INCREMENT);
    args[0] = missing.into_os_string();
    let borrowed: Vec<&std::ffi::OsStr> = args.iter().map(std::ffi::OsString::as_os_str).collect();
    let output = admit(&borrowed)?;
    refusal(&output, "invalid-envelope", 2);
    Ok(())
}
