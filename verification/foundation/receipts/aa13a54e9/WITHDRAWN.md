# Withdrawn: this receipt records a module that no longer exists

`aa13a54e9/` is a truthful record of one clean-source run at commit
`aa13a54e996947942e04f1d4e1a3eaba5182bba9`, tree
`3b48bbac7af1bfdd8fc66d63095d32ed69cc028e`. That run included
`proofs/foundation/NobleFoundation/RegionTrace.lean`, a second Lean subject beside the existing
`proofs/m5/RegionTrace.lean`.

**The module was withdrawn at `53c078b3a`** because this repository keeps one subject per
concern: the m5 module already owns the region-trace relation (`erase_preserves_*`,
`related_rejects_*`, `append_projection`, `take_projection`, the fixtures), so the foundation copy
and its inventory rows were deleted rather than maintained in parallel.

The receipt bytes stay exactly as measured — they are not rewritten, and the theorem count inside
them stays what that run reported. What changed is their **scope**: they are evidence for the tree
at `aa13a54e9` only, they do not describe the current foundation, and no theorem listed in
`foundation-check.json` that belonged to the withdrawn module exists in
`proofs/foundation/inventory.json` today. Do not cite this receipt for any current claim.

The replacement evidence for region-trace results is `proofs/m5/RegionTrace.lean` and whatever lane
owns its receipt.
