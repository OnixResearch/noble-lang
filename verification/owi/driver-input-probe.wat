(module
  (import "owi" "i64_symbol" (func $i64_symbol (result i64)))
  (import "subject" "begin" (func $begin (param i32 i32 i32 i32 i32 i32) (result i32)))
  (import "subject" "push_i64" (func $push_i64 (param i64) (result i32)))
  (import "subject" "submit" (func $submit (result i32)))
  (import "subject" "stack_length" (func $stack_length (result i32)))
  (import "subject" "stack_value" (func $stack_value (param i32) (result i64)))
  (func (export "run")
    i32.const 196608
    i32.const 256
    i32.const 256
    i32.const 2048
    i32.const 4096
    i32.const 100000
    call $begin
    if unreachable end

    call $i64_symbol
    call $push_i64
    if unreachable end
    call $i64_symbol
    call $push_i64
    if unreachable end

    call $submit
    if unreachable end

    call $stack_length
    i32.const 1
    i32.ne
    if unreachable end
    i32.const 0
    call $stack_value
    i64.const 26
    i64.ne
    if unreachable end
  )
)
