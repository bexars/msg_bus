use crate::*;
/// msg_bus is a simple to use Messaging system built using tokio::sync
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::Duration;

/// This is the main interface for MsgBus.  It's cloneable, can send to any registered listener and can create and register
/// infinite amount of listeners.  When register is called it will return a tokio::sync::mpsc::Receiver<H,M>.  You will need to
/// create the listen loop to handle messages.   Here's a simple example.
///
/// ```
/// use msgbus::*;
///
/// enum ExampleMsg {
///     PingBcast,
///     EchoReq,
///     Echo,
///     Pong(usize),  
/// }
/// #[tokio::main]
/// async fn main() {
///    let (mbus, mut mbushan) = MsgBus::<String, u32>::new();
///
/// tokio::task::spawn(async move {
///    let mut counter: u32 = 0;
///    let mut rx = mbushan.register("listener".to_string()).await.unwrap();
///    while let Some(msg) = rx.recv().await {
///      match msg {
///         Message::Rpc(input_num, resp_tx) => {
///             resp_tx.send(counter);
///         }
///         Message::Message(input_num) => {
///             mbushan.send("listener3".to_string(), input_num + 5).await;
///             counter = input_num;
///         }
///         Message::Shutdown => {}  // TODO Handle a shutdown notification?
///         _ => {}  // Ignore Broadcast
///     }
/// }
/// });
/// }
/// ```
///
///
/// This handle is returned as part of the tuple from `MsgBus::new()`
///

pub struct MsgBusHandle<H, M> {
    pub(crate) bus_tx: mpsc::Sender<IntMessage<H, M>>,
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
        let (tx, rx) = mpsc::channel::<Message<M>>(1);
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

    /// Wrapper function to call rpc_timeout with a 10_000 millisecond timeout

    pub async fn rpc_timeout(&mut self, dest: H, msg: M, wait: Duration) -> Result<M>
    where
        H: 'static,
        M: 'static,
    {
        tokio::time::timeout(wait, self.rpc(dest, msg))
            .await
            .unwrap_or(Err(MsgBusError::MsgBusTimeout))
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
            Err(e) => return Err(MsgBusError::from(e)),
            Ok(RpcResponse::Err(e)) => Err(e),
            Ok(RpcResponse::Ok(rpc_resp)) => Ok(rpc_resp.await?),
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
