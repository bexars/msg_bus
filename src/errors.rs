use std::error;
use std::fmt;
use tokio::sync::oneshot::error::{RecvError};
use tokio::sync::mpsc::error::SendError;




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
