use noble_ngraph::{check,compiled::Compiled,eval::{reference,f32_inputs,Control},ir::{GraphBuilder,SubProfile,Subject,ScalarType,Op,Provenance,Fact,OutOfRange},Obs,Value};
use noble_ngraph_experiment::{scheme::BLAKE3,fixtures::{range,RULES}};
fn main(){let mut cases=0;for len in [1,2,7,257]{for below in RULES{for above in RULES{
 let mut b=GraphBuilder::new(SubProfile::F32,Subject::authored(&BLAKE3,"table-bit-smoke"));let i=b.input(ScalarType::F32);let a=b.input(ScalarType::F32);let z=b.f32_const(-0.0);b.fact(i,Fact::Integral,Provenance::InputValidated);b.fact(i,range(-1.0,257.0),Provenance::InputValidated);let table:Vec<_>=(0..len).map(|k|if k%2==0{a}else{z}).collect();let out=b.op(Op::F32Index{index:i,table,below,above});b.output(out);b.output(z);let g=check(&BLAKE3,b.finish()).unwrap();let c=Compiled::new(&BLAKE3,&g);let mut ws=c.workspace();
 for index in [-1.0,-0.0,(len-1)as f32,len as f32]{for val in [-0.0,f32::from_bits(1),f32::INFINITY,f32::from_bits(0xffc00042)]{
  let inputs=f32_inputs(&[index,val]);let r=reference(&g,&inputs,None,&mut Control::default()).obs;let got=c.evaluate_in(&mut ws,&inputs,None,&mut Control::default()).obs;assert!(r.obs_eq(&got));
  let rule=if index<0.0{below}else{above};let selected=if index>=0.0&&(index as usize)<len{Some(index as usize)}else{match rule{OutOfRange::Clamp=>Some(if index<0.0{0}else{len-1}),OutOfRange::Zero=>None,OutOfRange::Refuse=>{assert!(matches!(got,Obs::GuardFailed{..}));cases+=1;continue;}}};
  let expected=selected.map_or(0.0,|k|if k%2==0{val}else{-0.0});assert!(got.obs_eq(&Obs::Normal(vec![Value::f32(expected),Value::f32(-0.0)])));cases+=1;
 }}
 let got=c.evaluate_in(&mut ws,&f32_inputs(&[f32::NAN,0.0]),None,&mut Control::default()).obs;assert!(matches!(got,Obs::Refused(_)));cases+=1;
}}}println!("{cases} explicit expected native table boundary/NaN/refusal cases passed");}
