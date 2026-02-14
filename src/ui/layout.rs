use ratatui::layout::{Constraint, Direction, Layout, Rect};

#[derive(Debug, Clone, Copy)]
pub struct LayoutInfo {
    pub area: Rect,
    pub header: Rect,
    pub body: Rect,
    pub left_panel: Rect,
    pub board: Rect,
    pub right_panel: Rect,
    pub status: Rect,
    pub log: Rect,
    pub left_inner: Rect,
    pub board_inner: Rect,
    pub right_inner: Rect,
}

pub fn compute_layout(area: Rect) -> LayoutInfo {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(18),
            Constraint::Min(40),
            Constraint::Length(18),
        ])
        .split(rows[1]);

    let left_panel = body[0];
    let board = body[1];
    let right_panel = body[2];

    let left_inner = inner_rect(left_panel);
    let board_inner = inner_rect(board);
    let right_inner = inner_rect(right_panel);

    LayoutInfo {
        area,
        header: rows[0],
        body: rows[1],
        left_panel,
        board,
        right_panel,
        status: rows[2],
        log: rows[3],
        left_inner,
        board_inner,
        right_inner,
    }
}

fn inner_rect(rect: Rect) -> Rect {
    let width = rect.width.saturating_sub(2);
    let height = rect.height.saturating_sub(2);
    Rect::new(rect.x + 1, rect.y + 1, width, height)
}
