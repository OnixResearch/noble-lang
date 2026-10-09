set pagination off
set confirm off
set disable-randomization off
set print elements 64
set print max-depth 2
set logging file /tmp/noble-ma1-universal-recovered-20261009-target/numeric-postscan-private-operands-dual-node-forensics-20261009-gdb-trace.txt
set logging overwrite on
set logging redirect on
set logging enabled on
break /home/brittonr/.cache/noble-ma1-universal-recovered-20261009/crates/noble-contracts/src/metering.rs:32
commands 1
silent
printf "NODE_EXHAUSTION_BREAKPOINT\n"
python
import gdb
for frame, label, expression in (
    (3, "NAMED_CALL", "call"),
    (3, "NAMED_DEFINITION", "*definition"),
    (4, "NAMED_NODE_ID", "node_id"),
    (4, "NAMED_NODE", "*node"),
):
    try:
        gdb.execute("frame " + str(frame), to_string=True)
        gdb.execute("set print max-depth 4", to_string=True)
        gdb.write(label + " " + gdb.execute("print " + expression, to_string=True))
    except Exception as error:
        gdb.write(label + "_ERROR " + str(error) + "\n")
try:
    gdb.execute("frame 0", to_string=True)
    gdb.execute("set print max-depth 4", to_string=True)
    gdb.write("METER_SNAPSHOT " + gdb.execute("print *self", to_string=True))
except Exception as error:
    gdb.write("METER_SNAPSHOT_ERROR " + str(error) + "\n")
finally:
    gdb.execute("set print max-depth 2", to_string=True)
end
bt
bt full
continue
end
run session --framed --declared-modules --text-byte-cursor --signed-self-recursion --source-bytes 65536 --source-nodes 16384 --source-depth 64 --source-work 2000000 --retained-source-bytes 262144 < /tmp/noble-ma1-universal-recovered-20261009-target/numeric-registration-postscan-private-operands-38-sourceonly-20261009-stdin.framed > /tmp/noble-ma1-universal-recovered-20261009-target/numeric-postscan-private-operands-dual-node-forensics-20261009-cli-stdout.jsonl 2> /tmp/noble-ma1-universal-recovered-20261009-target/numeric-postscan-private-operands-dual-node-forensics-20261009-cli-stderr.txt
set logging enabled off
quit
