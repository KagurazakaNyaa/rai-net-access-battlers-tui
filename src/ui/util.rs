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
            ui.message = ui.i18n.text_args(
                "msg-game-over",
                Some(crate::i18n::args_from_map(
                    [(
                        "player",
                        player_label_with_names(&ui.i18n, winner, &ui.player_names),
                    )]
                    .into_iter()
                    .collect(),
                )),
            );
        }
        _ => {
            ui.mode = crate::ui::state::UiMode::TurnPass;
            ui.message = ui.i18n.text_args(
                "help-turn-pass",
                Some(crate::i18n::args_from_map(
                    [(
                        "player",
                        player_label_with_names(&ui.i18n, game.current_player, &ui.player_names),
                    )]
                    .into_iter()
                    .collect(),
                )),
            );
        }
    }
}

pub fn player_label_with_names(
    i18n: &crate::i18n::I18n,
    player: PlayerId,
    names: &[String; 2],
) -> String {
    let role = match player {
        PlayerId::P1 => i18n.text("player-role-p1"),
        PlayerId::P2 => i18n.text("player-role-p2"),
    };
    let name = match player {
        PlayerId::P1 => names[0].clone(),
        PlayerId::P2 => names[1].clone(),
    };
    i18n.text_args(
        "player-label",
        Some(crate::i18n::args_from_map(
            [("role", role), ("name", name)].into_iter().collect(),
        )),
    )
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
        UiMode::Lobby => ui.i18n.text("help-lobby"),
        UiMode::JoinRoomInput => ui.i18n.text("help-room-input"),
        UiMode::Setup => ui.i18n.text("help-setup"),
        UiMode::MoveSelect => ui.i18n.text("help-move-select"),
        UiMode::MoveDest { .. } => ui.i18n.text("help-move-dest"),
        UiMode::TerminalMenu => ui.i18n.text("help-terminal"),
        UiMode::LineBoost => ui.i18n.text("help-lineboost"),
        UiMode::VirusCheck => ui.i18n.text("help-viruscheck"),
        UiMode::Firewall => ui.i18n.text("help-firewall"),
        UiMode::NotFoundFirst | UiMode::NotFoundSecond { .. } => ui.i18n.text("help-notfound"),
        UiMode::NotFoundSwap { .. } => ui.i18n.text("help-notfound-swap"),
        UiMode::ServerReveal { .. } => ui.i18n.text("help-server-reveal"),
        UiMode::ServerStack { .. } => ui.i18n.text("help-server-stack"),
        UiMode::TurnPass => ui.i18n.text_args(
            "help-turn-pass",
            Some(crate::i18n::args_from_map(
                [(
                    "player",
                    player_label_with_names(&ui.i18n, game.current_player, &ui.player_names),
                )]
                .into_iter()
                .collect(),
            )),
        ),
        UiMode::BoostContinue { .. } => ui.i18n.text("help-boost-continue"),
        UiMode::GameOver => ui.i18n.text("help-game-over"),
        UiMode::RoomConfirm { .. } => ui.i18n.text("help-confirm"),
        UiMode::RoomCreateDialog => ui.i18n.text("help-create-dialog"),
    }
}
