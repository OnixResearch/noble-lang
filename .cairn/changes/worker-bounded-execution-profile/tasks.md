## Phase 1: Implementation

- [ ] [serial] Add the opt-in worker compiler mode, the `noble/worker_interrupt_check` import and charge-then-check execution and observation ticks; keep every other profile's modules unchanged. r[H-LIMIT-02]
- [ ] [serial] Answer checks in the typed engine, record the observed interval, classify cancelled, host-deadline, trap and internal-failure outcomes, and report the profile from the Rust host. r[H-LIMIT-02]

## Phase 2: Verification

- [ ] [serial] Run each MW1 row of WORKER-09 through the real typed engine, including a mutation of the interval, and keep the WORKER-02 gate and worker suites passing. r[H-LIMIT-02]
- [ ] [serial] Validate Cairn; record the entry gate as partial in the roadmap without promoting WORKER-09 or removing WORKER-11's open gate. r[H-LIMIT-02]
