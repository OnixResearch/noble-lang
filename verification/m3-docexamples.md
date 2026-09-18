# M3 executed documentation examples (fragment v1)

Executed evidence per [m3-fragment.md](m3-fragment.md) "Frozen per-word
fixture list" and task 3.2: every one of the 23 bootstrap words has at
least one `noble-check` example executed against the actual kernel
checker by `crates/noble-kernel/tests/docexamples.rs`, which also
asserts the per-word coverage (a word without an executed example fails
the suite). Fence format and defaults are defined in
[m2-fragment.md](m2-fragment.md) "Executed documentation examples"; all
examples below use the supported `noble-candidate/v1` revision. Each
positive example is one invocation of the word against a request that
matches its documented contract; each rejection names the constraint it
trips.

## Positive example per word

```noble-check expect=accepted
{"request":{"expected":{"in":[{"i64":{}}],"out":[{"i64":{}},{"i64":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"dup","inst":{"v0":[],"v1":{"i64":{}}}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{"in":[{"i64":{}}],"out":[],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"drop","inst":{"v0":[],"v1":{"i64":{}}}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{"in":[{"i64":{}},{"bool":{}}],"out":[{"bool":{}},{"i64":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"swap","inst":{"v0":[],"v1":{"i64":{}},"v2":{"bool":{}}}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{"in":[{"i64":{}},{"program":{"in":[],"out":[{"unit":{}}],"effects":[]}}],
 "out":[{"unit":{}},{"i64":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"dip","inst":{"v0":[],"v1":{"i64":{}},"v2":[{"unit":{}}],"v3":[]}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{"in":[{"i64":{}},{"i64":{}}],"out":[{"i64":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"+","inst":{"v0":[]}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{"in":[{"i64":{}},{"i64":{}}],"out":[{"i64":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"-","inst":{"v0":[]}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{"in":[{"i64":{}},{"i64":{}}],"out":[{"i64":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"*","inst":{"v0":[]}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{"in":[{"i64":{}},{"i64":{}}],"out":[{"bool":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"=","inst":{"v0":[]}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{"in":[{"i64":{}}],
 "out":[{"program":{"in":[{"i64":{}}],"out":[{"i64":{}},{"i64":{}}],"effects":[]}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"quote","inst":{"v0":[],"v1":{"i64":{}},"v2":[{"i64":{}}]}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{
   "in":[{"program":{"in":[],"out":[{"i64":{}}],"effects":[]}},
         {"program":{"in":[{"i64":{}}],"out":[{"unit":{}}],"effects":[]}}],
   "out":[{"program":{"in":[],"out":[{"unit":{}}],"effects":[]}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"compose","inst":{"v0":[],"v1":[],"v2":[{"i64":{}}],"v3":[{"unit":{}}],"v4":[],"v5":[]}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{"in":[{"program":{"in":[],"out":[{"unit":{}}],"effects":[]}}],
 "out":[{"unit":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"run","inst":{"v0":[],"v1":[{"unit":{}}],"v2":[]}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{"in":[{"program":{"in":[],"out":[{"unit":{}}],"effects":[]}}],
 "out":[{"syntax":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"reflect","inst":{"v0":[],"v1":[],"v2":[{"unit":{}}],"v3":[]}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{"in":[],"out":[{"unit":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"unit","inst":{"v0":[]}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{"in":[{"i64":{}},{"bool":{}}],"out":[{"pair":[{"i64":{}},{"bool":{}}]}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"pair","inst":{"v0":[],"v1":{"i64":{}},"v2":{"bool":{}}}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{"in":[{"pair":[{"i64":{}},{"bool":{}}]}],"out":[{"i64":{}},{"bool":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"unpair","inst":{"v0":[],"v1":{"i64":{}},"v2":{"bool":{}}}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{"in":[{"i64":{}}],"out":[{"sum":[{"i64":{}},{"bool":{}}]}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"inl","inst":{"v0":[],"v1":{"i64":{}},"v2":{"bool":{}}}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{"in":[{"bool":{}}],"out":[{"sum":[{"i64":{}},{"bool":{}}]}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"inr","inst":{"v0":[],"v1":{"i64":{}},"v2":{"bool":{}}}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{
   "in":[{"sum":[{"i64":{}},{"bool":{}}]},
         {"program":{"in":[{"i64":{}}],"out":[{"unit":{}}],"effects":[]}},
         {"program":{"in":[{"bool":{}}],"out":[{"unit":{}}],"effects":[]}}],
   "out":[{"unit":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"case","inst":{"v0":[],"v1":{"i64":{}},"v2":{"bool":{}},"v3":[{"unit":{}}],"v4":[],"v5":[]}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{
   "in":[{"bool":{}},
         {"program":{"in":[],"out":[{"unit":{}}],"effects":[]}},
         {"program":{"in":[],"out":[{"unit":{}}],"effects":[]}}],
   "out":[{"unit":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"if","inst":{"v0":[],"v1":[{"unit":{}}],"v2":[],"v3":[]}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{"in":[],"out":[{"list":{"i64":{}}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"nil","inst":{"v0":[],"v1":{"i64":{}}}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{"in":[{"i64":{}},{"list":{"i64":{}}}],"out":[{"list":{"i64":{}}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"cons","inst":{"v0":[],"v1":{"i64":{}}}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{
   "in":[{"list":{"i64":{}}},
         {"program":{"in":[],"out":[{"unit":{}}],"effects":[]}},
         {"program":{"in":[{"i64":{}},{"list":{"i64":{}}}],"out":[{"unit":{}}],"effects":[]}}],
   "out":[{"unit":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"list.case","inst":{"v0":[],"v1":{"i64":{}},"v2":[{"unit":{}}],"v3":[],"v4":[]}}],
  "body":[0]}}
```

```noble-check expect=accepted
{"request":{"expected":{"in":[{"text":{}}],"out":[{"unit":{}}],"allowed_effects":["test.emit"]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"test.emit","inst":{"v0":[]}}],
  "body":[0]}}
```

## Rejection example per constrained word

```noble-check expect=invalid
{"request":{"expected":{"in":[{"resource":"test.counter"}],"out":[{"resource":"test.counter"},{"resource":"test.counter"}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"dup","inst":{"v0":[],"v1":{"resource":"test.counter"}}}],
  "body":[0]}}
```

```noble-check expect=invalid
{"request":{"expected":{"in":[{"resource":"test.counter"}],"out":[],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"drop","inst":{"v0":[],"v1":{"resource":"test.counter"}}}],
  "body":[0]}}
```

```noble-check expect=invalid
{"request":{"expected":{"in":[{"resource":"test.counter"}],"out":[{"resource":"test.counter"}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"quote","inst":{"v0":[],"v1":{"resource":"test.counter"},"v2":[{"resource":"test.counter"}]}}],
  "body":[0]}}
```

```noble-check expect=invalid
{"request":{"expected":{"in":[{"i64":{}},{"program":{"in":[],"out":[{"unit":{}}],"effects":[]}}],
 "out":[{"unit":{}},{"i64":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"dip","inst":{"v0":[],"v1":{"i64":{}},"v2":[{"unit":{}}],"v3":["test.emit"]}}],
  "body":[0]}}
```

```noble-check expect=invalid
{"request":{"expected":{
   "in":[{"program":{"in":[],"out":[{"i64":{}}],"effects":[]}},
         {"program":{"in":[{"i64":{}}],"out":[{"unit":{}}],"effects":[]}}],
   "out":[{"program":{"in":[],"out":[{"unit":{}}],"effects":[]}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"compose","inst":{"v0":[],"v1":[],"v2":[{"i64":{}}],"v3":[{"unit":{}}],"v4":[],"v5":["test.emit"]}}],
  "body":[0]}}
```

```noble-check expect=invalid
{"request":{"expected":{"in":[{"program":{"in":[],"out":[{"unit":{}}],"effects":[]}}],
 "out":[{"unit":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"run","inst":{"v0":[],"v1":[{"unit":{}}],"v2":["test.emit"]}}],
  "body":[0]}}
```

```noble-check expect=invalid
{"request":{"expected":{
   "in":[{"sum":[{"i64":{}},{"bool":{}}]},
         {"program":{"in":[{"i64":{}}],"out":[{"unit":{}}],"effects":[]}},
         {"program":{"in":[{"bool":{}}],"out":[{"unit":{}}],"effects":[]}}],
   "out":[],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"case","inst":{"v0":[],"v1":{"i64":{}},"v2":{"bool":{}},"v3":[],"v4":[],"v5":[]}}],
  "body":[0]}}
```

```noble-check expect=invalid
{"request":{"expected":{
   "in":[{"bool":{}},
         {"program":{"in":[],"out":[{"unit":{}}],"effects":["test.emit"]}},
         {"program":{"in":[],"out":[{"unit":{}}],"effects":[]}}],
   "out":[{"unit":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"if","inst":{"v0":[],"v1":[{"unit":{}}],"v2":["test.emit"],"v3":[]}}],
  "body":[0]}}
```

```noble-check expect=invalid
{"request":{"expected":{
   "in":[{"list":{"i64":{}}},
         {"program":{"in":[],"out":[{"unit":{}}],"effects":[]}},
         {"program":{"in":[{"i64":{}},{"list":{"i64":{}}}],"out":[{"unit":{}}],"effects":[]}}],
   "out":[],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"list.case","inst":{"v0":[],"v1":{"i64":{}},"v2":[],"v3":[],"v4":[]}}],
  "body":[0]}}
```

```noble-check expect=invalid
{"request":{"expected":{"in":[{"text":{}}],"out":[{"unit":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"invocation","def":"test.emit","inst":{"v0":[]}}],
  "body":[0]}}
```

```noble-check expect=invalid
{"request":{"expected":{"in":[{"i64":{}},{"i64":{}}],"out":[{"bool":{}}],"allowed_effects":[]}},
 "candidate":{"format":"noble-candidate/v1","nodes":[
   {"kind":"literal","lit":{"i64":1},"inst":{"v0":[]}},
   {"kind":"literal","lit":{"bool":true},"inst":{"v0":[{"i64":{}}]}},
   {"kind":"invocation","def":"=","inst":{"v0":[]}}],
  "body":[0,1,2]}}
```
