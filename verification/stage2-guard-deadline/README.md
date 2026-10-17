# Stage2 guard deadline fix — local source snapshot

This package records the exact r2 stage2 guard source snapshot and bounded local observations. It is not a deployment package, production acceptance, portability feature, or authorization to provision or activate Bonsai.

## Change

The guard carries the oldest resource-snapshot acquisition timestamp into heartbeat renewal. The full subsequent sample and process classification must complete before the previous acquisition-based two-second deadline; an expired queued renewal cannot revive that deadline. The frozen guard source in `source/` is byte-identical to SHA-256 `fce2063a036554a78aaff139dd2f94230c75681472073cf741fef240b30da261`.

The bounded before capture observed the r1 scope still populated 150.058 ms after the two-second sample expiry (sample age 2.150058483 s). With r2 under the delayed decision, the watcher began stopping just after the acquisition-based deadline and the exact scope was empty at sample age 2.150061066 s. A fresh real host statvfs sample/heartbeat renewed the scope normally. A queued fresh heartbeat after expiry was rejected. The corrected independent replay separately repeated the delayed, fresh, and queued-expiry observations. These are local component observations, not a universal real-time stop guarantee or quota-backend success.

## Snapshot-specific reproduction fixture

`fixture/deadline-regression.py` is the exact corrected verification-only fixture (SHA-256 `52a81aa369e0a336ca5fe9adbe255847a807143a47a6f13c6491fea7c972998e`); `fixture/local-lease-fixture.txt` is its bounded input. The frozen fixture deliberately retains its absolute local `SOURCE_ROOT` and output behavior. It requires the matching r2 source snapshot and local systemd/statvfs environment; it is not portable and this publication does not redesign its API. Do not silently rewrite its source-bound bytes. `evidence/summary.json` is a derived public-safe summary, not an original raw receipt. Raw capture packets, stdout/base64, broad environment and host inventories remain in private immutable custody; the summary lists raw receipt/output digests and states what was omitted.

The original independent queued-expired fixture invocation remains **FAIL** (raw packet SHA-256 `14bc4cf70eeb3e2c86f8f432099b8141efb42303a52498d5fcee355272427092`): the exact cgroup became empty before the watcher had committed its receipt, so the fixture raised `FileNotFoundError`. It was not rerun unchanged or relabelled. The corrected fixture joins the watcher before reading the receipt; corrected verification-package runs and actual independent replay outputs are separately identified in the summary.

Lease supervisor attempts 1 and 2 remain **FAIL_EXCEPTION** in the private record: attempt 1 failed before a lease test completed; attempt 2's manager early-read assertion failed despite its raw contender payload showing acquisition after exact stop. Attempt 3's separate local proof observed 25 failed flock attempts after controller death while the scope remained populated; the contender acquired only after exact stop and receipt completion. This bounded summary does not promote either earlier failure or claim a universal lease guarantee.

## HOLD and assurance limits

Bonsai remains **HOLD**. The read-only check found neither ext4 `quota` nor `project` features; private project usage is unmeasured; last observed free capacity was 186,661,830,656 bytes, below the unchanged 180 GiB threshold. The protected service/launcher/shared-lock setup and on-host source-bound harmless smokes remain absent. The original five production/compiler blockers remain **BLOCKED** despite this publication; compiler/full acceptance remains blocked. No CASE02 state, proof, archive, canonical spec, or global-gate acceptance changes here.

Native sandbox fixture observations were local only: the bounded native proof packet recorded outside sentinel unchanged and blocked create absent, plus a tiny C compile; overall packet status PASS. Its private proof/index packet digests are retained in `evidence/summary.json`; no whole host-attestation packet or process inventory is published.

## Change record

2026-10-17: published the frozen r2 source snapshot, corrected snapshot-specific regression fixture, and bounded local evidence summary to preserve the acquisition-based deadline fix and its explicit deployment hold. No production activation, quota provisioning, portability work, case/spec/archive promotion, or compiler acceptance is part of this change.
