use crate::game::board::Board;
use crate::game::player::PlayerState;
use crate::game::types::{
    GameError, GamePhase, MoveOutcome, OnlineCard, OnlineCardType, PlayerId, Position, StackChoice,
};

#[derive(Debug, Clone)]
pub struct GameState {
    pub board: Board,
    pub player1: PlayerState,
    pub player2: PlayerState,
    pub current_player: PlayerId,
    pub phase: GamePhase,
    pub pending_boost_move: Option<Position>,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            board: Board::new(),
            player1: PlayerState::new(PlayerId::P1),
            player2: PlayerState::new(PlayerId::P2),
            current_player: PlayerId::P1,
            phase: GamePhase::Setup(PlayerId::P1),
            pending_boost_move: None,
        }
    }

    pub fn player(&self, player: PlayerId) -> &PlayerState {
        match player {
            PlayerId::P1 => &self.player1,
            PlayerId::P2 => &self.player2,
        }
    }

    pub fn player_mut(&mut self, player: PlayerId) -> &mut PlayerState {
        match player {
            PlayerId::P1 => &mut self.player1,
            PlayerId::P2 => &mut self.player2,
        }
    }

    fn remove_line_boost_at(&mut self, player: PlayerId, pos: Position) {
        let player_state = self.player_mut(player);
        for slot in &mut player_state.line_boosts {
            if *slot == Some(pos) {
                *slot = None;
            }
        }
        if let Some(mut card) = self.board.get(pos) {
            if card.owner == player {
                card.line_boost_attached = false;
                self.board.set(pos, Some(card));
            }
        }
    }

    fn move_line_boost_with_card(&mut self, player: PlayerId, from: Position, to: Position) {
        let player_state = self.player_mut(player);
        for slot in &mut player_state.line_boosts {
            if *slot == Some(from) {
                *slot = Some(to);
            }
        }
    }

    fn sync_line_boost_flag(&mut self, player: PlayerId, pos: Position) {
        let attached = self
            .player(player)
            .line_boosts
            .iter()
            .any(|slot| *slot == Some(pos));
        if let Some(mut card) = self.board.get(pos) {
            if card.owner == player {
                card.line_boost_attached = attached;
                self.board.set(pos, Some(card));
            }
        }
    }

    pub fn is_exit(pos: Position) -> bool {
        (pos.row == 0 && (pos.col == 3 || pos.col == 4))
            || (pos.row == 7 && (pos.col == 3 || pos.col == 4))
    }

    pub fn exit_owner(pos: Position) -> Option<PlayerId> {
        if pos.row == 0 && (pos.col == 3 || pos.col == 4) {
            Some(PlayerId::P1)
        } else if pos.row == 7 && (pos.col == 3 || pos.col == 4) {
            Some(PlayerId::P2)
        } else {
            None
        }
    }

    pub fn can_place_setup(&self, player: PlayerId, pos: Position) -> bool {
        Board::in_bounds(pos)
            && player.setup_positions().contains(&pos)
            && self.board.get(pos).is_none()
    }

    pub fn place_setup_card(
        &mut self,
        player: PlayerId,
        pos: Position,
        card_type: OnlineCardType,
    ) -> Result<(), GameError> {
        match self.phase {
            GamePhase::Setup(active) if active == player => {}
            GamePhase::Setup(_) => return Err(GameError::SetupNotCurrentPlayer),
            _ => return Err(GameError::NotInSetupPhase),
        }

        if !self.can_place_setup(player, pos) {
            return Err(GameError::InvalidSetupPosition);
        }

        let setup_complete = {
            let player_state = self.player_mut(player);
            match card_type {
                OnlineCardType::Link if player_state.setup_links_left > 0 => {
                    player_state.setup_links_left -= 1
                }
                OnlineCardType::Virus if player_state.setup_viruses_left > 0 => {
                    player_state.setup_viruses_left -= 1
                }
                _ => return Err(GameError::SetupExhausted),
            }

            player_state.setup_placed += 1;
            player_state.setup_placed == 8
        };
        self.board.set(
            pos,
            Some(OnlineCard {
                card_type,
                revealed: false,
                line_boost_attached: false,
                owner: player,
            }),
        );

        if setup_complete {
            self.phase = match player {
                PlayerId::P1 => GamePhase::Setup(PlayerId::P2),
                PlayerId::P2 => GamePhase::Playing,
            };
        }

        Ok(())
    }

    pub fn remove_setup_card(&mut self, player: PlayerId, pos: Position) -> Result<(), GameError> {
        match self.phase {
            GamePhase::Setup(active) if active == player => {}
            GamePhase::Setup(_) => return Err(GameError::SetupNotCurrentPlayer),
            _ => return Err(GameError::NotInSetupPhase),
        }

        if !Board::in_bounds(pos) {
            return Err(GameError::OutOfBounds);
        }

        let card = self.board.get(pos).ok_or(GameError::NoCard)?;
        if card.owner != player {
            return Err(GameError::NotYourCard);
        }

        self.board.set(pos, None);
        let player_state = self.player_mut(player);
        player_state.setup_placed = player_state.setup_placed.saturating_sub(1);
        match card.card_type {
            OnlineCardType::Link => player_state.setup_links_left += 1,
            OnlineCardType::Virus => player_state.setup_viruses_left += 1,
        }

        Ok(())
    }

    pub fn start_move(&mut self, from: Position, to: Position) -> Result<MoveOutcome, GameError> {
        if self.pending_boost_move.is_some() {
            return Err(GameError::PendingBoostMove);
        }
        self.move_card(from, to, true)
    }

    pub fn continue_boost_move(
        &mut self,
        from: Position,
        to: Position,
    ) -> Result<MoveOutcome, GameError> {
        let pending = self
            .pending_boost_move
            .ok_or(GameError::NoPendingBoostMove)?;
        if pending != from {
            return Err(GameError::NoPendingBoostMove);
        }
        self.pending_boost_move = None;
        self.move_card(from, to, false)
    }

    fn move_card(
        &mut self,
        from: Position,
        to: Position,
        can_start_server_entry: bool,
    ) -> Result<MoveOutcome, GameError> {
        if !matches!(self.phase, GamePhase::Playing) {
            return Err(GameError::NotInPlayingPhase);
        }

        if !Board::in_bounds(from) || !Board::in_bounds(to) {
            return Err(GameError::OutOfBounds);
        }

        if from.manhattan_distance(to) != 1 {
            return Err(GameError::NotAdjacent);
        }

        let card = self.board.get(from).ok_or(GameError::NoCard)?;
        if card.owner != self.current_player {
            return Err(GameError::NotYourCard);
        }

        if self.board.has_own_card(to, self.current_player) {
            return Err(GameError::OccupiedByOwnCard);
        }

        if Self::exit_owner(to) == Some(self.current_player) {
            return Err(GameError::OwnExitBlocked);
        }

        if self.board.firewalls[to.row][to.col] == Some(self.current_player.opponent()) {
            return Err(GameError::OpponentFirewall);
        }

        let mut captured = None;
        if self.board.has_opponent_card(to, self.current_player) {
            let mut opponent_card = self.board.get(to).expect("checked above");
            opponent_card.revealed = true;
            captured = Some(opponent_card);
        }

        if let Some(opponent_card) = captured {
            if opponent_card.line_boost_attached {
                self.remove_line_boost_at(opponent_card.owner, to);
            }
            let player_state = self.player_mut(self.current_player);
            match opponent_card.card_type {
                OnlineCardType::Link => player_state.link_stack.push(opponent_card),
                OnlineCardType::Virus => player_state.virus_stack.push(opponent_card),
            }
        }

        if captured.is_some() {
            self.board.set(to, None);
        }

        self.board.set(from, None);
        self.board.set(to, Some(card));

        if card.line_boost_attached && captured.is_none() {
            self.move_line_boost_with_card(self.current_player, from, to);
            if !can_start_server_entry
                && Self::exit_owner(from) == Some(self.current_player.opponent())
            {
                return Ok(MoveOutcome::TurnEnds);
            }
            self.pending_boost_move = Some(to);
            return Ok(MoveOutcome::CanMoveAgain);
        }

        Ok(MoveOutcome::TurnEnds)
    }

    pub fn enter_server_center(
        &mut self,
        from: Position,
        reveal: bool,
        stack: StackChoice,
    ) -> Result<(), GameError> {
        if !matches!(self.phase, GamePhase::Playing) {
            return Err(GameError::NotInPlayingPhase);
        }
        if self.pending_boost_move.is_some() {
            return Err(GameError::CannotEnterServerWithBoost);
        }
        if !Board::in_bounds(from) {
            return Err(GameError::OutOfBounds);
        }

        let card = self.board.get(from).ok_or(GameError::NoCard)?;
        if card.owner != self.current_player {
            return Err(GameError::NotYourCard);
        }
        if Self::exit_owner(from) != Some(self.current_player.opponent()) {
            return Err(GameError::NotOnOpponentExit);
        }

        if card.line_boost_attached {
            self.remove_line_boost_at(self.current_player, from);
        }
        self.board.set(from, None);
        let mut scored = card;
        if reveal {
            scored.revealed = true;
        }
        let player_state = self.player_mut(self.current_player);
        player_state.add_to_stack(scored, stack);
        Ok(())
    }

    pub fn use_line_boost_attach(&mut self, index: usize, pos: Position) -> Result<(), GameError> {
        if !matches!(self.phase, GamePhase::Playing) {
            return Err(GameError::NotInPlayingPhase);
        }
        if index >= 2 {
            return Err(GameError::InvalidTarget);
        }
        if !Board::in_bounds(pos) {
            return Err(GameError::OutOfBounds);
        }

        let card = self.board.get(pos).ok_or(GameError::NoCard)?;
        if card.owner != self.current_player {
            return Err(GameError::NotYourCard);
        }

        let player_state = self.player_mut(self.current_player);
        player_state.line_boosts[index] = Some(pos);
        let mut updated = card;
        updated.line_boost_attached = true;
        self.board.set(pos, Some(updated));
        Ok(())
    }

    pub fn use_line_boost_detach(&mut self, index: usize) -> Result<(), GameError> {
        if !matches!(self.phase, GamePhase::Playing) {
            return Err(GameError::NotInPlayingPhase);
        }
        if index >= 2 {
            return Err(GameError::InvalidTarget);
        }
        let player_state = self.player_mut(self.current_player);
        let pos = player_state.line_boosts[index].ok_or(GameError::InvalidTarget)?;
        player_state.line_boosts[index] = None;
        if let Some(mut card) = self.board.get(pos) {
            if card.owner == self.current_player {
                card.line_boost_attached = false;
                self.board.set(pos, Some(card));
            }
        }
        Ok(())
    }

    pub fn use_virus_check(&mut self, index: usize, pos: Position) -> Result<(), GameError> {
        if !matches!(self.phase, GamePhase::Playing) {
            return Err(GameError::NotInPlayingPhase);
        }
        if index >= 2 {
            return Err(GameError::InvalidTarget);
        }
        let current_player = self.current_player;
        if self.player(current_player).virus_checks_used[index] {
            return Err(GameError::TerminalCardUsed);
        }

        if !Board::in_bounds(pos) {
            return Err(GameError::OutOfBounds);
        }
        let mut card = self.board.get(pos).ok_or(GameError::NoCard)?;
        if card.owner == current_player {
            return Err(GameError::InvalidTarget);
        }
        card.revealed = true;
        self.board.set(pos, Some(card));
        self.player_mut(current_player).virus_checks_used[index] = true;
        Ok(())
    }

    pub fn use_firewall_place(&mut self, index: usize, pos: Position) -> Result<(), GameError> {
        if !matches!(self.phase, GamePhase::Playing) {
            return Err(GameError::NotInPlayingPhase);
        }
        if index >= 2 {
            return Err(GameError::InvalidTarget);
        }
        if !Board::in_bounds(pos) {
            return Err(GameError::OutOfBounds);
        }
        if Self::is_exit(pos) {
            return Err(GameError::FirewallOnExit);
        }

        let player_state = self.player_mut(self.current_player);
        player_state.firewalls[index] = Some(pos);
        self.board.firewalls[pos.row][pos.col] = Some(self.current_player);
        Ok(())
    }

    pub fn use_firewall_remove(&mut self, index: usize) -> Result<(), GameError> {
        if !matches!(self.phase, GamePhase::Playing) {
            return Err(GameError::NotInPlayingPhase);
        }
        if index >= 2 {
            return Err(GameError::InvalidTarget);
        }
        let current_player = self.current_player;
        let pos = {
            let player_state = self.player_mut(current_player);
            let pos = player_state.firewalls[index];
            player_state.firewalls[index] = None;
            pos
        };
        if let Some(pos) = pos {
            self.board.firewalls[pos.row][pos.col] = None;
        }
        Ok(())
    }

    pub fn use_404(
        &mut self,
        index: usize,
        first: Position,
        second: Position,
        swap: bool,
    ) -> Result<(), GameError> {
        if !matches!(self.phase, GamePhase::Playing) {
            return Err(GameError::NotInPlayingPhase);
        }
        if index >= 2 {
            return Err(GameError::InvalidTarget);
        }
        let current_player = self.current_player;
        if self.player(current_player).not_found_used[index] {
            return Err(GameError::TerminalCardUsed);
        }
        let mut card_a = self.board.get(first).ok_or(GameError::NoCard)?;
        let mut card_b = self.board.get(second).ok_or(GameError::NoCard)?;
        if card_a.owner != current_player || card_b.owner != current_player {
            return Err(GameError::NotYourCard);
        }
        card_a.revealed = false;
        card_b.revealed = false;

        if swap {
            self.board.set(first, Some(card_b));
            self.board.set(second, Some(card_a));
        } else {
            self.board.set(first, Some(card_a));
            self.board.set(second, Some(card_b));
        }

        self.sync_line_boost_flag(current_player, first);
        self.sync_line_boost_flag(current_player, second);

        self.player_mut(current_player).not_found_used[index] = true;
        Ok(())
    }

    pub fn end_turn(&mut self) {
        if !matches!(self.phase, GamePhase::Playing) {
            return;
        }
        if let Some(winner) = self.check_winner() {
            self.phase = GamePhase::GameOver(winner);
            return;
        }
        self.current_player = self.current_player.opponent();
        self.pending_boost_move = None;
    }

    pub fn check_winner(&self) -> Option<PlayerId> {
        let p1 = &self.player1;
        let p2 = &self.player2;
        if p1.link_stack.len() >= 4 || p2.virus_stack.len() >= 4 {
            return Some(PlayerId::P1);
        }
        if p2.link_stack.len() >= 4 || p1.virus_stack.len() >= 4 {
            return Some(PlayerId::P2);
        }
        None
    }
}
