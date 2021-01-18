use tokio::time::sleep;

use super::*;
use std::time::Duration;
// use tokio::time::delay_for;

pub struct Marco {}

impl Marco {
    pub async fn start(
        mut handle: MsgBusHandle<String, MarcoPoloMsg>,
        mut bus: MsgBus<String, MarcoPoloMsg>,
    ) {
        let stdin: tokio::io::Stdin = tokio::io::stdin();
        let mut stdin = tokio::io::BufReader::new(stdin);

        let mut rx = handle.register(String::from("Marco")).await.unwrap();
        loop {
            let mut input = String::new();
            tokio::select! {
                Ok(_len) = stdin.read_line(&mut input) => {
                    let input = input.trim();
                    match &*input {
                        "quit" => {
                            handle.broadcast(MarcoPoloMsg::LifeguardYells("Pool is closing in 5 seconds!".to_string())).await.unwrap();
                            sleep(Duration::from_millis(5000)).await;
                            bus.shutdown().await.unwrap();
                            break;
                        }
                        _ => {
                            // Any unprocessed input is a Marco call
                            println!("Yelling \"Marco!\"");
                            handle.broadcast(MarcoPoloMsg::Marco).await.unwrap();
                        }
                    }
                    // Any input is a Marco call
                    println!("Yelling \"Marco!\"");
                    handle.broadcast(MarcoPoloMsg::Marco).await.unwrap();
                },
                Some(msg) = rx.recv() => {
                    match msg {
                        Message::Shutdown => {
                            // simply exit the loop
                            break;
                        }
                        Message::Broadcast(mp_msg ) => {
                            match mp_msg {
                                MarcoPoloMsg::Polo => {
                                    println!("Polo!");
                                }
                                MarcoPoloMsg::PlayerJumpedIn => {
                                    println!("A loud splash from the other side of the pool!  A new player to hunt?!");
                                }
                                MarcoPoloMsg::PlayerClimbedOut => {
                                    println!("Sounds like someone just climbed out of the pool...  Some people can't handle it.");
                                }
                                _ => {}
                            };
                        }
                        _ => {}  // Ignore directed and rpc messages
                    } // end match Message::...

                }
            }
        } // end while loop
    }
}
