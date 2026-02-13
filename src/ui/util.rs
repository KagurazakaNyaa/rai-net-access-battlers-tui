use crate::{GameState, PlayerId, Position};

pub fn handle_cursor_keys(key: crossterm::event::KeyEvent, cursor: &mut Position) {
    use crossterm::event::KeyCode;
    match key.code {
        KeyCode::Up => cursor.row = cursor.row.saturating_sub(1),
        KeyCode::Down => cursor.row = (cursor.row + 1).min(7),
        KeyCode::Left => cursor.col = cursor.col.saturating_sub(1),
        KeyCode::Right => cursor.col = (cursor.col + 1).min(7),
        KeyCode::Char('k') if key.modifiers.is_empty() => cursor.row = cursor.row.saturating_sub(1),
        KeyCode::Char('j') if key.modifiers.is_empty() => cursor.row = (cursor.row + 1).min(7),
        KeyCode::Char('h') if key.modifiers.is_empty() => cursor.col = cursor.col.saturating_sub(1),
        KeyCode::Char('l') if key.modifiers.is_empty() => cursor.col = (cursor.col + 1).min(7),
        _ => {}
    }
}

pub fn end_turn(game: &mut GameState, ui: &mut crate::ui::state::UiState) {
    game.end_turn();
    match game.phase {
        crate::GamePhase::GameOver(winner) => {
            ui.mode = crate::ui::state::UiMode::GameOver;
            ui.message = format!(
                "Game Over. Winner: {}",
                player_label_with_names(winner, &ui.player_names)
            );
        }
        _ => {
            ui.mode = crate::ui::state::UiMode::TurnPass;
            ui.message = format!(
                "Pass to {}. Press Enter.",
                player_label_with_names(game.current_player, &ui.player_names)
            );
        }
    }
}

pub fn player_label_with_names(player: PlayerId, names: &[String; 2]) -> String {
    match player {
        PlayerId::P1 => format!("P1({})", names[0]),
        PlayerId::P2 => format!("P2({})", names[1]),
    }
}

pub fn first_free_line_boost(game: &GameState) -> Option<usize> {
    game.player(game.current_player)
        .line_boosts
        .iter()
        .position(|slot| slot.is_none())
}

pub fn find_line_boost_slot(game: &GameState, pos: Position) -> Option<usize> {
    game.player(game.current_player)
        .line_boosts
        .iter()
        .position(|slot| *slot == Some(pos))
}

pub fn first_free_firewall(game: &GameState) -> Option<usize> {
    game.player(game.current_player)
        .firewalls
        .iter()
        .position(|slot| slot.is_none())
}

pub fn find_firewall_slot(game: &GameState, pos: Position) -> Option<usize> {
    game.player(game.current_player)
        .firewalls
        .iter()
        .position(|slot| *slot == Some(pos))
}

pub fn first_unused_virus_check(game: &GameState) -> Option<usize> {
    game.player(game.current_player)
        .virus_checks_used
        .iter()
        .position(|used| !used)
}

pub fn first_unused_not_found(game: &GameState) -> Option<usize> {
    game.player(game.current_player)
        .not_found_used
        .iter()
        .position(|used| !used)
}

pub fn help_text(game: &GameState, ui: &crate::ui::state::UiState) -> String {
    use crate::ui::state::UiMode;
    match ui.mode {
        UiMode::Setup => "Setup: arrows move, L/V place, Backspace remove".to_string(),
        UiMode::MoveSelect => {
            "Move: arrows move, Enter select, T terminal, E enter server".to_string()
        }
        UiMode::MoveDest { .. } => {
            "Move: arrows choose destination, Enter move, Esc cancel".to_string()
        }
        UiMode::TerminalMenu => {
            "Terminal: 1 LineBoost, 2 VirusCheck, 3 Firewall, 4 404".to_string()
        }
        UiMode::LineBoost => "LineBoost: Enter to attach/detach, Esc back".to_string(),
        UiMode::VirusCheck => "VirusCheck: Enter to reveal, Esc back".to_string(),
        UiMode::Firewall => "Firewall: Enter place/remove, Esc back".to_string(),
        UiMode::NotFoundFirst | UiMode::NotFoundSecond { .. } => {
            "404: select two own cards".to_string()
        }
        UiMode::NotFoundSwap { .. } => "404: Y swap, N no-swap".to_string(),
        UiMode::ServerReveal { .. } => "Server: Y reveal, N hide".to_string(),
        UiMode::ServerStack { .. } => "Server: L to link stack, V to virus stack".to_string(),
        UiMode::TurnPass => format!(
            "Pass to {}. Enter to continue",
            player_label_with_names(game.current_player, &ui.player_names)
        ),
        UiMode::BoostContinue { .. } => "Boost: Enter move, N end".to_string(),
        UiMode::GameOver => "Game over. Enter to exit".to_string(),
    }
}
