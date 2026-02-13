use std::collections::HashMap;
use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::sync::Mutex;
use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};

use crate::{GameError, GamePhase, GameState, PlayerId, Position};
use crate::net::protocol::{encode_state, parse_op, Op};

#[derive(Clone, Copy, Debug)]
pub enum ListenMode {
    TcpOnly,
    UnixOnly,
    Both,
}

#[derive(Clone)]
pub struct ServerConfig {
    pub tcp_addr: String,
    pub unix_path: String,
    pub log_path: Option<String>,
    pub listen_mode: ListenMode,
}

#[derive(Clone)]
struct ClientHandle {
    writer: Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send + 'static>>>,
}

struct ServerState {
    game: GameState,
    names: [String; 2],
    clients: HashMap<PlayerId, ClientHandle>,
    logger: Logger,
}

#[derive(Clone)]
struct Logger {
    sink: Arc<Mutex<LogSink>>,
}

enum LogSink {
    Stdout,
    File(std::fs::File),
}

pub async fn run_server(config: ServerConfig) -> io::Result<()> {
    let logger = Logger::new(config.log_path.as_deref()).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "Failed to open log file {}: {}",
                config.log_path.as_deref().unwrap_or("(stdout)"),
                err
            ),
        )
    })?;
    let state = Arc::new(Mutex::new(ServerState {
        game: GameState::new(),
        names: ["P1".to_string(), "P2".to_string()],
        clients: HashMap::new(),
        logger,
    }));

    if Path::new(&config.unix_path).exists() {
        let _ = std::fs::remove_file(&config.unix_path);
    }

    let tcp_listener_opt = match config.listen_mode {
        ListenMode::TcpOnly | ListenMode::Both => {
            let listener = TcpListener::bind(&config.tcp_addr).await.map_err(|err| {
                io::Error::new(
                    err.kind(),
                    format!("Failed to bind TCP listener at {}: {}", config.tcp_addr, err),
                )
            })?;
            Some(listener)
        }
        ListenMode::UnixOnly => None,
    };

    let unix_listener_opt = match config.listen_mode {
        ListenMode::UnixOnly | ListenMode::Both => {
            let listener = UnixListener::bind(&config.unix_path).map_err(|err| {
                io::Error::new(
                    err.kind(),
                    format!("Failed to bind Unix socket at {}: {}", config.unix_path, err),
                )
            })?;
            Some(listener)
        }
        ListenMode::TcpOnly => None,
    };

    {
        let server = state.lock().await;
        let mode_str = match config.listen_mode {
            ListenMode::TcpOnly => format!("tcp={}", config.tcp_addr),
            ListenMode::UnixOnly => format!("unix={}", config.unix_path),
            ListenMode::Both => format!("tcp={} unix={}", config.tcp_addr, config.unix_path),
        };
        server.logger.log(&format!("server_listen {}", mode_str)).await;
    }

    if let Some(tcp_listener) = tcp_listener_opt {
        let state_tcp = state.clone();
        tokio::spawn(async move {
            loop {
                if let Ok((stream, addr)) = tcp_listener.accept().await {
                    {
                        let server = state_tcp.lock().await;
                        server
                            .logger
                            .log(&format!("accept tcp {}", addr))
                            .await;
                    }
                    let _ = handle_tcp_client(stream, state_tcp.clone()).await;
                }
            }
        });
    }

    if let Some(unix_listener) = unix_listener_opt {
        loop {
            let (stream, _) = unix_listener.accept().await?;
            let state_unix = state.clone();
            tokio::spawn(async move {
                {
                    let server = state_unix.lock().await;
                    server.logger.log("accept unix").await;
                }
                let _ = handle_unix_client(stream, state_unix).await;
            });
        }
    } else {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3600));
        }
    }
}

async fn handle_tcp_client(stream: TcpStream, state: Arc<Mutex<ServerState>>) -> io::Result<()> {
    let (reader, writer) = tokio::io::split(stream);
    handle_client(reader, writer, state).await
}

async fn handle_unix_client(stream: UnixStream, state: Arc<Mutex<ServerState>>) -> io::Result<()> {
    let (reader, writer) = tokio::io::split(stream);
    handle_client(reader, writer, state).await
}

async fn handle_client<T>(
    reader: ReadHalf<T>,
    writer: WriteHalf<T>,
    state: Arc<Mutex<ServerState>>,
) -> io::Result<()>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let mut inbound = BufReader::new(reader).lines();
    let writer: Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send + 'static>>> =
        Arc::new(Mutex::new(Box::new(writer)));

    let mut assigned = None;
    while let Some(line) = inbound.next_line().await? {
        if line.starts_with("HELLO ") {
            let name = line.trim_start_matches("HELLO ").trim().to_string();
            let id = assign_player(&state, name.clone(), writer.clone()).await?;
            assigned = Some(id);
            {
                let mut writer = writer.lock().await;
                writer
                    .write_all(format!("YOU {}\n", if id == PlayerId::P1 { "P1" } else { "P2" }).as_bytes())
                    .await?;
                writer.flush().await?;
            }
            {
                let server = state.lock().await;
                server
                    .logger
                    .log(&format!("hello {} -> {:?}", name, id))
                    .await;
            }
            send_state(&state).await?;
            continue;
        }

        if let Some(op) = parse_op(&line) {
            let player = match assigned {
                Some(player) => player,
                None => continue,
            };
            apply_op(&state, player, op).await?;
        }
    }

    Ok(())
}

async fn assign_player(
    state: &Arc<Mutex<ServerState>>,
    name: String,
    writer: Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send + 'static>>>,
) -> io::Result<PlayerId> {
    let mut server = state.lock().await;
    let id = if !server.clients.contains_key(&PlayerId::P1) {
        PlayerId::P1
    } else {
        PlayerId::P2
    };
    if id == PlayerId::P1 {
        server.names[0] = name.clone();
    } else {
        server.names[1] = name.clone();
    }
    server.clients.insert(
        id,
        ClientHandle {
            writer,
        },
    );
    Ok(id)
}

async fn apply_op(
    state: &Arc<Mutex<ServerState>>,
    player: PlayerId,
    op: Op,
) -> io::Result<()> {
    let (payload, writers) = {
        let server = state.lock().await;
        let logger = server.logger.clone();
        drop(server);
        logger.log(&format!("op {:?} by {:?}", op, player)).await;
        let mut server = state.lock().await;
        let game = &mut server.game;
        if player != game.current_player && !matches!(game.phase, GamePhase::Setup(_)) {
            return Ok(());
        }

        let mut apply_result: Result<(), GameError> = Ok(());
        match op {
            Op::Setup { card, row, col } => {
                apply_result = game.place_setup_card(player, Position::new(row, col), card);
            }
            Op::Remove { row, col } => {
                apply_result = game.remove_setup_card(player, Position::new(row, col));
            }
            Op::Move { from, to } => {
                apply_result = match game.start_move(from, to) {
                    Ok(outcome) => {
                        if matches!(outcome, crate::MoveOutcome::TurnEnds) {
                            game.end_turn();
                        }
                        Ok(())
                    }
                    Err(err) => Err(err),
                };
            }
            Op::Boost { from, to } => {
                apply_result = match game.continue_boost_move(from, to) {
                    Ok(_) => {
                        game.end_turn();
                        Ok(())
                    }
                    Err(err) => Err(err),
                };
            }
            Op::Enter { from, reveal, stack } => {
                apply_result = game.enter_server_center(from, reveal, stack);
                if apply_result.is_ok() {
                    game.end_turn();
                }
            }
            Op::LineBoostAttach { pos } => {
                let index = game
                    .player(player)
                    .line_boosts
                    .iter()
                    .position(|slot| slot.is_none())
                    .unwrap_or(0);
                apply_result = game.use_line_boost_attach(index, pos);
                if apply_result.is_ok() {
                    game.end_turn();
                }
            }
            Op::LineBoostDetach { pos } => {
                let index = game
                    .player(player)
                    .line_boosts
                    .iter()
                    .position(|slot| *slot == Some(pos))
                    .unwrap_or(0);
                apply_result = game.use_line_boost_detach(index);
                if apply_result.is_ok() {
                    game.end_turn();
                }
            }
            Op::VirusCheck { pos } => {
                let index = game
                    .player(player)
                    .virus_checks_used
                    .iter()
                    .position(|used| !used)
                    .unwrap_or(0);
                apply_result = game.use_virus_check(index, pos);
                if apply_result.is_ok() {
                    game.end_turn();
                }
            }
            Op::FirewallPlace { pos } => {
                let index = game
                    .player(player)
                    .firewalls
                    .iter()
                    .position(|slot| slot.is_none())
                    .unwrap_or(0);
                apply_result = game.use_firewall_place(index, pos);
                if apply_result.is_ok() {
                    game.end_turn();
                }
            }
            Op::FirewallRemove { pos } => {
                let index = game
                    .player(player)
                    .firewalls
                    .iter()
                    .position(|slot| *slot == Some(pos))
                    .unwrap_or(0);
                apply_result = game.use_firewall_remove(index);
                if apply_result.is_ok() {
                    game.end_turn();
                }
            }
            Op::NotFound {
                first,
                second,
                swap,
            } => {
                let index = game
                    .player(player)
                    .not_found_used
                    .iter()
                    .position(|used| !used)
                    .unwrap_or(0);
                apply_result = game.use_404(index, first, second, swap);
                if apply_result.is_ok() {
                    game.end_turn();
                }
            }
            Op::EndTurn => game.end_turn(),
        }

        if apply_result.is_ok() {
            let payload = encode_state(&server.game, &server.names);
            let writers = server
                .clients
                .values()
                .map(|client| client.writer.clone())
                .collect::<Vec<_>>();
            (Some(payload), writers)
        } else {
            (None, Vec::new())
        }
    };

    if let Some(payload) = payload {
        for writer in writers {
            let mut writer_guard = writer.lock().await;
            writer_guard.write_all(payload.as_bytes()).await?;
            writer_guard.flush().await?;
        }
    }

    Ok(())
}

impl Logger {
    fn new(path: Option<&str>) -> io::Result<Self> {
        let sink = match path {
            Some(path) => {
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)?;
                LogSink::File(file)
            }
            None => LogSink::Stdout,
        };
        Ok(Self {
            sink: Arc::new(Mutex::new(sink)),
        })
    }

    async fn log(&self, message: &str) {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let line = format!("{} {}\n", ts, message);
        let mut sink = self.sink.lock().await;
        match &mut *sink {
            LogSink::Stdout => {
                let _ = io::stdout().write_all(line.as_bytes());
            }
            LogSink::File(file) => {
                let _ = file.write_all(line.as_bytes());
            }
        }
    }
}

async fn send_state(state: &Arc<Mutex<ServerState>>) -> io::Result<()> {
    let (payload, writers) = {
        let server = state.lock().await;
        let payload = encode_state(&server.game, &server.names);
        let writers = server
            .clients
            .values()
            .map(|client| client.writer.clone())
            .collect::<Vec<_>>();
        (payload, writers)
    };
    for writer in writers {
        let mut writer_guard = writer.lock().await;
        writer_guard.write_all(payload.as_bytes()).await?;
        writer_guard.flush().await?;
    }
    Ok(())
}
