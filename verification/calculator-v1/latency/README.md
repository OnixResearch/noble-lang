# Calculator line latency (#16)

These are the latency measurements behind the `#16` reuse change
(exact canonical-epoch module reuse, `NOBLE_CALCULATOR_TRACE`). They were
taken on the shared aspen3 workstation (x86_64) while other builds ran, so
every table gives the 1-minute load average at the start and end of each run.
Wall times are timings under that load, not benchmarks.
[`summary.json`](summary.json) records the binary SHA-256s, build profiles,
concurrency and load.

Runs used the per-module trace (`NOBLE_CALCULATOR_TRACE`, schema
`noble-calculator-trace/1`), summarized per run. Every table below is the
summary of committed traces; the raw traces were not committed.

## Binaries

| Label | Commit | Profile | Change |
|---|---|---|---|
| before | `6468294` | release and debug | per-module trace only |
| after | `7bae596` | release and debug | plus exact canonical-epoch module reuse (option 1B) |
| merged | `0b8c7f9` | debug | #16 merged with #17's committed session |

## Baseline: modules per vector and per-module split (`6468294`)

Each vector ran once, four processes at a time, with load average 32.43 at the
start. Four runs were interrupted before they printed a result; their module
counts and per-module means cover only the modules that ran.

| Run | Output | Complete | Wall | Modules | Distinct sources / WAT | Modules per line | Mean ms per module: check / backend / worker prepare / re-hash / assemble / V8 compile / execute / commit / total |
|---|---|---|---|---|---|---|---|
| `release-mul23` | 6 | yes | 7:20.92 | 438 | 205 / 437 | 1:437 | 414.3 / 418.3 / 102.5 / 37.9 / 31.6 / 29.4 / 1.4 / 45.8 / 989.7 |
| `release-fd03` | 0.3 | yes | 7:51.31 | 405 | 191 / 404 | 1:404 | 497 / 499.3 / 100.1 / 36.9 / 31.4 / 28.4 / 1.3 / 44.4 / 1149 |
| `release-dec1234` | — | no (interrupted) | — | 242 | 107 / 241 | 1:241 | 315.5 / 310.8 / 89.3 / 34 / 26.9 / 25.1 / 1.4 / 41.1 / 765.6 |
| `release-session` | — | no (interrupted) | — | 1193 | 139 / 581 | 1:253 2:581 3:358 | 475.7 / 474.5 / 95.7 / 37.9 / 28.7 / 25.7 / 1.3 / 49.6 / 1103 |
| `debug-fd03` | 0.3 | yes | 19:01.48 | 405 | 191 / 404 | 1:404 | 1011 / 1044.8 / 660.3 / 506.1 / 118.4 / 31.8 / 1.5 / 57.1 / 2784 |
| `debug-mul23` | — | no (interrupted) | — | 195 | 129 / 194 | 1:194 | 706.9 / 734 / 555.1 / 427.1 / 96.8 / 26.9 / 1.5 / 52.1 / 2065.6 |
| `debug-session` | — | no (interrupted) | — | 514 | 117 / 260 | 1:253 2:260 | 919.4 / 952.7 / 601.4 / 472.4 / 98 / 27 / 1.6 / 59 / 2540.8 |

`Modules per line` counts guest modules per input line (line 0 is session
setup). In the `session` vector (`fn f(y)=y*2`, `fn g(x)=f(x)+1`, `g(2)`),
later lines replayed earlier definitions before #17.

## Before/after pairs, release (`6468294` → `7bae596`)

For each vector the before and after binaries ran concurrently in the same
window, two vector pairs (four processes) at a time.

| Vector | Output (identical?) | Load 1-min start→end | Wall s before | Wall s after | Speedup | Modules per line | Host hits / worker hits | Check s b/a | Backend s b/a | Worker prepare s b/a | Commit s b/a |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `1+2` | 3 (yes) | 52.45→48.67 | 464 | 267 | 1.74× | 1:449 | 225 / 225 of 450 | 188.8 / 106.4 | 188.8 / 105.6 | 45.4 / 24.1 | 21.0 / 12.9 |
| `2*3` | 6 (yes) | 56.23→60.54 | 538 | 296 | 1.82× | 1:437 | 220 / 220 of 438 | 220.3 / 119.5 | 222.4 / 119.8 | 54.4 / 28.3 | 24.9 / 14.9 |
| `12.34` | 617/50 (yes) | 60.54→52.45 | 349 | 240 | 1.45× | 1:317 | 127 / 127 of 318 | 139.4 / 96.2 | 140.1 / 94.3 | 38.2 / 24.0 | 17.0 / 12.4 |
| `0.3` --finite-decimal | 0.3 (yes) | 47.83→56.23 | 569 | 292 | 1.95× | 1:404 | 193 / 193 of 405 | 241.2 / 118.2 | 243.0 / 120.5 | 48.8 / 27.2 | 22.4 / 14.5 |
| `fn f(y)=y*2` / `fn g(x)=f(x)+1` / `g(2)` | 5 (yes) | 47.83→45.74 | 2721 | 866 | 3.14× | 1:253 2:581 3:1358 | 1613 / 1489 of 2193 | 1152.9 / 354.9 | 1150.6 / 351.7 | 242.7 / 84.2 | 128.7 / 42.3 |

Mean per module, ms (before / after):

| Vector | check | backend | worker prepare | re-hash | assemble | V8 compile | execute | commit | total | WAT KiB |
|---|---|---|---|---|---|---|---|---|---|---|
| `1+2` | 419.6 / 236.4 | 419.5 / 234.8 | 100.9 / 53.6 | 34.6 / 17.8 | 32.7 / 17.6 | 29 / 14.9 | 1.7 / 1.4 | 46.8 / 28.7 | 995.9 / 559.1 | 219 / 219 |
| `2*3` | 503 / 272.8 | 507.7 / 273.6 | 124.1 / 64.5 | 41.3 / 20.7 | 42.9 / 22.6 | 34.7 / 17.5 | 1.9 / 1.4 | 56.8 / 34 | 1202.4 / 651.4 | 218 / 218 |
| `12.34` | 438.4 / 302.5 | 440.7 / 296.4 | 120.2 / 75.4 | 41.1 / 24.9 | 39.8 / 25.4 | 34.5 / 21.3 | 1.9 / 1.6 | 53.3 / 39 | 1063.6 / 721.2 | 216 / 216 |
| `0.3` --finite-decimal | 595.7 / 291.9 | 600 / 297.4 | 120.4 / 67.2 | 40.1 / 21.9 | 41.3 / 24.1 | 34.1 / 17.6 | 1.9 / 1.5 | 55.3 / 35.8 | 1382.4 / 699.5 | 232 / 232 |
| `fn f(y)=y*2` / `fn g(x)=f(x)+1` / `g(2)` | 525.7 / 161.9 | 524.7 / 160.4 | 110.7 / 38.4 | 40.8 / 12.5 | 35.7 / 13.2 | 29.5 / 10 | 1.8 / 1.2 | 58.7 / 19.3 | 1229.8 / 384.4 | 197 / 197 |

After binary, mean per module by host/worker cache state, ms:

| Vector | host/worker | modules | check | backend | worker prepare | execute | commit | total |
|---|---|---|---|---|---|---|---|---|
| `1+2` | bypass/miss | 2 | 323 | 279.7 | 118.8 | 10.1 | 63.2 | 800.2 |
| `1+2` | hit/hit | 225 | 0 | 0.3 | 2.2 | 1.1 | 0.2 | 3.9 |
| `1+2` | miss/miss | 223 | 474.1 | 470.9 | 104.9 | 1.7 | 57.1 | 1117.1 |
| `2*3` | bypass/miss | 2 | 300.2 | 259 | 113.7 | 12.5 | 66.7 | 758.2 |
| `2*3` | hit/hit | 220 | 0 | 0.3 | 2 | 0.8 | 0.2 | 3.5 |
| `2*3` | miss/miss | 216 | 550.5 | 552.1 | 127.7 | 1.9 | 68.1 | 1310.2 |
| `12.34` | bypass/miss | 2 | 304.3 | 249.4 | 95.2 | 17 | 61.4 | 732.5 |
| `12.34` | hit/hit | 127 | 0 | 0.3 | 2.1 | 0.9 | 0.2 | 3.7 |
| `12.34` | miss/miss | 189 | 505.7 | 495.9 | 124.5 | 1.8 | 64.8 | 1203.3 |
| `0.3` --finite-decimal | bypass/miss | 2 | 295.7 | 257.6 | 120.6 | 19 | 74.7 | 773.6 |
| `0.3` --finite-decimal | hit/hit | 193 | 0 | 0.3 | 1.7 | 0.8 | 0.2 | 3.2 |
| `0.3` --finite-decimal | miss/miss | 210 | 560.2 | 570.9 | 126.9 | 1.9 | 68.1 | 1338.7 |
| `fn f(y)=y*2` / `fn g(x)=f(x)+1` / `g(2)` | bypass/miss | 4 | 298.4 | 261.7 | 109.4 | 13.2 | 73.2 | 762.2 |
| `fn f(y)=y*2` / `fn g(x)=f(x)+1` / `g(2)` | hit/hit | 1318 | 0 | 0.3 | 1.7 | 0.8 | 0.3 | 3.2 |
| `fn f(y)=y*2` / `fn g(x)=f(x)+1` / `g(2)` | hit/miss | 295 | 0 | 0.4 | 99.1 | 1.6 | 0.4 | 101.6 |
| `fn f(y)=y*2` / `fn g(x)=f(x)+1` / `g(2)` | miss/hit | 171 | 543.7 | 545.1 | 3.5 | 1.3 | 58.8 | 1161.2 |
| `fn f(y)=y*2` / `fn g(x)=f(x)+1` / `g(2)` | miss/miss | 405 | 643.9 | 634.5 | 127.5 | 1.9 | 77.7 | 1499.2 |

## Before/after pairs, debug (`6468294` → `7bae596`)

Same procedure as the release pairs.

| Vector | Output (identical?) | Load 1-min start→end | Wall s before | Wall s after | Speedup | Modules per line | Host hits / worker hits | Check s b/a | Backend s b/a | Worker prepare s b/a | Commit s b/a |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `1+2` | 3 (yes) | 21.46→14.98 | 710 | 422 | 1.68× | 1:449 | 225 / 225 of 450 | 197.9 / 116.4 | 205.0 / 120.0 | 282.5 / 166.2 | 14.2 / 9.2 |
| `2*3` | 6 (yes) | 33.24→25.06 | 967 | 559 | 1.73× | 1:437 | 220 / 220 of 438 | 292.4 / 168.4 | 304.9 / 174.0 | 331.9 / 187.3 | 20.3 / 13.0 |
| `12.34` | 617/50 (yes) | 25.06→21.46 | 544 | 395 | 1.38× | 1:317 | 127 / 127 of 318 | 149.9 / 107.9 | 154.0 / 110.0 | 216.4 / 156.9 | 11.8 / 9.0 |
| `0.3` --finite-decimal | 0.3 (yes) | 44.65→33.24 | 1131 | 615 | 1.84× | 1:404 | 193 / 193 of 405 | 358.8 / 188.0 | 369.7 / 192.0 | 357.4 / 199.8 | 21.5 / 14.6 |
| `fn f(y)=y*2` / `fn g(x)=f(x)+1` / `g(2)` | 5 (yes) | 44.65→13.38 | 4261 | 1754 | 2.43× | 1:253 2:581 3:1358 | 1613 / 1489 of 2193 | 1346.4 / 523.3 | 1375.0 / 538.9 | 1397.1 / 604.1 | 93.2 / 40.1 |

Mean per module, ms (before / after):

| Vector | check | backend | worker prepare | re-hash | assemble | V8 compile | execute | commit | total | WAT KiB |
|---|---|---|---|---|---|---|---|---|---|---|
| `1+2` | 439.9 / 258.7 | 455.5 / 266.7 | 627.8 / 369.3 | 552.1 / 329.1 | 58 / 30.3 | 15.7 / 8.1 | 0.8 / 0.8 | 31.7 / 20.4 | 1562.1 / 920.3 | 219 / 219 |
| `2*3` | 667.5 / 384.4 | 696.1 / 397.3 | 757.8 / 427.7 | 649.3 / 368.3 | 83.7 / 45.5 | 22.1 / 11.6 | 1 / 1 | 46.3 / 29.6 | 2177.5 / 1246 | 218 / 218 |
| `12.34` | 471.4 / 339.3 | 484.3 / 345.9 | 680.6 / 493.5 | 595.3 / 438.3 | 65.2 / 42 | 18 / 11.3 | 0.9 / 0.9 | 37.3 / 28.3 | 1682.8 / 1214.1 | 216 / 216 |
| `0.3` --finite-decimal | 885.8 / 464.2 | 912.8 / 474 | 882.6 / 493.4 | 754.8 / 416.4 | 98.8 / 58.9 | 25.2 / 14.6 | 1.5 / 1.5 | 53.1 / 36.1 | 2745.8 / 1476.9 | 232 / 232 |
| `fn f(y)=y*2` / `fn g(x)=f(x)+1` / `g(2)` | 614 / 238.6 | 627 / 245.7 | 637.1 / 275.5 | 554.3 / 237.4 | 63.6 / 28.6 | 16.9 / 7.2 | 0.9 / 1 | 42.5 / 18.3 | 1927.1 / 782.2 | 197 / 197 |

After binary, mean per module by host/worker cache state, ms:

| Vector | host/worker | modules | check | backend | worker prepare | execute | commit | total |
|---|---|---|---|---|---|---|---|---|
| `1+2` | bypass/miss | 2 | 198.7 | 186.5 | 704.8 | 2.1 | 27.8 | 1122.5 |
| `1+2` | hit/hit | 225 | 0 | 0.3 | 1.7 | 0.8 | 0.5 | 3.6 |
| `1+2` | miss/miss | 223 | 520.2 | 536.2 | 737.1 | 0.8 | 40.5 | 1843.5 |
| `2*3` | bypass/miss | 2 | 327.9 | 317.3 | 864.8 | 2.3 | 44.4 | 1560.7 |
| `2*3` | hit/hit | 220 | 0 | 0.4 | 1.6 | 0.7 | 0.5 | 3.4 |
| `2*3` | miss/miss | 216 | 776.4 | 802.3 | 857.7 | 1.2 | 59.1 | 2508.6 |
| `12.34` | bypass/miss | 2 | 239.9 | 217.2 | 742.3 | 2.3 | 34.4 | 1239.3 |
| `12.34` | hit/hit | 127 | 0 | 0.4 | 1.4 | 0.7 | 0.5 | 3.2 |
| `12.34` | miss/miss | 189 | 568.3 | 579.5 | 821.5 | 1 | 46.9 | 2027.6 |
| `0.3` --finite-decimal | bypass/miss | 2 | 460.7 | 401.4 | 1069.8 | 6.8 | 67.7 | 2013.2 |
| `0.3` --finite-decimal | hit/hit | 193 | 0 | 0.4 | 2.3 | 1 | 0.5 | 4.5 |
| `0.3` --finite-decimal | miss/miss | 210 | 890.8 | 909.9 | 939.4 | 1.9 | 68.6 | 2825.1 |
| `fn f(y)=y*2` / `fn g(x)=f(x)+1` / `g(2)` | bypass/miss | 4 | 420.8 | 399.9 | 921 | 4.9 | 63.8 | 1816.2 |
| `fn f(y)=y*2` / `fn g(x)=f(x)+1` / `g(2)` | hit/hit | 1318 | 0 | 0.4 | 1.7 | 0.8 | 0.5 | 3.6 |
| `fn f(y)=y*2` / `fn g(x)=f(x)+1` / `g(2)` | hit/miss | 295 | 0 | 0.9 | 789.8 | 1.1 | 0.7 | 792.8 |
| `fn f(y)=y*2` / `fn g(x)=f(x)+1` / `g(2)` | miss/hit | 171 | 855 | 884.6 | 2.8 | 1.5 | 61.7 | 1813.7 |
| `fn f(y)=y*2` / `fn g(x)=f(x)+1` / `g(2)` | miss/miss | 405 | 927 | 951.3 | 900.6 | 1.5 | 70.2 | 2862.6 |

## Merged build: module cache on vs off (`0b8c7f9`, debug)

`1+2` was entered twice in one session. Two processes ran concurrently, one with
`NOBLE_CALCULATOR_MODULE_CACHE=on` and one with `off`; load average was 12.01 at
the start and 8.78 at the end.

| Cache | Output | Wall | Max RSS KiB | Modules (line 1 / line 2) | Line 1 ms | Line 2 ms | Host/worker cache states |
|---|---|---|---|---|---|---|---|
| on | 3, 3 | 5:08.12 | 503,836 | 441 / 441 | 303,293 | 673 | hit/hit 660, miss/miss 222, bypass/miss 1 |
| off | 3, 3 | 18:08.07 | 357,000 | 441 / 441 | 547,095 | 537,345 | off/miss 883 |

With the cache on, the second `1+2` fully reuses the first line's modules.
The two traces match across all 883 modules: each module has the same
`source_sha256`, `wat_sha256`, `wasm_sha256`, `report_sha256` and
`work_issued` in both modes.

## Non-claims

- These wall times are not benchmarks. They were taken on a shared, loaded
  host, and concurrent runs competed with each other.
- Reuse changes preparation time only. Every module still runs in the guest,
  and outputs are identical with the cache on or off.
- No CALC case state is changed by these measurements.
