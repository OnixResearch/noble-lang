#!/usr/bin/env python3
"""Exercise the real local cgroup stopper; never substitute fake quota replies."""
import importlib.util
import json
import os
import pathlib
import secrets
import signal
import subprocess
import sys
import time

SOURCE_ROOT = pathlib.Path('/home/brittonr/.cache/noble-octet-stage2-successor-impl-r2-20261017')
OUTPUT_ROOT = pathlib.Path(__file__).resolve().parent
PY_SCRIPT = 'stage2-guard-union-cgroup-v10.py'
SYSTEMCTL = '/run/current-system/sw/bin/systemctl'
SYSTEMD_RUN = '/run/current-system/sw/bin/systemd-run'
SLEEP = '/run/current-system/sw/bin/sleep'


def run(mode):
    if mode not in ('after-late', 'after-fresh', 'after-queued-expired'):
        raise SystemExit('usage: deadline-regression.py after-late|after-fresh|after-queued-expired')
    source = SOURCE_ROOT / PY_SCRIPT
    spec = importlib.util.spec_from_file_location('deadline_guard', source)
    guard = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(guard)
    guard.SYSTEMCTL = SYSTEMCTL
    uid = os.getuid()
    unit = 'noble-octet-r2-deadline-' + str(os.getpid()) + '-' + secrets.token_hex(3) + '.scope'
    expected = f'/user.slice/user-{uid}.slice/user@{uid}.service/app.slice/{unit}'
    command = [SYSTEMD_RUN, '--user', '--scope', '--collect', '--unit=' + unit,
               '--property=RuntimeMaxSec=1min', '--', SLEEP, '12']
    process = subprocess.Popen(command, stdin=subprocess.DEVNULL,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    scope_pin = None
    watcher_pid = None
    watcher_paused = False
    write_fd = None
    lease_fd = None
    try:
        startup_limit = time.monotonic() + 3
        while time.monotonic() < startup_limit:
            if process.poll() is not None:
                raise RuntimeError('local scope exited before exact pin')
            props = guard.unit_properties(unit)
            if (props.get('LoadState') == 'loaded' and
                props.get('ControlGroup') == expected and
                guard.scope_members(expected)):
                break
            time.sleep(.05)
        else:
            raise RuntimeError('unique local cgroup could not be pinned: ' + repr(props))
        scope_pin = guard.pin_scope(unit, expected)
        lease_fd = os.open(OUTPUT_ROOT / 'local-lease-fixture.txt', os.O_RDONLY | os.O_CLOEXEC)
        emergency = OUTPUT_ROOT / ('failsafe-' + unit + '.json')
        watcher_pid, write_fd = guard.start_failsafe(
            lease_fd, unit, expected, scope_pin, emergency)
        host_pin = guard.pin_host_root(expected_device=os.stat('/').st_dev)
        acquired_ns = time.monotonic_ns()
        actual_host = guard.cheap_snapshot(host_pin)
        assert actual_host['host_free_bytes'] > 0
        if mode == 'after-queued-expired':
            guard.heartbeat(write_fd, expected, scope_pin, acquired_ns)
            time.sleep(.05)
            os.kill(watcher_pid, signal.SIGSTOP)
            watcher_paused = True
            until_ns = acquired_ns + 2_150_000_000
            while time.monotonic_ns() < until_ns:
                time.sleep(max(0, min(.02, (until_ns - time.monotonic_ns()) / 1e9)))
            assert not guard.scope_empty(expected, scope_pin)
            renewed_ns = time.monotonic_ns()
            assert guard.cheap_snapshot(host_pin)['host_free_bytes'] > 0
            guard.heartbeat(write_fd, expected, scope_pin, renewed_ns)
            os.kill(watcher_pid, signal.SIGCONT)
            watcher_paused = False
            timeout = time.monotonic() + 3
            while time.monotonic() < timeout and not guard.scope_empty(expected, scope_pin):
                time.sleep(.01)
            assert guard.scope_empty(expected, scope_pin)
            os.close(write_fd)
            write_fd = None
            _, wait_status = os.waitpid(watcher_pid, 0)
            watcher_pid = None
            assert os.waitstatus_to_exitcode(wait_status) == 3
            result = json.loads(emergency.read_text())
            assert (result['status'] == 'failsafe-stopped' and
                    'exact cgroup empty' in result['stop_result'] and
                    result['deadline_monotonic_ns'] <= acquired_ns + guard.SAMPLE_DEADLINE_NS)
            print(json.dumps({'mode': mode, 'guard_path': str(source),
                              'unit': unit, 'oldest_acquired_ns': acquired_ns,
                              'original_expiry_ns': acquired_ns + guard.SAMPLE_DEADLINE_NS,
                              'queued_fresh_acquired_ns': renewed_ns,
                              'late_renewal_rejected': True, 'emergency': result},
                             sort_keys=True), flush=True)
            return
        time.sleep(1.4 if mode == 'after-late' else .7)
        guard.heartbeat(write_fd, expected, scope_pin, acquired_ns)
        sent_ns = time.monotonic_ns()
        refreshed_ns = None
        if mode == 'after-fresh':
            refreshed_ns = time.monotonic_ns()
            second_host = guard.cheap_snapshot(host_pin)
            assert second_host['host_free_bytes'] > 0
            guard.heartbeat(write_fd, expected, scope_pin, refreshed_ns)
        target_ns = acquired_ns + 2_150_000_000
        while time.monotonic_ns() < target_ns:
            time.sleep(max(0, min(.02, (target_ns - time.monotonic_ns()) / 1e9)))
        check_ns = time.monotonic_ns()
        empty_at_check = guard.scope_empty(expected, scope_pin)
        result = {'mode': mode, 'guard_path': str(source), 'unit': unit,
                  'expected_cgroup': expected, 'acquired_ns': acquired_ns,
                  'decision_delay_ns': sent_ns - acquired_ns,
                  'first_expiry_ns': acquired_ns + guard.SAMPLE_DEADLINE_NS,
                  'check_ns': check_ns, 'checked_after_original_expiry_ns':
                      check_ns - acquired_ns - guard.SAMPLE_DEADLINE_NS,
                  'scope_empty_at_check': empty_at_check,
                  'members_at_check': sorted(guard.scope_members(expected)),
                  'refreshed_ns': refreshed_ns}
        if mode == 'after-late':
            assert empty_at_check, 'r2 did not stop on oldest sample expiry'
        else:
            assert not empty_at_check, 'r2 incorrectly stopped after a fresh complete renewal'
        if not empty_at_check:
            result['manual_exact_stop'] = guard.stop_verified_scope(unit, expected, scope_pin)
        assert guard.scope_empty(expected, scope_pin)
        os.close(write_fd)
        write_fd = None
        completed_pid, watcher_status = os.waitpid(watcher_pid, 0)
        watcher_pid = None
        result['watcher_exit'] = os.waitstatus_to_exitcode(watcher_status)
        if mode == 'after-late':
            assert result['watcher_exit'] == 3
            result['emergency'] = json.loads(emergency.read_text())
            assert result['emergency']['status'] == 'failsafe-stopped'
            assert result['emergency']['deadline_monotonic_ns'] == result['first_expiry_ns']
            assert 'exact cgroup empty' in result['emergency']['stop_result']
        else:
            assert result['watcher_exit'] == 0
        result['cgroup_empty_final'] = guard.scope_empty(expected, scope_pin)
        print(json.dumps(result, sort_keys=True), flush=True)
    finally:
        if watcher_paused and watcher_pid is not None:
            os.kill(watcher_pid, signal.SIGCONT)
        if write_fd is not None:
            os.close(write_fd)
        if scope_pin is not None:
            if not guard.scope_empty(expected, scope_pin):
                guard.stop_verified_scope(unit, expected, scope_pin)
            os.close(scope_pin[1])
            os.close(scope_pin[0])
        if lease_fd is not None:
            os.close(lease_fd)
        if watcher_pid is not None:
            os.waitpid(watcher_pid, 0)
        if process.poll() is None:
            process.wait(timeout=8)


if __name__ == '__main__':
    run(sys.argv[1] if len(sys.argv) == 2 else '')
