use super::*;

pub struct Pool {
}

impl Pool {
    pub async fn start(mut handle: MsgBusHandle<String, MarcoPoloMsg>) -> io::Result<()> {
        let mut rx = handle.register(String::from("Pool")).await.unwrap();
        handle.broadcast(MarcoPoloMsg::LifeguardYells(String::from("Hold your britches!  We'll be open in a sec..."))).await.unwrap();
        // port 7780 is "MP" in ascii decimal MP = Marco Polo
        let mut players: Vec<String> = Vec::new();

        let mut listener = TcpListener::bind("0.0.0.0:7780").await.unwrap();
        loop {  
            tokio::select!{
                Some(stream) = listener.next() => {
                    let handle = handle.clone();
                    let stream = stream.unwrap();
                    println!("Inbound swimmer!");
                    tokio::spawn(Player::start(handle,stream));
                    // tokio::task::spawn_blocking(|| async move { Player::start(handle, stream) } );
                },
                Some(msg) = rx.recv() => {
                    match msg {
                        Message::Shutdown => {
                            // simply exit the loop
                            break;
                        }
                        Message::Rpc(mp_msg, response_handle) => {
                            match mp_msg {
                                MarcoPoloMsg::LookAtPool => {      
                                    response_handle.send(MarcoPoloMsg::Players(players.clone())).unwrap();
                                },
                                _ => {}  // don't care about other possibilities
                            }
                        },
                        Message::Message(mp_msg) => {
                            match mp_msg {
                                MarcoPoloMsg::JumpIn(player_name) => {
                                    players.push(player_name);
                                    handle.broadcast(MarcoPoloMsg::PlayerJumpedIn).await.unwrap();
                                }
                                _ => {},
                            }
                        }
                        Message::Broadcast(_mp_msg ) => {}
                    }; // end match Message::...
        
                },
            }      
            
        }; // end while loop
        Ok(())
    
    }
}
