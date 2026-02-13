use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::ui::layout::compute_layout;
use crate::ui::state::UiState;
use crate::ui::util::player_label_with_names;
use crate::{GamePhase, GameState, OnlineCardType, PlayerId, Position};

pub fn draw(frame: &mut ratatui::Frame, game: &GameState, ui: &UiState) {
    let layout = compute_layout(frame.area());

    let header = Paragraph::new(Line::from(vec![
        Span::styled("RaiNet AccessBattlers", Style::default().fg(Color::Cyan)),
        Span::raw(" | "),
        Span::styled(
            format!(
                "Turn: {}",
                player_label_with_names(game.current_player, &ui.player_names)
            ),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw(" | "),
        Span::raw(format!("Mode: {:?}", ui.mode)),
        Span::raw(" | Q to quit | ? help"),
    ]));
    frame.render_widget(header, layout.header);

    let p1 = stacks_panel(game, PlayerId::P1, &ui.player_names);
    let p2 = stacks_panel(game, PlayerId::P2, &ui.player_names);
    frame.render_widget(p1, layout.left_panel);
    frame.render_widget(p2, layout.right_panel);

    let board = board_panel(game, ui);
    frame.render_widget(board, layout.board);

    let status = Paragraph::new(ui.message.clone()).block(Block::default().borders(Borders::TOP));
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
            .block(Block::default().borders(Borders::ALL).title("Actions"))
            .style(Style::default().bg(Color::Black).fg(Color::White));
        frame.render_widget(Clear, menu.rect);
        frame.render_widget(menu_widget, menu.rect);
    }
}

fn stacks_panel(game: &GameState, player: PlayerId, names: &[String; 2]) -> Paragraph<'static> {
    let state = game.player(player);
    let link_count = state.link_stack.len();
    let virus_count = state.virus_stack.len();
    let line_boost = state.line_boosts.iter().filter(|s| s.is_some()).count();
    let firewall = state.firewalls.iter().filter(|s| s.is_some()).count();
    let virus_checks_used = state.virus_checks_used.iter().filter(|u| **u).count();
    let not_found_used = state.not_found_used.iter().filter(|u| **u).count();

    let lines = vec![
        Line::from(format!("{} STACK", player_label_with_names(player, names))),
        Line::from(format!("Link: {}/4", link_count)),
        Line::from(format!("Virus: {}/4", virus_count)),
        Line::from(""),
        Line::from("TERMINAL"),
        Line::from(format!("LineBoost: {}/2", line_boost)),
        Line::from(format!("VirusCheck: {}/2", 2 - virus_checks_used)),
        Line::from(format!("Firewall: {}/2", firewall)),
        Line::from(format!("404: {}/2", 2 - not_found_used)),
    ];
    Paragraph::new(lines).block(Block::default().borders(Borders::ALL))
}

fn board_panel(game: &GameState, ui: &UiState) -> Paragraph<'static> {
    let mut lines = Vec::new();
    let mut header = vec![Span::raw("   ")];
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

    Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Board"))
}

fn cell_display(game: &GameState, ui: &UiState, pos: Position) -> (String, Style) {
    if let Some(card) = game.board.cards[pos.row][pos.col] {
        let is_local = card.owner == ui.local_player;
        let symbol = if is_local {
            match card.card_type {
                OnlineCardType::Link => "L",
                OnlineCardType::Virus => "V",
            }
        } else if card.revealed {
            match card.card_type {
                OnlineCardType::Link => "l",
                OnlineCardType::Virus => "v",
            }
        } else {
            "?"
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
        return ("E".to_string(), Style::default().fg(Color::Yellow));
    }

    if let Some(owner) = game.board.firewalls[pos.row][pos.col] {
        let style = match owner {
            PlayerId::P1 => Style::default().fg(Color::LightRed),
            PlayerId::P2 => Style::default().fg(Color::LightBlue),
        };
        return ("F".to_string(), style);
    }

    (".".to_string(), Style::default())
}
