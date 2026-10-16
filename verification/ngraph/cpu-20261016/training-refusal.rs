use noble_ngraph::{check,compiled::Compiled,eval::{reference,f32_inputs,Control,ScratchCounters},grad::DerivOptions,ir::{GraphBuilder,SubProfile,Subject,ScalarType,Op,Fact},snapshot::ParamSnapshot,train::{self,Adam,Point,Contract,TrainControl}};
use noble_ngraph_experiment::{scheme::BLAKE3,fixtures};
fn main(){
 let mut b=GraphBuilder::new(SubProfile::F32,Subject::authored(&BLAKE3,"training-guard-resource-smoke"));let x=b.input(ScalarType::F32);let p=b.param();b.guard(Fact::Finite,x);let y=b.op(Op::F32Mul(x,p));b.output(y);
 let model=check(&BLAKE3,b.finish()).unwrap();let loss=check(&BLAKE3,fixtures::mse_loss()).unwrap();let contract=Contract{params:model.param_slots().to_vec(),model:model.clone(),loss,deriv:DerivOptions::default(),adam:Adam::default(),seed:17,data:vec![Point{inputs:vec![f32::INFINITY],targets:vec![0.0]}]};
 let s=ParamSnapshot::new(&BLAKE3,&model,&train::init_params(17,1)).unwrap();let original=s.bytes().to_vec();let mut n=0;let mut scratch=ScratchCounters::default();
 let stopped=train::train(&BLAKE3,&contract,Some(&s),1,&mut TrainControl{evaluated_nodes:Some(&mut n),scratch:Some(&mut scratch),..TrainControl::default()}).unwrap_err();
 assert_eq!(stopped.published_steps,0);assert_eq!(stopped.last.unwrap().id(),s.id());assert_eq!(s.bytes(),original);assert_eq!(n,4);assert_eq!(scratch.snapshot(),(1,1,0));
 let mut baseline_scratch=ScratchCounters::default();let baseline=train::train(&BLAKE3,&contract,Some(&s),1,&mut TrainControl{scratch:Some(&mut baseline_scratch),..TrainControl::default()}).unwrap_err();assert_eq!(baseline.diagnostic,stopped.diagnostic);assert_eq!(baseline_scratch,scratch);
 let r=reference(&model,&f32_inputs(&[f32::INFINITY]),Some(&s),&mut Control::default());let c=Compiled::new(&BLAKE3,&model).evaluate(&f32_inputs(&[f32::INFINITY]),Some(&s),&mut Control::default());assert!(r.obs.obs_eq(&c.obs));assert_eq!(r.evaluated_nodes,3);
 println!("training guard refusal actual forward/validation events={n}; reference stopped after {} canonical nodes; original full-forward-then-scan semantics preserved, no cutoff claim; published0, snapshot unchanged, scratch {:?}",r.evaluated_nodes,scratch.snapshot());
 let mut n=0;let mut scratch=ScratchCounters::default();let stopped=train::train(&BLAKE3,&contract,Some(&s),1,&mut TrainControl{evaluated_nodes:Some(&mut n),scratch:Some(&mut scratch),cancel_before_step:Some(&|_|true),..TrainControl::default()}).unwrap_err();assert_eq!(stopped.published_steps,0);assert_eq!(n,0);assert_eq!(scratch.snapshot(),(0,0,0));println!("cancel before step: actual0, published0, scratch0/0/0");
}
