use std::{future::Future, pin::Pin, sync::Arc};

use tokio::sync::{mpsc, oneshot, Mutex};

use crate::{
    InterfaceContract, InterfaceHandlerContext, InterfaceInvocationFailure,
    InterfaceInvocationReceipt, InterfaceStreamStateError, InterfaceStreamTerminal,
    InterfaceTargetFailure, InvocationPrincipal, UserPrincipal,
};

type StreamTerminalSender<O, E> =
    Arc<Mutex<Option<oneshot::Sender<InterfaceStreamTerminal<O, E>>>>>;
type InterfaceStreamCompletionFuture<O, E> = Pin<
    Box<
        dyn Future<
                Output = Result<InterfaceStreamTerminalOutcome<O, E>, InterfaceInvocationFailure>,
            > + Send,
    >,
>;

pub type InterfaceStreamHandlerFuture<S, O, E> = Pin<
    Box<
        dyn Future<Output = Result<InterfaceEventStream<S, O, E>, InterfaceTargetFailure<E>>>
            + Send
            + 'static,
    >,
>;

pub trait InterfaceStreamHandler<I, S, O, E, P = UserPrincipal>: Send + Sync + 'static
where
    I: InterfaceContract,
    S: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    fn invoke_stream(
        &self,
        context: InterfaceHandlerContext<P>,
        input: I,
    ) -> InterfaceStreamHandlerFuture<S, O, E>;
}

pub struct InterfaceStreamPublisher<S, O, E>
where
    S: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
{
    events: mpsc::Sender<S>,
    terminal: StreamTerminalSender<O, E>,
}

impl<S, O, E> InterfaceStreamPublisher<S, O, E>
where
    S: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
{
    pub async fn emit(&self, event: S) -> Result<(), InterfaceStreamStateError> {
        let terminal = self.terminal.lock().await;
        if terminal.is_none() {
            return Err(InterfaceStreamStateError::EventAfterTerminal);
        }
        let result = self
            .events
            .send(event)
            .await
            .map_err(|_| InterfaceStreamStateError::EventAfterTerminal);
        drop(terminal);
        result
    }

    pub async fn finish(
        &self,
        terminal: InterfaceStreamTerminal<O, E>,
    ) -> Result<(), InterfaceStreamStateError> {
        self.terminal
            .lock()
            .await
            .take()
            .ok_or(InterfaceStreamStateError::DuplicateTerminal)?
            .send(terminal)
            .map_err(|_| InterfaceStreamStateError::DuplicateTerminal)
    }
}

pub struct InterfaceEventStream<S, O, E>
where
    S: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
{
    pub(crate) events: mpsc::Receiver<S>,
    pub(crate) terminal: oneshot::Receiver<InterfaceStreamTerminal<O, E>>,
}

pub fn interface_stream_channel<S, O, E>(
    capacity: usize,
) -> (
    InterfaceStreamPublisher<S, O, E>,
    InterfaceEventStream<S, O, E>,
)
where
    S: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
{
    let (events_tx, events_rx) = mpsc::channel(capacity.max(1));
    let (terminal_tx, terminal_rx) = oneshot::channel();
    (
        InterfaceStreamPublisher {
            events: events_tx,
            terminal: Arc::new(Mutex::new(Some(terminal_tx))),
        },
        InterfaceEventStream {
            events: events_rx,
            terminal: terminal_rx,
        },
    )
}

pub struct InterfaceStreamInvocation<S, O, E>
where
    S: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
{
    pub(crate) events: mpsc::Receiver<S>,
    pub(crate) completion: InterfaceStreamCompletion<O, E>,
}

impl<S, O, E> InterfaceStreamInvocation<S, O, E>
where
    S: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
{
    pub fn into_parts(self) -> (mpsc::Receiver<S>, InterfaceStreamCompletion<O, E>) {
        (self.events, self.completion)
    }
}

pub struct InterfaceStreamCompletion<O, E>
where
    O: InterfaceContract,
    E: InterfaceContract,
{
    pub(crate) completion: InterfaceStreamCompletionFuture<O, E>,
}

impl<O, E> InterfaceStreamCompletion<O, E>
where
    O: InterfaceContract,
    E: InterfaceContract,
{
    pub async fn complete(
        self,
    ) -> Result<InterfaceStreamTerminalOutcome<O, E>, InterfaceInvocationFailure> {
        self.completion.await
    }
}

pub struct InterfaceStreamTerminalOutcome<O, E>
where
    O: InterfaceContract,
    E: InterfaceContract,
{
    pub(crate) terminal: InterfaceStreamTerminal<O, E>,
    pub(crate) receipt: InterfaceInvocationReceipt,
}

impl<O, E> InterfaceStreamTerminalOutcome<O, E>
where
    O: InterfaceContract,
    E: InterfaceContract,
{
    pub fn terminal(&self) -> &InterfaceStreamTerminal<O, E> {
        &self.terminal
    }

    pub fn receipt(&self) -> &InterfaceInvocationReceipt {
        &self.receipt
    }

    pub fn into_parts(self) -> (InterfaceStreamTerminal<O, E>, InterfaceInvocationReceipt) {
        (self.terminal, self.receipt)
    }
}
