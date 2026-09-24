use crate::driver::{Runtime, Shared, event};
use anyhow::{Context, Result, bail, ensure};
use noble_kernel::async_tasks::{Callback, Outcome, State};
use std::{
    future::Future,
    pin::Pin,
    task::{Context as PollContext, Poll},
    time::Duration,
};
use tokio::sync::oneshot;
use wasmtime::component::{
    Accessor, Destination, FutureConsumer, FutureProducer, FutureReader, Source, StreamConsumer,
    StreamProducer, StreamReader, StreamResult, Val,
};
use wasmtime::{AsContextMut, StoreContextMut};

type CompletionSignal = oneshot::Receiver<Result<()>>;
type DomainValue = Result<i64, String>;

struct NativeFuture {
    receive: CompletionSignal,
    value: Option<DomainValue>,
}
impl FutureProducer<Runtime> for NativeFuture {
    type Item = DomainValue;
    fn poll_produce(
        mut self: Pin<&mut Self>,
        cx: &mut PollContext<'_>,
        _: StoreContextMut<Runtime>,
        finish: bool,
    ) -> Poll<Result<Option<DomainValue>>> {
        if finish {
            return Poll::Ready(Ok(None));
        }
        match Pin::new(&mut self.receive).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(error)) => Poll::Ready(Err(error.into())),
            Poll::Ready(Ok(Err(error))) => Poll::Ready(Err(error)),
            Poll::Ready(Ok(Ok(()))) => {
                Poll::Ready(Ok(Some(self.value.take().expect("future produced once"))))
            }
        }
    }
}

pub fn future(
    store: &mut StoreContextMut<Runtime>,
    receive: CompletionSignal,
    _: Callback,
    domain: bool,
) -> Val {
    FutureReader::new(
        store,
        NativeFuture {
            receive,
            value: Some(if domain {
                Err("domain-error".into())
            } else {
                Ok(42)
            }),
        },
    )
    .into_val()
}

pub fn number(
    store: &mut StoreContextMut<Runtime>,
    receive: CompletionSignal,
    domain: bool,
) -> Val {
    FutureReader::new(store, async move {
        receive.await.context("native numeric future vanished")??;
        ensure!(
            !domain,
            "numeric future has no typed domain-error alternative"
        );
        Ok::<i64, anyhow::Error>(42)
    })
    .into_val()
}

struct NativeStream {
    receive: Option<CompletionSignal>,
    bytes: [u8; 3],
    index: usize,
    domain: bool,
}
impl StreamProducer<Runtime> for NativeStream {
    type Item = u8;
    type Buffer = Option<u8>;
    fn poll_produce<'a>(
        mut self: Pin<&mut Self>,
        cx: &mut PollContext<'_>,
        _: StoreContextMut<Runtime>,
        mut destination: Destination<'a, u8, Option<u8>>,
        finish: bool,
    ) -> Poll<Result<StreamResult>> {
        if finish {
            return Poll::Ready(Ok(StreamResult::Cancelled));
        }
        if let Some(receive) = &mut self.receive {
            match Pin::new(receive).poll(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error.into())),
                Poll::Ready(Ok(Err(error))) => return Poll::Ready(Err(error)),
                Poll::Ready(Ok(Ok(()))) => self.receive = None,
            }
        }
        if self.domain || self.index == self.bytes.len() {
            return Poll::Ready(Ok(StreamResult::Dropped));
        }
        destination.set_buffer(Some(self.bytes[self.index]));
        self.index += 1;
        // Exactly one byte in the engine buffer; no hidden unbounded queue.
        Poll::Ready(Ok(StreamResult::Completed))
    }
}

pub fn stream(
    store: &mut StoreContextMut<Runtime>,
    receive: CompletionSignal,
    _: Callback,
    domain: bool,
) -> Val {
    StreamReader::new(
        store,
        NativeStream {
            receive: Some(receive),
            bytes: [0, 127, 255],
            index: 0,
            domain,
        },
    )
    .into_val()
}

struct ReceiveFuture<T> {
    host: Shared,
    callback: Callback,
    send: Option<oneshot::Sender<Result<T>>>,
}
impl<T: wasmtime::component::Lift + Send + Unpin + 'static> FutureConsumer<Runtime>
    for ReceiveFuture<T>
{
    type Item = T;
    fn poll_consume(
        mut self: Pin<&mut Self>,
        _: &mut PollContext<'_>,
        store: StoreContextMut<Runtime>,
        mut source: Source<'_, T>,
        finish: bool,
    ) -> Poll<Result<()>> {
        if finish {
            return Poll::Ready(Ok(()));
        }
        let mut value = None;
        source.read(store, &mut value)?;
        let value = value.context("future completion had no payload")?;
        self.host.lock().deliver(self.callback)?;
        if let Some(send) = self.send.take() {
            let _ = send.send(Ok(value));
        }
        Poll::Ready(Ok(()))
    }
}
impl<T> Drop for ReceiveFuture<T> {
    fn drop(&mut self) {
        if let Some(send) = self.send.take() {
            let _ = send.send(Err(anyhow::anyhow!(
                "future reader cancelled before delivery"
            )));
        }
    }
}

struct ReceiveStream {
    host: Shared,
    callback: Callback,
    send: Option<oneshot::Sender<Result<Result<Vec<u8>, String>>>>,
    bytes: [u8; 64],
    length: usize,
}
impl StreamConsumer<Runtime> for ReceiveStream {
    type Item = u8;
    fn poll_consume(
        mut self: Pin<&mut Self>,
        _: &mut PollContext<'_>,
        store: StoreContextMut<Runtime>,
        mut source: Source<'_, u8>,
        finish: bool,
    ) -> Poll<Result<StreamResult>> {
        if finish {
            return Poll::Ready(Ok(StreamResult::Cancelled));
        }
        if self.length >= self.bytes.len() {
            return Poll::Ready(Err(anyhow::anyhow!("stream receive reservation exceeded")));
        }
        let mut byte = None;
        source.read(store, &mut byte)?;
        if let Some(byte) = byte {
            let index = self.length;
            self.bytes[index] = byte;
            self.length += 1;
        }
        Poll::Ready(Ok(StreamResult::Completed))
    }
}
impl Drop for ReceiveStream {
    fn drop(&mut self) {
        if let Some(send) = self.send.take() {
            let result = (|| {
                let mut host = self.host.lock();
                let outcome = host.terminal_outcome(self.callback)?;
                host.deliver(self.callback)?;
                Ok(match outcome {
                    Outcome::Success => Ok(self.bytes[..self.length].to_vec()),
                    Outcome::DomainError => Err("domain-error".to_owned()),
                })
            })();
            let _ = send.send(result);
        }
    }
}

pub async fn consume(
    accessor: &Accessor<Runtime>,
    name: &str,
    value: &Val,
    callback: Callback,
    numeric: bool,
    results: &mut [Val],
) -> Result<()> {
    let (case, host, events) = accessor.with(|mut access| {
        (
            access.get().case.clone(),
            access.get().host.clone(),
            access.get().events.clone(),
        )
    });
    if matches!(
        case.as_str(),
        "cancel-before-completion" | "late-completion"
    ) {
        tokio::time::sleep(Duration::from_millis(1)).await;
        host.lock().cancel(callback)?;
        event(&events, "cancel-ack");
        if case == "cancel-before-completion" {
            accessor.with(|mut access| -> Result<()> {
                if name == "finish-future" {
                    if numeric {
                        FutureReader::<i64>::from_val(access.as_context_mut(), value)?
                            .close(access);
                    } else {
                        FutureReader::<DomainValue>::from_val(access.as_context_mut(), value)?
                            .close(access);
                    }
                } else {
                    StreamReader::<u8>::from_val(access.as_context_mut(), value)?.close(access);
                }
                Ok(())
            })?;
            bail!("invocation cancelled before native completion");
        }
    }
    if case == "ready-cancel" {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        while host.lock().state(callback)? == State::Pending {
            ensure!(
                tokio::time::Instant::now() < deadline,
                "native completion deadline"
            );
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        host.lock().cancel(callback)?;
        event(&events, "cancel-ready-ack");
    }
    event(&events, &format!("consume-{name}"));
    if name == "finish-future" && numeric {
        let (send, receive) = oneshot::channel();
        accessor.with(|mut access| -> Result<()> {
            let reader = FutureReader::<i64>::from_val(access.as_context_mut(), value)?;
            reader.pipe(
                access,
                ReceiveFuture {
                    host: host.clone(),
                    callback,
                    send: Some(send),
                },
            );
            Ok(())
        })?;
        results[0] = Val::S64(
            receive
                .await
                .context("numeric future consumer vanished")??,
        );
    } else if name == "finish-future" {
        let (send, receive) = oneshot::channel();
        accessor.with(|mut access| -> Result<()> {
            let reader = FutureReader::<DomainValue>::from_val(access.as_context_mut(), value)?;
            reader.pipe(
                access,
                ReceiveFuture {
                    host: host.clone(),
                    callback,
                    send: Some(send),
                },
            );
            Ok(())
        })?;
        let value = receive.await.context("future consumer vanished")??;
        results[0] = match value {
            Ok(value) => Val::Result(Ok(Some(Box::new(Val::S64(value))))),
            Err(error) => Val::Result(Err(Some(Box::new(Val::String(error))))),
        };
    } else if name == "drain-stream" {
        let (send, receive) = oneshot::channel();
        accessor.with(|mut access| -> Result<()> {
            let reader = StreamReader::<u8>::from_val(access.as_context_mut(), value)?;
            reader.pipe(
                access,
                ReceiveStream {
                    host: host.clone(),
                    callback,
                    send: Some(send),
                    bytes: [0; 64],
                    length: 0,
                },
            );
            Ok(())
        })?;
        let value = receive.await.context("stream consumer vanished")??;
        results[0] = match value {
            Ok(bytes) => Val::Result(Ok(Some(Box::new(Val::List(
                bytes.into_iter().map(Val::U8).collect(),
            ))))),
            Err(error) => Val::Result(Err(Some(Box::new(Val::String(error))))),
        };
    } else {
        bail!("unapproved live consumer")
    }
    if case == "delivery-before-cancel" {
        host.lock().cancel(callback)?;
    }
    event(&events, "terminal-delivered");
    Ok(())
}
