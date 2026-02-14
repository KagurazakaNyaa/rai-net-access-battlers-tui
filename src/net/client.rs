use std::io;
use std::sync::mpsc::{self, Receiver, Sender};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::net::{TcpStream, UnixStream};

use crate::PlayerId;
use crate::net::protocol::{parse_rooms, parse_state, RoomInfo};

#[derive(Clone)]
pub struct ClientConfig {
    pub tcp_addr: Option<String>,
    pub unix_path: Option<String>,
    pub name: String,
    pub client_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientRole {
    Player(PlayerId),
    Spectator,
    Lobby,
}

pub enum ClientEvent {
    Assigned(ClientRole),
    State(crate::game::GameState, [String; 2]),
    Rooms(Vec<RoomInfo>),
    RoomPlayers(Vec<String>),
    RoomSpectators(Vec<String>),
    Error(String),
}

pub async fn connect_client(
    config: ClientConfig,
) -> io::Result<(Receiver<ClientEvent>, Sender<String>)> {
    if let Some(addr) = config.tcp_addr {
        let stream = TcpStream::connect(addr).await?;
        let (reader, writer) = tokio::io::split(stream);
        return connect_stream(reader, writer, config.name, config.client_id).await;
    }

    if let Some(path) = config.unix_path {
        let stream = UnixStream::connect(path).await?;
        let (reader, writer) = tokio::io::split(stream);
        return connect_stream(reader, writer, config.name, config.client_id).await;
    }

    Err(io::Error::new(io::ErrorKind::InvalidInput, "no address"))
}

async fn connect_stream<T>(
    reader: ReadHalf<T>,
    mut writer: WriteHalf<T>,
    name: String,
    client_id: String,
) -> io::Result<(Receiver<ClientEvent>, Sender<String>)>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    writer
        .write_all(format!("HELLO {} {}\n", client_id, name).as_bytes())
        .await?;
    writer.flush().await?;

    let (state_tx, state_rx) = mpsc::channel();
    let (op_tx, op_rx) = mpsc::channel::<String>();

    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        let mut buffer = Vec::new();
        let mut room_buffer = Vec::new();
        let mut in_room_list = false;
        while let Ok(Some(line)) = lines.next_line().await {
            if line.starts_with("YOU ") {
                if let Some(role) = parse_role(line.trim_start_matches("YOU ")) {
                    let _ = state_tx.send(ClientEvent::Assigned(role));
                }
                continue;
            }
            if line.starts_with("ERR ") {
                let msg = line.trim_start_matches("ERR ").to_string();
                let _ = state_tx.send(ClientEvent::Error(msg));
                continue;
            }
            if line == "ROOMS_BEGIN" {
                room_buffer.clear();
                in_room_list = true;
                continue;
            }
            if line == "ROOMS_END" {
                in_room_list = false;
                if let Some(rooms) = parse_rooms(&room_buffer) {
                    let _ = state_tx.send(ClientEvent::Rooms(rooms));
                }
                continue;
            }
            if in_room_list {
                room_buffer.push(line);
                continue;
            }
            if line.starts_with("ROOMPLAYERS ") {
                let list = line.trim_start_matches("ROOMPLAYERS ");
                let names = if list.is_empty() {
                    Vec::new()
                } else {
                    list.split(',').map(|s| s.to_string()).collect()
                };
                let _ = state_tx.send(ClientEvent::RoomPlayers(names));
                continue;
            }
            if line.starts_with("ROOMSPECTATORS ") {
                let list = line.trim_start_matches("ROOMSPECTATORS ");
                let names = if list.is_empty() {
                    Vec::new()
                } else {
                    list.split(',').map(|s| s.to_string()).collect()
                };
                let _ = state_tx.send(ClientEvent::RoomSpectators(names));
                continue;
            }
            if line == "STATE_BEGIN" {
                buffer.clear();
            } else if line == "STATE_END" {
                if let Some((state, names)) = parse_state(&buffer) {
                    let _ = state_tx.send(ClientEvent::State(state, names));
                }
            } else {
                buffer.push(line);
            }
        }
    });

    tokio::spawn(async move {
        while let Ok(line) = op_rx.recv() {
            let _ = writer.write_all(line.as_bytes()).await;
            let _ = writer.write_all(b"\n").await;
            let _ = writer.flush().await;
        }
    });

    Ok((state_rx, op_tx))
}

fn parse_role(token: &str) -> Option<ClientRole> {
    match token.trim() {
        "P1" => Some(ClientRole::Player(PlayerId::P1)),
        "P2" => Some(ClientRole::Player(PlayerId::P2)),
        "SPEC" => Some(ClientRole::Spectator),
        "LOBBY" => Some(ClientRole::Lobby),
        _ => None,
    }
}
