use crate::{PlayerId, Position};
use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
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

pub struct UiState {
    pub cursor: Position,
    pub mode: UiMode,
    pub message: String,
    pub log: Vec<String>,
    pub menu: Option<ActionMenu>,
    pub player_names: [String; 2],
    pub local_player: PlayerId,
    pub op_sender: Option<std::sync::mpsc::Sender<String>>,
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
