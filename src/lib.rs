enum OnlineCardType {
    Link,
    Virus,
}

enum TurnResult {
    Defeat,
    Victory,
    ChangePlayer,
}

struct Position {
    row: u8,
    col: u8,
}

struct OnlineCard {
    card_type: OnlineCardType,
    revealed: bool,
    line_boost_attatched: bool,
    locate: Position,
    owner: u8,
}

struct Player {
    id: u8,
    virus_stack: Vec<OnlineCard>,
    link_stack: Vec<OnlineCard>,
    virus_check_used: bool,
    not_found_used: bool,
}

impl Player {
    fn check_turn_result(&self) -> TurnResult {
        let mut virus_count: u8 = 0;
        let mut links_count: u8 = 0;
        for card in &self.virus_stack {
            match card.card_type {
                OnlineCardType::Virus => virus_count += 1,
                OnlineCardType::Link => links_count += 1,
                _ => {}
            }
        }
        if virus_count >= 4 {
            return TurnResult::Defeat;
        } else if links_count >= 4 {
            return TurnResult::Victory;
        } else {
            return TurnResult::ChangePlayer;
        }
    }
}
