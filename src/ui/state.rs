use crate::{PlayerId, Position};
use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    Lobby,
    JoinRoomInput,
    RoomConfirm { action: RoomConfirmAction },
    RoomCreateDialog,
    TurnPass,
    Setup,
    MoveSelect,
    MoveDest { from: Position },
    BoostContinue { from: Position },
    TerminalMenu,
    LineBoost,
    VirusCheck,
    Firewall,
    NotFoundFirst,
    NotFoundSecond { first: Position },
    NotFoundSwap { first: Position, second: Position },
    ServerReveal { from: Position },
    ServerStack { from: Position, reveal: bool },
    GameOver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomConfirmAction {
    JoinSelected,
    Create,
    Leave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateFocus {
    Name,
    Id,
    AutoJoin,
    ShowId,
    Confirm,
    Cancel,
}

pub struct UiState {
    pub cursor: Position,
    pub mode: UiMode,
    pub message: String,
    pub log: Vec<String>,
    pub menu: Option<ActionMenu>,
    pub player_names: [String; 2],
    pub local_player: PlayerId,
    pub op_sender: Option<std::sync::mpsc::Sender<String>>,
    pub rooms: Vec<crate::net::protocol::RoomInfo>,
    pub selected_room: usize,
    pub room_input: String,
    pub auto_join: bool,
    pub show_room_id: bool,
    pub room_id_input: String,
    pub is_spectator: bool,
    pub client_id: String,
    pub room_players: Vec<String>,
    pub room_spectators: Vec<String>,
    pub i18n: crate::i18n::I18n,
    pub confirm_message: String,
    pub create_focus: CreateFocus,
}

pub struct ActionMenu {
    pub rect: Rect,
    pub items: Vec<MenuItem>,
}

pub struct MenuItem {
    pub label: String,
    pub action: MenuAction,
}

#[derive(Clone, Copy)]
pub enum MenuAction {
    Key(crossterm::event::KeyCode),
}
