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
pub enum Mode {
    SendThenCancel,
    CancelBeforeSend,
    InjectFallbackAuthorityError,
    InjectReceiveError,
    InjectReceiveCorruption,
    InjectProducerPanic,
    InjectCancelError,
    InjectCancelPanic,
    BufferQuotaPreflight,
}
impl Mode {
    pub fn parse(text:&str) -> Result<Self> {
        match text { "send-then-cancel"=>Ok(Self::SendThenCancel),
            "cancel-before-send"=>Ok(Self::CancelBeforeSend),
            "inject-fallback-authority-error"=>Ok(Self::InjectFallbackAuthorityError),
            "inject-receive-error"=>Ok(Self::InjectReceiveError),
            "inject-receive-corruption"=>Ok(Self::InjectReceiveCorruption),
            "inject-producer-panic"=>Ok(Self::InjectProducerPanic),
            "inject-cancel-error"=>Ok(Self::InjectCancelError),
            "inject-cancel-panic"=>Ok(Self::InjectCancelPanic),
            "buffer-quota-preflight"=>Ok(Self::BufferQuotaPreflight),
            _=>bail!("unknown host-selected native Send mode") }
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

// The producer's gate is last. Attempt every independent revocation even
// after a refusal; the producer cannot issue a post-gate Send in any mode.
// Native pins cannot settle until the released producer has exited and joined.
struct Cancellation<'a> {
    pending:&'a mut Pending,
    tasks:&'a mut task::Table,
    authority:&'a mut auth::Authority,
    resources:&'a mut owner::Table,
    cleanup_failures:&'a mut Vec<String>,
    pins_at_cancel:&'a mut usize,
    task_retired:bool,
    authority_cancelled:bool,
    resource_revoked:bool,
}

impl Cancellation<'_> {
    fn record_failure(&mut self, mut diagnostic:String) {
        // Three independent fallbacks plus the physical-pin check. Kernel
        // errors here are fixed-size enums; still bound the exposed report.
        if self.cleanup_failures.len() >= 4 { return; }
        if diagnostic.len() > 192 {
            let boundary=diagnostic.char_indices().map(|(offset,_)|offset)
                .take_while(|offset|*offset<=189).last().unwrap_or(0);
            diagnostic.truncate(boundary);
            diagnostic.push_str("...");
        }
        self.cleanup_failures.push(diagnostic);
    }
}

impl Drop for Cancellation<'_> {
    fn drop(&mut self) {
        if !self.task_retired {
            match self.tasks.retire(self.pending.callback.task,task::Failure::Internal) {
                Ok(decision) if decision.record.state==task::State::Retiring => {
                    // A rejected move-only Task stays owned by Pending unless
                    // trusted host retirement actually consumes guest access.
                    self.pending.token.take();
                }
                Ok(decision)=>self.record_failure(
                    format!("task.retire: unexpected state: {:?}",decision.record.state)),
                Err(error)=>self.record_failure(
                    format!("task.retire: retained host decision: {error:?}")),
            }
        }
        if !self.authority_cancelled {
            if let Err(error)=self.authority.cancel_invocation() {
                self.record_failure(format!(
                    "authority.cancel_invocation: retained host decision: {error:?}"));
            }
        }
        if !self.resource_revoked {
            match self.resources.retire(self.pending.scope.handle,owner::Retirement::HostCleanup) {
                Ok(decision) if decision.record.state
                    ==owner::State::Retiring(self.pending.scope.serial)=>{}
                Ok(decision)=>self.record_failure(
                    format!("resources.retire: unexpected state: {:?}",decision.record.state)),
                Err(error)=>self.record_failure(
                    format!("resources.retire: retained host decision: {error:?}")),
            }
        }
        // Neither cancellation nor host retirement releases the physical pin.
        // Even on refusal, record the actual state before opening the gate.
        let task_pins=self.tasks.observation().outstanding_pins;
        let resource_pins=self.resources.observation().native_pins;
        *self.pins_at_cancel=resource_pins;
        if task_pins!=1 || resource_pins!=1 {
            self.record_failure(format!(
                "pre-gate native pin mismatch: task={task_pins}, resource={resource_pins}"));
        }
        if let Some(release)=self.pending.release.take() {let _=release.send(());}
    }
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
    quota_refusal:Option<&'static str>,
    native_sent:usize,
    native_error:Option<String>,
    receiver_reads:usize,
    late_retired:bool,
    late_task_outcome:Option<task::Outcome>,
    pins_at_cancel:usize,
    cleanup_failures:Vec<String>,
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
        // This selected local control admits one byte less than the exact
        // grant's input/result/parked reservation, without changing authority.
        let bytes=if mode==Mode::BufferQuotaPreflight {
            3*(8+grant.text.len())+255
        } else {3*auth::MAX_ARGUMENT_BYTES+256};
        let tasks = checked(task::Table::new(task::TableId(id),task::Limits {
            tasks:1,terminal_results:1,bytes,
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
            denied:false,quota_refusal:None,native_sent:0,native_error:None,receiver_reads:0,
            late_retired:false,late_task_outcome:None,pins_at_cancel:0,
            cleanup_failures:Vec::new()};
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
        let prepared=match self.tasks.prepare(request) {
            Ok(prepared)=>prepared,
            Err(error)=>{
                if self.mode==Mode::BufferQuotaPreflight && error==task::Error::ByteCapacity {
                    self.quota_refusal=Some("byte-capacity");
                    bail!("task buffer reservation denied before Send admission");
                }
                bail!("retained host decision: {error:?}");
            }
        };
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
                let sent=if matches!(mode,Mode::CancelBeforeSend
                    | Mode::InjectFallbackAuthorityError) {
                    // Acknowledging readiness does not complete the operation.
                    // No post-gate Send is possible even if cancellation
                    // reports a revocation failure; the pin retires on join.
                    let _=started_tx.send(Ok(0));
                    let _=release_rx.recv();
                    Ok(0)
                } else {
                    let sent=sender.send(&wire).map_err(|error|error.to_string());
                    let _=started_tx.send(sent.clone());
                    let _=release_rx.recv();
                    if mode==Mode::InjectProducerPanic {
                        panic!("injected native producer panic after Send");
                    }
                    sent
                };
                // Skipping a Send after cancellation is not a successful
                // result, even though the closed future cannot deliver it.
                let result=match (mode,&sent) {
                    (Mode::CancelBeforeSend | Mode::InjectFallbackAuthorityError,Ok(0))=>
                        Err("cancelled before native Send".to_owned()),
                    _=>sent.clone().map(|_|attempt),
                };
                let _=native_tx.send(result);
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
        let previous_cleanup_failures=self.cleanup_failures.len();
        let pending=self.pending.as_mut().context("no admitted task to cancel")?;
        ensure!(pending.callback==callback,"foreign or stale future cancellation");
        let mut cancellation=Cancellation {pending,tasks:&mut self.tasks,
            authority:&mut self.authority,resources:&mut self.resources,
            cleanup_failures:&mut self.cleanup_failures,
            pins_at_cancel:&mut self.pins_at_cancel,
            task_retired:false,authority_cancelled:false,resource_revoked:false};
        let token=cancellation.pending.token.take().context("duplicate future cancellation")?;
        // Deliberately exercise the real move-only rejection contract in the
        // host-selected fault mode; never treat it as a cancelled task.
        let context=if self.mode==Mode::InjectCancelError {
            owner::Context(self.context.0 ^ (1_u64<<63))
        } else {self.context};
        let decision=match cancellation.tasks.cancel(token,context) {
            Ok(decision)=>decision,
            Err(rejected)=>{
                cancellation.pending.token=Some(rejected.input);
                bail!("retained host decision: {:?}",rejected.error);
            }
        };
        cancellation.task_retired=decision.record.state==task::State::Retiring;
        if self.mode==Mode::InjectCancelPanic {
            panic!("injected host Send cancellation panic");
        }
        let task_retained=decision.action==task::Action::CancellationAcknowledged
            && decision.record.state==task::State::Retiring && decision.record.pins==1
            && decision.record.retiring_inputs==1
            && cancellation.tasks.observation().reserved.bytes
                ==3*cancellation.pending.receive_buffer.len()+256;
        if self.mode==Mode::InjectFallbackAuthorityError {
            // Deliberately enter the real terminal Failed state. The primary
            // cancellation below and the guard's fallback both receive the
            // kernel's InvocationFinished refusal, not a fabricated error.
            let failed=checked(cancellation.authority.fail_invocation())?;
            ensure!(failed==auth::InvocationOutcome::Failed,
                "injected invocation failure did not become terminal");
        }
        let cancelled=checked(cancellation.authority.cancel_invocation());
        cancellation.authority_cancelled=cancelled.is_ok();
        let revoked=checked(cancellation.resources.revoke(
            cancellation.pending.scope,owner::Retirement::Cancelled));
        cancellation.resource_revoked=revoked.as_ref().is_ok_and(|decision|
            decision.record.state==owner::State::Retiring(cancellation.pending.scope.serial));
        ensure!(task_retained,"task owner or reserved storage was not retained at cancellation");
        cancelled?;
        let revoked=revoked?;
        ensure!(revoked.record.state==owner::State::Retiring(cancellation.pending.scope.serial)
            && cancellation.resources.observation().native_pins==1
            && cancellation.resources.native_access(&cancellation.pending.borrow).is_err(),
            "resource access or native pin violated cancellation");
        drop(cancellation);
        ensure!(self.cleanup_failures.len()==previous_cleanup_failures,
            "host cancellation cleanup failed; see cleanup_failures");
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
        let receiver=self.receiver.take();
        // Drop physical buffers and attempt both ownership-table settlements
        // before returning; a refused retirement remains an explicit failure.
        let validation=(|| -> Result<bool> {
            let receiver=receiver.as_ref().context("missing native receiver after join")?;
            match (self.mode,&outcome) {
                (Mode::SendThenCancel | Mode::InjectReceiveError
                    | Mode::InjectReceiveCorruption | Mode::InjectCancelError
                    | Mode::InjectCancelPanic,Ok(count))=>{
                    if self.mode==Mode::InjectReceiveError {
                        // Consume the real datagram first. The ordinary read
                        // then deterministically gets WouldBlock, exercising
                        // the same cleanup path as a failed host recv syscall.
                        let drained=receiver.recv(&mut pending.receive_buffer)
                            .context("injected host receive failed before validation")?;
                        self.receiver_reads+=1;
                        ensure!(drained==8+self.grant.text.len()
                            && &pending.receive_buffer[..8]==self.grant.attempt.to_le_bytes().as_slice()
                            && &pending.receive_buffer[8..]==self.grant.text.as_bytes(),
                            "injected drain did not receive the actual authorized datagram");
                    }
                    let size=receiver.recv(&mut pending.receive_buffer)
                        .context("native receiver read failed")?;
                    self.receiver_reads+=1;
                    if self.mode==Mode::InjectReceiveCorruption {
                        pending.receive_buffer[0]^=1;
                    }
                    ensure!(size==8+self.grant.text.len()
                        && &pending.receive_buffer[..8]==self.grant.attempt.to_le_bytes().as_slice()
                        && &pending.receive_buffer[8..]==self.grant.text.as_bytes(),
                        "native receiver did not observe exact authorized Send");
                    ensure!(*count==size,"native producer emitted incomplete Send");
                    let mut extra=[0];
                    let additional=receiver.recv(&mut extra);
                    if additional.is_ok(){self.receiver_reads+=1;}
                    ensure!(additional.is_err_and(|error|
                        error.kind()==std::io::ErrorKind::WouldBlock),"duplicate datagram observed");
                    Ok(true)
                }
                (Mode::CancelBeforeSend | Mode::InjectFallbackAuthorityError,Ok(0))=>{
                    let mut extra=[0];
                    let unexpected=receiver.recv(&mut extra);
                    if unexpected.is_ok(){self.receiver_reads+=1;}
                    ensure!(unexpected.is_err_and(|error|
                        error.kind()==std::io::ErrorKind::WouldBlock),
                        "cancelled operation sent a datagram");
                    // No native Send occurred, and no result can be delivered.
                    Ok(false)
                }
                (_,Err(error))=>{
                    self.native_error=Some(error.clone());
                    let mut extra=[0];
                    let unexpected=receiver.recv(&mut extra);
                    if unexpected.is_ok(){self.receiver_reads+=1;}
                    ensure!(unexpected.is_err_and(|problem|
                        problem.kind()==std::io::ErrorKind::WouldBlock),
                        "failed native Send unexpectedly reached receiver");
                    Ok(false)
                }
                _=>bail!("cancelled native operation sent an unauthorized datagram"),
            }
        })();
        drop(receiver);
        drop(pending.receive_buffer);
        let success=matches!(&validation,Ok(true));
        let task_outcome=if success {task::Outcome::Success} else {task::Outcome::DomainError};
        // A rejected fallback retirement keeps the move-only task in Pending.
        // The joined producer is now stopped; retry host retirement before
        // accepting its late callback, without skipping independent resource
        // completion or pin settlement if the retry also refuses.
        let retirement=if pending.token.is_some() {
            let decision=checked(self.tasks.retire(pending.callback.task,task::Failure::Internal))
                .and_then(|decision| {
                    ensure!(decision.record.state==task::State::Retiring,
                        "joined task host retirement did not revoke guest access");
                    Ok(())
                });
            if decision.is_ok() {pending.token.take();}
            decision
        } else {Ok(())};
        let late=checked(self.tasks.complete(pending.callback,task_outcome,
            task::Completion {inputs:task::Disposition {returned:0,consumed:0,retired:1},
                produced:0,bytes:0}));
        self.late_task_outcome=late.as_ref().ok().and_then(|decision|match decision.action {
            task::Action::CompletionRetired(outcome)=>Some(outcome),
            _=>None,
        });
        let late_retired=self.late_task_outcome==Some(task_outcome);
        let stopped=checked(self.tasks.native_stopped(pending.callback));
        let completion=if success {owner::Completion::Success} else {owner::Completion::DomainError};
        let released=checked(self.resources.complete(pending.scope,completion));
        let resource_retired=released.as_ref().is_ok_and(|released|released.owner.is_none()
            && released.decision.accounting().pins_released==1
            && released.decision.accounting().local_releases==1);
        // These are independent debts after the producer has stopped. A task
        // callback rejection must not skip resource completion, nor may a
        // resource error skip the task's native-stop/pin/cleanup transitions.
        let settled=self.settle_retired(pending.callback);
        retirement?;
        late?;
        stopped?;
        released?;
        settled?;
        ensure!(late_retired,"late native result crossed canceled future boundary");
        ensure!(resource_retired,"late native result resurrected resource owner");
        self.late_retired=true;
        validation.map(|_|())
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
        let late_task_outcome=self.late_task_outcome.map(|outcome|match outcome {
            task::Outcome::Success=>"success",task::Outcome::DomainError=>"domain-error"
        });
        json!({"schema":"noble-worker-send-wit-peer/v1",
            "profile":"Worker-Send-WIT-Local", "denied":self.denied,
            "quota_refusal":self.quota_refusal,
            "task_retired":task.retired,"task_outstanding_pins":task.outstanding_pins,
            "task_reserved_bytes":task.reserved.bytes,
            "resource_retired":resource.retired,"resource_native_pins":resource.native_pins,
            "protected_operation_attempts":counters.protected_operations,
            "successful_result_deliveries":counters.successful_deliveries,
            "pins_at_cancel":self.pins_at_cancel,"late_completion_retired":self.late_retired,
            "late_task_outcome":late_task_outcome,
            "cleanup_failures":self.cleanup_failures,
            "native_send_bytes":self.native_sent,"native_error":self.native_error,
            "receiver_datagrams":self.receiver_reads,
            "external_send_outcome":"not-attempted", "external_rollback_claimed":false})
    }
}
