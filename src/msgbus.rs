use crate::*;
/// msg_bus is a simple to use Messaging system built using tokio::sync
use log::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::RwLock;

///
/// # MsgBus
/// This is the entry point for running the bus.  There are no options yet.  new() returns a tuple with a
/// clone of MsgBus and a clone of [MsgBusHandle](struct.MsgBusHandle.html).  This the only copy of MsgBusHandle provided.  Don't lose it.
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
        let (tx, rx) = mpsc::channel::<IntMessage<H, M>>(1);
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
    pub async fn shutdown(&mut self) -> Result<()> {
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

    async fn int_shutdown(&self) {
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
                    }
                    Err(_e) => {
                        resp_tx
                            .send(RpcResponse::Err(crate::MsgBusError::MsgBusClosed))
                            .unwrap();
                    }
                }
            }
            None => {
                resp_tx
                    .send(RpcResponse::Err(MsgBusError::UnknownRecipient))
                    .unwrap();
                return;
            }
        };
    }

    pub(crate) async fn broadcast(&self, msg: M) {
        let senders = &mut self.senders.write().await;
        let mut to_remove = Vec::new();
        for (k,v) in senders.iter_mut() {
            match futures::executor::block_on(v.send(Message::Broadcast(msg.clone()))) {
                Ok(_) => {}
                Err(_) => {
                    debug!("Dropped in broadcast {:?}", k);
                    let k = k.clone();
                    to_remove.push(k);
                    // senders.remove(&k);
                    
                }
            }
        }
        for k in to_remove {
            senders.remove(&k);
        }
    }

    async fn msg_to(&self, key: H, msg: M) {
        let mut senders = self.senders.write().await;
        if let Some(tx) = &mut senders.get_mut(&key) {
            match tx.send(Message::Message(msg)).await {
                Ok(()) => {}
                Err(_) => {
                    senders.remove(&key);
                }
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

impl<
        H: Send + std::hash::Hash + Eq + PartialEq + Sync + std::fmt::Debug,
        M: Send + Clone + Sync + std::fmt::Debug,
    > Clone for MsgBus<H, M>
{
    fn clone(&self) -> Self {
        Self {
            rx: self.rx.clone(),
            tx: self.tx.clone(),
            senders: self.senders.clone(),
        }
    }
}
