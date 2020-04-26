

/// msg_bus is a simple to use Messaging system built using tokio::sync

use tokio::sync::mpsc;
use tokio::sync::oneshot;
pub use crate::msgbushandle::MsgBusHandle as MsgBusHandle;
pub use crate::msgbus::MsgBus as MsgBus;
pub use crate::errors::MsgBusError as MsgBusError;

mod tests;
mod msgbus;
mod msgbushandle;
mod errors;

pub type Result<T> = std::result::Result<T, MsgBusError>;

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



