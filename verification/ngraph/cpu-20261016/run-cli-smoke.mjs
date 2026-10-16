const exe = '/home/brittonr/.cache/noble-issues/targets/ngraph-cpu-selected-20261016/release/noble';
const root = '/home/brittonr/.cache/noble-issues/evidence/ngraph-cpu-20261016/cli';
const commands = [];
function run(args, code = 0) {
  const p = Bun.spawnSync([exe, 'ngraph', ...args], {cwd:'/home/brittonr/.cache/noble-issues/wt/ngraph-cpu-20261016'});
  const stdout=p.stdout.toString(), stderr=p.stderr.toString();
  const records=stdout.trim().split('\n').filter(Boolean).map(x=>JSON.parse(x));
  commands.push({args,exit:p.exitCode,stdout,stderr});
  if(p.exitCode!==code) throw Error(`unexpected ${p.exitCode}: ${stdout} ${stderr}`);
  return records;
}
const common=[`${root}/model.ngraph`,'--loss',`${root}/loss.ngraph`,'--data',`${root}/data.ngraph`,'--policy','ngraph-deriv-real/0','--optimizer','adam/0','--seed','123'];
const seed=run(['train',...common,'--steps','0','--out',`${root}/selected-seed.snapshot`])[0];
const original=await Bun.file(`${root}/selected-seed.snapshot`).arrayBuffer();
const trained=run(['train',...common,'--steps','10','--from',`${root}/selected-seed.snapshot`,'--out',`${root}/selected-trained.snapshot`])[0];
const replay=run(['verify',`${root}/model.ngraph`,`${root}/selected-trained.snapshot`,'--replay','--loss',`${root}/loss.ngraph`,'--data',`${root}/data.ngraph`,'--from',`${root}/selected-seed.snapshot`])[0];
if(!replay.replayed||trained.parent!==seed.snapshot)throw Error('lineage/replay mismatch');
const evArgs=['eval',`${root}/model.ngraph`,`${root}/vectors.ngraph`,'--snapshot',`${root}/selected-trained.snapshot`];
const compiled=run(evArgs), reference=run([...evArgs,'--reference']);
if(JSON.stringify(compiled)!==JSON.stringify(reference))throw Error('L3/L4 mismatch');
const output=compiled.slice(1).map(r=>r.outputs[0]);
if(JSON.stringify(output)!==JSON.stringify(['f32:0x80000000','f32:0x00000000','f32:nan','f32:0xff800000','f32:0x3ecc195f']))throw Error('numerical output mismatch');
const refused=run(['eval',`${root}/model.ngraph`,`${root}/vectors.ngraph`],2);
if(refused.length!==1||refused[0].code!=='E-SNAPSHOT-LAYOUT')throw Error('binding refusal mismatch');
for(const mode of [[],['--reference']]) {
 const guard=run(['eval',`${root}/guard.ngraph`,`${root}/vectors.ngraph`,...mode],1);
 if(guard[3].code!=='G-FACT-FALSE'||guard[3].site!=='node %1')throw Error('guard refusal mismatch');
 const fuel=run([...evArgs,...mode,'--fuel','0'],5);
 if(!fuel.slice(1).every(r=>r.code==='E-BUDGET-FUEL'))throw Error('fuel refusal mismatch');
}
const after=await Bun.file(`${root}/selected-seed.snapshot`).arrayBuffer();
if(!Buffer.from(original).equals(Buffer.from(after)))throw Error('parent mutated');
const hash=(bytes)=>new Bun.CryptoHasher('sha256').update(bytes).digest('hex');
const packet={source_revision:'7e1de8f1eccdece595dea295a6e00a14c93b2e62',executable_sha256:hash(await Bun.file(exe).arrayBuffer()),checks:{l3_l4_exact_reports:true,parent_bytes_unchanged:true,replayed:true,actual_train_publication:true,guard_and_binding_refusal:true,fuel_refusal:true},commands};
await Bun.write('/home/brittonr/.cache/noble-issues/evidence/ngraph-cpu-20261016/cli-selected-smoke.json',JSON.stringify(packet,null,2)+'\n');
console.log(JSON.stringify(packet.checks));
