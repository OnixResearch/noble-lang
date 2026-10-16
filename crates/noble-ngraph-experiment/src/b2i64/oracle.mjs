// L2: independent ngraph-i64/0 reference oracle (B2).
//
// Evaluates an extracted graph over exact unbounded integers (JavaScript
// BigInt) and reduces every arithmetic result modulo 2^64 into the signed
// range by explicit integer arithmetic. It shares no code, language runtime
// or arithmetic primitive with the Rust L3/L4 evaluators (which use
// two's-complement wrapping machine operations).
//
// stdin:  {"nodes": [[op, ...]], "outputs": [id], "vectors": [["i:5", "b:1"]]}
//   ops:  ["input", k] ["i64", "decimal"] ["bool", 0|1]
//         ["add"|"sub"|"mul"|"eq", a, b]   (a = deeper operand)
// stdout: [["i:24", "b:0"], ...] one array of outputs per vector.

const M = 18446744073709551616n; // 2^64
const H = 9223372036854775808n; // 2^63

function reduce(x) {
  let r = x % M;
  if (r < 0n) r += M;
  return r >= H ? r - M : r;
}

function decode(cell) {
  if (cell.startsWith("i:")) return BigInt(cell.slice(2));
  if (cell === "b:1") return true;
  if (cell === "b:0") return false;
  throw new Error("bad input cell " + cell);
}

function encode(v) {
  return typeof v === "boolean" ? (v ? "b:1" : "b:0") : "i:" + v.toString();
}

let text = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (chunk) => (text += chunk));
process.stdin.on("end", () => {
  const job = JSON.parse(text);
  const out = [];
  for (const vector of job.vectors) {
    const v = [];
    for (const n of job.nodes) {
      switch (n[0]) {
        case "input": v.push(decode(vector[n[1]])); break;
        case "i64": v.push(BigInt(n[1])); break;
        case "bool": v.push(n[1] === 1); break;
        case "add": v.push(reduce(v[n[1]] + v[n[2]])); break;
        case "sub": v.push(reduce(v[n[1]] - v[n[2]])); break;
        case "mul": v.push(reduce(v[n[1]] * v[n[2]])); break;
        case "eq": v.push(v[n[1]] === v[n[2]]); break;
        default: throw new Error("op outside ngraph-i64/0: " + n[0]);
      }
    }
    out.push(job.outputs.map((o) => encode(v[o])));
  }
  process.stdout.write(JSON.stringify(out));
});
