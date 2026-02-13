use std::io::{self, Stdout};
use std::time::Duration;

use clap::{Parser, Subcommand, ValueEnum};
use crossterm::event::{self, Event};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{execute, terminal};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use rai_net_access_battlers_tui::net::client::{connect_client, ClientConfig, ClientEvent};
use rai_net_access_battlers_tui::net::server::{run_server, ListenMode, ServerConfig};
use rai_net_access_battlers_tui::ui::{draw, handle_key, handle_mouse, UiMode, UiState};
use rai_net_access_battlers_tui::{GamePhase, GameState, PlayerId};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Server {
        #[arg(long, default_value = "0.0.0.0:2321")]
        tcp: String,
        #[arg(long, default_value = "/tmp/rainet.sock")]
        unix: String,
        #[arg(long)]
        log: Option<String>,
        #[arg(long, value_enum, default_value = "both")]
        mode: ListenModeArg,
    },
    Client {
        #[arg(long)]
        tcp: Option<String>,
        #[arg(long)]
        unix: Option<String>,
        #[arg(long)]
        name: String,
    },
}

#[derive(ValueEnum, Clone, Debug)]
enum ListenModeArg {
    #[value(name = "tcp")]
    Tcp,
    #[value(name = "unix")]
    Unix,
    #[value(name = "both")]
    Both,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Server {
            tcp,
            unix,
            log,
            mode,
        } => {
            let listen_mode = match mode {
                ListenModeArg::Tcp => ListenMode::TcpOnly,
                ListenModeArg::Unix => ListenMode::UnixOnly,
                ListenModeArg::Both => ListenMode::Both,
            };
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            runtime.block_on(run_server(ServerConfig {
                tcp_addr: tcp,
                unix_path: unix,
                log_path: log,
                listen_mode,
            }))
        }
        Command::Client { tcp, unix, name } => run_client(tcp, unix, name),
    }
}

fn run_client(tcp: Option<String>, unix: Option<String>, name: String) -> io::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let (state_rx, op_tx) = runtime.block_on(connect_client(ClientConfig {
        tcp_addr: tcp.or_else(|| Some("127.0.0.1:2321".to_string())),
        unix_path: unix,
        name,
    }))?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    terminal::enable_raw_mode()?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, state_rx, op_tx);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state_rx: std::sync::mpsc::Receiver<ClientEvent>,
    op_tx: std::sync::mpsc::Sender<String>,
) -> io::Result<()> {
    let mut game = GameState::new();
    let mut ui = UiState {
        cursor: PlayerId::P1.setup_positions()[0],
        mode: UiMode::TurnPass,
        message: "Waiting for server".to_string(),
        log: Vec::new(),
        menu: None,
        player_names: ["P1".to_string(), "P2".to_string()],
        local_player: PlayerId::P1,
        op_sender: Some(op_tx),
    };

    loop {
        while let Ok(event) = state_rx.try_recv() {
            match event {
                ClientEvent::Assigned(player) => {
                    ui.local_player = player;
                }
                ClientEvent::State(state, names) => {
                    game = state;
                    ui.player_names = names;
                    ui.mode = match game.phase {
                        GamePhase::Setup(player) => {
                            if player == ui.local_player {
                                ui.cursor = player.setup_positions()[0];
                                UiMode::Setup
                            } else {
                                UiMode::TurnPass
                            }
                        }
                        GamePhase::Playing => {
                            if game.current_player == ui.local_player {
                                UiMode::MoveSelect
                            } else {
                                UiMode::TurnPass
                            }
                        }
                        GamePhase::GameOver(_) => UiMode::GameOver,
                    };
                }
            }
        }

        terminal.draw(|frame| draw(frame, &game, &ui))?;

        if event::poll(Duration::from_millis(80))? {
            match event::read()? {
                Event::Key(key) => {
                    if handle_key(key, &mut game, &mut ui)? {
                        return Ok(());
                    }
                }
                Event::Mouse(mouse) => {
                    let area = terminal.size()?.into();
                    if handle_mouse(mouse, area, &mut game, &mut ui)? {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }
    }
}
