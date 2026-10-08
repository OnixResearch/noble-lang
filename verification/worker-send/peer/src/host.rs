use anyhow::{Context, Result, anyhow, bail, ensure};
use noble_kernel::{async_tasks as task, authority as auth, resources as owner, types::{EffId, ResourceKind}};
use serde_json::{Value, json};
use std::os::unix::fs::OpenOptionsExt;
use std::io::Read;
use std::{fs, os::unix::net::UnixDatagram, path::Path, sync::{atomic::{AtomicU64, Ordering}, mpsc}, thread::JoinHandle, time::Duration};
use tokio::sync::oneshot;

const OPERATION: &str = "noble-test:worker-send/host@1.0.0#begin-send";
const NOW: auth::Checkpoint = auth::Checkpoint { sequence:1, now:10 };
const SOURCE: auth::SourceId = auth::SourceId(1);
const POLICY: auth::PolicyRevision = auth::PolicyRevision(1);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn checked<T,E:std::fmt::Debug>(result: std::result::Result<T,E>) -> Result<T> {
    result.map_err(|error| anyhow!("retained host decision: {error:?}"))
}

pub struct Grant { attempt:i64, text:String, allow:bool, expires_at:u64,
    source_sha256:String }

fn positive(value:&Value) -> Result<i64> {
    let spelling = value.as_str().context("host grant attempt must be decimal text")?;
    let attempt = spelling.parse::<i64>()?;
    ensure!(attempt > 0 && attempt.to_string() == spelling, "noncanonical host grant attempt");
    Ok(attempt)
}

impl Grant {
    pub fn selected(path:&Path) -> Result<Self> {
        let mut file=fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK)
            .open(path).context("selected host grant unavailable")?;
        let metadata=file.metadata()?;
        ensure!(metadata.is_file() && metadata.len() <= 4096, "host grant must be a bounded regular file");
        let mut bytes=vec![0;usize::try_from(metadata.len())?];
        file.read_exact(&mut bytes)?;
        let mut extra=[0];
        ensure!(file.read(&mut extra)?==0,"host grant changed beyond selected bound");
        let value:Value = serde_json::from_slice(&bytes)?;
        ensure!(serde_json::to_vec(&value)? == bytes, "host grant must be canonical JSON");
        let fields = value.as_object().context("host grant must be an object")?;
        ensure!(fields.len() == 7 && value["format"] == "noble-worker-send-grant/v1"
            && value["operation"] == OPERATION && fields.contains_key("attempt")
            && fields.contains_key("text") && fields.contains_key("allow")
            && fields.contains_key("expires_at") && fields.contains_key("source_sha256"),
            "candidate grant, wrong operation or unknown host grant fields");
        let attempt = positive(&value["attempt"])?;
        let text = value["text"].as_str().filter(|text| text.len() <= auth::MAX_ARGUMENT_BYTES - 8)
            .context("host grant Text exceeds operation bound")?.to_owned();
        let allow = value["allow"].as_bool().context("host allow must be boolean")?;
        let expires_at = value["expires_at"].as_u64().context("host expiry must be unsigned")?;
        let source_sha256=value["source_sha256"].as_str()
            .filter(|value|value.len()==64 && value.bytes().all(|byte|
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
            .context("host grant needs exact checked worker source digest")?.to_owned();
        Ok(Self { attempt, text, allow, expires_at, source_sha256 })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { SendThenCancel, CancelBeforeSend }
impl Mode {
    pub fn parse(text:&str) -> Result<Self> {
        match text { "send-then-cancel"=>Ok(Self::SendThenCancel),
            "cancel-before-send"=>Ok(Self::CancelBeforeSend),
            _=>bail!("mode must be send-then-cancel or cancel-before-send") }
    }
}

fn plan(owner:auth::OwnerContext,attempt:i64,text:&str,expires_at:u64) -> Result<auth::Plan> {
    ensure!(text.len() <= auth::MAX_ARGUMENT_BYTES-8, "Send Text exceeds plan bound");
    let mut arguments = Vec::with_capacity(8+text.len());
    arguments.extend_from_slice(&attempt.to_le_bytes());
    arguments.extend_from_slice(text.as_bytes());
    checked(auth::Plan::new(auth::PlanDescription {
        operation:auth::OperationContract { name:OPERATION.as_bytes().to_vec(), effect:EffId(8) },
        arguments, actor:auth::ActorId(1), owner, policy:POLICY,
        constraints:auth::Constraints { not_before:1, expires_at,
            quota:auth::QuotaId(1), units:1, scope:1 },
    }))
}

fn facts(grant:&auth::Plan,allow:bool)->auth::CurrentFacts {
    auth::CurrentFacts { source:SOURCE, checkpoint:NOW,
        grant:allow.then(||auth::Grant {plan:grant.clone(),kind:auth::WITNESS_KIND,
            rights:auth::EXECUTE_RIGHT}), credentials_valid:Some(allow),
        revoked:Some(false),policy:Some(POLICY),quota_remaining:Some(1) }
}

struct Pending {
    callback:task::Callback,
    token:Option<task::Task>,
    scope:owner::Scope,
    borrow:owner::Borrow,
    release:Option<mpsc::SyncSender<()>>,
    joined:Option<JoinHandle<std::result::Result<usize,String>>>,
    receive_buffer:Vec<u8>,
}

pub struct Host {
    authority:auth::Authority,
    tasks:task::Table,
    resources:owner::Table,
    required:owner::Requirement,
    owner:Option<owner::Owner>,
    sender:Option<UnixDatagram>,
    receiver:Option<UnixDatagram>,
    pending:Option<Pending>,
    grant:Grant,
    mode:Mode,
    context:owner::Context,
    denied:bool,
    native_sent:usize,
    native_error:Option<String>,
    receiver_reads:usize,
    late_retired:bool,
    pins_at_cancel:usize,
}

impl Host {
    pub fn new(grant:Grant,mode:Mode,checked_source_sha256:&str)->Result<Self> {
        ensure!(grant.source_sha256==checked_source_sha256,
            "independent Send grant selected a different checked worker source");
        let id = NEXT_ID.fetch_update(Ordering::Relaxed, Ordering::Relaxed,
            |value| value.checked_add(1)).map_err(|_| anyhow!("host namespace exhausted"))?;
        let context = owner::Context(id);
        let required = owner::Requirement {context,kind:ResourceKind(1),rights:owner::Rights(1)};
        let resources = checked(owner::Table::new(owner::TableId(id),owner::Limits {
            slots:1,pins:1,owners_per_context:1,generations:1,scopes:1 }))?;
        let tasks = checked(task::Table::new(task::TableId(id),task::Limits {
            tasks:1,terminal_results:1,bytes:3*auth::MAX_ARGUMENT_BYTES+256,
            parked_payloads:1,pins:1,wakeups:0,retirement_work:5,generations:1 }))?;
        let authority = checked(auth::Authority::new(auth::Profile {
            owner:auth::OwnerContext {instance:id,invocation:1,generation:1},
            witness_kind:auth::WITNESS_KIND,authority_source:SOURCE,
            observation_source:auth::SourceId(2),invocation_source:auth::SourceId(3),
            checkpoint:NOW,limits:auth::Limits {witnesses:1,attempts:1,requests:2},
        }))?;
        let (sender,receiver) = UnixDatagram::pair()?;
        sender.set_nonblocking(true)?;
        receiver.set_nonblocking(true)?;
        let mut host=Self {authority,tasks,resources,required,owner:None,
            sender:Some(sender),receiver:Some(receiver),pending:None,grant,mode,context,
            denied:false,native_sent:0,native_error:None,receiver_reads:0,
            late_retired:false,pins_at_cancel:0};
        host.owner=Some(checked(host.resources.register(required))?);
        Ok(host)
    }

    pub fn begin(&mut self,attempt:i64,text:&str)
        ->Result<(task::Callback,oneshot::Receiver<std::result::Result<i64,String>>)> {
        ensure!(self.pending.is_none() && self.owner.is_some(),"one Send per host-owned invocation");
        ensure!(attempt>0 && text.len()<=auth::MAX_ARGUMENT_BYTES-8,"invalid bounded Send action");
        let owner_context=auth::OwnerContext {instance:self.context.0,invocation:1,generation:1};
        let selected=plan(owner_context,self.grant.attempt,&self.grant.text,self.grant.expires_at)?;
        let proposed=plan(owner_context,attempt,text,self.grant.expires_at)?;
        let bytes=8+text.len();
        let request=task::Request {context:self.context,native:task::NativeId(self.context.0),
            inputs:1,results:0,input_bytes:bytes,result_bytes:bytes,
            parked_bytes:bytes+256,pins:1,wakeups:0};
        let prepared=checked(self.tasks.prepare(request))?;
        let mut wire=Vec::with_capacity(bytes);
        wire.extend_from_slice(&attempt.to_le_bytes());
        wire.extend_from_slice(text.as_bytes());
        let receive_buffer=vec![0;bytes];
        // The independently selected file supplies facts. Guest arguments
        // cannot substitute a generated grant or change the operation owner.
        let current=facts(&selected,self.grant.allow);
        let witness=match self.authority.authorize_from_trusted_host(proposed.clone(),&current) {
            auth::Authorization::Authorized(value)=>value,
            auth::Authorization::Denied(_) | auth::Authorization::Preflight(_)=>{
                self.denied=true;
                drop(prepared);
                self.release_unused()?;
                bail!("independent host Send grant refused")
            }
        };
        let admission=auth::AdmissionRequest {claim:witness.claim(),plan:proposed};
        let execution=match self.authority.admit(Some(witness),&admission,&current) {
            auth::Admission::Committed(value)=>value,
            auth::Admission::Denied {witness,..} | auth::Admission::Preflight {witness,..}=>{
                self.denied=true;
                drop(prepared);
                self.release_unused()?;
                if let Some(witness)=witness {checked(self.authority.retire_witness(witness))?;}
                bail!("exact host Send admission refused")
            }
        };
        let resource=self.owner.take().context("missing host socket owner")?;
        let borrowed=match self.resources.begin(resource,self.required) {
            Ok(borrowed)=>borrowed,
            Err(rejected)=>{
                self.owner=Some(rejected.input);
                drop(prepared);
                self.release_unused()?;
                checked(self.authority.fail_invocation())?;
                bail!("native Send resource could not be admitted: {:?}",rejected.error)
            }
        };
        let token=prepared.commit(());
        let callback=token.callback;
        let scope=borrowed.borrow.scope();
        if let Err(error)=self.resources.native_access(&borrowed.borrow) {
            drop((wire,receive_buffer));
            self.retire_unstarted(token.task,callback,scope)?;
            bail!("retained Send resource lost access before native start: {error:?}");
        }
        let started=match self.authority.start_execution(execution) {
            Ok(started)=>started,
            Err(error)=>{
                drop((wire,receive_buffer));
                self.retire_unstarted(token.task,callback,scope)?;
                bail!("committed host Send authority could not start: {error:?}");
            }
        };
        if started.plan().description().arguments.as_slice()!=wire.as_slice() {
            drop((wire,receive_buffer));
            self.retire_unstarted(token.task,callback,scope)?;
            bail!("native Send differs from consumed exact authority");
        }
        let sender=self.sender.take().context("native socket already transferred")?;
        let (release_tx,release_rx)=mpsc::sync_channel(1);
        let (started_tx,started_rx)=mpsc::sync_channel(1);
        let (native_tx,native_rx)=oneshot::channel();
        let mode=self.mode;
        let joined=std::thread::Builder::new().name("noble-worker-wit-send".into())
            .spawn(move || {
                let _permit=started;
                let sent=if mode==Mode::SendThenCancel {
                    sender.send(&wire).map_err(|error|error.to_string())
                } else { Ok(0) };
                let _=started_tx.send(sent.clone());
                let _=release_rx.recv();
                let _=native_tx.send(sent.clone().map(|_|attempt));
                sent
            });
        let joined=match joined {
            Ok(joined)=>joined,
            Err(error)=>{
                drop(receive_buffer);
                self.retire_unstarted(token.task,callback,scope)?;
                bail!("native Send producer could not start: {error}");
            }
        };
        self.pending=Some(Pending {callback,token:Some(token.task),scope,
            borrow:borrowed.borrow,
            release:Some(release_tx),joined:Some(joined),receive_buffer});
        let first=started_rx.recv_timeout(Duration::from_secs(5))?;
        // Even a failed native syscall leaves the task's pin retained until
        // its joined producer reports actual stop, never on this observation.
        self.native_sent=first.as_ref().copied().unwrap_or(0);
        first.map_err(|error|anyhow!("native Send attempt failed: {error}"))?;
        Ok((callback,native_rx))
    }

    pub fn cancel(&mut self,callback:task::Callback)->Result<()> {
        let pending=self.pending.as_mut().context("no admitted task to cancel")?;
        ensure!(pending.callback==callback,"foreign or stale future cancellation");
        let token=pending.token.take().context("duplicate future cancellation")?;
        let decision=checked(self.tasks.cancel(token,self.context).map_err(|error|error.error))?;
        let task_retained=decision.action==task::Action::CancellationAcknowledged
            && decision.record.state==task::State::Retiring && decision.record.pins==1
            && decision.record.retiring_inputs==1
            && self.tasks.observation().reserved.bytes==3*pending.receive_buffer.len()+256;
        let cancelled=checked(self.authority.cancel_invocation());
        let revoked=checked(self.resources.revoke(pending.scope,owner::Retirement::Cancelled));
        // Release the producer even if a post-cancel invariant is violated:
        // failure must not strand the native thread and its socket pin forever.
        if let Some(release)=pending.release.take(){let _=release.send(());}
        ensure!(task_retained,"task owner or reserved storage was not retained at cancellation");
        cancelled?;
        let revoked=revoked?;
        ensure!(revoked.record.state==owner::State::Retiring(pending.scope.serial)
            && self.resources.observation().native_pins==1
            && self.resources.native_access(&pending.borrow).is_err(),
            "resource access or native pin violated cancellation");
        self.pins_at_cancel=1;
        Ok(())
    }

    pub fn abort_unconsumed(&mut self)->Result<()> {
        if let Some(callback)=self.pending.as_ref()
            .filter(|pending|pending.token.is_some()).map(|pending|pending.callback) {
            self.cancel(callback)?;
        }
        Ok(())
    }

    pub fn take_native(&mut self)->Option<JoinHandle<std::result::Result<usize,String>>> {
        self.pending.as_mut()?.joined.take()
    }

    fn retire_unstarted(&mut self,token:task::Task,callback:task::Callback,
                        scope:owner::Scope)->Result<()> {
        self.sender.take();
        self.receiver.take();
        checked(self.tasks.cancel(token,self.context).map_err(|error|error.error))?;
        checked(self.authority.fail_invocation())?;
        checked(self.resources.revoke(scope,owner::Retirement::HostCleanup))?;
        checked(self.tasks.native_stopped(callback))?;
        let released=checked(self.resources.complete(scope,owner::Completion::DomainError))?;
        ensure!(released.owner.is_none(),"failed producer start returned retired socket owner");
        self.settle_retired(callback)
    }

    fn settle_retired(&mut self,callback:task::Callback)->Result<()> {
        checked(self.tasks.settle_pins(callback,1))?;
        let snapshot=checked(self.tasks.inspect(callback.task,self.context))?;
        ensure!(snapshot.native_stopped && snapshot.pins==0 && snapshot.state==task::State::Retiring,
            "native storage not stopped before task cleanup");
        let obligations=task::Obligations {inputs:snapshot.retiring_inputs,
            results:snapshot.results,buffers:snapshot.buffers};
        if obligations!=task::Obligations::empty(){checked(self.tasks.cleanup(callback.task,obligations))?;}
        checked(self.tasks.finish(callback.task))?;
        Ok(())
    }

    pub fn finish_native(&mut self,outcome:std::result::Result<usize,String>)->Result<()> {
        let mut pending=self.pending.take().context("late native completion without task")?;
        ensure!(pending.token.is_none(),"native completion preceded cancellation");
        let receiver=self.receiver.take().context("missing native receiver")?;
        let success=match (&self.mode,&outcome) {
            (Mode::SendThenCancel,Ok(count))=>{
                let size=receiver.recv(&mut pending.receive_buffer)?;
                ensure!(size==8+self.grant.text.len()
                    && &pending.receive_buffer[..8]==self.grant.attempt.to_le_bytes().as_slice()
                    && &pending.receive_buffer[8..]==self.grant.text.as_bytes(),
                    "native receiver did not observe exact authorized Send");
                self.receiver_reads=1;
                ensure!(*count==size,"native producer emitted incomplete Send");
                let mut extra=[0];
                ensure!(receiver.recv(&mut extra).is_err_and(|error|
                    error.kind()==std::io::ErrorKind::WouldBlock),"duplicate datagram observed");
                true
            }
            (Mode::CancelBeforeSend,Ok(0))=>{
                let mut extra=[0];
                ensure!(receiver.recv(&mut extra).is_err_and(|error|
                    error.kind()==std::io::ErrorKind::WouldBlock),"cancelled operation sent a datagram");
                true
            }
            (_,Err(error))=>{
                let mut extra=[0];
                ensure!(receiver.recv(&mut extra).is_err_and(|problem|
                    problem.kind()==std::io::ErrorKind::WouldBlock),
                    "failed native Send unexpectedly reached receiver");
                self.native_error=Some(error.clone());
                false
            }
            _=>bail!("cancelled native operation sent an unauthorized datagram"),
        };
        drop(receiver);
        drop(pending.receive_buffer);
        let task_outcome=if success {task::Outcome::Success} else {task::Outcome::DomainError};
        let late=checked(self.tasks.complete(pending.callback,task_outcome,
            task::Completion {inputs:task::Disposition {returned:0,consumed:0,retired:1},
                produced:0,bytes:0}))?;
        ensure!(late.action==task::Action::CompletionRetired(task_outcome),
            "late native result crossed canceled future boundary");
        checked(self.tasks.native_stopped(pending.callback))?;
        let completion=if success {owner::Completion::Success} else {owner::Completion::DomainError};
        let released=checked(self.resources.complete(pending.scope,completion))?;
        ensure!(released.owner.is_none() && released.decision.accounting().pins_released==1
            && released.decision.accounting().local_releases==1,
            "late native result resurrected resource owner");
        self.settle_retired(pending.callback)?;
        self.late_retired=true;
        Ok(())
    }

    pub fn release_unused(&mut self)->Result<()> {
        if let Some(resource)=self.owner.take() {
            checked(self.resources.release(resource,self.required).map_err(|error|error.error))?;
        }
        self.sender.take();
        self.receiver.take();
        Ok(())
    }

    pub fn outstanding_pins(&self)->usize {
        self.tasks.observation().outstanding_pins
    }

    pub fn observation(&self)->Value {
        let task=self.tasks.observation();
        let resource=self.resources.observation();
        let counters=self.authority.counters();
        json!({"schema":"noble-worker-send-wit-peer/v1",
            "profile":"Worker-Send-WIT-Local", "denied":self.denied,
            "task_retired":task.retired,"task_outstanding_pins":task.outstanding_pins,
            "task_reserved_bytes":task.reserved.bytes,
            "resource_retired":resource.retired,"resource_native_pins":resource.native_pins,
            "protected_operation_attempts":counters.protected_operations,
            "successful_result_deliveries":counters.successful_deliveries,
            "pins_at_cancel":self.pins_at_cancel,"late_completion_retired":self.late_retired,
            "native_send_bytes":self.native_sent,"native_error":self.native_error,
            "receiver_datagrams":self.receiver_reads,
            "external_send_outcome":"not-attempted", "external_rollback_claimed":false})
    }
}
