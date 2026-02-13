use crate::{
    GamePhase, GameState, OnlineCard, OnlineCardType, PlayerId, PlayerState, Position, StackChoice,
};

#[derive(Debug, Clone)]
pub enum Op {
    Setup {
        card: OnlineCardType,
        row: usize,
        col: usize,
    },
    Remove {
        row: usize,
        col: usize,
    },
    Move {
        from: Position,
        to: Position,
    },
    Boost {
        from: Position,
        to: Position,
    },
    Enter {
        from: Position,
        reveal: bool,
        stack: StackChoice,
    },
    LineBoostAttach {
        pos: Position,
    },
    LineBoostDetach {
        pos: Position,
    },
    VirusCheck {
        pos: Position,
    },
    FirewallPlace {
        pos: Position,
    },
    FirewallRemove {
        pos: Position,
    },
    NotFound {
        first: Position,
        second: Position,
        swap: bool,
    },
    EndTurn,
}

pub fn parse_op(line: &str) -> Option<Op> {
    let mut parts = line.trim().split_whitespace();
    match parts.next()? {
        "OP" => match parts.next()? {
            "SETUP" => {
                let card = match parts.next()? {
                    "L" => OnlineCardType::Link,
                    "V" => OnlineCardType::Virus,
                    _ => return None,
                };
                let row = parts.next()?.parse().ok()?;
                let col = parts.next()?.parse().ok()?;
                Some(Op::Setup { card, row, col })
            }
            "REMOVE" => {
                let row = parts.next()?.parse().ok()?;
                let col = parts.next()?.parse().ok()?;
                Some(Op::Remove { row, col })
            }
            "MOVE" => {
                let from = parse_pos(&mut parts)?;
                let to = parse_pos(&mut parts)?;
                Some(Op::Move { from, to })
            }
            "BOOST" => {
                let from = parse_pos(&mut parts)?;
                let to = parse_pos(&mut parts)?;
                Some(Op::Boost { from, to })
            }
            "ENTER" => {
                let from = parse_pos(&mut parts)?;
                let reveal = parse_bool(parts.next()?)?;
                let stack = match parts.next()? {
                    "L" => StackChoice::Link,
                    "V" => StackChoice::Virus,
                    _ => return None,
                };
                Some(Op::Enter {
                    from,
                    reveal,
                    stack,
                })
            }
            "LINEBOOST" => match parts.next()? {
                "ATTACH" => Some(Op::LineBoostAttach {
                    pos: parse_pos(&mut parts)?,
                }),
                "DETACH" => Some(Op::LineBoostDetach {
                    pos: parse_pos(&mut parts)?,
                }),
                _ => None,
            },
            "VIRUSCHECK" => Some(Op::VirusCheck {
                pos: parse_pos(&mut parts)?,
            }),
            "FIREWALL" => match parts.next()? {
                "PLACE" => Some(Op::FirewallPlace {
                    pos: parse_pos(&mut parts)?,
                }),
                "REMOVE" => Some(Op::FirewallRemove {
                    pos: parse_pos(&mut parts)?,
                }),
                _ => None,
            },
            "NOTFOUND" => {
                let first = parse_pos(&mut parts)?;
                let second = parse_pos(&mut parts)?;
                let swap = parse_bool(parts.next()?)?;
                Some(Op::NotFound {
                    first,
                    second,
                    swap,
                })
            }
            "ENDTURN" => Some(Op::EndTurn),
            _ => None,
        },
        _ => None,
    }
}

pub fn encode_state(game: &GameState, names: &[String; 2]) -> String {
    let mut out = String::new();
    push_line(&mut out, "STATE_BEGIN");
    match game.phase {
        GamePhase::Setup(player) => {
            push_line(&mut out, &format!("PHASE SETUP {}", player_id(player)))
        }
        GamePhase::Playing => push_line(&mut out, "PHASE PLAYING"),
        GamePhase::GameOver(winner) => {
            push_line(&mut out, &format!("PHASE GAMEOVER {}", player_id(winner)))
        }
    }
    push_line(
        &mut out,
        &format!("CURRENT {}", player_id(game.current_player)),
    );
    if let Some(pos) = game.pending_boost_move {
        push_line(&mut out, &format!("PENDING {} {}", pos.row, pos.col));
    } else {
        push_line(&mut out, "PENDING NONE");
    }
    write_player_state(&mut out, PlayerId::P1, &game.player1);
    write_player_state(&mut out, PlayerId::P2, &game.player2);
    push_line(
        &mut out,
        &format!(
            "STACKS P1 LINK {} VIRUS {}",
            game.player1.link_stack.len(),
            game.player1.virus_stack.len()
        ),
    );
    push_line(
        &mut out,
        &format!(
            "STACKS P2 LINK {} VIRUS {}",
            game.player2.link_stack.len(),
            game.player2.virus_stack.len()
        ),
    );

    let mut cards = Vec::new();
    for row in 0..8 {
        for col in 0..8 {
            if let Some(card) = game.board.cards[row][col] {
                cards.push((row, col, card));
            }
        }
    }
    push_line(&mut out, &format!("CARDS {}", cards.len()));
    for (row, col, card) in cards {
        push_line(
            &mut out,
            &format!(
                "CARD {} {} {} {} {} {}",
                row,
                col,
                player_id(card.owner),
                card_type(card.card_type),
                bool_num(card.revealed),
                bool_num(card.line_boost_attached)
            ),
        );
    }

    let mut firewalls = Vec::new();
    for row in 0..8 {
        for col in 0..8 {
            if let Some(owner) = game.board.firewalls[row][col] {
                firewalls.push((row, col, owner));
            }
        }
    }
    push_line(&mut out, &format!("FIREWALLS {}", firewalls.len()));
    for (row, col, owner) in firewalls {
        push_line(
            &mut out,
            &format!("FW {} {} {}", row, col, player_id(owner)),
        );
    }

    push_line(&mut out, &format!("NAMES {} {}", names[0], names[1]));
    push_line(&mut out, "STATE_END");
    out
}

pub fn parse_state(lines: &[String]) -> Option<(GameState, [String; 2])> {
    let mut game = GameState::new();
    let mut names = ["P1".to_string(), "P2".to_string()];
    game.board.cards = [[None; 8]; 8];
    game.board.firewalls = [[None; 8]; 8];

    let mut iter = lines.iter();
    while let Some(line) = iter.next() {
        let mut parts = line.split_whitespace();
        match parts.next()? {
            "PHASE" => match parts.next()? {
                "SETUP" => {
                    let player = parse_player(parts.next()?)?;
                    game.phase = GamePhase::Setup(player);
                }
                "PLAYING" => game.phase = GamePhase::Playing,
                "GAMEOVER" => {
                    let winner = parse_player(parts.next()?)?;
                    game.phase = GamePhase::GameOver(winner);
                }
                _ => return None,
            },
            "CURRENT" => {
                game.current_player = parse_player(parts.next()?)?;
            }
            "PENDING" => {
                let token = parts.next()?;
                game.pending_boost_move = if token == "NONE" {
                    None
                } else {
                    let row = token.parse().ok()?;
                    let col = parts.next()?.parse().ok()?;
                    Some(Position::new(row, col))
                };
            }
            "PLAYER" => {
                let player = parse_player(parts.next()?)?;
                let key = parts.next()?;
                match key {
                    "SETUP_LINKS" => {
                        let links = parts.next()?.parse().ok()?;
                        parts.next()?;
                        let viruses = parts.next()?.parse().ok()?;
                        parts.next()?;
                        let placed = parts.next()?.parse().ok()?;
                        let state = match player {
                            PlayerId::P1 => &mut game.player1,
                            PlayerId::P2 => &mut game.player2,
                        };
                        state.setup_links_left = links;
                        state.setup_viruses_left = viruses;
                        state.setup_placed = placed;
                    }
                    "LINEBOOST" => {
                        let pos1 = parse_optional_pos(parts.next()?)?;
                        let pos2 = parse_optional_pos(parts.next()?)?;
                        let state = match player {
                            PlayerId::P1 => &mut game.player1,
                            PlayerId::P2 => &mut game.player2,
                        };
                        state.line_boosts = [pos1, pos2];
                    }
                    "FIREWALL" => {
                        let pos1 = parse_optional_pos(parts.next()?)?;
                        let pos2 = parse_optional_pos(parts.next()?)?;
                        let state = match player {
                            PlayerId::P1 => &mut game.player1,
                            PlayerId::P2 => &mut game.player2,
                        };
                        state.firewalls = [pos1, pos2];
                    }
                    "VIRUSCHECK" => {
                        let v1 = parse_bool(parts.next()?)?;
                        let v2 = parse_bool(parts.next()?)?;
                        let state = match player {
                            PlayerId::P1 => &mut game.player1,
                            PlayerId::P2 => &mut game.player2,
                        };
                        state.virus_checks_used = [v1, v2];
                    }
                    "NOTFOUND" => {
                        let v1 = parse_bool(parts.next()?)?;
                        let v2 = parse_bool(parts.next()?)?;
                        let state = match player {
                            PlayerId::P1 => &mut game.player1,
                            PlayerId::P2 => &mut game.player2,
                        };
                        state.not_found_used = [v1, v2];
                    }
                    _ => return None,
                }
            }
            "STACKS" => {
                let player = parse_player(parts.next()?)?;
                parts.next()?;
                let link_count = parts.next()?.parse().ok()?;
                parts.next()?;
                let virus_count = parts.next()?.parse().ok()?;
                let state = match player {
                    PlayerId::P1 => &mut game.player1,
                    PlayerId::P2 => &mut game.player2,
                };
                state.link_stack = build_stack(player, OnlineCardType::Link, link_count);
                state.virus_stack = build_stack(player, OnlineCardType::Virus, virus_count);
            }
            "CARDS" => {
                let count: usize = parts.next()?.parse().ok()?;
                for _ in 0..count {
                    let line = iter.next()?;
                    let mut parts = line.split_whitespace();
                    if parts.next()? != "CARD" {
                        return None;
                    }
                    let row: usize = parts.next()?.parse().ok()?;
                    let col: usize = parts.next()?.parse().ok()?;
                    let owner = parse_player(parts.next()?)?;
                    let card_type = parse_card_type(parts.next()?)?;
                    let revealed = parse_bool(parts.next()?)?;
                    let boost = parse_bool(parts.next()?)?;
                    game.board.cards[row][col] = Some(OnlineCard {
                        card_type,
                        revealed,
                        line_boost_attached: boost,
                        owner,
                    });
                }
            }
            "FIREWALLS" => {
                let count: usize = parts.next()?.parse().ok()?;
                for _ in 0..count {
                    let line = iter.next()?;
                    let mut parts = line.split_whitespace();
                    if parts.next()? != "FW" {
                        return None;
                    }
                    let row: usize = parts.next()?.parse().ok()?;
                    let col: usize = parts.next()?.parse().ok()?;
                    let owner = parse_player(parts.next()?)?;
                    game.board.firewalls[row][col] = Some(owner);
                }
            }
            "NAMES" => {
                names[0] = parts.next()?.to_string();
                names[1] = parts.next()?.to_string();
            }
            _ => {}
        }
    }

    Some((game, names))
}

fn write_player_state(out: &mut String, player: PlayerId, state: &PlayerState) {
    push_line(
        out,
        &format!(
            "PLAYER {} SETUP_LINKS {} SETUP_VIRUSES {} SETUP_PLACED {}",
            player_id(player),
            state.setup_links_left,
            state.setup_viruses_left,
            state.setup_placed
        ),
    );
    push_line(
        out,
        &format!(
            "PLAYER {} LINEBOOST {} {}",
            player_id(player),
            format_pos(state.line_boosts[0]),
            format_pos(state.line_boosts[1])
        ),
    );
    push_line(
        out,
        &format!(
            "PLAYER {} FIREWALL {} {}",
            player_id(player),
            format_pos(state.firewalls[0]),
            format_pos(state.firewalls[1])
        ),
    );
    push_line(
        out,
        &format!(
            "PLAYER {} VIRUSCHECK {} {}",
            player_id(player),
            bool_num(state.virus_checks_used[0]),
            bool_num(state.virus_checks_used[1])
        ),
    );
    push_line(
        out,
        &format!(
            "PLAYER {} NOTFOUND {} {}",
            player_id(player),
            bool_num(state.not_found_used[0]),
            bool_num(state.not_found_used[1])
        ),
    );
}

fn push_line(out: &mut String, line: &str) {
    out.push_str(line);
    out.push('\n');
}

fn format_pos(pos: Option<Position>) -> String {
    match pos {
        Some(pos) => format!("{},{}", pos.row, pos.col),
        None => "-".to_string(),
    }
}

fn parse_optional_pos(token: &str) -> Option<Option<Position>> {
    if token == "-" {
        return Some(None);
    }
    let mut parts = token.split(',');
    let row = parts.next()?.parse().ok()?;
    let col = parts.next()?.parse().ok()?;
    Some(Some(Position::new(row, col)))
}

fn parse_pos(parts: &mut std::str::SplitWhitespace<'_>) -> Option<Position> {
    let row = parts.next()?.parse().ok()?;
    let col = parts.next()?.parse().ok()?;
    Some(Position::new(row, col))
}

fn parse_player(token: &str) -> Option<PlayerId> {
    match token {
        "P1" => Some(PlayerId::P1),
        "P2" => Some(PlayerId::P2),
        _ => None,
    }
}

fn parse_card_type(token: &str) -> Option<OnlineCardType> {
    match token {
        "L" => Some(OnlineCardType::Link),
        "V" => Some(OnlineCardType::Virus),
        _ => None,
    }
}

fn player_id(player: PlayerId) -> &'static str {
    match player {
        PlayerId::P1 => "P1",
        PlayerId::P2 => "P2",
    }
}

fn card_type(card: OnlineCardType) -> &'static str {
    match card {
        OnlineCardType::Link => "L",
        OnlineCardType::Virus => "V",
    }
}

fn bool_num(value: bool) -> u8 {
    if value {
        1
    } else {
        0
    }
}

fn parse_bool(token: &str) -> Option<bool> {
    match token {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }
}

fn build_stack(player: PlayerId, card_type: OnlineCardType, count: usize) -> Vec<OnlineCard> {
    (0..count)
        .map(|_| OnlineCard {
            card_type,
            revealed: true,
            line_boost_attached: false,
            owner: player,
        })
        .collect()
}
