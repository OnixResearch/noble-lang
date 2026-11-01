# Proposed focus sugar (not selected syntax, not compiled here)

These two snippets are what a finite focus surface *would* look like at the use sites above. No
parser, elaborator or checker admits them today, so `measure.mjs` does not compile them and the
lane README reports only their authored size (tokens), labelled model-only.

```
# nested-product: descend to the first component of the pair, apply one checked operation there,
# and rebuild. Untouched components are never rebuilt or re-observed.
1 5 pair focus .0 (+ 1)
```

Expansion (the `nested-product.library` form, which *is* compiled and timed):

```
1 5 pair [ unpair ] [ swap 1 + swap ] compose [ pair ] compose run
```

```
# type-changing-step: one active focus whose operation has a different input and output type.
7 focus (0 =)
```

Expansion (the `type-changing-step.library` form):

```
7 [ 0 ] [ = ] compose run
```

The design that a sugar would need in order to be selected is stated in the issue comment with
these numbers: a finite path grammar, exactly one active focus, a residual operation, an error-return
rule at each focus boundary, and the move-threaded refusal (the sugar must not borrow the
immutable-local lowering for a must-use capability).
