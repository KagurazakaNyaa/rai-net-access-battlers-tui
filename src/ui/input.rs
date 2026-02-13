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

    if key.code == KeyCode::Char('?') {
        ui.message = help_text(game, ui);
        ui.menu = None;
        return Ok(false);
    }

    if let Some(action) = key_to_menu_action(&key) {
        if let Some(menu_action) = apply_menu_action(action, game, ui)? {
            return Ok(menu_action);
        }
    }

    if ui.op_sender.is_some() && ui.local_player != game.current_player {
        ui.message = "Wait for opponent".to_string();
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

    if rect_contains(layout.board, mouse.column, mouse.row) {
        if let Some(pos) = board_position_from_mouse(mouse, layout.board) {
            ui.cursor = pos;
            ui.menu = build_cell_menu(game, ui, layout, pos);
        }
        return Ok(false);
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

fn handle_setup_keys(key: KeyEvent, game: &mut GameState, ui: &mut UiState) {
    handle_cursor_keys(key, &mut ui.cursor);
    let phase_before = game.phase;
    if let GamePhase::Setup(player) = game.phase {
        match key.code {
            KeyCode::Char('l') | KeyCode::Char('L') => {
                match game.place_setup_card(player, ui.cursor, OnlineCardType::Link) {
                    Ok(()) => {
                        ui.message = "Placed Link".to_string();
                        if let Some(sender) = &ui.op_sender {
                            let _ = sender
                                .send(format!("OP SETUP L {} {}", ui.cursor.row, ui.cursor.col));
                        }
                    }
                    Err(err) => ui.message = format!("Setup error: {:?}", err),
                }
            }
            KeyCode::Char('v') | KeyCode::Char('V') => {
                match game.place_setup_card(player, ui.cursor, OnlineCardType::Virus) {
                    Ok(()) => {
                        ui.message = "Placed Virus".to_string();
                        if let Some(sender) = &ui.op_sender {
                            let _ = sender
                                .send(format!("OP SETUP V {} {}", ui.cursor.row, ui.cursor.col));
                        }
                    }
                    Err(err) => ui.message = format!("Setup error: {:?}", err),
                }
            }
            KeyCode::Backspace => match game.remove_setup_card(player, ui.cursor) {
                Ok(()) => {
                    ui.message = "Removed card".to_string();
                    if let Some(sender) = &ui.op_sender {
                        let _ =
                            sender.send(format!("OP REMOVE {} {}", ui.cursor.row, ui.cursor.col));
                    }
                }
                Err(err) => ui.message = format!("Setup error: {:?}", err),
            },
            _ => {}
        }
    }

    if phase_before != game.phase {
        ui.mode = UiMode::TurnPass;
        ui.message = match game.phase {
            GamePhase::Setup(player) => format!(
                "Pass to {} for setup. Press Enter.",
                player_label_with_names(player, &ui.player_names)
            ),
            GamePhase::Playing => "Setup complete. Press Enter to start.".to_string(),
            GamePhase::GameOver(winner) => {
                format!(
                    "Game Over. Winner: {}",
                    player_label_with_names(winner, &ui.player_names)
                )
            }
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

    if key.code == KeyCode::Char('?') {
        ui.message = help_text(game, ui);
        ui.menu = None;
        return Ok(false);
    }

    match ui.mode {
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
                    ui.message = format!(
                        "{} setup: place 4 Link and 4 Virus on highlighted cells",
                        player_label_with_names(player, &ui.player_names)
                    );
                }
                GamePhase::Playing => {
                    ui.mode = UiMode::MoveSelect;
                    ui.message = format!(
                        "{} turn: select a card to move",
                        player_label_with_names(game.current_player, &ui.player_names)
                    );
                }
                GamePhase::GameOver(winner) => {
                    ui.mode = UiMode::GameOver;
                    ui.message = format!(
                        "Game Over. Winner: {}",
                        player_label_with_names(winner, &ui.player_names)
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
                    label: "L: Place Link".to_string(),
                    action: MenuAction::Key(KeyCode::Char('l')),
                });
                items.push(MenuItem {
                    label: "V: Place Virus".to_string(),
                    action: MenuAction::Key(KeyCode::Char('v')),
                });
            } else if game.board.cards[pos.row][pos.col].is_some() {
                items.push(MenuItem {
                    label: "Backspace: Remove".to_string(),
                    action: MenuAction::Key(KeyCode::Backspace),
                });
            }
        }
        UiMode::MoveSelect => {
            if let Some(card) = game.board.cards[pos.row][pos.col] {
                if card.owner == game.current_player {
                    items.push(MenuItem {
                        label: "Enter: Select".to_string(),
                        action: MenuAction::Key(KeyCode::Enter),
                    });
                    if crate::GameState::exit_owner(pos) == Some(game.current_player.opponent()) {
                        items.push(MenuItem {
                            label: "E: Enter Server".to_string(),
                            action: MenuAction::Key(KeyCode::Char('e')),
                        });
                    }
                }
            }
            items.push(MenuItem {
                label: "T: Terminal".to_string(),
                action: MenuAction::Key(KeyCode::Char('t')),
            });
        }
        UiMode::MoveDest { .. } => {
            items.push(MenuItem {
                label: "Enter: Move".to_string(),
                action: MenuAction::Key(KeyCode::Enter),
            });
        }
        UiMode::BoostContinue { .. } => {
            items.push(MenuItem {
                label: "Enter: Boost Move".to_string(),
                action: MenuAction::Key(KeyCode::Enter),
            });
            items.push(MenuItem {
                label: "N: End Turn".to_string(),
                action: MenuAction::Key(KeyCode::Char('n')),
            });
        }
        UiMode::LineBoost
        | UiMode::VirusCheck
        | UiMode::Firewall
        | UiMode::NotFoundFirst
        | UiMode::NotFoundSecond { .. } => {
            items.push(MenuItem {
                label: "Enter: Apply".to_string(),
                action: MenuAction::Key(KeyCode::Enter),
            });
        }
        UiMode::NotFoundSwap { .. } => {
            items.push(MenuItem {
                label: "Y: Swap".to_string(),
                action: MenuAction::Key(KeyCode::Char('y')),
            });
            items.push(MenuItem {
                label: "N: Keep".to_string(),
                action: MenuAction::Key(KeyCode::Char('n')),
            });
        }
        UiMode::ServerReveal { .. } => {
            items.push(MenuItem {
                label: "Y: Reveal".to_string(),
                action: MenuAction::Key(KeyCode::Char('y')),
            });
            items.push(MenuItem {
                label: "N: Hide".to_string(),
                action: MenuAction::Key(KeyCode::Char('n')),
            });
        }
        UiMode::ServerStack { .. } => {
            items.push(MenuItem {
                label: "L: Link Stack".to_string(),
                action: MenuAction::Key(KeyCode::Char('l')),
            });
            items.push(MenuItem {
                label: "V: Virus Stack".to_string(),
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

fn handle_move_select_keys(key: KeyEvent, game: &mut GameState, ui: &mut UiState) {
    handle_cursor_keys(key, &mut ui.cursor);
    match key.code {
        KeyCode::Enter => {
            if let Some(card) = game.board.cards[ui.cursor.row][ui.cursor.col] {
                if card.owner == game.current_player {
                    ui.mode = UiMode::MoveDest { from: ui.cursor };
                    ui.message = "Select destination".to_string();
                } else {
                    ui.message = "Not your card".to_string();
                }
            } else {
                ui.message = "No card here".to_string();
            }
        }
        KeyCode::Char('t') | KeyCode::Char('T') => {
            ui.mode = UiMode::TerminalMenu;
            ui.message = "Terminal: 1 LineBoost, 2 VirusCheck, 3 Firewall, 4 404".to_string();
        }
        KeyCode::Char('e') | KeyCode::Char('E') => {
            if let Some(card) = game.board.cards[ui.cursor.row][ui.cursor.col] {
                if card.owner == game.current_player
                    && crate::GameState::exit_owner(ui.cursor)
                        == Some(game.current_player.opponent())
                {
                    ui.mode = UiMode::ServerReveal { from: ui.cursor };
                    ui.message = "Enter server: reveal? (Y/N)".to_string();
                } else {
                    ui.message = "Not on opponent exit".to_string();
                }
            } else {
                ui.message = "No card here".to_string();
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
                    ui.message = "Boost: move again or press N to end".to_string();
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
            Err(err) => ui.message = format!("Move error: {:?}", err),
        },
        KeyCode::Esc => {
            ui.mode = UiMode::MoveSelect;
            ui.message = "Select a card".to_string();
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
            Err(err) => ui.message = format!("Boost move error: {:?}", err),
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
            ui.message = "LineBoost: select your card (Enter to attach/detach)".to_string();
        }
        KeyCode::Char('2') => {
            ui.mode = UiMode::VirusCheck;
            ui.message = "VirusCheck: select opponent unrevealed card".to_string();
        }
        KeyCode::Char('3') => {
            ui.mode = UiMode::Firewall;
            ui.message = "Firewall: select cell (Enter to place/remove)".to_string();
        }
        KeyCode::Char('4') => {
            ui.mode = UiMode::NotFoundFirst;
            ui.message = "404: select first own card".to_string();
        }
        KeyCode::Esc => {
            ui.mode = UiMode::MoveSelect;
            ui.message = "Select a card".to_string();
        }
        _ => {}
    }
}

fn handle_line_boost_keys(key: KeyEvent, game: &mut GameState, ui: &mut UiState) {
    handle_cursor_keys(key, &mut ui.cursor);
    if key.code == KeyCode::Enter {
        if let Some(card) = game.board.cards[ui.cursor.row][ui.cursor.col] {
            if card.owner != game.current_player {
                ui.message = "Not your card".to_string();
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
                        Err(err) => ui.message = format!("LineBoost error: {:?}", err),
                    }
                } else {
                    ui.message = "No line boost found".to_string();
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
                    Err(err) => ui.message = format!("LineBoost error: {:?}", err),
                }
            } else {
                ui.message = "No line boost slot free".to_string();
            }
        } else {
            ui.message = "No card here".to_string();
        }
    } else if key.code == KeyCode::Esc {
        ui.mode = UiMode::MoveSelect;
        ui.message = "Select a card".to_string();
    }
}

fn handle_virus_check_keys(key: KeyEvent, game: &mut GameState, ui: &mut UiState) {
    handle_cursor_keys(key, &mut ui.cursor);
    if key.code == KeyCode::Enter {
        if let Some(card) = game.board.cards[ui.cursor.row][ui.cursor.col] {
            if card.owner == game.current_player {
                ui.message = "Select opponent card".to_string();
                return;
            }
            if card.revealed {
                ui.message = "Card already revealed".to_string();
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
                    Err(err) => ui.message = format!("VirusCheck error: {:?}", err),
                }
            } else {
                ui.message = "VirusCheck used up".to_string();
            }
        } else {
            ui.message = "No card here".to_string();
        }
    } else if key.code == KeyCode::Esc {
        ui.mode = UiMode::MoveSelect;
        ui.message = "Select a card".to_string();
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
                    Err(err) => ui.message = format!("Firewall error: {:?}", err),
                }
            } else {
                ui.message = "No firewall found".to_string();
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
                Err(err) => ui.message = format!("Firewall error: {:?}", err),
            }
        } else {
            ui.message = "No firewall slot free".to_string();
        }
    } else if key.code == KeyCode::Esc {
        ui.mode = UiMode::MoveSelect;
        ui.message = "Select a card".to_string();
    }
}

fn handle_not_found_first_keys(key: KeyEvent, game: &mut GameState, ui: &mut UiState) {
    handle_cursor_keys(key, &mut ui.cursor);
    if key.code == KeyCode::Enter {
        if let Some(card) = game.board.cards[ui.cursor.row][ui.cursor.col] {
            if card.owner != game.current_player {
                ui.message = "Select your own card".to_string();
                return;
            }
            ui.mode = UiMode::NotFoundSecond { first: ui.cursor };
            ui.message = "404: select second own card".to_string();
        } else {
            ui.message = "No card here".to_string();
        }
    } else if key.code == KeyCode::Esc {
        ui.mode = UiMode::MoveSelect;
        ui.message = "Select a card".to_string();
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
                ui.message = "Select your own card".to_string();
                return;
            }
            ui.mode = UiMode::NotFoundSwap {
                first,
                second: ui.cursor,
            };
            ui.message = "404: swap positions? (Y/N)".to_string();
        } else {
            ui.message = "No card here".to_string();
        }
    } else if key.code == KeyCode::Esc {
        ui.mode = UiMode::MoveSelect;
        ui.message = "Select a card".to_string();
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
            ui.message = "Select a card".to_string();
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
            ui.message = "Server: place into Link (L) or Virus (V) stack".to_string();
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            ui.mode = UiMode::ServerStack {
                from,
                reveal: false,
            };
            ui.message = "Server: place into Link (L) or Virus (V) stack".to_string();
        }
        KeyCode::Esc => {
            ui.mode = UiMode::MoveSelect;
            ui.message = "Select a card".to_string();
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
                Err(err) => ui.message = format!("Server entry error: {:?}", err),
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
                Err(err) => ui.message = format!("Server entry error: {:?}", err),
            }
        }
        KeyCode::Esc => {
            ui.mode = UiMode::MoveSelect;
            ui.message = "Select a card".to_string();
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
            Err(err) => ui.message = format!("404 error: {:?}", err),
        }
    } else {
        ui.message = "404 used up".to_string();
    }
}
