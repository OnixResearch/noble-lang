# Proposed focus sugar (not selected syntax, not compiled here)

These snippets are what a finite focus surface *would* look like at the use sites the compiled forms
cover. No parser, elaborator or checker admits them today, so `measure.mjs` never compiles them: it
costs them by the same whitespace-token rule it applies to the `.noble` files, marks them
`"compilable": false`, and records the compiled form each one expands to. A sugar row is an authored
size, never a compile time, a module size or a step count.

The fenced blocks carry markers in their info string so the cost is machine-read rather than
hand-copied: `role=sugar` blocks are the proposal, `role=expansion` blocks restate a compiled form,
and the script refuses a run in which an expansion block no longer equals the form it names.

## nested-product

Descend to the first component of the pair, apply one checked operation there, and rebuild.
Untouched components are never rebuilt or re-observed.

```noble role=sugar name=nested-product expands-to=nested-product.dip compare=nested-product.explicit,nested-product.dip,nested-product.library
1 5 pair focus .0 (+ 1)
```

Issue step 3 prefers static inlining over a residual captured as a first-class Program, so the
comparison that matters is against what a static elaboration would emit. In this profile that is
either the plain explicit framing or `dip`, the present word for reaching the cell under the top
one; both are emitted here without capturing anything.

```noble role=expansion name=nested-product.explicit form=nested-product.explicit
1 5 pair unpair swap 1 + swap pair
```

```noble role=expansion name=nested-product.dip form=nested-product.dip
1 5 pair unpair [ 1 + ] dip pair
```

A derived wiring library would instead supply composed quotations, which is what the third
expansion (and the lane's `library` form) is:

```noble role=expansion name=nested-product.library form=nested-product.library
1 5 pair [ unpair ] [ swap 1 + swap ] compose [ pair ] compose run
```

## type-changing-nested

A nested update whose inner operation has a different input and output type: the case the sugar
exists for, and the pair the 2026-10-31 revision of this lane was missing.

```noble role=sugar name=type-changing-nested expands-to=type-changing-nested.dip compare=type-changing-nested.explicit,type-changing-nested.dip,type-changing-nested.library
7 5 pair focus .0 (0 =)
```

```noble role=expansion name=type-changing-nested.explicit form=type-changing-nested.explicit
7 5 pair unpair swap 0 = swap pair
```

```noble role=expansion name=type-changing-nested.dip form=type-changing-nested.dip
7 5 pair unpair [ 0 = ] dip pair
```

```noble role=expansion name=type-changing-nested.library form=type-changing-nested.library
7 5 pair [ unpair ] [ swap 0 = swap ] compose [ pair ] compose run
```

## type-changing-step

One active focus whose operation has a different input and output type, with no surrounding pair.

```noble role=sugar name=type-changing-step expands-to=type-changing-step.library compare=type-changing-step.explicit,type-changing-step.library
7 focus (0 =)
```

```noble role=expansion name=type-changing-step.explicit form=type-changing-step.explicit
7 0 =
```

```noble role=expansion name=type-changing-step.library form=type-changing-step.library
7 [ 0 ] [ = ] compose run
```

## What this notation is *not*, and what selecting it would require

The notes above establish authored size only. The snippets assume a design that does not exist: a
finite path grammar (`.0` is a component index, not a word), exactly one active focus, a residual
operation, an error-return rule at each focus boundary, and the move-threaded refusal — the sugar
must not borrow the immutable-local lowering (`crates/noble-contracts/src/source/locals.rs`) for a
must-use capability, because that path has an implicit cleanup `drop` and no owner return.
`decision.md` records the comparison these snippets feed and what would reopen the decision.
