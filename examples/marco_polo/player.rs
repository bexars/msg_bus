use super::*;

pub struct Player {}
impl Player {
    pub async fn start(
        mut handle: MsgBusHandle<String, MarcoPoloMsg>,
        stream: TcpStream,
    ) -> tokio::io::Result<()> {
        println!("Player in start()");
        stream.set_nodelay(true)?;
        let mut stream = BufStream::new(stream);
        // let (t_in, t_out) = stream.split();
        // let mut t_in = BufReader::new(t_in);
        // let mut t_out = BufWriter::new(t_out);
        // let t_out = BufWriter::new(stream);
        // let t_in = BufReader::new(stream);

        stream
            .write_all(b"A bored looking lifeguard sees you approaching.\n\r")
            .await?;
        stream
            .write_all(b"  \"What's your name buddy?\"\n\r")
            .await?;
        stream.flush().await?;
        let mut p_name = String::new();
        stream.read_line(&mut p_name).await?;
        let name = p_name.trim();
        //        name.clear();
        //        t_in.read_line(name).await?;
        //        t_out.flush().await?;
        stream
            .write_all(
                format!(
                    "Welcome to the pool {}.  Looks like a game of Marco Polo is being played.\n\r",
                    name
                )
                .as_bytes(),
            )
            .await?;
        stream
            .write_all(b"JUMP in and take a LOOK around.\n\r")
            .await?;
        stream.flush().await?;

        let mut rx = handle.register(String::from(name.clone())).await.unwrap();
        // let mut t_in = t_in.into_inner();
        // let mut t_in_lines = stream.lines();
        loop {
            let mut input = String::new();
            stream.flush().await.unwrap();
            tokio::select! {
                Ok(len) = stream.read_line(&mut input) => {
                //Some(Ok(input)) = t_in_lines.next()  => {
                    println!("Echo: {}", input);
                    if len == 0 { break; };  // EOF from the client
                    let input = input.trim();
                    stream.flush().await.unwrap();
                    match &*input {
                        "" => {
                            handle.send("Pool".to_string(), MarcoPoloMsg::Polo).await.unwrap();
                        },
                        "jump" => {
                            handle.send("Pool".to_string(), MarcoPoloMsg::JumpIn(name.to_string())).await.unwrap();
                        },
                        "look" => {
                            let players = if let MarcoPoloMsg::Players(players) = handle.rpc("Pool".to_string(),MarcoPoloMsg::LookAtPool).await.unwrap() {
                                players
                            } else { continue; };

                            stream.write_all(b"Players in the pool\r\n").await.unwrap();
                            stream.write_all(b"-------------------\r\n").await.unwrap();
                            for p in players {
                                stream.write_all(p.as_bytes()).await.unwrap();
                                stream.write_all(b"\r\n").await.unwrap();
                            }
                            // stream.flush();
                            // tokio::task::yield_now().await;
                        }
                        _ => {},
                    }
                    // if ch == 0 { continue; };
                    // let cmd = String::from_utf8(input).unwrap();
                    // Any input is a Polo call
                    // println!("Yelling \"Polo!\"");
                    // let input = input.trim();
                    // println!("a{}", ch);
                    // t_out.write_all(b"Polo!\n\r").await?;
                    // t_out.flush().await?;
                    // handle.broadcast(MarcoPoloMsg::Polo).await.unwrap();
                },
                Some(msg) = rx.recv() => {
                    match msg {
                        Message::Shutdown => {
                            // simply exit the loop
                            break;
                        }
                        Message::Rpc(_mp_msg, _response_handle) => {},
                        Message::Message(_mp_msg) => {}
                        Message::Broadcast(mp_msg ) => {
                            match mp_msg {
                                MarcoPoloMsg::Marco => {
                                    stream.write_all(b"A loud \"Marco!!\" resonates across the pool.\n\r").await?;
                                    stream.flush().await?;
                                }
                                MarcoPoloMsg::Polo => {
                                    stream.write_all(b"Anotther player yells \"POLO\"\n\r").await?;
                                    stream.flush().await?;
                                }
                                MarcoPoloMsg::PlayerJumpedIn => {
                                    stream.write_all(b"Someone else jumped in, should LOOK around and see who...\n\r").await?;
                                    stream.flush().await?;
                                }
                                MarcoPoloMsg::PlayerClimbedOut => {
                                    stream.write_all(b"Sounds like someone just climbed out of the pool...  Some people can't handle it.\n\r").await?;
                                    stream.flush().await?;
                                }
                                MarcoPoloMsg::LifeguardYells(shout) => {
                                    stream.write_all(format!("Lifeguard just yelled \"{}\".\n\r", shout).as_bytes()).await?;
                                    stream.flush().await?;
                                }
                                _ => {}
                            };
                        }
                    } // end match Message::...

                }
            }
        } // end while loop
        Ok(())
    }
}
