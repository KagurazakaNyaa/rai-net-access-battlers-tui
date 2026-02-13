use crate::game::types::{OnlineCard, PlayerId, Position};

#[derive(Debug, Clone)]
pub struct Board {
    pub cards: [[Option<OnlineCard>; 8]; 8],
    pub firewalls: [[Option<PlayerId>; 8]; 8],
}

impl Board {
    pub fn new() -> Self {
        Self {
            cards: [[None; 8]; 8],
            firewalls: [[None; 8]; 8],
        }
    }

    pub fn in_bounds(pos: Position) -> bool {
        pos.row < 8 && pos.col < 8
    }

    pub fn get(&self, pos: Position) -> Option<OnlineCard> {
        self.cards[pos.row][pos.col]
    }

    pub fn set(&mut self, pos: Position, card: Option<OnlineCard>) {
        self.cards[pos.row][pos.col] = card;
    }

    pub fn has_own_card(&self, pos: Position, player: PlayerId) -> bool {
        self.get(pos).is_some_and(|card| card.owner == player)
    }

    pub fn has_opponent_card(&self, pos: Position, player: PlayerId) -> bool {
        self.get(pos)
            .is_some_and(|card| card.owner == player.opponent())
    }
}
