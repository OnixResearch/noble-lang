import {createHash} from 'node:crypto';
import {readFileSync,writeFileSync} from 'node:fs';

const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const files=['verification/decoder-experiment/review-snapshot.mjs',
  'specs/conformance/adaptation-cases.json',
  '.cairn/specs/program-contracts/spec.md'];
const source=Object.fromEntries(files.map(path=>[path,hash(readFileSync(path))]));
const item=JSON.parse(readFileSync('specs/conformance/adaptation-cases.json')).cases.find(row=>row.id==='ADAPT-14');
if(item?.kind!=='review' || item.input.harness!=='initial-final-contract-counterexample')
  throw Error('ADAPT-14 review input missing');
const {before,after,amount,snapshot_condition,incorrect_condition}=item.input;
if([before,after,amount].some(value=>value.type!=='I64')) throw Error('unsupported arithmetic subject');
const env={before:BigInt(before.value),after:BigInt(after.value),amount:BigInt(amount.value)};
const limit=(1n<<63n)-1n;
const preconditions=env.before>=0n && env.amount>=0n &&
  env.before<=limit && env.amount<=limit && env.before+env.amount<=limit;
if(!preconditions) throw Error('preconditions unestablished');
const expression=(text,values)=>{
  const match=/^(before|after|amount) = (before|after|amount) \+ (before|after|amount)$/.exec(text);
  if(!match) throw Error('unsupported contract expression: '+text);
  return values[match[1]]===values[match[2]]+values[match[3]];
};
const snapshot=expression(snapshot_condition,env);
const selfReference=expression(incorrect_condition,env);
if(snapshot!==true || selfReference!==false) throw Error('initial/final distinction not observed');
const zeroAmount={...env,amount:0n,after:env.before};
if(!expression(snapshot_condition,zeroAmount) || !expression(incorrect_condition,zeroAmount))
  throw Error('control failed to distinguish nonzero witness from coincidental zero-amount match');
const wrongAfter={...env,after:120n};
if(expression(snapshot_condition,wrongAfter) || expression(incorrect_condition,wrongAfter))
  throw Error('changed final value was not rejected');
const receipt={schema:'noble-adapt14-snapshot-review/v1',case:'ADAPT-14',kind:'review',result:'passed',
  claim:'Under the recorded nonnegative signed-I64/no-overflow premises, actual initial 100, amount 10 and final 110 make after=before+amount true and after=after+amount false. An altered final value fails both; zero amount makes both true and therefore is not a discriminating witness. This finite counterexample is not universal proof.',
  observations:{initial:before.value,amount:amount.value,final:after.value,preconditions,
    snapshot_condition:{expression:snapshot_condition,result:snapshot},
    incorrect_condition:{expression:incorrect_condition,result:selfReference},
    zero_amount_not_discriminating:true,wrong_final_120_rejected:true,
    universal_proof_claimed:false},source:{input_sha256:source},
  assumptions:['Only the exact finite I64 valuations in this review were evaluated; no Noble source program, state transition or universal theorem was proved.',
    'Initial/final namespaces are distinct model coordinates. Substituting final for initial makes this recorded nonzero-amount law false.']};
const text=JSON.stringify(receipt,null,2)+'\n';
if(process.argv.includes('--record')) writeFileSync('verification/decoder-experiment/adapt14-review.json',text,{flag:'wx'});
process.stdout.write(text);
