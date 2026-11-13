; ngraph-llvm-i64/0 module: one straight-line function, no memory, no calls
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

define i64 @noble_i64(i64 noundef %a0, i64 %a1) {
entry:
  %v4 = add i64 %a0, 9223372036854775807
  %v5 = mul i64 -9223372036854775808, %v4
  %v6 = mul i64 %v5, %a1
  ret i64 %v6
}
