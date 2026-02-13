use std::io;
use std::sync::mpsc::{self, Receiver, Sender};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::net::{TcpStream, UnixStream};

use crate::PlayerId;
use crate::net::protocol::parse_state;

#[derive(Clone)]
pub struct ClientConfig {
    pub tcp_addr: Option<String>,
    pub unix_path: Option<String>,
    pub name: String,
}

pub enum ClientEvent {
    Assigned(PlayerId),
    State(crate::game::GameState, [String; 2]),
}

pub async fn connect_client(
    config: ClientConfig,
) -> io::Result<(Receiver<ClientEvent>, Sender<String>)> {
    if let Some(addr) = config.tcp_addr {
        let stream = TcpStream::connect(addr).await?;
        let (reader, writer) = tokio::io::split(stream);
        return connect_stream(reader, writer, config.name).await;
    }

    if let Some(path) = config.unix_path {
        let stream = UnixStream::connect(path).await?;
        let (reader, writer) = tokio::io::split(stream);
        return connect_stream(reader, writer, config.name).await;
    }

    Err(io::Error::new(io::ErrorKind::InvalidInput, "no address"))
}

async fn connect_stream<T>(
    reader: ReadHalf<T>,
    mut writer: WriteHalf<T>,
    name: String,
) -> io::Result<(Receiver<ClientEvent>, Sender<String>)>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    writer
        .write_all(format!("HELLO {}\n", name).as_bytes())
        .await?;
    writer.flush().await?;

    let (state_tx, state_rx) = mpsc::channel();
    let (op_tx, op_rx) = mpsc::channel::<String>();

    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        let mut buffer = Vec::new();
        while let Ok(Some(line)) = lines.next_line().await {
            if line.starts_with("YOU ") {
                if let Some(player) = parse_player(line.trim_start_matches("YOU ")) {
                    let _ = state_tx.send(ClientEvent::Assigned(player));
                }
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

fn parse_player(token: &str) -> Option<PlayerId> {
    match token.trim() {
        "P1" => Some(PlayerId::P1),
        "P2" => Some(PlayerId::P2),
        _ => None,
    }
}
