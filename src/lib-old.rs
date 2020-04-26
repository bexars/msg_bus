/// msg_bus is a simple to use Messaging system built using tokio::sync
use log::*;
use std::collections::HashMap;
use std::sync::Arc;
#[allow(dead_code)]
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::{RecvError};
use tokio::sync::RwLock;
use std::error;
use std::fmt;

mod tests;

type Result<T> = std::result::Result<T, MsgBusError>;

#[derive(Debug)]
enum IntMessage<H, M> {
    // H = Hashable key, M = Message type enum
    Register(H, mpsc::Sender<Message<M>>),
    Unregister(H), // Unregister listener
    Broadcast(M),  // Broadcast to all listeners, from H

    Rpc(H, M, oneshot::Sender<RpcResponse<M>>),
    Message(H, M),
    Shutdown,
}

#[derive(Debug)]
enum RpcResponse<M> {
    Ok(oneshot::Receiver<M>),
    Err(MsgBusError),
}
/// Enum that all listeners will need to process.  
/// 
/// # Message
/// * Broadcast - A message that has been sent to all registered listeners
/// * Rpc - A message that is asking for a response.  Send() the response through the provided oneshot::Sender
/// * Message - Generic message in whatever type (M)essage you provided
/// * Shutdown - Sent when shutdown() is called on msg_bus::MsgBus

#[derive(Debug)]
pub enum Message<M> {
    Broadcast(M),
    Rpc(M, oneshot::Sender<M>),
    Message(M),
    Shutdown,
}

///
/// # MsgBus
/// This is the entry point for running the bus.  There are no options yet.  new() returns a tuple with a 
/// clone of MsgBus and a clone of MsgBusHandle.  This the only copy of MsgBusHandle provided.  Don't lose it.
/// 
/// * Type Parameters
///     * H - Hash or Handle.  This identifies listeners and is stored in a Hashmap
///     * M - Message.  This is the payload, it will be wrapped in a Message enum.  It can be any data structure that
/// can be cloned
/// 
/// * `new()` - Returns a tuple of `(MsgBus<H,M>, MsgBusHandle<H,M>)` 
/// * `shutdown()` - Sends a Message::Shutdown message to all listeners and closes the receive port.  Once the queue
/// is empty it will exit

#[derive(Debug)]
pub struct MsgBus<H, M>
where
    H: std::hash::Hash,
    H: std::cmp::Eq,
{
    senders: Arc<RwLock<HashMap<H, mpsc::Sender<Message<M>>>>>,
    rx: Arc<RwLock<mpsc::Receiver<IntMessage<H, M>>>>,
    tx: mpsc::Sender<IntMessage<H, M>>,
}

impl<
        H: Send + Clone + std::hash::Hash + Eq + PartialEq + Sync + std::fmt::Debug,
        M: Send + Clone + Sync + std::fmt::Debug,
    > MsgBus<H, M>
{
    #[allow(dead_code)]

    pub fn new() -> (MsgBus<H, M>, MsgBusHandle<H, M>)
    where
        H: 'static,
        M: 'static,
    {
        let (tx, rx) = mpsc::channel::<IntMessage<H, M>>(50);
        let rx = Arc::new(RwLock::new(rx));

        let bus = Self {
            senders: Arc::new(RwLock::new(HashMap::<H, mpsc::Sender<Message<M>>>::new())),
            rx,
            tx: tx.clone(),
        };
        let bus2 = bus.clone();
        let s = async move {
            let bus = bus.clone();
            MsgBus::run(bus).await;
        };
        tokio::spawn(s);

        (
            bus2,
            MsgBusHandle {
                bus_tx: tx,
                //id: None,
            },
        )
    }
    /// Sends a `Message::Shutdown` to all registered listeners.  Then shuts down the `MsgBus` process.  
    pub async fn shutdown(mut self) -> Result<()> {
        Ok(self.tx.send(IntMessage::Shutdown).await?)
    }

    async fn run(self)
    where
        H: 'static,
        M: 'static,
    {
        // let rx = self.rx.clone();
        while let Some(msg) = self.rx.write().await.recv().await {
            debug!("Got {:?}", msg);
            // if let IntMessage::Shutdown = msg {
            //     debug!("Shutting down rx");
            //     self.rx.write().await.close();
            // }
            let bus = self.clone();
            tokio::spawn(async move {
                debug!("In spawn with msg: {:?}", msg);
                match msg {
                    IntMessage::Register(key, tx) => {
                        bus.reg(key, tx).await;
                    }
                    IntMessage::Unregister(key) => {
                        bus.unreg(key).await;
                    }
                    IntMessage::Message(key, i_msg) => {
                        bus.msg_to(key, i_msg).await;
                    }
                    IntMessage::Broadcast(i_msg) => {
                        bus.broadcast(i_msg).await;
                    }
                    IntMessage::Rpc(key, i_msg, r_tx) => {
                        bus.rpc(key, i_msg, r_tx).await;
                    }
                    IntMessage::Shutdown => {
                        bus.int_shutdown().await;
                    }
                }
            });
        }
    }

    async fn int_shutdown(&self)  {
        debug!("Begin int_shutdown");
        let mut senders = self.senders.write().await;
        for (_, s) in senders.iter_mut() {
            let _res = s.send(Message::Shutdown).await;
            // drop(s);
            // tokio::task::yield_now().await;
        }
        senders.clear();
        // tokio::task::yield_now().await;
        debug!("Done looping the shutdown");
        self.rx.write().await.close();
        // tokio::task::yield_now().await;

        debug!("Leaving int_shutdown");
    }



    async fn rpc(&self, key: H, msg: M, resp_tx: oneshot::Sender<RpcResponse<M>>) {
        let mut s = self.senders.write().await;
        match s.get_mut(&key) {
            Some(tx) => { 
                let (new_oneshot_tx, rx) = oneshot::channel::<M>();
                match tx.send(Message::Rpc(msg, new_oneshot_tx)).await {
                    Ok(_x) => {
                        resp_tx.send(RpcResponse::Ok(rx)).unwrap();
                    },
                    Err(_e) => {
                        resp_tx.send(RpcResponse::Err(crate::MsgBusError::MsgBusClosed)).unwrap();
                    }
                }
            },
            None => {
                resp_tx.send(RpcResponse::Err(MsgBusError::UnknownRecipient)).unwrap();
                return;
            }
        };
    }

    async fn broadcast(&self, msg: M) {
        let mut senders = self.senders.write().await;

        senders.retain(|k,v| -> bool {
            match futures::executor::block_on(v.send(Message::Broadcast(msg.clone()))) {
                Ok(_) => { true },
                Err(_) => { 
                    debug!("Dropped in broadcast {:?}", k);
                    false },
            }
        })
    }

    async fn msg_to(&self, key: H, msg: M) {
        let mut s = self.senders.write().await;
        if let Some(tx) = &mut s.get_mut(&key) {
            match tx.send(Message::Message(msg)).await {
                Ok(()) => {},
                Err(_) => { s.remove(&key); },
            }
        } 
    }

    async fn unreg(&self, key: H) {
        self.senders.write().await.remove(&key);
    }

    async fn reg(&self, key: H, tx: mpsc::Sender<Message<M>>) {
        self.senders.write().await.insert(key, tx);
    }
}

impl<H: Send + std::hash::Hash + Eq + PartialEq + Sync + std::fmt::Debug,
    M: Send + Clone + Sync + std::fmt::Debug, > Clone for MsgBus<H,M> {
    fn clone(&self) -> Self {
        Self {
            rx: self.rx.clone(),
            tx: self.tx.clone(),
            senders: self.senders.clone(),
        }
    }
}

/// Error type returned when trying to send a message through `MsgBusHandle` and `MsgBus` is shut down

#[derive(Debug)]
pub enum MsgBusError {
    MsgBusClosed,
    UnknownRecipient,
}

// This is important for other errors to wrap this one.
impl error::Error for MsgBusError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

impl fmt::Display for MsgBusError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MsgBusError::MsgBusClosed => { write!(f,"MsgBus is shutdown") },
            MsgBusError::UnknownRecipient => { write!(f, "Destination was not registered")}, 
        }
    }
}

// tokio::sync::oneshot::error::RecvError
impl From<RecvError> for MsgBusError {
    fn from(_: RecvError) -> MsgBusError {
        MsgBusError::MsgBusClosed
    }
}

impl<T> From<SendError<T>> for MsgBusError {
    fn from(_: SendError<T>) -> MsgBusError {
        MsgBusError::MsgBusClosed
    }
}

/// This is the main interface for MsgBus.  It's cloneable, can send to any registered listener and can create and register
/// infinite amount of listeners.  When register is called it will return a tokio::sync::mpsc::Receiver<H,M>.  You will need to
/// create the listen loop to handle messages.   Here's a simple example.
/// 
/// ```
/// tokio::task::spawn(async move {
///    let mut counter = 0;
///    let mut rx = mbh2.register("listener2").await.unwrap();
///    while let Some(msg) = rx.recv().await {
///      match msg {
///         Message::Rpc(input_num, resp_tx) => {
///             resp_tx.send(counter);
///         }
///         Message::Message(input_num) => {
///             mbh2.send("listener3", input_num + 5).await;
///             counter = input_num;
///         }
///         Message::Shutdown => {}  // TODO Handle a shutdown notification?
///         _ => {}  // Ignore Broadcast
///     }
/// }
/// });
/// ```
/// 
/// 
/// This handle is returned as part of the tuple from `MsgBus::new()`
/// 



pub struct MsgBusHandle<H, M> {
    bus_tx: mpsc::Sender<IntMessage<H, M>>,
    // id: Option<H>,
}

impl<H: Send + Sync, M: Send + Sync> Clone for MsgBusHandle<H, M> {
    /// Cloneing is the only way to get more handles.  Each handle has no memory except for how to talk to the `MsgBus`.  Any handle can send
    /// to any listener
    fn clone(&self) -> Self {
        Self {
            bus_tx: self.bus_tx.clone(),
        }
    }
}

impl<H: Send + Sync, M: Send + Sync> MsgBusHandle<H, M> {
    // pub fn setId(mut self, id: H) {
    //     self.id = Some(id);
    // }



    /// Returns a Receiver that will get any messages destined for `id`.  The messages will be encased in the `Message` enum.
    pub async fn register(&mut self, id: H) -> Result<mpsc::Receiver<Message<M>>>
    where
        H: 'static,
        M: 'static,
    {
        let (tx, rx) = mpsc::channel::<Message<M>>(50);
        if let Err(e) = self._send(IntMessage::Register(id, tx)).await {
            Err(e)
        } else {
            Ok(rx)
        }
    }

    pub async fn unregister(&mut self, id: H) -> Result<()>
    where
        H: 'static,
        M: 'static,
    {
        if let Err(e) = self._send(IntMessage::Unregister(id)).await {
            Err(e)
        } else {
            Ok(())
        }
    }


    /// Sends a message of type M to all listeners/receivers.  It will show up as `Message::Broadcast(M)` at the listeners
    pub async fn broadcast(&mut self, msg: M) -> Result<()>
    where
        H: 'static,
        M: 'static,
    {
        if let Err(e) = self._send(IntMessage::Broadcast(msg)).await {
            Err(e)
        } else {
            Ok(())
        }
    }


    /// A simple RPC function that sends a message to a specific listener and gives them a `tokio::sync::oneshot::Sender<M>` to reply with.
    /// The listener will receive a `Message::Rpc(M, oneshot::Sender<M>`).  There are no timeouts, though the Receiver will error if the Sender Drops
    /// 
    pub async fn rpc(&mut self, dest: H, msg: M) -> Result<M>
    where
        H: 'static,
        M: 'static,
    {
        let (tx, rx) = oneshot::channel::<RpcResponse<M>>();
        if let Err(e) = self._send(IntMessage::Rpc(dest, msg, tx)).await {
            return Err(e);
        };
        match rx.await {
            Err(e) => { return Err(MsgBusError::from(e)) },
            Ok(RpcResponse::Err(e)) => { Err(e) },
            Ok(RpcResponse::Ok(rpc_resp)) => {
                Ok(rpc_resp.await?)
            }
        }
        // {
        //     Err(_) => Err(MsgBusError::MsgBusClosed),
        //     Ok(in_msg) => Ok(in_msg),
        // }
    }



    /// Straightforward message sending function.  The selected listener on 'dest' will receive a `Message::Message(M)` enum.
    pub async fn send(&mut self, dest: H, msg: M) -> Result<()>
    where
        H: 'static,
        M: 'static,
    {
        Ok(self._send(IntMessage::Message(dest, msg)).await?)
        // if let Err(e) = self._send(IntMessage::Message(dest, msg)).await {
        //     Err(e)
        // } else {
        //     Ok(())
        // }
    }

    async fn _send<'a>(&self, msg: IntMessage<H, M>) -> std::result::Result<(), MsgBusError>
    where
        H: 'static,
        M: 'static,
    {
        let msg = msg;
        let mut bus_tx = self.bus_tx.clone();
        Ok(bus_tx.send(msg).await?)
    }
        }

