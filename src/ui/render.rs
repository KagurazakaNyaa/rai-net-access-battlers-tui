use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::ui::layout::compute_layout;
use crate::ui::state::UiState;
use crate::ui::util::player_label_with_names;
use crate::{GamePhase, GameState, OnlineCardType, PlayerId, Position};

pub fn draw(frame: &mut ratatui::Frame, game: &GameState, ui: &UiState) {
    let layout = compute_layout(frame.area());

    if matches!(
        ui.mode,
        crate::ui::state::UiMode::Lobby
            | crate::ui::state::UiMode::JoinRoomInput
            | crate::ui::state::UiMode::RoomConfirm { .. }
            | crate::ui::state::UiMode::RoomCreateDialog
    ) {
        let lobby = lobby_panel(ui);
        frame.render_widget(lobby, layout.body);
        if matches!(ui.mode, crate::ui::state::UiMode::RoomConfirm { .. }) {
            let dialog = confirm_panel(layout.area);
            frame.render_widget(Clear, dialog);
            frame.render_widget(confirm_panel_content(ui), dialog);
        }
        if matches!(ui.mode, crate::ui::state::UiMode::RoomCreateDialog) {
            let dialog = confirm_panel(layout.area);
            frame.render_widget(Clear, dialog);
            frame.render_widget(create_panel_content(ui), dialog);
        }
        return;
    }

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            ui.i18n.text("header-title"),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw(" | "),
        Span::styled(
            ui.i18n.text_args(
                "header-turn",
                Some(crate::i18n::args_from_map(
                    [(
                        "player",
                        player_label_with_names(&ui.i18n, game.current_player, &ui.player_names),
                    )]
                    .into_iter()
                    .collect(),
                )),
            ),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw(" | "),
        Span::raw(ui.i18n.text_args(
            "header-mode",
            Some(crate::i18n::args_from_map(
                [("mode", format!("{:?}", ui.mode))].into_iter().collect(),
            )),
        )),
        Span::raw(" | "),
        Span::raw(ui.i18n.text_args(
            "header-id",
            Some(crate::i18n::args_from_map(
                [("id", ui.client_id.clone())].into_iter().collect(),
            )),
        )),
        Span::raw(" | "),
        Span::raw(ui.i18n.text("header-help")),
    ]));
    frame.render_widget(header, layout.header);

    let p1 = stacks_panel(game, ui, PlayerId::P1, &ui.player_names);
    let p2 = stacks_panel(game, ui, PlayerId::P2, &ui.player_names);
    frame.render_widget(p1, layout.left_panel);
    frame.render_widget(p2, layout.right_panel);

    let board = board_panel(game, ui);
    frame.render_widget(board, layout.board);

    let mut status_lines = vec![Line::from(ui.message.clone())];
    if !ui.room_players.is_empty() {
        status_lines.push(Line::from(
            ui.i18n.text_args(
                "status-players",
                Some(crate::i18n::args_from_map(
                    [("players", ui.room_players.join(", "))]
                        .into_iter()
                        .collect(),
                )),
            ),
        ));
    }
    if !ui.room_spectators.is_empty() {
        status_lines.push(Line::from(
            ui.i18n.text_args(
                "status-spectators",
                Some(crate::i18n::args_from_map(
                    [("spectators", ui.room_spectators.join(", "))]
                        .into_iter()
                        .collect(),
                )),
            ),
        ));
    }
    let leave_line = format!(
        "{:>width$}",
        ui.i18n.text("status-leave-button"),
        width = layout.status.width.saturating_sub(2) as usize
    );
    status_lines.push(Line::from(leave_line));
    let status = Paragraph::new(status_lines).block(Block::default().borders(Borders::TOP));
    frame.render_widget(status, layout.status);

    let log_text = ui
        .log
        .iter()
        .rev()
        .take(2)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ");
    let log = Paragraph::new(log_text).block(Block::default().borders(Borders::TOP));
    frame.render_widget(log, layout.log);

    if let Some(menu) = &ui.menu {
        let lines = menu
            .items
            .iter()
            .map(|item| Line::from(item.label.clone()))
            .collect::<Vec<_>>();
        let menu_widget = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(ui.i18n.text("menu-actions")),
            )
            .style(Style::default().bg(Color::Black).fg(Color::White));
        frame.render_widget(Clear, menu.rect);
        frame.render_widget(menu_widget, menu.rect);
    }
}

fn stacks_panel(
    game: &GameState,
    ui: &UiState,
    player: PlayerId,
    names: &[String; 2],
) -> Paragraph<'static> {
    let state = game.player(player);
    let link_count = state.link_stack.len();
    let virus_count = state.virus_stack.len();
    let line_boost = state.line_boosts.iter().filter(|s| s.is_some()).count();
    let firewall = state.firewalls.iter().filter(|s| s.is_some()).count();
    let virus_checks_used = state.virus_checks_used.iter().filter(|u| **u).count();
    let not_found_used = state.not_found_used.iter().filter(|u| **u).count();

    let lines = vec![
        Line::from(
            ui.i18n.text_args(
                "stack-title",
                Some(crate::i18n::args_from_map(
                    [("player", player_label_with_names(&ui.i18n, player, names))]
                        .into_iter()
                        .collect(),
                )),
            ),
        ),
        Line::from(ui.i18n.text_args(
            "stack-link",
            Some(crate::i18n::args_from_map(
                [("count", link_count.to_string())].into_iter().collect(),
            )),
        )),
        Line::from(ui.i18n.text_args(
            "stack-virus",
            Some(crate::i18n::args_from_map(
                [("count", virus_count.to_string())].into_iter().collect(),
            )),
        )),
        Line::from(""),
        Line::from(ui.i18n.text("stack-terminal")),
        Line::from(ui.i18n.text_args(
            "stack-lineboost",
            Some(crate::i18n::args_from_map(
                [("count", line_boost.to_string())].into_iter().collect(),
            )),
        )),
        Line::from(
            ui.i18n.text_args(
                "stack-viruscheck",
                Some(crate::i18n::args_from_map(
                    [("count", (2 - virus_checks_used).to_string())]
                        .into_iter()
                        .collect(),
                )),
            ),
        ),
        Line::from(ui.i18n.text_args(
            "stack-firewall",
            Some(crate::i18n::args_from_map(
                [("count", firewall.to_string())].into_iter().collect(),
            )),
        )),
        Line::from(
            ui.i18n.text_args(
                "stack-notfound",
                Some(crate::i18n::args_from_map(
                    [("count", (2 - not_found_used).to_string())]
                        .into_iter()
                        .collect(),
                )),
            ),
        ),
    ];
    Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(ui.i18n.text("board-title")),
    )
}

fn board_panel(game: &GameState, ui: &UiState) -> Paragraph<'static> {
    let mut lines = Vec::new();
    let mut header = vec![Span::raw(ui.i18n.text("board-corner"))];
    for col in 0..8 {
        header.push(Span::raw(format!(" {} ", col)));
    }
    lines.push(Line::from(header));

    let setup_positions = match game.phase {
        GamePhase::Setup(player) => Some(player.setup_positions()),
        _ => None,
    };

    for row in 0..8 {
        let mut spans = vec![Span::raw(format!(" {} ", row))];
        for col in 0..8 {
            let pos = Position::new(row, col);
            let (label, style) = cell_display(game, ui, pos);
            let mut cell_style = style;
            if game.board.cards[pos.row][pos.col].is_none() {
                if let Some(positions) = setup_positions {
                    if positions.contains(&pos) {
                        cell_style = cell_style.bg(Color::Green);
                    }
                }
            }
            if ui.cursor == pos {
                cell_style = cell_style.bg(Color::DarkGray).add_modifier(Modifier::BOLD);
            }
            spans.push(Span::styled(format!(" {} ", label), cell_style));
        }
        lines.push(Line::from(spans));
    }

    Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(ui.i18n.text("board-title")),
    )
}

fn cell_display(game: &GameState, ui: &UiState, pos: Position) -> (String, Style) {
    if let Some(card) = game.board.cards[pos.row][pos.col] {
        let is_local = card.owner == ui.local_player;
        let symbol = if is_local {
            match card.card_type {
                OnlineCardType::Link => &ui.i18n.text("card-link"),
                OnlineCardType::Virus => &ui.i18n.text("card-virus"),
            }
        } else if card.revealed {
            match card.card_type {
                OnlineCardType::Link => &ui.i18n.text("card-link-hidden"),
                OnlineCardType::Virus => &ui.i18n.text("card-virus-hidden"),
            }
        } else {
            &ui.i18n.text("card-unknown")
        };

        let style = match card.owner {
            PlayerId::P1 => Style::default().fg(Color::Red),
            PlayerId::P2 => Style::default().fg(Color::Blue),
        };

        let label = if card.line_boost_attached {
            format!("{}*", symbol)
        } else {
            symbol.to_string()
        };
        return (label, style);
    }

    if crate::GameState::is_exit(pos) {
        return (
            ui.i18n.text("cell-exit"),
            Style::default().fg(Color::Yellow),
        );
    }

    if let Some(owner) = game.board.firewalls[pos.row][pos.col] {
        let style = match owner {
            PlayerId::P1 => Style::default().fg(Color::LightRed),
            PlayerId::P2 => Style::default().fg(Color::LightBlue),
        };
        return (ui.i18n.text("cell-firewall"), style);
    }

    (ui.i18n.text("cell-empty"), Style::default())
}

fn lobby_panel(ui: &UiState) -> Paragraph<'static> {
    let mut lines = Vec::new();
    lines.push(Line::from(ui.i18n.text("lobby-help")));
    lines.push(Line::from(""));
    for (idx, room) in ui.rooms.iter().enumerate() {
        let status = match room.status {
            crate::net::protocol::RoomStatus::Waiting => ui.i18n.text("lobby-status-wait"),
            crate::net::protocol::RoomStatus::Playing => ui.i18n.text("lobby-status-play"),
        };
        let auto = if room.auto_join {
            ui.i18n.text("lobby-auto")
        } else {
            "".to_string()
        };
        let line = ui.i18n.text_args(
            "lobby-room-line",
            Some(crate::i18n::args_from_map(
                [
                    (
                        "marker",
                        if idx == ui.selected_room { ">" } else { " " }.to_string(),
                    ),
                    ("id", room.id.clone().unwrap_or_else(|| "".to_string())),
                    ("name", room.name.clone()),
                    ("players", room.player_count.to_string()),
                    ("spectators", room.spectator_count.to_string()),
                    ("status", status),
                    ("auto", auto),
                ]
                .into_iter()
                .collect(),
            )),
        );
        lines.push(Line::from(line));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(ui.i18n.text_args(
        "lobby-input",
        Some(crate::i18n::args_from_map(
            [("input", ui.room_input.clone())].into_iter().collect(),
        )),
    )));
    lines.push(Line::from(ui.i18n.text_args(
        "lobby-id-input",
        Some(crate::i18n::args_from_map(
            [("input", ui.room_id_input.clone())].into_iter().collect(),
        )),
    )));
    lines.push(Line::from(
        ui.i18n.text_args(
            "lobby-create-options",
            Some(crate::i18n::args_from_map(
                [
                    (
                        "auto",
                        ui.i18n.text(if ui.auto_join {
                            "msg-auto-on"
                        } else {
                            "msg-auto-off"
                        }),
                    ),
                    (
                        "show",
                        ui.i18n.text(if ui.show_room_id {
                            "msg-show-on"
                        } else {
                            "msg-show-off"
                        }),
                    ),
                ]
                .into_iter()
                .collect(),
            )),
        ),
    ));
    Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(ui.i18n.text("lobby-title")),
    )
}

fn confirm_panel(area: ratatui::layout::Rect) -> ratatui::layout::Rect {
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

fn confirm_panel_content(ui: &UiState) -> Paragraph<'static> {
    let lines = vec![
        Line::from(ui.confirm_message.clone()),
        Line::from(""),
        Line::from(ui.i18n.text("confirm-buttons")),
    ];
    Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(ui.i18n.text("confirm-title")),
    )
}

fn create_panel_content(ui: &UiState) -> Paragraph<'static> {
    let focus = ui.create_focus;
    let name = if matches!(focus, crate::ui::state::CreateFocus::Name) {
        format!("> {}", ui.room_input)
    } else {
        ui.room_input.clone()
    };
    let id = if matches!(focus, crate::ui::state::CreateFocus::Id) {
        format!("> {}", ui.room_id_input)
    } else {
        ui.room_id_input.clone()
    };
    let auto = if ui.auto_join {
        ui.i18n.text("msg-auto-on")
    } else {
        ui.i18n.text("msg-auto-off")
    };
    let show = if ui.show_room_id {
        ui.i18n.text("msg-show-on")
    } else {
        ui.i18n.text("msg-show-off")
    };
    let auto_line = if matches!(focus, crate::ui::state::CreateFocus::AutoJoin) {
        format!("> {}: {}", ui.i18n.text("create-auto"), auto)
    } else {
        format!("{}: {}", ui.i18n.text("create-auto"), auto)
    };
    let show_line = if matches!(focus, crate::ui::state::CreateFocus::ShowId) {
        format!("> {}: {}", ui.i18n.text("create-show"), show)
    } else {
        format!("{}: {}", ui.i18n.text("create-show"), show)
    };
    let confirm = if matches!(focus, crate::ui::state::CreateFocus::Confirm) {
        format!("> {}", ui.i18n.text("confirm-yes"))
    } else {
        ui.i18n.text("confirm-yes")
    };
    let cancel = if matches!(focus, crate::ui::state::CreateFocus::Cancel) {
        format!("> {}", ui.i18n.text("confirm-no"))
    } else {
        ui.i18n.text("confirm-no")
    };
    let lines = vec![
        Line::from(ui.i18n.text("create-title")),
        Line::from(""),
        Line::from(format!("{} {}", ui.i18n.text("create-name"), name)),
        Line::from(format!("{} {}", ui.i18n.text("create-id"), id)),
        Line::from(auto_line),
        Line::from(show_line),
        Line::from(""),
        Line::from(format!("{} / {}", confirm, cancel)),
        Line::from(ui.i18n.text("create-hint")),
    ];
    Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(ui.i18n.text("create-title")),
    )
}
