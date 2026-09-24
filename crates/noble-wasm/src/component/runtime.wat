  (memory (export "memory") 16 16)
  (global $heap (mut i32) (i32.const 65536))
  (global $quota (mut i32) (i32.const 983040))
  (global $busy (mut i32) (i32.const 0))
  (global $allocations (mut i64) (i64.const 0))
  (global $allocated (mut i64) (i64.const 0))
  (global $copied (mut i64) (i64.const 0))
  (global $cleanups (mut i64) (i64.const 0))
  ;; '$' cannot occur in a WIT identifier; diagnostics never alias guest exports.
  (func (export "noble$set-allocation-limit") (param $limit i32)
    global.get $busy if unreachable end
    global.get $heap i32.const 65536 i32.ne if unreachable end
    local.get $limit i32.const 983040 i32.gt_u if unreachable end
    local.get $limit global.set $quota)
  (func (export "noble$allocation-count") (result i64) global.get $allocations)
  (func (export "noble$allocated-bytes") (result i64) global.get $allocated)
  (func (export "noble$copied-bytes") (result i64) global.get $copied)
  (func (export "noble$live-bytes") (result i32) global.get $heap i32.const 65536 i32.sub)
  (func (export "noble$cleanup-count") (result i64) global.get $cleanups)
  (func $cleanup (export "noble$cleanup")
    i32.const 65536 global.set $heap
    i32.const 0 global.set $busy
    global.get $cleanups i64.const 1 i64.add global.set $cleanups)
  (func $enter
    global.get $busy if unreachable end
    i32.const 1 global.set $busy)
  (func $range (param $ptr i32) (param $len i32)
    local.get $ptr i32.const 1048576 i32.gt_u if unreachable end
    local.get $len i32.const 1048576 local.get $ptr i32.sub i32.gt_u if unreachable end)
  (func $alloc (param $align i32) (param $size i32) (result i32)
    (local $ptr i32) (local $end i32)
    local.get $align i32.eqz if unreachable end
    local.get $align i32.const 8 i32.gt_u if unreachable end
    local.get $align local.get $align i32.const 1 i32.sub i32.and if unreachable end
    global.get $heap local.get $align i32.const 1 i32.sub i32.add
    i32.const 0 local.get $align i32.sub i32.and local.set $ptr
    local.get $ptr i32.const 1048576 i32.gt_u if unreachable end
    local.get $size i32.const 1048576 local.get $ptr i32.sub i32.gt_u if unreachable end
    local.get $ptr local.get $size i32.add local.set $end
    local.get $end i32.const 65536 i32.sub global.get $quota i32.gt_u if unreachable end
    local.get $end global.set $heap
    global.get $allocations i64.const 1 i64.add global.set $allocations
    global.get $allocated local.get $size i64.extend_i32_u i64.add global.set $allocated
    local.get $ptr)
  (func $copy (param $old i32) (param $size i32) (result i32)
    (local $ptr i32)
    local.get $old local.get $size call $range
    i32.const 1 local.get $size call $alloc local.set $ptr
    local.get $ptr local.get $old local.get $size memory.copy
    global.get $copied local.get $size i64.extend_i32_u i64.add global.set $copied
    local.get $ptr)
  (func (export "cabi_realloc") (param $old i32) (param $old_size i32) (param $align i32) (param $size i32) (result i32)
    (local $ptr i32) (local $count i32)
    local.get $old_size if
      local.get $old local.get $old_size call $range
      local.get $old i32.const 65536 i32.lt_u if unreachable end
      local.get $old local.get $old_size i32.add global.get $heap i32.gt_u if unreachable end
    end
    local.get $size i32.eqz if i32.const 0 return end
    local.get $align local.get $size call $alloc local.set $ptr
    local.get $old_size local.get $size local.get $old_size local.get $size i32.lt_u select local.set $count
    local.get $count if
      local.get $ptr local.get $old local.get $count memory.copy
      global.get $copied local.get $count i64.extend_i32_u i64.add global.set $copied
    end
    local.get $ptr)
  (func $utf8 (param $ptr i32) (param $len i32)
    (local $at i32) (local $lead i32) (local $next i32) (local $need i32)
    local.get $ptr local.get $len call $range
    block $done loop $scan
      local.get $at local.get $len i32.eq br_if $done
      local.get $ptr local.get $at i32.add i32.load8_u local.set $lead
      local.get $at i32.const 1 i32.add local.set $at
      local.get $lead i32.const 128 i32.lt_u br_if $scan
      local.get $lead i32.const 194 i32.lt_u if unreachable end
      local.get $lead i32.const 244 i32.gt_u if unreachable end
      i32.const 1 local.set $need
      local.get $lead i32.const 224 i32.ge_u if i32.const 2 local.set $need end
      local.get $lead i32.const 240 i32.ge_u if i32.const 3 local.set $need end
      local.get $need local.get $len local.get $at i32.sub i32.gt_u if unreachable end
      local.get $ptr local.get $at i32.add i32.load8_u local.set $next
      local.get $lead i32.const 224 i32.eq local.get $next i32.const 160 i32.lt_u i32.and if unreachable end
      local.get $lead i32.const 237 i32.eq local.get $next i32.const 160 i32.ge_u i32.and if unreachable end
      local.get $lead i32.const 240 i32.eq local.get $next i32.const 144 i32.lt_u i32.and if unreachable end
      local.get $lead i32.const 244 i32.eq local.get $next i32.const 144 i32.ge_u i32.and if unreachable end
      loop $continuations
        local.get $ptr local.get $at i32.add i32.load8_u local.set $next
        local.get $next i32.const 128 i32.lt_u if unreachable end
        local.get $next i32.const 191 i32.gt_u if unreachable end
        local.get $at i32.const 1 i32.add local.set $at
        local.get $need i32.const 1 i32.sub local.tee $need br_if $continuations
      end
      br $scan
    end end)
