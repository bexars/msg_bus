use env_logger;
use log::*;
use tokio::time::sleep;

use crate::*;
use std::time::Duration;
use std::{thread, time};
// use tokio::time::sleep;
//use log::Level;
#[cfg(test)]
fn init() {
    let _ = env_logger::builder().is_test(true).try_init();
}

#[tokio::test]
async fn test_register() {
    let (_, mut mbh) = MsgBus::<String, String>::new();
    let mut rx = mbh.register("test".to_string()).await.unwrap();
    mbh.send("test".to_string(), "registered?".to_string())
        .await
        .unwrap();
    let result = if let Message::Message(result) = rx.recv().await.unwrap() {
        result
    } else {
        "failure".to_string()
    };
    assert_eq!(&result, "registered?");
}

#[tokio::test]
#[should_panic]
async fn test_unregister() {
    let (_, mut mbh) = MsgBus::<String, String>::new();
    let mut rx = mbh.register("test".to_string()).await.unwrap();
    mbh.send("test".to_string(), "registered?".to_string())
        .await
        .unwrap();
    let _result = if let Message::Message(result) = rx.recv().await.unwrap() {
        result
    } else {
        "failure".to_string()
    };
    mbh.unregister("test".to_string()).await.unwrap();
    mbh.send("test".to_string(), "should be dropped".to_string())
        .await
        .unwrap();
    let _result = rx.recv().await.unwrap(); // Unwrap should panic
}

#[tokio::test]

async fn test_send_unregistered_silent() {
    // should drop without panicking
    let (_, mut mbh) = MsgBus::<String, u32>::new();
    mbh.send("drop".to_string(), 1234).await.unwrap();
}

#[tokio::test]
async fn test_send_message() {
    // env_logger::init();

    let (_, mut mbh) = MsgBus::<usize, String>::new();
    let mut mbh2 = mbh.clone();

    let mut rx = mbh.register(1001).await.unwrap();
    mbh2.send(1001, "Hello".to_string()).await.unwrap();
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
    mbh.broadcast("Hello".to_string()).await.unwrap();
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

// #[tokio::test]
// // #[tokio::test(threaded_scheduler)]
// async fn test_broadcast_after_drop_rx() {
//     init();
//     let (bus, mut mbh) = MsgBus::<usize, String>::new();
//     let mut mbh2 = mbh.clone();
//     let mut mbh3 = mbh.clone();

//     let mut rx = mbh.register(1001).await.unwrap();
//     let rx2 = mbh2.register(2002).await.unwrap();
//     let mut rx3 = mbh3.register(3003).await.unwrap();
//     drop(rx2);
//     println!("before broadcast");
//     let _resp = bus.broadcast("Direct".to_string()).await;
//     tokio::task::yield_now().await;
//     tokio::task::yield_now().await;
//     tokio::task::yield_now().await;
//     tokio::task::yield_now().await;
//     tokio::task::yield_now().await;
//     tokio::task::yield_now().await;
//     tokio::task::yield_now().await;
//     tokio::task::yield_now().await;

//     //.await.unwrap();
//     //        panic!("Fail now!");
//     let resp = mbh.broadcast("Hello".to_string()).await;
//     println!("{:?}", resp);
//     tokio::task::yield_now().await;

//     println!("after broadcast");
//     let resp = rx.recv().await.unwrap();
//     // let resp2 = rx2.recv().await.unwrap();
//     let resp3 = rx3.recv().await.unwrap();
//     let ans1 = match resp {
//         Message::Broadcast(text) => text,
//         _ => "Failure".to_string(),
//     };
//     // let ans2 = match resp2 {
//     //     Message::Broadcast(text) => text,
//     //     _ => "Failure".to_string(),
//     // };
//     let ans3 = match resp3 {
//         Message::Broadcast(text) => text,
//         _ => "Failure".to_string(),
//     };

//     assert_eq!(ans1, "Hello".to_string());
//     // assert_eq!(ans2, "Hello".to_string());
//     assert_eq!(ans3, "Hello".to_string());
// }

#[tokio::test]
async fn test_broadcast_after_unregister() {
    let (_, mut mbh) = MsgBus::<usize, String>::new();
    let mut mbh2 = mbh.clone();
    let mut mbh3 = mbh.clone();

    let mut rx = mbh.register(1001).await.unwrap();
    let _rx2 = mbh2.register(2002).await.unwrap();
    let mut rx3 = mbh3.register(3003).await.unwrap();
    mbh.unregister(2002).await.unwrap();
    mbh.broadcast("Hello".to_string()).await.unwrap();
    let resp = rx.recv().await.unwrap();
    //        let resp2 = rx2.recv().await.unwrap();
    let resp3 = rx3.recv().await.unwrap();
    let ans1 = match resp {
        Message::Broadcast(text) => text,
        _ => "Failure".to_string(),
    };
    // let ans2 = match resp2 {
    //     Message::Broadcast(text) => text,
    //     _ => "Failure".to_string(),
    // };
    let ans3 = match resp3 {
        Message::Broadcast(text) => text,
        _ => "Failure".to_string(),
    };

    assert_eq!(ans1, "Hello".to_string());
    // assert_eq!(ans2, "Hello".to_string());
    assert_eq!(ans3, "Hello".to_string());
}

#[tokio::test]
async fn test_nonexistant_destination_send() {
    let (_msg_bus, mut mbh) = MsgBus::<usize, usize>::new();
    mbh.send(1000, 0).await.unwrap();
}

#[tokio::test]
async fn test_nonexistant_destination_rpc() {
    let (_msg_bus, mut mbh) = MsgBus::<usize, usize>::new();
    let resp = mbh.rpc(1000, 0).await;
    if let Err(MsgBusError::UnknownRecipient) = resp {
        return;
    } else {
        panic!("")
    };
}

#[tokio::test]
async fn test_rpc_timeout() {
    let (_msg_bus, mut mbh) = MsgBus::<usize, usize>::new();
    let _rx = mbh.register(1000).await.unwrap();

    let resp = mbh.rpc_timeout(1000, 0, Duration::from_millis(1000)).await;
    println!("resp = {:?}", resp);
    if let Err(MsgBusError::MsgBusTimeout) = resp {
        return;
    } else {
        panic!("")
    };
}

#[should_panic]
#[tokio::test]
async fn test_shutdown_panic2() {
    init();

    let (mut msg_bus, mut mbh) = MsgBus::<usize, String>::new();
    let mut mbh2 = mbh.clone();

    let mut rx = mbh.register(1001).await.unwrap();
    tokio::task::spawn(async move {
        mbh2.send(1001, "Hello".to_string()).await.unwrap();
        sleep(Duration::from_millis(1000)).await;
        debug!("Awake");
        mbh2.send(1001, "Hello".to_string()).await.unwrap();
        msg_bus.shutdown().await.unwrap();
        sleep(Duration::from_millis(1000)).await;
        debug!("Awake");
        mbh2.send(1001, "Hello".to_string()).await.unwrap();
    });

    while let Some(response) = rx.recv().await {
        let _answer = match response {
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

#[should_panic]
// #[tokio::test]
#[tokio::test()]

async fn test_shutdown_panic1() {
    init();
    let (mut msg_bus, mut mbh) = MsgBus::<usize, String>::new();
    let mut mbh2 = mbh.clone();

    let mut rx = mbh.register(1001).await.unwrap();
    tokio::task::spawn(async move {
        mbh2.send(1001, "Hello".to_string()).await.unwrap();
        sleep(Duration::from_millis(1000)).await;
        debug!("Awake");
        mbh2.send(1001, "Hello".to_string()).await.unwrap();
        msg_bus.shutdown().await.unwrap();
        sleep(Duration::from_millis(1000)).await;
        debug!("Awake");
        //mbh2.send(1001, "Hello".to_string()).await.unwrap();
    });
    let _answer = String::from("");
    while let Some(response) = rx.recv().await {
        match response {
            Message::Shutdown => {
                break;
            }
            Message::Message(text) => text,
            _ => "Failure".to_string(),
        };
    }
    let should_be_none = rx.recv().await.unwrap();
    debug!("should be none: {:?}", should_be_none);
    // assert_eq!(answer, "Shutdown".to_string());
    // assert(let None = should_be_none);
}

#[tokio::test]
async fn test_shutdown_message() {
    let (mut msg_bus, mut mbh) = MsgBus::<usize, String>::new();
    let mut mbh2 = mbh.clone();

    let mut rx = mbh.register(1001).await.unwrap();
    mbh2.send(1002, "Hello".to_string()).await.unwrap();
    let response = rx.recv();

    msg_bus.shutdown().await.unwrap();
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
    // let now = time::Instant::now();
    thread::sleep(ten_millis);

    assert_eq!(answer, "Shutdown".to_string());
    //assert(let None = should_be_none);
}

#[tokio::test()]
async fn test_rpc() {
    // env_logger::init();

    let (mut msg_bus, mut mbh) = MsgBus::<usize, usize>::new();
    let mut mbh2 = mbh.clone();

    let _rx = mbh.register(1001).await.unwrap();
    tokio::task::spawn(async move {
        // mbh2.send(1001, "Hello".to_string()).await;
        let mut rx = mbh2.register(2000).await.unwrap();
        while let Some(msg) = rx.recv().await {
            match msg {
                Message::Rpc(input_num, resp_tx) => {
                    resp_tx.send(input_num + 69).unwrap();
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
    sleep(Duration::from_millis(1000)).await;
    mbh.send(2000, 1000).await.unwrap();
    assert_eq!(mbh.rpc(2000, 420).await.unwrap(), 489);
    msg_bus.shutdown().await.unwrap();
}

#[tokio::test()]
async fn test_pingpong() {
    let (_msg_bus, mut mbh) = MsgBus::<usize, usize>::new();
    let mut mbh2 = mbh.clone();
    let mut mbh3 = mbh.clone();

    let _rx = mbh.register(1001).await.unwrap();
    tokio::task::spawn(async move {
        let mut counter = 0;
        let mut rx = mbh2.register(2000).await.unwrap();
        while let Some(msg) = rx.recv().await {
            match msg {
                Message::Rpc(_input_num, resp_tx) => {
                    resp_tx.send(counter).unwrap();
                }
                Message::Message(input_num) => {
                    mbh2.send(3000, input_num + 5).await.unwrap();
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
                    mbh3.send(2000, input_num + 1).await.unwrap();
                }
                Message::Shutdown => {}
                _ => {}
            }
        }
    });
    sleep(Duration::from_millis(100)).await;

    let mut num = 0;
    mbh.send(2000, 0).await.unwrap();
    while num < 5000 {
        sleep(Duration::from_millis(100)).await;
        num = mbh.rpc(2000, 0).await.unwrap();
        // info!("Num = {}", num);
    }

    // tokio::task::yield_now().await;
    // tokio::task::yield_now().await;
    // tokio::task::yield_now().await;
    sleep(Duration::from_millis(1000)).await;
    // mbh.send(2000, 1000).await;
    let result = mbh.rpc(2000, 420).await.unwrap();
    println!("result: {}", result);
    assert!(result > 5000);
    // msg_bus.shutdown().await;
}
