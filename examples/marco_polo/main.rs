
use msgbus::*;
use tokio::prelude::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufStream };
use tokio::net::{ TcpStream, TcpListener };
use tokio::stream::StreamExt;
use msgbus::MsgBusHandle;
use player::Player;
use marco::Marco;
use pool::Pool;

pub mod player;
pub mod pool;
pub mod marco;

/// Enums give the most flexibile data type for message passing 

#[derive(Debug, Clone)]
pub enum MarcoPoloMsg {
    Marco,
    Polo,          
    PlayerJumpedIn,
    PlayerClimbedOut,
    JumpIn(String),   // Yell your name as you jump in
    ClimbOut(String),         // Quitter(player_name)
    LifeguardYells(String),  // Randomness
    PlayerTaunt(String, String), // Player name and then the taunt
    Players(Vec<String>),
    LookAtPool,  // RPC call to the Pool, returns Players(vec[Player Names])
}

#[tokio::main]
async fn main() {

    let (mbus, mbushan) = MsgBus::<String, MarcoPoloMsg>::new();

    let pool_han = tokio::spawn(Pool::start(mbushan.clone()));
    let marco_han = tokio::spawn(Marco::start(mbushan.clone(), mbus));
    let _result = tokio::join!(pool_han, marco_han);
    
    
}




