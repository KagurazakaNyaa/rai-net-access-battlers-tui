#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayerId {
    P1,
    P2,
}

impl PlayerId {
    pub fn opponent(self) -> PlayerId {
        match self {
            PlayerId::P1 => PlayerId::P2,
            PlayerId::P2 => PlayerId::P1,
        }
    }

    pub fn setup_row(self) -> usize {
        match self {
            PlayerId::P1 => 1,
            PlayerId::P2 => 6,
        }
    }

    pub fn setup_positions(self) -> [Position; 8] {
        match self {
            PlayerId::P1 => [
                Position::new(0, 0),
                Position::new(0, 1),
                Position::new(0, 2),
                Position::new(0, 5),
                Position::new(0, 6),
                Position::new(0, 7),
                Position::new(1, 3),
                Position::new(1, 4),
            ],
            PlayerId::P2 => [
                Position::new(7, 0),
                Position::new(7, 1),
                Position::new(7, 2),
                Position::new(7, 5),
                Position::new(7, 6),
                Position::new(7, 7),
                Position::new(6, 3),
                Position::new(6, 4),
            ],
        }
    }

    pub fn exit_row(self) -> usize {
        match self {
            PlayerId::P1 => 0,
            PlayerId::P2 => 7,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnlineCardType {
    Link,
    Virus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub row: usize,
    pub col: usize,
}

impl Position {
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }

    pub fn manhattan_distance(self, other: Position) -> usize {
        self.row.abs_diff(other.row) + self.col.abs_diff(other.col)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnlineCard {
    pub card_type: OnlineCardType,
    pub revealed: bool,
    pub line_boost_attached: bool,
    pub owner: PlayerId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackChoice {
    Link,
    Virus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePhase {
    Setup(PlayerId),
    Playing,
    GameOver(PlayerId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveOutcome {
    TurnEnds,
    CanMoveAgain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameError {
    OutOfBounds,
    NotAdjacent,
    NoCard,
    NotYourCard,
    OccupiedByOwnCard,
    OwnExitBlocked,
    OpponentFirewall,
    InvalidSetupPosition,
    SetupExhausted,
    SetupNotCurrentPlayer,
    NotInSetupPhase,
    NotInPlayingPhase,
    NotOnOpponentExit,
    FirewallOnExit,
    TerminalCardUsed,
    InvalidTarget,
    PendingBoostMove,
    NoPendingBoostMove,
    CannotEnterServerWithBoost,
}
