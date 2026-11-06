# Reviewed offline identities for the pinned Lean backend sources.
# Captured from the reviewed lake-manifest.json revisions. Data only.
{
  schema_version | String = "noble-lean-backend-inputs/v1",
  owner | String = "noble-maintainers",
  source | String = "proofs/m1/lake-manifest.json",
  note | String = "Each narHash was captured by resolving the reviewed revision on the operator host. The aeneas narHash equals the reviewed tool-selection `aeneas_lean` pin (the Lean library; the extractor is pinned separately).",
  sources | { _ : { url | String, rev | String, narHash | String, narSize | Number } } = {
    aeneas = {
      url = "https://github.com/AeneasVerif/aeneas",
      rev = "1238879f9ea550512096e9bf8bd6fe7407b3eaef",
      narHash = "sha256-HXAeoR35sHdqQj9FEz4ulPvVAdxqZlBcMG2ly7OYNDk=",
      narSize = 8220568,
    },
    mathlib = {
      url = "https://github.com/leanprover-community/mathlib4.git",
      rev = "56a005855ea17e0b0958f73ec801ae140b0a1679",
      narHash = "sha256-XEUgaImb2x71pRXOEAWHITT1MieA79uaIezyPAm3LOA=",
      narSize = 105917720,
    },
    plausible = {
      url = "https://github.com/leanprover-community/plausible",
      rev = "dfa4f38190d07c37e315953e08b90fe2d95b3c24",
      narHash = "sha256-xm+PPwM8a58hV/Wo3mzG6Lcb4P+OXJy9SBteTavorqg=",
      narSize = 171800,
    },
    LeanSearchClient = {
      url = "https://github.com/leanprover-community/LeanSearchClient",
      rev = "d6b4c7be5be75ec4917c873ac50c756dc1436372",
      narHash = "sha256-Yp3i1ygUXANiA1sRJc54QdjXeospB4IbGEeioY5x7ec=",
      narSize = 58648,
    },
    importGraph = {
      url = "https://github.com/leanprover-community/import-graph",
      rev = "a41f0e9a4dea23188ea9cc1c41ac9f8bbafe0b93",
      narHash = "sha256-YjbZ++zp7slpt6PpiXFMfWUzTs/j+1CuynH6xa9OGFE=",
      narSize = 687408,
    },
    proofwidgets = {
      url = "https://github.com/leanprover-community/ProofWidgets4",
      rev = "df72b022e558978a1c50264cfc4d7fec86fa8e04",
      narHash = "sha256-gh6xMZ/JmTmg4ARfWRaEY7QbJ//a7YZ1AN0D/AT3dUM=",
      narSize = 9628776,
    },
    aesop = {
      url = "https://github.com/leanprover-community/aesop",
      rev = "4a174c490d8707121c4a9290800489f105d85e07",
      narHash = "sha256-ZKoq/n59JAt7TJgb4yVFsiMdIvqTlGuuz/TnU95NMdA=",
      narSize = 889200,
    },
    Qq = {
      url = "https://github.com/leanprover-community/quote4",
      rev = "dce0ad79c486cb1fb504af04f953c3466b0c6db8",
      narHash = "sha256-mcSBP2pqVRdoPc2fVN45xxw7e13L5tuRf+v0j01Co8I=",
      narSize = 119440,
    },
    batteries = {
      url = "https://github.com/leanprover-community/batteries",
      rev = "66e469188fe07963e5cf666c7d4433f45b8131c5",
      narHash = "sha256-NwqmoIl8Ph79liBor3ZaIgJYtA4tXZI0KWMLsd433UE=",
      narSize = 1548368,
    },
    Cli = {
      url = "https://github.com/leanprover/lean4-cli",
      rev = "c24fe6078c630e89b489bffe59b3e20ac30d12c8",
      narHash = "sha256-DAhtgb9B3ZZsOb82QXhD7Z3dMvjsQCfTALCcZDfZQUw=",
      narSize = 118456,
    },
  },
}
