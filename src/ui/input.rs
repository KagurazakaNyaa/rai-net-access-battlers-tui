use std::io;

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};

use crate::ui::layout::compute_layout;
use crate::ui::state::{ActionMenu, MenuAction, MenuItem, UiMode, UiState};
use crate::ui::util::{
    end_turn, find_firewall_slot, find_line_boost_slot, first_free_firewall, first_free_line_boost,
    first_unused_not_found, first_unused_virus_check, handle_cursor_keys, help_text,
    player_label_with_names,
};
use crate::{GamePhase, GameState, OnlineCardType, Position, StackChoice};

pub fn handle_key(key: KeyEvent, game: &mut GameState, ui: &mut UiState) -> io::Result<bool> {
    if key.code == KeyCode::Char('q') || key.code == KeyCode::Char('Q') {
        return Ok(true);
    }

    if key.code == KeyCode::Char('h') || key.code == KeyCode::Char('H') {
        ui.message = help_text(game, ui);
        ui.menu = None;
        return Ok(false);
    }

    if let Some(action) = key_to_menu_action(&key) {
        if let Some(menu_action) = apply_menu_action(action, game, ui)? {
            return Ok(menu_action);
        }
    }

    if ui.is_spectator {
        ui.message = ui.i18n.text("status-spectator");
        return Ok(false);
    }

    if ui.op_sender.is_some() && ui.local_player != game.current_player {
        ui.message = ui.i18n.text("status-wait-opponent");
        return Ok(false);
    }

    let exit = handle_key_inner(key, game, ui)?;
    ui.menu = None;
    Ok(exit)
}

pub fn handle_mouse(
    mouse: MouseEvent,
    area: ratatui::layout::Rect,
    game: &mut GameState,
    ui: &mut UiState,
) -> io::Result<bool> {
    if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
        return Ok(false);
    }

    let layout = compute_layout(area);
    if let Some(menu) = &ui.menu {
        if rect_contains(menu.rect, mouse.column, mouse.row) {
            if let Some(action) = menu_action_at(menu, mouse.column, mouse.row) {
                return apply_menu_action(action, game, ui).map(|_| false);
            }
        }
        ui.menu = None;
    }

    if matches!(ui.mode, UiMode::RoomConfirm { .. }) {
        if let Some(action) = confirm_action_from_mouse(mouse, area) {
            return handle_confirm_action(action, ui);
        }
        return Ok(false);
    }

    if matches!(ui.mode, UiMode::RoomCreateDialog) {
        if let Some(action) = create_dialog_action_from_mouse(mouse, area) {
            return handle_create_dialog_mouse(action, ui);
        }
        return Ok(false);
    }

    if matches!(ui.mode, UiMode::RoomCreateDialog) {
        if let Some(action) = create_dialog_action_from_mouse(mouse, area) {
            return handle_create_dialog_mouse(action, ui);
        }
        return Ok(false);
    }

    if matches!(ui.mode, UiMode::Lobby | UiMode::JoinRoomInput) {
        handle_lobby_mouse(mouse, layout, ui);
        return Ok(false);
    }

    if rect_contains(layout.board, mouse.column, mouse.row) {
        if let Some(pos) = board_position_from_mouse(mouse, layout.board) {
            ui.cursor = pos;
            ui.menu = build_cell_menu(game, ui, layout, pos);
        }
        return Ok(false);
    }

    if rect_contains(layout.status, mouse.column, mouse.row) {
        if status_action_from_mouse(mouse, layout.status) {
            ui.mode = UiMode::RoomConfirm {
                action: crate::ui::state::RoomConfirmAction::Leave,
            };
            ui.confirm_message = ui.i18n.text("confirm-leave");
            return Ok(false);
        }
    }

    if rect_contains(layout.left_panel, mouse.column, mouse.row)
        || rect_contains(layout.right_panel, mouse.column, mouse.row)
    {
        if let Some(action) = panel_action_from_mouse(game, ui, layout, mouse.column, mouse.row) {
            return apply_menu_action(action, game, ui).map(|_| false);
        }
    }

    Ok(false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfirmAction {
    Yes,
    No,
}

fn handle_confirm_action(action: ConfirmAction, ui: &mut UiState) -> io::Result<bool> {
    if action == ConfirmAction::Yes {
        if let UiMode::RoomConfirm { action } = ui.mode {
            match action {
                crate::ui::state::RoomConfirmAction::JoinSelected => {
                    if !ui.room_id_input.is_empty() {
                        if let Some(sender) = &ui.op_sender {
                            let _ = sender.send(format!("OP ROOM JOIN {}", ui.room_id_input));
                        }
                    } else if let Some(room) = ui.rooms.get(ui.selected_room) {
                        if let Some(id) = &room.id {
                            if let Some(sender) = &ui.op_sender {
                                let _ = sender.send(format!("OP ROOM JOIN {}", id));
                            }
                        }
                    }
                }
                crate::ui::state::RoomConfirmAction::Create => {
                    if let Some(sender) = &ui.op_sender {
                        let name = if ui.room_input.is_empty() {
                            ui.i18n.text("msg-room-default")
                        } else {
                            ui.room_input.clone()
                        };
                        let id = if ui.room_id_input.is_empty() {
                            "-".to_string()
                        } else {
                            ui.room_id_input.clone()
                        };
                        let auto = if ui.auto_join { 1 } else { 0 };
                        let show = if ui.show_room_id { 1 } else { 0 };
                        let _ = sender
                            .send(format!("OP ROOM CREATE {} {} {} {}", name, id, auto, show));
                    }
                }
                crate::ui::state::RoomConfirmAction::Leave => {
                    if let Some(sender) = &ui.op_sender {
                        let _ = sender.send("OP ROOM LEAVE".to_string());
                    }
                }
            }
        }
    }
    ui.mode = UiMode::Lobby;
    Ok(false)
}

fn confirm_action_from_mouse(
    mouse: MouseEvent,
    area: ratatui::layout::Rect,
) -> Option<ConfirmAction> {
    let dialog = confirm_rect(area);
    if !rect_contains(dialog, mouse.column, mouse.row) {
        return None;
    }
    let inner = ratatui::layout::Rect::new(
        dialog.x + 1,
        dialog.y + 1,
        dialog.width.saturating_sub(2),
        dialog.height.saturating_sub(2),
    );
    let yes_rect = ratatui::layout::Rect::new(
        inner.x,
        inner.y + inner.height.saturating_sub(1),
        inner.width / 2,
        1,
    );
    let no_rect = ratatui::layout::Rect::new(
        inner.x + inner.width / 2,
        inner.y + inner.height.saturating_sub(1),
        inner.width - inner.width / 2,
        1,
    );
    if rect_contains(yes_rect, mouse.column, mouse.row) {
        Some(ConfirmAction::Yes)
    } else if rect_contains(no_rect, mouse.column, mouse.row) {
        Some(ConfirmAction::No)
    } else {
        None
    }
}

fn confirm_rect(area: ratatui::layout::Rect) -> ratatui::layout::Rect {
    use ratatui::layout::{Constraint, Direction, Layout};
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Length(7),
            Constraint::Percentage(35),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(vertical[1]);
    horizontal[1]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CreateDialogAction {
    Focus(crate::ui::state::CreateFocus),
    Confirm,
    Cancel,
}

fn create_dialog_action_from_mouse(
    mouse: MouseEvent,
    area: ratatui::layout::Rect,
) -> Option<CreateDialogAction> {
    let dialog = confirm_rect(area);
    if !rect_contains(dialog, mouse.column, mouse.row) {
        return None;
    }
    let inner = ratatui::layout::Rect::new(
        dialog.x + 1,
        dialog.y + 1,
        dialog.width.saturating_sub(2),
        dialog.height.saturating_sub(2),
    );
    let line = mouse.row.saturating_sub(inner.y) as usize;
    match line {
        2 => Some(CreateDialogAction::Focus(
            crate::ui::state::CreateFocus::Name,
        )),
        3 => Some(CreateDialogAction::Focus(crate::ui::state::CreateFocus::Id)),
        4 => Some(CreateDialogAction::Focus(
            crate::ui::state::CreateFocus::AutoJoin,
        )),
        5 => Some(CreateDialogAction::Focus(
            crate::ui::state::CreateFocus::ShowId,
        )),
        7 => {
            let mid = inner.x + inner.width / 2;
            if mouse.column < mid {
                Some(CreateDialogAction::Confirm)
            } else {
                Some(CreateDialogAction::Cancel)
            }
        }
        _ => None,
    }
}

fn handle_create_dialog_mouse(action: CreateDialogAction, ui: &mut UiState) -> io::Result<bool> {
    match action {
        CreateDialogAction::Focus(focus) => {
            ui.create_focus = focus;
            if matches!(focus, crate::ui::state::CreateFocus::AutoJoin) {
                ui.auto_join = !ui.auto_join;
            }
            if matches!(focus, crate::ui::state::CreateFocus::ShowId) {
                ui.show_room_id = !ui.show_room_id;
            }
        }
        CreateDialogAction::Confirm => {
            ui.mode = UiMode::RoomConfirm {
                action: crate::ui::state::RoomConfirmAction::Create,
            };
            ui.confirm_message = ui.i18n.text("confirm-create");
        }
        CreateDialogAction::Cancel => {
            ui.mode = UiMode::Lobby;
        }
    }
    Ok(false)
}

fn handle_lobby_mouse(mouse: MouseEvent, layout: crate::ui::layout::LayoutInfo, ui: &mut UiState) {
    if !rect_contains(layout.body, mouse.column, mouse.row) {
        return;
    }
    let inner = ratatui::layout::Rect::new(
        layout.body.x + 1,
        layout.body.y + 1,
        layout.body.width.saturating_sub(2),
        layout.body.height.saturating_sub(2),
    );
    let line = mouse.row.saturating_sub(inner.y) as usize;
    if line >= 2 && line < 2 + ui.rooms.len() {
        ui.selected_room = line - 2;
        ui.mode = UiMode::RoomConfirm {
            action: crate::ui::state::RoomConfirmAction::JoinSelected,
        };
        ui.confirm_message = ui.i18n.text("confirm-join");
    }
}

fn handle_setup_keys(key: KeyEvent, game: &mut GameState, ui: &mut UiState) {
    let phase_before = game.phase;

    // Handle placement keys (L/V/Backspace) - these should NOT move cursor
    if let GamePhase::Setup(player) = game.phase {
        match key.code {
            KeyCode::Char('l') | KeyCode::Char('L') => {
                match game.place_setup_card(player, ui.cursor, OnlineCardType::Link) {
                    Ok(()) => {
                        ui.message = ui.i18n.text("msg-place-link");
                        if let Some(sender) = &ui.op_sender {
                            let _ = sender
                                .send(format!("OP SETUP L {} {}", ui.cursor.row, ui.cursor.col));
                        }
                    }
                    Err(err) => {
                        ui.message = ui.i18n.text_args(
                            "msg-setup-error",
                            Some(crate::i18n::args_from_map(
                                [("error", format!("{:?}", err))].into_iter().collect(),
                            )),
                        );
                    }
                }
                apply_phase_transition(phase_before, game, ui);
                return;
            }
            KeyCode::Char('v') | KeyCode::Char('V') => {
                match game.place_setup_card(player, ui.cursor, OnlineCardType::Virus) {
                    Ok(()) => {
                        ui.message = ui.i18n.text("msg-place-virus");
                        if let Some(sender) = &ui.op_sender {
                            let _ = sender
                                .send(format!("OP SETUP V {} {}", ui.cursor.row, ui.cursor.col));
                        }
                    }
                    Err(err) => {
                        ui.message = ui.i18n.text_args(
                            "msg-setup-error",
                            Some(crate::i18n::args_from_map(
                                [("error", format!("{:?}", err))].into_iter().collect(),
                            )),
                        );
                    }
                }
                apply_phase_transition(phase_before, game, ui);
                return;
            }
            KeyCode::Backspace => {
                match game.remove_setup_card(player, ui.cursor) {
                    Ok(()) => {
                        ui.message = ui.i18n.text("msg-remove-card");
                        if let Some(sender) = &ui.op_sender {
                            let _ = sender
                                .send(format!("OP REMOVE {} {}", ui.cursor.row, ui.cursor.col));
                        }
                    }
                    Err(err) => {
                        ui.message = ui.i18n.text_args(
                            "msg-setup-error",
                            Some(crate::i18n::args_from_map(
                                [("error", format!("{:?}", err))].into_iter().collect(),
                            )),
                        );
                    }
                }
                apply_phase_transition(phase_before, game, ui);
                return;
            }
            _ => {}
        }
    }

    // For all other keys, allow cursor movement
    handle_cursor_keys(key, &mut ui.cursor);
}

fn handle_lobby_keys(key: KeyEvent, ui: &mut UiState) {
    match key.code {
        KeyCode::Up => {
            if ui.selected_room > 0 {
                ui.selected_room -= 1;
            }
        }
        KeyCode::Down => {
            if ui.selected_room + 1 < ui.rooms.len() {
                ui.selected_room += 1;
            }
        }
        KeyCode::Char('l') | KeyCode::Char('L') => {
            if let Some(sender) = &ui.op_sender {
                let _ = sender.send("OP ROOM LIST".to_string());
            }
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            if let Some(sender) = &ui.op_sender {
                let _ = sender.send("OP ROOM AUTO".to_string());
            }
        }
        KeyCode::Char('c') | KeyCode::Char('C') => {
            ui.room_input.clear();
            ui.room_id_input.clear();
            ui.mode = UiMode::RoomCreateDialog;
            ui.create_focus = crate::ui::state::CreateFocus::Name;
            ui.message = ui.i18n.text("msg-create-room");
        }
        KeyCode::Char('j') | KeyCode::Char('J') => {
            ui.room_id_input.clear();
            ui.mode = UiMode::JoinRoomInput;
            ui.message = ui.i18n.text("msg-join-room");
        }
        KeyCode::Char('t') | KeyCode::Char('T') => {
            ui.auto_join = !ui.auto_join;
            ui.message = ui.i18n.text_args(
                "msg-auto-join-toggle",
                Some(crate::i18n::args_from_map(
                    [(
                        "state",
                        ui.i18n.text(if ui.auto_join {
                            "msg-auto-on"
                        } else {
                            "msg-auto-off"
                        }),
                    )]
                    .into_iter()
                    .collect(),
                )),
            );
        }
        KeyCode::Char('i') | KeyCode::Char('I') => {
            ui.show_room_id = !ui.show_room_id;
            ui.message = ui.i18n.text_args(
                "msg-show-id-toggle",
                Some(crate::i18n::args_from_map(
                    [(
                        "state",
                        ui.i18n.text(if ui.show_room_id {
                            "msg-show-on"
                        } else {
                            "msg-show-off"
                        }),
                    )]
                    .into_iter()
                    .collect(),
                )),
            );
        }
        KeyCode::Char('s') | KeyCode::Char('S') => {
            if let Some(room) = ui.rooms.get(ui.selected_room) {
                if let Some(id) = &room.id {
                    if let Some(sender) = &ui.op_sender {
                        let _ = sender.send(format!("OP ROOM SPECTATE {}", id));
                    }
                }
            }
        }
        KeyCode::Enter => {
            if let Some(room) = ui.rooms.get(ui.selected_room) {
                if room.id.is_some() {
                    ui.mode = UiMode::RoomConfirm {
                        action: crate::ui::state::RoomConfirmAction::JoinSelected,
                    };
                    ui.confirm_message = ui.i18n.text("confirm-join");
                }
            }
        }
        _ => {}
    }
}

fn handle_join_room_input(key: KeyEvent, ui: &mut UiState) {
    match key.code {
        KeyCode::Esc => {
            ui.mode = UiMode::Lobby;
            ui.message = ui.i18n.text("msg-lobby");
        }
        KeyCode::Backspace => {
            if ui.message == ui.i18n.text("msg-join-room") {
                ui.room_id_input.pop();
            } else {
                ui.room_input.pop();
            }
        }
        KeyCode::Enter => {
            if ui.message == ui.i18n.text("msg-create-room") {
                ui.mode = UiMode::RoomCreateDialog;
                ui.create_focus = crate::ui::state::CreateFocus::Name;
            } else {
                ui.mode = UiMode::RoomConfirm {
                    action: crate::ui::state::RoomConfirmAction::JoinSelected,
                };
                ui.confirm_message = ui.i18n.text("confirm-join");
            }
        }
        KeyCode::Char(ch) => {
            if !ch.is_control() {
                if ui.message == ui.i18n.text("msg-join-room") {
                    ui.room_id_input.push(ch);
                } else {
                    ui.room_input.push(ch);
                }
            }
        }
        _ => {}
    }
}

fn handle_create_dialog_key(key: KeyEvent, ui: &mut UiState) -> io::Result<bool> {
    use crate::ui::state::CreateFocus;
    match key.code {
        KeyCode::Tab => {
            ui.create_focus = match ui.create_focus {
                CreateFocus::Name => CreateFocus::Id,
                CreateFocus::Id => CreateFocus::AutoJoin,
                CreateFocus::AutoJoin => CreateFocus::ShowId,
                CreateFocus::ShowId => CreateFocus::Confirm,
                CreateFocus::Confirm => CreateFocus::Cancel,
                CreateFocus::Cancel => CreateFocus::Name,
            };
        }
        KeyCode::Esc => {
            ui.mode = UiMode::Lobby;
            return Ok(false);
        }
        KeyCode::Enter => match ui.create_focus {
            CreateFocus::Confirm => {
                ui.mode = UiMode::RoomConfirm {
                    action: crate::ui::state::RoomConfirmAction::Create,
                };
                ui.confirm_message = ui.i18n.text("confirm-create");
            }
            CreateFocus::Cancel => {
                ui.mode = UiMode::Lobby;
            }
            _ => {}
        },
        KeyCode::Char(' ') => match ui.create_focus {
            CreateFocus::AutoJoin => ui.auto_join = !ui.auto_join,
            CreateFocus::ShowId => ui.show_room_id = !ui.show_room_id,
            _ => {}
        },
        KeyCode::Backspace => match ui.create_focus {
            CreateFocus::Name => {
                ui.room_input.pop();
            }
            CreateFocus::Id => {
                ui.room_id_input.pop();
            }
            _ => {}
        },
        KeyCode::Char(ch) => {
            if !ch.is_control() {
                match ui.create_focus {
                    CreateFocus::Name => ui.room_input.push(ch),
                    CreateFocus::Id => ui.room_id_input.push(ch),
                    _ => {}
                }
            }
        }
        _ => {}
    }
    Ok(false)
}

fn apply_phase_transition(phase_before: GamePhase, game: &GameState, ui: &mut UiState) {
    if phase_before != game.phase {
        ui.mode = UiMode::TurnPass;
        ui.message = match game.phase {
            GamePhase::Setup(player) => ui.i18n.text_args(
                "msg-pass-setup",
                Some(crate::i18n::args_from_map(
                    [(
                        "player",
                        player_label_with_names(&ui.i18n, player, &ui.player_names),
                    )]
                    .into_iter()
                    .collect(),
                )),
            ),
            GamePhase::Playing => ui.i18n.text("msg-setup-complete"),
            GamePhase::GameOver(winner) => ui.i18n.text_args(
                "msg-game-over",
                Some(crate::i18n::args_from_map(
                    [(
                        "player",
                        player_label_with_names(&ui.i18n, winner, &ui.player_names),
                    )]
                    .into_iter()
                    .collect(),
                )),
            ),
        };
    }
}

fn key_to_menu_action(key: &KeyEvent) -> Option<MenuAction> {
    match key.code {
        KeyCode::Char('l') | KeyCode::Char('L') => Some(MenuAction::Key(KeyCode::Char('l'))),
        KeyCode::Char('v') | KeyCode::Char('V') => Some(MenuAction::Key(KeyCode::Char('v'))),
        KeyCode::Char('y') | KeyCode::Char('Y') => Some(MenuAction::Key(KeyCode::Char('y'))),
        KeyCode::Char('n') | KeyCode::Char('N') => Some(MenuAction::Key(KeyCode::Char('n'))),
        KeyCode::Char('t') | KeyCode::Char('T') => Some(MenuAction::Key(KeyCode::Char('t'))),
        KeyCode::Char('e') | KeyCode::Char('E') => Some(MenuAction::Key(KeyCode::Char('e'))),
        KeyCode::Char('1') => Some(MenuAction::Key(KeyCode::Char('1'))),
        KeyCode::Char('2') => Some(MenuAction::Key(KeyCode::Char('2'))),
        KeyCode::Char('3') => Some(MenuAction::Key(KeyCode::Char('3'))),
        KeyCode::Char('4') => Some(MenuAction::Key(KeyCode::Char('4'))),
        KeyCode::Enter => Some(MenuAction::Key(KeyCode::Enter)),
        KeyCode::Esc => Some(MenuAction::Key(KeyCode::Esc)),
        KeyCode::Backspace => Some(MenuAction::Key(KeyCode::Backspace)),
        _ => None,
    }
}

fn apply_menu_action(
    action: MenuAction,
    game: &mut GameState,
    ui: &mut UiState,
) -> io::Result<Option<bool>> {
    let MenuAction::Key(key) = action;
    let key_event = KeyEvent::new(key, crossterm::event::KeyModifiers::empty());
    ui.menu = None;
    let exit = handle_key_inner(key_event, game, ui)?;
    Ok(Some(exit))
}

fn handle_key_inner(key: KeyEvent, game: &mut GameState, ui: &mut UiState) -> io::Result<bool> {
    if key.code == KeyCode::Char('q') || key.code == KeyCode::Char('Q') {
        return Ok(true);
    }

    if key.code == KeyCode::Char('h') || key.code == KeyCode::Char('H') {
        ui.message = help_text(game, ui);
        ui.menu = None;
        return Ok(false);
    }

    match ui.mode {
        UiMode::Lobby => handle_lobby_keys(key, ui),
        UiMode::JoinRoomInput => handle_join_room_input(key, ui),
        UiMode::RoomConfirm { .. } => match key.code {
            KeyCode::Enter => {
                return handle_confirm_action(ConfirmAction::Yes, ui);
            }
            KeyCode::Esc => {
                return handle_confirm_action(ConfirmAction::No, ui);
            }
            _ => {}
        },
        UiMode::RoomCreateDialog => {
            return handle_create_dialog_key(key, ui);
        }
        UiMode::GameOver => {
            if key.code == KeyCode::Enter {
                return Ok(true);
            }
        }
        UiMode::TurnPass => match key.code {
            KeyCode::Enter => match game.phase {
                GamePhase::Setup(player) => {
                    ui.mode = UiMode::Setup;
                    ui.cursor = player.setup_positions()[0];
                    ui.message = ui.i18n.text_args(
                        "msg-setup-instructions",
                        Some(crate::i18n::args_from_map(
                            [(
                                "player",
                                player_label_with_names(&ui.i18n, player, &ui.player_names),
                            )]
                            .into_iter()
                            .collect(),
                        )),
                    );
                }
                GamePhase::Playing => {
                    ui.mode = UiMode::MoveSelect;
                    ui.message = ui.i18n.text_args(
                        "msg-turn-select",
                        Some(crate::i18n::args_from_map(
                            [(
                                "player",
                                player_label_with_names(
                                    &ui.i18n,
                                    game.current_player,
                                    &ui.player_names,
                                ),
                            )]
                            .into_iter()
                            .collect(),
                        )),
                    );
                }
                GamePhase::GameOver(winner) => {
                    ui.mode = UiMode::GameOver;
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
            },
            _ => {}
        },
        UiMode::Setup => handle_setup_keys(key, game, ui),
        UiMode::MoveSelect => handle_move_select_keys(key, game, ui),
        UiMode::MoveDest { from } => handle_move_dest_keys(key, game, ui, from),
        UiMode::BoostContinue { from } => handle_boost_continue_keys(key, game, ui, from),
        UiMode::TerminalMenu => handle_terminal_menu_keys(key, game, ui),
        UiMode::LineBoost => handle_line_boost_keys(key, game, ui),
        UiMode::VirusCheck => handle_virus_check_keys(key, game, ui),
        UiMode::Firewall => handle_firewall_keys(key, game, ui),
        UiMode::NotFoundFirst => handle_not_found_first_keys(key, game, ui),
        UiMode::NotFoundSecond { first } => handle_not_found_second_keys(key, game, ui, first),
        UiMode::NotFoundSwap { first, second } => {
            handle_not_found_swap_keys(key, game, ui, first, second)
        }
        UiMode::ServerReveal { from } => handle_server_reveal_keys(key, game, ui, from),
        UiMode::ServerStack { from, reveal } => {
            handle_server_stack_keys(key, game, ui, from, reveal)
        }
    }

    Ok(false)
}

fn rect_contains(rect: ratatui::layout::Rect, column: u16, row: u16) -> bool {
    column >= rect.x && column < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

fn board_position_from_mouse(mouse: MouseEvent, board: ratatui::layout::Rect) -> Option<Position> {
    let inner_x = board.x + 1;
    let inner_y = board.y + 1;

    if mouse.row <= inner_y {
        return None;
    }
    let row = mouse.row.saturating_sub(inner_y + 1) as usize;
    if mouse.column < inner_x + 3 {
        return None;
    }
    let col_area = mouse.column.saturating_sub(inner_x + 3);
    let col = (col_area / 3) as usize;
    if row < 8 && col < 8 {
        Some(Position::new(row, col))
    } else {
        None
    }
}

fn build_cell_menu(
    game: &GameState,
    ui: &UiState,
    layout: crate::ui::layout::LayoutInfo,
    pos: Position,
) -> Option<ActionMenu> {
    let mut items = Vec::new();
    match ui.mode {
        UiMode::Setup => {
            if game.board.cards[pos.row][pos.col].is_none()
                && game.can_place_setup(game.current_player, pos)
            {
                items.push(MenuItem {
                    label: ui.i18n.text("menu-place-link"),
                    action: MenuAction::Key(KeyCode::Char('l')),
                });
                items.push(MenuItem {
                    label: ui.i18n.text("menu-place-virus"),
                    action: MenuAction::Key(KeyCode::Char('v')),
                });
            } else if game.board.cards[pos.row][pos.col].is_some() {
                items.push(MenuItem {
                    label: ui.i18n.text("menu-remove"),
                    action: MenuAction::Key(KeyCode::Backspace),
                });
            }
        }
        UiMode::MoveSelect => {
            if let Some(card) = game.board.cards[pos.row][pos.col] {
                if card.owner == game.current_player {
                    items.push(MenuItem {
                        label: ui.i18n.text("menu-select"),
                        action: MenuAction::Key(KeyCode::Enter),
                    });
                    if crate::GameState::exit_owner(pos) == Some(game.current_player.opponent()) {
                        items.push(MenuItem {
                            label: ui.i18n.text("menu-enter-server"),
                            action: MenuAction::Key(KeyCode::Char('e')),
                        });
                    }
                }
            }
            items.push(MenuItem {
                label: ui.i18n.text("menu-terminal"),
                action: MenuAction::Key(KeyCode::Char('t')),
            });
        }
        UiMode::MoveDest { .. } => {
            items.push(MenuItem {
                label: ui.i18n.text("menu-move"),
                action: MenuAction::Key(KeyCode::Enter),
            });
        }
        UiMode::BoostContinue { .. } => {
            items.push(MenuItem {
                label: ui.i18n.text("menu-boost-move"),
                action: MenuAction::Key(KeyCode::Enter),
            });
            items.push(MenuItem {
                label: ui.i18n.text("menu-end-turn"),
                action: MenuAction::Key(KeyCode::Char('n')),
            });
        }
        UiMode::LineBoost
        | UiMode::VirusCheck
        | UiMode::Firewall
        | UiMode::NotFoundFirst
        | UiMode::NotFoundSecond { .. } => {
            items.push(MenuItem {
                label: ui.i18n.text("menu-apply"),
                action: MenuAction::Key(KeyCode::Enter),
            });
        }
        UiMode::NotFoundSwap { .. } => {
            items.push(MenuItem {
                label: ui.i18n.text("menu-swap"),
                action: MenuAction::Key(KeyCode::Char('y')),
            });
            items.push(MenuItem {
                label: ui.i18n.text("menu-keep"),
                action: MenuAction::Key(KeyCode::Char('n')),
            });
        }
        UiMode::ServerReveal { .. } => {
            items.push(MenuItem {
                label: ui.i18n.text("menu-reveal"),
                action: MenuAction::Key(KeyCode::Char('y')),
            });
            items.push(MenuItem {
                label: ui.i18n.text("menu-hide"),
                action: MenuAction::Key(KeyCode::Char('n')),
            });
        }
        UiMode::ServerStack { .. } => {
            items.push(MenuItem {
                label: ui.i18n.text("menu-link-stack"),
                action: MenuAction::Key(KeyCode::Char('l')),
            });
            items.push(MenuItem {
                label: ui.i18n.text("menu-virus-stack"),
                action: MenuAction::Key(KeyCode::Char('v')),
            });
        }
        _ => {}
    }

    if items.is_empty() {
        return None;
    }

    let menu_width = items
        .iter()
        .map(|item| item.label.len() as u16)
        .max()
        .unwrap_or(10)
        .saturating_add(2);
    let menu_height = items.len() as u16 + 2;

    let mut x = layout.board.x + 2;
    let mut y = layout.board.y + 2;
    let max_x = layout.area.x + layout.area.width;
    let max_y = layout.area.y + layout.area.height;
    if x + menu_width > max_x {
        x = max_x.saturating_sub(menu_width);
    }
    if y + menu_height > max_y {
        y = max_y.saturating_sub(menu_height);
    }

    Some(ActionMenu {
        rect: ratatui::layout::Rect::new(x, y, menu_width, menu_height),
        items,
    })
}

fn panel_action_from_mouse(
    game: &GameState,
    ui: &UiState,
    layout: crate::ui::layout::LayoutInfo,
    column: u16,
    row: u16,
) -> Option<MenuAction> {
    if !matches!(game.phase, GamePhase::Playing) || !matches!(ui.mode, UiMode::MoveSelect) {
        return None;
    }
    let panel = if rect_contains(layout.left_panel, column, row) {
        layout.left_inner
    } else if rect_contains(layout.right_panel, column, row) {
        layout.right_inner
    } else {
        return None;
    };
    let line = row.saturating_sub(panel.y) as usize;
    match line {
        5 => Some(MenuAction::Key(KeyCode::Char('1'))),
        6 => Some(MenuAction::Key(KeyCode::Char('2'))),
        7 => Some(MenuAction::Key(KeyCode::Char('3'))),
        8 => Some(MenuAction::Key(KeyCode::Char('4'))),
        _ => None,
    }
}

fn menu_action_at(menu: &ActionMenu, column: u16, row: u16) -> Option<MenuAction> {
    if !rect_contains(menu.rect, column, row) {
        return None;
    }
    let index = row.saturating_sub(menu.rect.y + 1) as usize;
    menu.items.get(index).map(|item| item.action.clone())
}

fn status_action_from_mouse(mouse: MouseEvent, status: ratatui::layout::Rect) -> bool {
    if !rect_contains(status, mouse.column, mouse.row) {
        return false;
    }
    let inner = ratatui::layout::Rect::new(
        status.x + 1,
        status.y + 1,
        status.width.saturating_sub(2),
        status.height.saturating_sub(2),
    );
    let button_width = 10u16;
    let button_x = inner
        .x
        .saturating_add(inner.width.saturating_sub(button_width));
    let button_rect = ratatui::layout::Rect::new(
        button_x,
        inner.y + inner.height.saturating_sub(1),
        button_width,
        1,
    );
    rect_contains(button_rect, mouse.column, mouse.row)
}

fn handle_move_select_keys(key: KeyEvent, game: &mut GameState, ui: &mut UiState) {
    handle_cursor_keys(key, &mut ui.cursor);
    match key.code {
        KeyCode::Enter => {
            if let Some(card) = game.board.cards[ui.cursor.row][ui.cursor.col] {
                if card.owner == game.current_player {
                    ui.mode = UiMode::MoveDest { from: ui.cursor };
                    ui.message = ui.i18n.text("msg-select-dest");
                } else {
                    ui.message = ui.i18n.text("msg-not-your-card");
                }
            } else {
                ui.message = ui.i18n.text("msg-no-card");
            }
        }
        KeyCode::Char('t') | KeyCode::Char('T') => {
            ui.mode = UiMode::TerminalMenu;
            ui.message = ui.i18n.text("msg-terminal-open");
        }
        KeyCode::Char('e') | KeyCode::Char('E') => {
            if let Some(card) = game.board.cards[ui.cursor.row][ui.cursor.col] {
                if card.owner == game.current_player
                    && crate::GameState::exit_owner(ui.cursor)
                        == Some(game.current_player.opponent())
                {
                    ui.mode = UiMode::ServerReveal { from: ui.cursor };
                    ui.message = ui.i18n.text("msg-enter-server");
                } else {
                    ui.message = ui.i18n.text("msg-not-on-exit");
                }
            } else {
                ui.message = ui.i18n.text("msg-no-card");
            }
        }
        _ => {}
    }
}

fn handle_move_dest_keys(key: KeyEvent, game: &mut GameState, ui: &mut UiState, from: Position) {
    handle_cursor_keys(key, &mut ui.cursor);
    match key.code {
        KeyCode::Enter => match game.start_move(from, ui.cursor) {
            Ok(outcome) => match outcome {
                crate::MoveOutcome::CanMoveAgain => {
                    if let Some(sender) = &ui.op_sender {
                        let _ = sender.send(format!(
                            "OP MOVE {} {} {} {}",
                            from.row, from.col, ui.cursor.row, ui.cursor.col
                        ));
                    }
                    ui.mode = UiMode::BoostContinue { from: ui.cursor };
                    ui.message = ui.i18n.text("msg-boost-again");
                }
                crate::MoveOutcome::TurnEnds => {
                    if let Some(sender) = &ui.op_sender {
                        let _ = sender.send(format!(
                            "OP MOVE {} {} {} {}",
                            from.row, from.col, ui.cursor.row, ui.cursor.col
                        ));
                    }
                    end_turn(game, ui)
                }
            },
            Err(err) => {
                ui.message = ui.i18n.text_args(
                    "msg-move-error",
                    Some(crate::i18n::args_from_map(
                        [("error", format!("{:?}", err))].into_iter().collect(),
                    )),
                );
            }
        },
        KeyCode::Esc => {
            ui.mode = UiMode::MoveSelect;
            ui.message = ui.i18n.text("msg-select-card");
        }
        _ => {}
    }
}

fn handle_boost_continue_keys(
    key: KeyEvent,
    game: &mut GameState,
    ui: &mut UiState,
    from: Position,
) {
    handle_cursor_keys(key, &mut ui.cursor);
    match key.code {
        KeyCode::Enter => match game.continue_boost_move(from, ui.cursor) {
            Ok(_) => {
                if let Some(sender) = &ui.op_sender {
                    let _ = sender.send(format!(
                        "OP BOOST {} {} {} {}",
                        from.row, from.col, ui.cursor.row, ui.cursor.col
                    ));
                }
                end_turn(game, ui)
            }
            Err(err) => {
                ui.message = ui.i18n.text_args(
                    "msg-boost-error",
                    Some(crate::i18n::args_from_map(
                        [("error", format!("{:?}", err))].into_iter().collect(),
                    )),
                );
            }
        },
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            if let Some(sender) = &ui.op_sender {
                let _ = sender.send("OP ENDTURN".to_string());
            }
            end_turn(game, ui)
        }
        _ => {}
    }
}

fn handle_terminal_menu_keys(_key: KeyEvent, _game: &mut GameState, ui: &mut UiState) {
    match _key.code {
        KeyCode::Char('1') => {
            ui.mode = UiMode::LineBoost;
            ui.message = ui.i18n.text("msg-lineboost-select");
        }
        KeyCode::Char('2') => {
            ui.mode = UiMode::VirusCheck;
            ui.message = ui.i18n.text("msg-viruscheck-select");
        }
        KeyCode::Char('3') => {
            ui.mode = UiMode::Firewall;
            ui.message = ui.i18n.text("msg-firewall-select");
        }
        KeyCode::Char('4') => {
            ui.mode = UiMode::NotFoundFirst;
            ui.message = ui.i18n.text("msg-404-first");
        }
        KeyCode::Esc => {
            ui.mode = UiMode::MoveSelect;
            ui.message = ui.i18n.text("msg-select-card");
        }
        _ => {}
    }
}

fn handle_line_boost_keys(key: KeyEvent, game: &mut GameState, ui: &mut UiState) {
    handle_cursor_keys(key, &mut ui.cursor);
    if key.code == KeyCode::Enter {
        if let Some(card) = game.board.cards[ui.cursor.row][ui.cursor.col] {
            if card.owner != game.current_player {
                ui.message = ui.i18n.text("msg-not-your-card");
                return;
            }
            if card.line_boost_attached {
                if let Some(index) = find_line_boost_slot(game, ui.cursor) {
                    match game.use_line_boost_detach(index) {
                        Ok(()) => {
                            if let Some(sender) = &ui.op_sender {
                                let _ = sender.send(format!(
                                    "OP LINEBOOST DETACH {} {}",
                                    ui.cursor.row, ui.cursor.col
                                ));
                            }
                            end_turn(game, ui)
                        }
                        Err(err) => {
                            ui.message = ui.i18n.text_args(
                                "msg-lineboost-error",
                                Some(crate::i18n::args_from_map(
                                    [("error", format!("{:?}", err))].into_iter().collect(),
                                )),
                            );
                        }
                    }
                } else {
                    ui.message = ui.i18n.text("msg-lineboost-none");
                }
            } else if let Some(index) = first_free_line_boost(game) {
                match game.use_line_boost_attach(index, ui.cursor) {
                    Ok(()) => {
                        if let Some(sender) = &ui.op_sender {
                            let _ = sender.send(format!(
                                "OP LINEBOOST ATTACH {} {}",
                                ui.cursor.row, ui.cursor.col
                            ));
                        }
                        end_turn(game, ui)
                    }
                    Err(err) => {
                        ui.message = ui.i18n.text_args(
                            "msg-lineboost-error",
                            Some(crate::i18n::args_from_map(
                                [("error", format!("{:?}", err))].into_iter().collect(),
                            )),
                        );
                    }
                }
            } else {
                ui.message = ui.i18n.text("msg-lineboost-slot-full");
            }
        } else {
            ui.message = ui.i18n.text("msg-no-card");
        }
    } else if key.code == KeyCode::Esc {
        ui.mode = UiMode::MoveSelect;
        ui.message = ui.i18n.text("msg-select-card");
    }
}

fn handle_virus_check_keys(key: KeyEvent, game: &mut GameState, ui: &mut UiState) {
    handle_cursor_keys(key, &mut ui.cursor);
    if key.code == KeyCode::Enter {
        if let Some(card) = game.board.cards[ui.cursor.row][ui.cursor.col] {
            if card.owner == game.current_player {
                ui.message = ui.i18n.text("msg-opponent-card");
                return;
            }
            if card.revealed {
                ui.message = ui.i18n.text("msg-card-revealed");
                return;
            }
            if let Some(index) = first_unused_virus_check(game) {
                match game.use_virus_check(index, ui.cursor) {
                    Ok(()) => {
                        if let Some(sender) = &ui.op_sender {
                            let _ = sender
                                .send(format!("OP VIRUSCHECK {} {}", ui.cursor.row, ui.cursor.col));
                        }
                        end_turn(game, ui)
                    }
                    Err(err) => {
                        ui.message = ui.i18n.text_args(
                            "msg-viruscheck-error",
                            Some(crate::i18n::args_from_map(
                                [("error", format!("{:?}", err))].into_iter().collect(),
                            )),
                        );
                    }
                }
            } else {
                ui.message = ui.i18n.text("msg-viruscheck-used");
            }
        } else {
            ui.message = ui.i18n.text("msg-no-card");
        }
    } else if key.code == KeyCode::Esc {
        ui.mode = UiMode::MoveSelect;
        ui.message = ui.i18n.text("msg-select-card");
    }
}

fn handle_firewall_keys(key: KeyEvent, game: &mut GameState, ui: &mut UiState) {
    handle_cursor_keys(key, &mut ui.cursor);
    if key.code == KeyCode::Enter {
        if game.board.firewalls[ui.cursor.row][ui.cursor.col] == Some(game.current_player) {
            if let Some(index) = find_firewall_slot(game, ui.cursor) {
                match game.use_firewall_remove(index) {
                    Ok(()) => {
                        if let Some(sender) = &ui.op_sender {
                            let _ = sender.send(format!(
                                "OP FIREWALL REMOVE {} {}",
                                ui.cursor.row, ui.cursor.col
                            ));
                        }
                        end_turn(game, ui)
                    }
                    Err(err) => {
                        ui.message = ui.i18n.text_args(
                            "msg-firewall-error",
                            Some(crate::i18n::args_from_map(
                                [("error", format!("{:?}", err))].into_iter().collect(),
                            )),
                        );
                    }
                }
            } else {
                ui.message = ui.i18n.text("msg-firewall-none");
            }
        } else if let Some(index) = first_free_firewall(game) {
            match game.use_firewall_place(index, ui.cursor) {
                Ok(()) => {
                    if let Some(sender) = &ui.op_sender {
                        let _ = sender.send(format!(
                            "OP FIREWALL PLACE {} {}",
                            ui.cursor.row, ui.cursor.col
                        ));
                    }
                    end_turn(game, ui)
                }
                Err(err) => {
                    ui.message = ui.i18n.text_args(
                        "msg-firewall-error",
                        Some(crate::i18n::args_from_map(
                            [("error", format!("{:?}", err))].into_iter().collect(),
                        )),
                    );
                }
            }
        } else {
            ui.message = ui.i18n.text("msg-firewall-slot-full");
        }
    } else if key.code == KeyCode::Esc {
        ui.mode = UiMode::MoveSelect;
        ui.message = ui.i18n.text("msg-select-card");
    }
}

fn handle_not_found_first_keys(key: KeyEvent, game: &mut GameState, ui: &mut UiState) {
    handle_cursor_keys(key, &mut ui.cursor);
    if key.code == KeyCode::Enter {
        if let Some(card) = game.board.cards[ui.cursor.row][ui.cursor.col] {
            if card.owner != game.current_player {
                ui.message = ui.i18n.text("msg-own-card");
                return;
            }
            ui.mode = UiMode::NotFoundSecond { first: ui.cursor };
            ui.message = ui.i18n.text("msg-404-second");
        } else {
            ui.message = ui.i18n.text("msg-no-card");
        }
    } else if key.code == KeyCode::Esc {
        ui.mode = UiMode::MoveSelect;
        ui.message = ui.i18n.text("msg-select-card");
    }
}

fn handle_not_found_second_keys(
    key: KeyEvent,
    game: &mut GameState,
    ui: &mut UiState,
    first: Position,
) {
    handle_cursor_keys(key, &mut ui.cursor);
    if key.code == KeyCode::Enter {
        if let Some(card) = game.board.cards[ui.cursor.row][ui.cursor.col] {
            if card.owner != game.current_player {
                ui.message = ui.i18n.text("msg-own-card");
                return;
            }
            ui.mode = UiMode::NotFoundSwap {
                first,
                second: ui.cursor,
            };
            ui.message = ui.i18n.text("msg-404-swap");
        } else {
            ui.message = ui.i18n.text("msg-no-card");
        }
    } else if key.code == KeyCode::Esc {
        ui.mode = UiMode::MoveSelect;
        ui.message = ui.i18n.text("msg-select-card");
    }
}

fn handle_not_found_swap_keys(
    key: KeyEvent,
    game: &mut GameState,
    ui: &mut UiState,
    first: Position,
    second: Position,
) {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => apply_not_found(game, ui, first, second, true),
        KeyCode::Char('n') | KeyCode::Char('N') => apply_not_found(game, ui, first, second, false),
        KeyCode::Esc => {
            ui.mode = UiMode::MoveSelect;
            ui.message = ui.i18n.text("msg-select-card");
        }
        _ => {}
    }
}

fn handle_server_reveal_keys(
    key: KeyEvent,
    _game: &mut GameState,
    ui: &mut UiState,
    from: Position,
) {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            ui.mode = UiMode::ServerStack { from, reveal: true };
            ui.message = ui.i18n.text("msg-server-stack");
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            ui.mode = UiMode::ServerStack {
                from,
                reveal: false,
            };
            ui.message = ui.i18n.text("msg-server-stack");
        }
        KeyCode::Esc => {
            ui.mode = UiMode::MoveSelect;
            ui.message = ui.i18n.text("msg-select-card");
        }
        _ => {}
    }
}

fn handle_server_stack_keys(
    key: KeyEvent,
    game: &mut GameState,
    ui: &mut UiState,
    from: Position,
    reveal: bool,
) {
    match key.code {
        KeyCode::Char('l') | KeyCode::Char('L') => {
            match game.enter_server_center(from, reveal, StackChoice::Link) {
                Ok(()) => {
                    if let Some(sender) = &ui.op_sender {
                        let reveal_num = if reveal { 1 } else { 0 };
                        let _ = sender.send(format!(
                            "OP ENTER {} {} {} L",
                            from.row, from.col, reveal_num
                        ));
                    }
                    end_turn(game, ui)
                }
                Err(err) => {
                    ui.message = ui.i18n.text_args(
                        "msg-server-error",
                        Some(crate::i18n::args_from_map(
                            [("error", format!("{:?}", err))].into_iter().collect(),
                        )),
                    );
                }
            }
        }
        KeyCode::Char('v') | KeyCode::Char('V') => {
            match game.enter_server_center(from, reveal, StackChoice::Virus) {
                Ok(()) => {
                    if let Some(sender) = &ui.op_sender {
                        let reveal_num = if reveal { 1 } else { 0 };
                        let _ = sender.send(format!(
                            "OP ENTER {} {} {} V",
                            from.row, from.col, reveal_num
                        ));
                    }
                    end_turn(game, ui)
                }
                Err(err) => {
                    ui.message = ui.i18n.text_args(
                        "msg-server-error",
                        Some(crate::i18n::args_from_map(
                            [("error", format!("{:?}", err))].into_iter().collect(),
                        )),
                    );
                }
            }
        }
        KeyCode::Esc => {
            ui.mode = UiMode::MoveSelect;
            ui.message = ui.i18n.text("msg-select-card");
        }
        _ => {}
    }
}

fn apply_not_found(
    game: &mut GameState,
    ui: &mut UiState,
    first: Position,
    second: Position,
    swap: bool,
) {
    if let Some(index) = first_unused_not_found(game) {
        match game.use_404(index, first, second, swap) {
            Ok(()) => {
                if let Some(sender) = &ui.op_sender {
                    let swap_num = if swap { 1 } else { 0 };
                    let _ = sender.send(format!(
                        "OP NOTFOUND {} {} {} {} {}",
                        first.row, first.col, second.row, second.col, swap_num
                    ));
                }
                end_turn(game, ui)
            }
            Err(err) => {
                ui.message = ui.i18n.text_args(
                    "msg-404-error",
                    Some(crate::i18n::args_from_map(
                        [("error", format!("{:?}", err))].into_iter().collect(),
                    )),
                );
            }
        }
    } else {
        ui.message = ui.i18n.text("msg-404-used");
    }
}
