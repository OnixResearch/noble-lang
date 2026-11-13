; ngraph-llvm-i64/0 module: one straight-line function, no memory, no calls
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

define i64 @noble_i64(i64 noundef %a0, i64 noundef %a1) {
entry:
  %v2 = mul i64 %a0, %a0
  %v3 = add i64 %v2, %v2
  %v5 = mul i64 %v3, -1
  %v6 = mul i64 %v2, -1
  %v7 = sub i64 %v5, %v6
  %v8 = add i64 %v7, %a1
  ret i64 %v3
}
