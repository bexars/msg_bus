/// msg_bus is a simple to use Messaging system built using tokio::sync
use log::*;
use std::collections::HashMap;
use std::sync::Arc;
#[allow(dead_code)]
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::RwLock;

#[derive(Debug)]
enum IntMessage<H, M> {
    // H = Hashable key, M = Message type enum
    Register(H, mpsc::Sender<Message<M>>),
    Unregister(H), // Unregister listener
    Broadcast(M),  // Broadcast to all listeners, from H
    Rpc(H, M, oneshot::Sender<M>),
    Message(H, M),
    Shutdown,
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

#[derive(Clone, Debug)]
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
        H: Send + std::hash::Hash + Eq + PartialEq + Sync + std::fmt::Debug,
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
    pub async fn shutdown(mut self) {
        self.tx.send(IntMessage::Shutdown).await;
    }

    async fn run(self)
    where
        H: 'static,
        M: 'static,
    {
        while let Some(msg) = self.rx.write().await.recv().await {
            debug!("Got {:?}", msg);
            // if let IntMessage::Shutdown = msg {
            //     debug!("Shutting down rx");
            //     //self.rx.write().await.close();
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
        for (_, s) in self.senders.write().await.iter_mut() {
            s.clone().send(Message::Shutdown).await;
            tokio::task::yield_now().await;
        }
        tokio::task::yield_now().await;
        debug!("Done looping the shutdown");
        self.rx.write().await.close();
        tokio::task::yield_now().await;

        debug!("Leaving int_shutdown");
    }

    async fn rpc(&self, key: H, msg: M, resp_tx: oneshot::Sender<M>) {
        let s = self.senders.write().await;
        let mut tx = s.get(&key).unwrap().clone();
        tx.send(Message::Rpc(msg, resp_tx)).await;
    }

    async fn broadcast(&self, msg: M) {
        for (_, s) in self.senders.write().await.iter_mut() {
            s.clone().send(Message::Broadcast(msg.clone())).await;
        }
    }

    async fn msg_to(&self, key: H, msg: M) {
        let s = self.senders.write().await;
        let mut tx = s.get(&key).unwrap().clone();
        tx.send(Message::Message(msg)).await;
    }

    async fn unreg(&self, key: H) {
        self.senders.write().await.remove(&key);
    }

    async fn reg(&self, key: H, tx: mpsc::Sender<Message<M>>) {
        self.senders.write().await.insert(key, tx);
    }

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
pub struct MsgBusClosed {}

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

impl<H: Send + Sync, M: Send + Sync> MsgBusHandle<H, M> {
    // pub fn setId(mut self, id: H) {
    //     self.id = Some(id);
    // }

    /// Cloneing is the only way to get more handles.  Each handle has no memory except for how to talk to the `MsgBus`.  Any handle can send
    /// to any listener
    pub fn clone(&self) -> Self {
        Self {
            bus_tx: self.bus_tx.clone(),
        }
    }

    /// Returns a Receiver that will get any messages destined for `id`.  The messages will be encased in the `Message` enum.
    pub async fn register(&mut self, id: H) -> Result<mpsc::Receiver<Message<M>>, MsgBusClosed>
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

    /// Sends a message of type M to all listeners/receivers.  It will show up as `Message::Broadcast(M)` at the listeners
    pub async fn broadcast(&mut self, msg: M) -> Result<(), MsgBusClosed>
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
    pub async fn rpc(&mut self, dest: H, msg: M) -> Result<M, MsgBusClosed>
    where
        H: 'static,
        M: 'static,
    {
        let (tx, rx) = oneshot::channel::<M>();
        if let Err(e) = self._send(IntMessage::Rpc(dest, msg, tx)).await {
            return Err(e);
        };
        match rx.await {
            Err(e) => Err(MsgBusClosed {}),
            Ok(in_msg) => Ok(in_msg),
        }
    }


    /// Straightforward message sending function.  The selected listener on 'dest' will receive a `Message::Message(M)` enum.
    pub async fn send(&mut self, dest: H, msg: M) -> Result<(), MsgBusClosed>
    where
        H: 'static,
        M: 'static,
    {
        if let Err(e) = self._send(IntMessage::Message(dest, msg)).await {
            Err(e)
        } else {
            Ok(())
        }
    }

    async fn _send<'a>(&self, msg: IntMessage<H, M>) -> Result<(), MsgBusClosed>
    where
        H: 'static,
        M: 'static,
    {
        let msg = msg;
        let mut bus_tx = self.bus_tx.clone();

        if let Err(_) = bus_tx.send(msg).await {
            Err(MsgBusClosed {})
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use std::{thread, time};
    use tokio::time::delay_for;
    // enum TestMessage {
    //     Hello(String),
    //     World(usize),
    // }
    #[tokio::test]
    async fn test_send_message() {
        // env_logger::init();

        let (_, mut mbh) = MsgBus::<usize, String>::new();
        let mut mbh2 = mbh.clone();

        let mut rx = mbh.register(1001).await.unwrap();
        mbh2.send(1001, "Hello".to_string()).await;
        println!("rx: {:?}", rx);
        let response = rx.recv().await;
        println!("response: {:?}", response);

        let response = response.unwrap();
        let answer = match response {
            Message::Message(text) => text,
            _ => "Failure".to_string(),
        };
        assert_eq!(answer, "Hello".to_string());
    }

    #[tokio::test]
    async fn test_broadcast() {
        let (_, mut mbh) = MsgBus::<usize, String>::new();
        let mut mbh2 = mbh.clone();
        let mut mbh3 = mbh.clone();

        let mut rx = mbh.register(1001).await.unwrap();
        let mut rx2 = mbh2.register(2002).await.unwrap();
        let mut rx3 = mbh3.register(3003).await.unwrap();
        mbh.broadcast("Hello".to_string()).await;
        let resp = rx.recv().await.unwrap();
        let resp2 = rx2.recv().await.unwrap();
        let resp3 = rx3.recv().await.unwrap();
        let ans1 = match resp {
            Message::Broadcast(text) => text,
            _ => "Failure".to_string(),
        };
        let ans2 = match resp2 {
            Message::Broadcast(text) => text,
            _ => "Failure".to_string(),
        };
        let ans3 = match resp3 {
            Message::Broadcast(text) => text,
            _ => "Failure".to_string(),
        };

        assert_eq!(ans1, "Hello".to_string());
        assert_eq!(ans2, "Hello".to_string());
        assert_eq!(ans3, "Hello".to_string());
    }

    #[should_panic]
    #[tokio::test]
    async fn test_shutdown_panic() {
        env_logger::init();

        let (mut msg_bus, mut mbh) = MsgBus::<usize, String>::new();
        let mut mbh2 = mbh.clone();

        let mut rx = mbh.register(1001).await.unwrap();
        tokio::task::spawn(async move {
            mbh2.send(1001, "Hello".to_string()).await;
            delay_for(Duration::from_millis(1000)).await;
            debug!("Awake");
            mbh2.send(1001, "Hello".to_string()).await;
            msg_bus.shutdown().await;
            delay_for(Duration::from_millis(1000)).await;
            debug!("Awake");
            mbh2.send(1001, "Hello".to_string()).await;
        });

        while let Some(response) = rx.recv().await {
            let answer = match response {
                Message::Shutdown => {
                    debug!("SHUT DOWN");

                    "Shutdown".to_string()
                }
                Message::Message(text) => text,
                _ => "Failure".to_string(),
            };
        }
        let should_be_none = rx.recv().await.unwrap();
        debug!("should be none: {:?}", should_be_none);
        tokio::task::yield_now().await;

        //assert_eq!(answer, "Shutdown".to_string());
        //assert(let None = should_be_none);
    }

    #[tokio::test]
    async fn test_shutdown_message() {
        env_logger::init();

        let (mut msg_bus, mut mbh) = MsgBus::<usize, String>::new();
        let mut mbh2 = mbh.clone();

        let mut rx = mbh.register(1001).await.unwrap();
        mbh2.send(1002, "Hello".to_string()).await;
        let response = rx.recv();

        msg_bus.shutdown().await;
        let response = response.await.unwrap();
        debug!("Response progressed");
        let answer = match response {
            Message::Shutdown => "Shutdown".to_string(),
            Message::Message(text) => text,
            _ => "Failure".to_string(),
        };
        // let should_be_none = rx.recv().await.unwrap();
        tokio::task::yield_now().await;
        let ten_millis = time::Duration::from_millis(1000);
        let now = time::Instant::now();
        thread::sleep(ten_millis);

        assert_eq!(answer, "Shutdown".to_string());
        //assert(let None = should_be_none);
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_rpc() {
        env_logger::init();

        let (mut msg_bus, mut mbh) = MsgBus::<usize, usize>::new();
        let mut mbh2 = mbh.clone();

        let mut rx = mbh.register(1001).await.unwrap();
        tokio::task::spawn(async move {
            // mbh2.send(1001, "Hello".to_string()).await;
            let mut rx = mbh2.register(2000).await.unwrap();
            while let Some(msg) = rx.recv().await {
                match msg {
                    Message::Rpc(input_num, resp_tx) => {
                        resp_tx.send(input_num + 69);
                    }
                    Message::Message(input_num) => {
                        debug!("In loop Message: {}", input_num);
                    }
                    Message::Shutdown => {}
                    _ => {}
                }
            }
        });
        // tokio::task::yield_now().await;
        // tokio::task::yield_now().await;
        // tokio::task::yield_now().await;
        delay_for(Duration::from_millis(1000)).await;
        mbh.send(2000, 1000).await;
        assert_eq!(mbh.rpc(2000, 420).await.unwrap(), 489);
        msg_bus.shutdown().await;
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_pingpong() {
        env_logger::init();

        let (mut msg_bus, mut mbh) = MsgBus::<usize, usize>::new();
        let mut mbh2 = mbh.clone();
        let mut mbh3 = mbh.clone();

        let mut rx = mbh.register(1001).await.unwrap();
        tokio::task::spawn(async move {
            let mut counter = 0;
            let mut rx = mbh2.register(2000).await.unwrap();
            while let Some(msg) = rx.recv().await {
                match msg {
                    Message::Rpc(input_num, resp_tx) => {
                        resp_tx.send(counter);
                    }
                    Message::Message(input_num) => {
                        mbh2.send(3000, input_num + 5).await;
                        counter = input_num;
                    }
                    Message::Shutdown => {}
                    _ => {}
                }
            }
        });
        tokio::task::spawn(async move {
            let mut rx = mbh3.register(3000).await.unwrap();
            while let Some(msg) = rx.recv().await {
                match msg {
                    Message::Message(input_num) => {
                        mbh3.send(2000, input_num + 1).await;
                    }
                    Message::Shutdown => {}
                    _ => {}
                }
            }
        });
        delay_for(Duration::from_millis(100)).await;

        let mut num = 0;
        mbh.send(2000, 0).await;
        while num < 500000 {
            delay_for(Duration::from_millis(100)).await;
            num = mbh.rpc(2000, 0).await.unwrap();
            info!("Num = {}", num);
        }

        // tokio::task::yield_now().await;
        // tokio::task::yield_now().await;
        // tokio::task::yield_now().await;
        delay_for(Duration::from_millis(1000)).await;
        mbh.send(2000, 1000).await;
        assert!(mbh.rpc(2000, 420).await.unwrap() > 500000);
        msg_bus.shutdown().await;
    }
}
