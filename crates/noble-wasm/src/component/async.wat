  ;; One direct-style import is outstanding at a time. Offsets 0..8 are
  ;; reserved event scratch, below literals; $busy excludes guest reentrancy.
  ;; The engine, not a guest scheduler, suspends this stack at wait.
  (func $await-subtask (param $packed i32)
    (local $task i32) (local $set i32) (local $state i32)
    (local $next i32) (local $remaining i32)
    ;; An eagerly returned call has no native subtask handle to drop.
    local.get $packed i32.const 2 i32.eq if return end
    local.get $packed i32.const 15 i32.and local.tee $state
    i32.const 1 i32.gt_u if unreachable end
    local.get $packed i32.const 4 i32.shr_u local.tee $task
    i32.eqz if unreachable end
    call $waitable-set-new local.set $set
    local.get $task local.get $set call $waitable-join
    i32.const 2 local.set $remaining
    block $returned
      loop $progress
        local.get $remaining i32.eqz if unreachable end
        local.get $remaining i32.const 1 i32.sub local.set $remaining
        local.get $set i32.const 0 call $waitable-set-wait
        i32.const 1 i32.ne if unreachable end
        i32.const 0 i32.load local.get $task i32.ne if unreachable end
        i32.const 4 i32.load local.tee $next
        local.get $state i32.le_u if unreachable end
        local.get $next i32.const 4 i32.gt_u if unreachable end
        local.get $next local.set $state
        local.get $state i32.const 2 i32.ge_u br_if $returned
        br $progress
      end
    end
    local.get $task i32.const 0 call $waitable-join
    local.get $task call $subtask-drop
    local.get $set call $waitable-set-drop
    ;; Native cancellation is not a successful result. The shell retains
    ;; abnormal ownership/retirement obligations after this trap.
    local.get $state i32.const 2 i32.ne if unreachable end)
