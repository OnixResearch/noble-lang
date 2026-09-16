# Reviewed offline identities for the pinned Lean backend sources.
# Captured from the reviewed lake-manifest.json revisions. Data only.
{
  schema_version | String = "noble-lean-backend-inputs/v1",
  owner | String = "noble-maintainers",
  source | String = "proofs/m1/lake-manifest.json",
  note | String = "Each narHash was captured by resolving the reviewed revision on the operator host. The aeneas narHash equals the reviewed tool-selection pin.",
  sources | { _ : { url | String, rev | String, narHash | String, narSize | Number } } = {
    aeneas = {
      url = "https://github.com/AeneasVerif/aeneas",
      rev = "505b6ca35217e7be5c96c3e2f8045edfbdf47291",
      narHash = "sha256-V6H12+Qg3qlZBGdjoTHCbYWSOwcdZ86XCxOkWURo6Qs=",
      narSize = 7958080,
    },
    mathlib = {
      url = "https://github.com/leanprover-community/mathlib4.git",
      rev = "fabf563a7c95a166b8d7b6efca11c8b4dc9d911f",
      narHash = "sha256-0eBeRdZCK/mxbuzmZqb8+3ssCuykGbGrDWWwD3ItVto=",
      narSize = 101020616,
    },
    plausible = {
      url = "https://github.com/leanprover-community/plausible",
      rev = "63045536fe95024e6c18fc7b48e03f506701c5bc",
      narHash = "sha256-jajg1nEPO0OaiTNYoR31302YKdD1E2Y+ZmyohmKaAHI=",
      narSize = 171664,
    },
    LeanSearchClient = {
      url = "https://github.com/leanprover-community/LeanSearchClient",
      rev = "c5d5b8fe6e5158def25cd28eb94e4141ad97c843",
      narHash = "sha256-L2aAwn3OeRLVt/VccLdBS0ogqmIIKAwnz94PpAOhaRc=",
      narSize = 58392,
    },
    importGraph = {
      url = "https://github.com/leanprover-community/import-graph",
      rev = "5c7542ed018c78194f1e2b903eaf6a792b74c03d",
      narHash = "sha256-waPk0erYC5b2JvS/Qy1NmI4iEa+LBzybW4VJYdDLGmU=",
      narSize = 414712,
    },
    proofwidgets = {
      url = "https://github.com/leanprover-community/ProofWidgets4",
      rev = "24b0d9dc081c5423f8eec7e866c441e5184f29d9",
      narHash = "sha256-2maEePJiEbN4S+IQb10ahm6E6mzYrUBzOe2KQHhSgFw=",
      narSize = 7580728,
    },
    aesop = {
      url = "https://github.com/leanprover-community/aesop",
      rev = "e3cb2f741431ce31bf73549fb52316a57368b06f",
      narHash = "sha256-g/dxj2uPYoZBxoR9fH0Hco8V0fF5gLJoxD9lt1YUE/Q=",
      narSize = 890216,
    },
    Qq = {
      url = "https://github.com/leanprover-community/quote4",
      rev = "f46324995fca5f0483b742e4eb4daec7f4ee50d2",
      narHash = "sha256-WUxmVOyKclhASP+galjYy2y/lYJbTizTKuRhDV6oPFs=",
      narSize = 117376,
    },
    batteries = {
      url = "https://github.com/leanprover-community/batteries",
      rev = "fa08db58b30eb033edcdab331bba000827f9f785",
      narHash = "sha256-htlU9MGOgcT6vhBtH6HCuLBjf5LVAUQ3mBpUYujYKkc=",
      narSize = 1486608,
    },
    Cli = {
      url = "https://github.com/leanprover/lean4-cli",
      rev = "92564e5770e4d09f2d86dfbf8ada1e9c715b384c",
      narHash = "sha256-0yBEdleX+alK+V28gKahvejUYjWjUDRKLo5tAVjXRUg=",
      narSize = 118456,
    },
  },
}
