use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::sync::Mutex;
use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};
use tracing::{info, warn};

use crate::{GameError, GamePhase, GameState, PlayerId, Position};
use crate::net::protocol::{encode_rooms, encode_state, parse_op, Op, RoomInfo, RoomStatus};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClientRole {
    Player(PlayerId),
    Spectator,
    Pending,
}

#[derive(Clone)]
struct ClientHandle {
    writer: Option<Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send + 'static>>>>,
    name: String,
    room_id: Option<String>,
    role: ClientRole,
}

#[derive(Clone)]
struct RoomState {
    id: String,
    name: String,
    auto_join: bool,
    show_id: bool,
    game: GameState,
    names: [String; 2],
    players: HashMap<PlayerId, String>,
    spectators: HashSet<String>,
}

struct ServerState {
    rooms: HashMap<String, RoomState>,
    clients: HashMap<String, ClientHandle>,
}

pub async fn run_server(config: ServerConfig) -> io::Result<()> {
    init_logging(config.log_path.as_deref());
    let state = Arc::new(Mutex::new(ServerState {
        rooms: HashMap::new(),
        clients: HashMap::new(),
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
        let _server = state.lock().await;
        let mode_str = match config.listen_mode {
            ListenMode::TcpOnly => format!("tcp={}", config.tcp_addr),
            ListenMode::UnixOnly => format!("unix={}", config.unix_path),
            ListenMode::Both => format!("tcp={} unix={}", config.tcp_addr, config.unix_path),
        };
        info!("server_listen {}", mode_str);
    }

    if let Some(tcp_listener) = tcp_listener_opt {
        let state_tcp = state.clone();
        tokio::spawn(async move {
            loop {
                if let Ok((stream, addr)) = tcp_listener.accept().await {
                    info!("accept tcp {}", addr);
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
                info!("accept unix");
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

    let mut client_id: Option<String> = None;
    while let Some(line) = inbound.next_line().await? {
        if line.starts_with("HELLO ") {
            let payload = line.trim_start_matches("HELLO ").trim();
            let mut parts = payload.splitn(2, ' ');
            let client_key = parts.next().unwrap_or("").to_string();
            let name = parts.next().unwrap_or("").to_string();
            if client_key.is_empty() {
                continue;
            }
            let id = register_client(&state, client_key, name.clone(), writer.clone()).await?;
            client_id = Some(id.clone());
            {
                let mut writer = writer.lock().await;
                writer.write_all(format!("YOU LOBBY {}\n", id).as_bytes()).await?;
                writer.flush().await?;
            }
            info!("hello {} -> {}", name, id);
            send_rooms_to(&state, id).await?;
            continue;
        }

        if let Some(op) = parse_op(&line) {
            let id = match &client_id {
                Some(id) => id.clone(),
                None => continue,
            };
            apply_op(&state, id, op).await?;
        }
    }

    if let Some(id) = client_id {
        handle_disconnect(&state, id).await?;
    }

    Ok(())
}

async fn register_client(
    state: &Arc<Mutex<ServerState>>,
    client_key: String,
    name: String,
    writer: Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send + 'static>>>,
) -> io::Result<String> {
    let mut server = state.lock().await;
    let entry = server.clients.entry(client_key.clone());
    use std::collections::hash_map::Entry;
    match entry {
        Entry::Occupied(mut existing) => {
            let handle = existing.get_mut();
            handle.writer = Some(writer);
            if !name.is_empty() {
                handle.name = name;
            }
        }
        Entry::Vacant(vacant) => {
            vacant.insert(ClientHandle {
                writer: Some(writer),
                name,
                room_id: None,
                role: ClientRole::Pending,
            });
        }
    }
    Ok(client_key)
}

async fn apply_op(
    state: &Arc<Mutex<ServerState>>,
    client_id: String,
    op: Op,
) -> io::Result<()> {
    info!("op {:?} by {}", op, client_id);
    let mut payload = None;
    let mut writers: Vec<Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send + 'static>>>> = Vec::new();
    let mut err_msg: Option<String> = None;
    let mut err_writer: Option<Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send + 'static>>>> = None;

    match op {
        Op::RoomList => {
            send_rooms_to(state, client_id.clone()).await?;
        }
        Op::RoomCreate {
            name,
            id,
            auto_join,
            show_id,
        } => {
            let room_id = create_room(state, name, id, auto_join, show_id).await;
            if let Err(err) = join_room_as_player(state, client_id.clone(), &room_id).await {
                err_msg = Some(format!("JOIN_FAILED {}", err));
            }
            send_rooms_to_all(state).await?;
        }
        Op::RoomJoin { id } => {
            if let Err(err) = join_room_as_player(state, client_id.clone(), &id).await {
                err_msg = Some(format!("JOIN_FAILED {}", err));
            }
            send_rooms_to_all(state).await?;
        }
        Op::RoomJoinAsSpectator { id } => {
            if let Err(err) = join_room_as_spectator(state, client_id.clone(), &id).await {
                err_msg = Some(format!("SPECTATE_FAILED {}", err));
            }
            send_rooms_to_all(state).await?;
        }
        Op::RoomAutoJoin => {
            let room_id = auto_join_room(state).await;
            if let Err(err) = join_room_as_player(state, client_id.clone(), &room_id).await {
                err_msg = Some(format!("AUTO_JOIN_FAILED {}", err));
            }
            send_rooms_to_all(state).await?;
        }
        Op::RoomLeave => {
            leave_room(state, client_id.clone()).await?;
            send_rooms_to_all(state).await?;
        }
        _ => {
            let (room_id, role, writer) = {
                let server = state.lock().await;
                let client = match server.clients.get(&client_id) {
                    Some(client) => client,
                    None => return Ok(()),
                };
                (client.room_id.clone(), client.role, client.writer.clone())
            };
            let room_id = match room_id {
                Some(id) => id,
                None => {
                    err_msg = Some("NOT_IN_ROOM".to_string());
                    err_writer = writer;
                    return send_error_if_needed(err_msg, err_writer).await;
                }
            };
            let player = match role {
                ClientRole::Player(id) => id,
                _ => {
                    err_msg = Some("NOT_A_PLAYER".to_string());
                    err_writer = writer;
                    return send_error_if_needed(err_msg, err_writer).await;
                }
            };

            let mut server = state.lock().await;
            let room = match server.rooms.get_mut(&room_id) {
                Some(room) => room,
                None => {
                    err_msg = Some("ROOM_NOT_FOUND".to_string());
                    err_writer = writer;
                    return send_error_if_needed(err_msg, err_writer).await;
                }
            };
            if room.players.len() < 2 {
                err_msg = Some("ROOM_NOT_READY".to_string());
                err_writer = writer;
                return send_error_if_needed(err_msg, err_writer).await;
            }
            let game = &mut room.game;
            if player != game.current_player {
                err_msg = Some("NOT_YOUR_TURN".to_string());
                err_writer = writer;
                return send_error_if_needed(err_msg, err_writer).await;
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
                _ => {}
            }

            if apply_result.is_ok() {
                let payload_local = encode_state(&room.game, &room.names);
                let room_clients = room
                    .players
                    .values()
                    .cloned()
                    .chain(room.spectators.iter().cloned())
                    .collect::<Vec<_>>();
                for id in room_clients {
                    if let Some(client) = server.clients.get(&id) {
                        if let Some(writer) = &client.writer {
                            writers.push(writer.clone());
                        }
                    }
                }
                payload = Some(payload_local);
            } else {
                err_msg = Some(format!(
                    "INVALID_OP {}",
                    game_error_code(apply_result.err().unwrap())
                ));
                err_writer = writer;
            }
        }
    }

    if let Some(payload) = payload {
        for writer in writers {
            let mut writer_guard = writer.lock().await;
            writer_guard.write_all(payload.as_bytes()).await?;
            writer_guard.flush().await?;
        }
    }

    send_error_if_needed(err_msg, err_writer).await
}

async fn send_error_if_needed(
    err_msg: Option<String>,
    writer: Option<Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send + 'static>>>>,
) -> io::Result<()> {
    if let (Some(msg), Some(writer)) = (err_msg, writer) {
        let mut writer_guard = writer.lock().await;
        writer_guard.write_all(format!("ERR {}\n", msg).as_bytes()).await?;
        writer_guard.flush().await?;
    }
    Ok(())
}

fn game_error_code(err: GameError) -> &'static str {
    match err {
        GameError::OutOfBounds => "OUT_OF_BOUNDS",
        GameError::NotAdjacent => "NOT_ADJACENT",
        GameError::NoCard => "NO_CARD",
        GameError::NotYourCard => "NOT_YOUR_CARD",
        GameError::OccupiedByOwnCard => "OCCUPIED_BY_OWN_CARD",
        GameError::OwnExitBlocked => "OWN_EXIT_BLOCKED",
        GameError::OpponentFirewall => "OPPONENT_FIREWALL",
        GameError::InvalidSetupPosition => "INVALID_SETUP_POSITION",
        GameError::SetupExhausted => "SETUP_EXHAUSTED",
        GameError::SetupNotCurrentPlayer => "SETUP_NOT_CURRENT_PLAYER",
        GameError::NotInSetupPhase => "NOT_IN_SETUP_PHASE",
        GameError::NotInPlayingPhase => "NOT_IN_PLAYING_PHASE",
        GameError::NotOnOpponentExit => "NOT_ON_OPPONENT_EXIT",
        GameError::FirewallOnExit => "FIREWALL_ON_EXIT",
        GameError::TerminalCardUsed => "TERMINAL_CARD_USED",
        GameError::InvalidTarget => "INVALID_TARGET",
        GameError::PendingBoostMove => "PENDING_BOOST_MOVE",
        GameError::NoPendingBoostMove => "NO_PENDING_BOOST_MOVE",
        GameError::CannotEnterServerWithBoost => "CANNOT_ENTER_SERVER_WITH_BOOST",
    }
}

fn init_logging(log_path: Option<&str>) {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    if let Some(path) = log_path {
        match std::fs::OpenOptions::new().create(true).append(true).open(path) {
            Ok(file) => {
                let writer = std::sync::Mutex::new(file);
                tracing_subscriber::fmt()
                    .with_env_filter(env_filter)
                    .with_target(false)
                    .with_writer(writer)
                    .init();
            }
            Err(_) => {
                warn!("Failed to open log file {}, using stdout", path);
                tracing_subscriber::fmt()
                    .with_env_filter(env_filter)
                    .with_target(false)
                    .init();
            }
        }
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(false)
            .init();
    }
}

async fn send_rooms_to(state: &Arc<Mutex<ServerState>>, client_id: String) -> io::Result<()> {
    let (payload, writer) = {
        let server = state.lock().await;
        let rooms = rooms_snapshot(&server);
        let payload = format!("ROOMS_BEGIN\n{}ROOMS_END\n", encode_rooms(&rooms));
        let writer = server.clients.get(&client_id).and_then(|client| client.writer.clone());
        (payload, writer)
    };
    if let Some(writer) = writer {
        let mut writer_guard = writer.lock().await;
        writer_guard.write_all(payload.as_bytes()).await?;
        writer_guard.flush().await?;
    }
    Ok(())
}

async fn send_rooms_to_all(state: &Arc<Mutex<ServerState>>) -> io::Result<()> {
    let (payload, writers) = {
        let server = state.lock().await;
        let rooms = rooms_snapshot(&server);
        let payload = format!("ROOMS_BEGIN\n{}ROOMS_END\n", encode_rooms(&rooms));
        let writers = server
            .clients
            .values()
            .filter_map(|client| client.writer.clone())
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

fn rooms_snapshot(server: &ServerState) -> Vec<RoomInfo> {
    server
        .rooms
        .values()
        .map(|room| RoomInfo {
            id: if room.show_id {
                Some(room.id.clone())
            } else {
                None
            },
            name: room.name.clone(),
            player_count: room.players.len(),
            spectator_count: room.spectators.len(),
            auto_join: room.auto_join,
            status: if matches!(room.game.phase, GamePhase::Playing) {
                RoomStatus::Playing
            } else {
                RoomStatus::Waiting
            },
        })
        .collect()
}

async fn create_room(
    state: &Arc<Mutex<ServerState>>,
    name: String,
    id: Option<String>,
    auto_join: bool,
    show_id: bool,
) -> String {
    let mut server = state.lock().await;
    let id = id.unwrap_or_else(|| format!("room-{}", server.rooms.len() + 1));
    server.rooms.insert(
        id.clone(),
        RoomState {
            id: id.clone(),
            name,
            auto_join,
            show_id,
            game: GameState::new(),
            names: ["P1".to_string(), "P2".to_string()],
            players: HashMap::new(),
            spectators: HashSet::new(),
        },
    );
    id
}

async fn auto_join_room(state: &Arc<Mutex<ServerState>>) -> String {
    let room_id = {
        let server = state.lock().await;
        server
            .rooms
            .values()
            .find(|room| room.auto_join && room.players.len() < 2)
            .map(|room| room.id.clone())
    };
    match room_id {
        Some(id) => id,
        None => create_room(state, "AutoRoom".to_string(), None, true, true).await,
    }
}

async fn join_room_as_player(
    state: &Arc<Mutex<ServerState>>,
    client_id: String,
    room_id: &str,
) -> io::Result<()> {
    leave_room(state, client_id.clone()).await?;
    let (player_id, player_name, writer) = {
        let mut server = state.lock().await;
        let room = match server.rooms.get_mut(room_id) {
            Some(room) => room,
            None => return Ok(()),
        };
        if room.players.len() >= 2 {
            return Ok(());
        }
        let player_id = if !room.players.contains_key(&PlayerId::P1) {
            PlayerId::P1
        } else {
            PlayerId::P2
        };
        room.players.insert(player_id, client_id.clone());
        let client = match server.clients.get_mut(&client_id) {
            Some(client) => client,
            None => return Ok(()),
        };
        client.room_id = Some(room_id.to_string());
        client.role = ClientRole::Player(player_id);
        (player_id, client.name.clone(), client.writer.clone())
    };

    {
        let mut server = state.lock().await;
        if let Some(room) = server.rooms.get_mut(room_id) {
            if player_id == PlayerId::P1 {
                room.names[0] = player_name;
            } else {
                room.names[1] = player_name;
            }
        }
    }

    if let Some(writer) = writer {
        let mut writer = writer.lock().await;
        writer
            .write_all(format!("YOU {}\n", if player_id == PlayerId::P1 { "P1" } else { "P2" }).as_bytes())
            .await?;
        writer.flush().await?;
    }
    let should_start = {
        let server = state.lock().await;
        server
            .rooms
            .get(room_id)
            .map(|room| room.players.len() == 2)
            .unwrap_or(false)
    };
    if should_start {
        send_state_for_room(state, room_id).await?;
    }
    Ok(())
}

async fn join_room_as_spectator(
    state: &Arc<Mutex<ServerState>>,
    client_id: String,
    room_id: &str,
) -> io::Result<()> {
    leave_room(state, client_id.clone()).await?;
    let mut server = state.lock().await;
    let room = match server.rooms.get_mut(room_id) {
        Some(room) => room,
        None => return Ok(()),
    };
    room.spectators.insert(client_id.clone());
    if let Some(client) = server.clients.get_mut(&client_id) {
        client.room_id = Some(room_id.to_string());
        client.role = ClientRole::Spectator;
        if let Some(writer) = &client.writer {
            let mut writer = writer.lock().await;
            writer.write_all(b"YOU SPEC\n").await?;
            writer.flush().await?;
        }
    }
    let should_send = {
        let server = state.lock().await;
        server
            .rooms
            .get(room_id)
            .map(|room| room.players.len() == 2)
            .unwrap_or(false)
    };
    if should_send {
        send_state_for_room(state, room_id).await?;
    }
    Ok(())
}

async fn leave_room(state: &Arc<Mutex<ServerState>>, client_id: String) -> io::Result<()> {
    let mut server = state.lock().await;
    let room_id = match server.clients.get(&client_id).and_then(|c| c.room_id.clone()) {
        Some(id) => id,
        None => return Ok(()),
    };
    if let Some(room) = server.rooms.get_mut(&room_id) {
        room.players.retain(|_, id| *id != client_id);
        room.spectators.remove(&client_id);
        if room.players.is_empty() {
            room.game = GameState::new();
            room.names = ["P1".to_string(), "P2".to_string()];
        }
    }
    if let Some(client) = server.clients.get_mut(&client_id) {
        client.room_id = None;
        client.role = ClientRole::Pending;
    }
    Ok(())
}

async fn send_state_for_room(state: &Arc<Mutex<ServerState>>, room_id: &str) -> io::Result<()> {
    let (payload, writers) = {
        let server = state.lock().await;
        let room = match server.rooms.get(room_id) {
            Some(room) => room,
            None => return Ok(()),
        };
        let mut payload = encode_state(&room.game, &room.names);
        let player_names = room
            .players
            .values()
            .filter_map(|id| server.clients.get(id).map(|c| c.name.clone()))
            .collect::<Vec<_>>();
        let spectator_names = room
            .spectators
            .iter()
            .filter_map(|id| server.clients.get(id).map(|c| c.name.clone()))
            .collect::<Vec<_>>();
        payload.push_str(&format!(
            "ROOMPLAYERS {}\n",
            player_names.join(",")
        ));
        payload.push_str(&format!(
            "ROOMSPECTATORS {}\n",
            spectator_names.join(",")
        ));
        let writers = room
            .players
            .values()
            .cloned()
            .chain(room.spectators.iter().cloned())
            .filter_map(|id| server.clients.get(&id).and_then(|c| c.writer.clone()))
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

async fn handle_disconnect(state: &Arc<Mutex<ServerState>>, client_id: String) -> io::Result<()> {
    let mut server = state.lock().await;
    if let Some(client) = server.clients.get_mut(&client_id) {
        client.writer = None;
    }
    Ok(())
}
