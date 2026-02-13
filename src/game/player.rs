use crate::game::types::Position;
use crate::game::types::{OnlineCard, PlayerId, StackChoice};

#[derive(Debug, Clone)]
pub struct PlayerState {
    pub id: PlayerId,
    pub link_stack: Vec<OnlineCard>,
    pub virus_stack: Vec<OnlineCard>,
    pub line_boosts: [Option<Position>; 2],
    pub firewalls: [Option<Position>; 2],
    pub virus_checks_used: [bool; 2],
    pub not_found_used: [bool; 2],
    pub setup_links_left: u8,
    pub setup_viruses_left: u8,
    pub setup_placed: u8,
}

impl PlayerState {
    pub fn new(id: PlayerId) -> Self {
        Self {
            id,
            link_stack: Vec::new(),
            virus_stack: Vec::new(),
            line_boosts: [None, None],
            firewalls: [None, None],
            virus_checks_used: [false, false],
            not_found_used: [false, false],
            setup_links_left: 4,
            setup_viruses_left: 4,
            setup_placed: 0,
        }
    }

    pub fn add_to_stack(&mut self, card: OnlineCard, stack: StackChoice) {
        match stack {
            StackChoice::Link => self.link_stack.push(card),
            StackChoice::Virus => self.virus_stack.push(card),
        }
    }
}
