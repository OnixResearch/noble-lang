#!/usr/bin/env bash
# Usage: run.sh <name> <binary> <stdin>; writes <name>.{stdout,stderr,status,ALL_DONE|FAILED}
set -u
name=$1; bin=$2; input=$3
dir=/home/brittonr/.cache/noble-issues/receipts/issue-17
mkdir -p "$dir/tmp/$name"
start=$(date +%s%N)
env -i PATH=/nonexistent LANG=C LC_ALL=C TMPDIR="$dir/tmp/$name" "$bin" calculator < "$input" > "$dir/$name.stdout" 2> "$dir/$name.stderr"
status=$?
end=$(date +%s%N)
echo "{\"name\":\"$name\",\"binary\":\"$bin\",\"sha256\":\"$(sha256sum "$bin" | cut -d' ' -f1)\",\"stdin\":\"$input\",\"exit\":$status,\"start\":$start,\"end\":$end,\"wall_ms\":$(( (end - start) / 1000000 ))}" > "$dir/$name.status"
if [ -s "$dir/$name.status" ]; then touch "$dir/$name.ALL_DONE"; else touch "$dir/$name.FAILED"; fi
