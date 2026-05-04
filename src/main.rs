//! 10×30 보드 테트리스 — rltk(crossterm) 기반 TUI

use rltk::prelude::*;

const BOARD_W: usize = 10;
const BOARD_H: usize = 30;
const CELL_W: usize = 2;
const BOARD_PX_W: i32 = (BOARD_W * CELL_W) as i32 + 2;
const BOARD_PX_H: i32 = BOARD_H as i32 + 2;

/// 보드 오른쪽 간격 + 우측 패널(문자 열)
const PANEL_COLS: i32 = 26;
/// 보드 프레임 + 패널까지 가로 너비
const CLUSTER_W: i32 = BOARD_PX_W + 2 + PANEL_COLS;
/// 프레임 상단 ~ 점수 줄까지 세로 높이
const CLUSTER_H: i32 = BOARD_PX_H + 2;
/// 최소 콘솔 크기 (여백 두고 가운데 정렬)
const MIN_VIEW_W: u32 = 80;
const MIN_VIEW_H: u32 = 48;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
}

impl Kind {
    fn random(rng: &mut RandomNumberGenerator) -> Self {
        match rng.roll_dice(1, 7) {
            1 => Kind::I,
            2 => Kind::O,
            3 => Kind::T,
            4 => Kind::S,
            5 => Kind::Z,
            6 => Kind::J,
            _ => Kind::L,
        }
    }

    fn color(self) -> (u8, u8, u8) {
        match self {
            Kind::I => (80, 220, 250),
            Kind::O => (240, 220, 60),
            Kind::T => (180, 80, 200),
            Kind::S => (80, 200, 100),
            Kind::Z => (220, 80, 80),
            Kind::J => (80, 100, 220),
            Kind::L => (220, 140, 60),
        }
    }

    /// (dx, dy) 보드 칸 기준, 회전 인덱스 0..4
    fn cells(self, rot: usize) -> &'static [(i32, i32)] {
        match self {
            Kind::I => match rot % 4 {
                0 => &[(0, 1), (1, 1), (2, 1), (3, 1)],
                1 => &[(2, 0), (2, 1), (2, 2), (2, 3)],
                2 => &[(0, 2), (1, 2), (2, 2), (3, 2)],
                _ => &[(1, 0), (1, 1), (1, 2), (1, 3)],
            },
            Kind::O => &[(0, 0), (1, 0), (0, 1), (1, 1)],
            Kind::T => match rot % 4 {
                0 => &[(1, 0), (0, 1), (1, 1), (2, 1)],
                1 => &[(1, 0), (1, 1), (2, 1), (1, 2)],
                2 => &[(0, 1), (1, 1), (2, 1), (1, 2)],
                _ => &[(1, 0), (0, 1), (1, 1), (1, 2)],
            },
            Kind::S => match rot % 4 {
                0 | 2 => &[(1, 0), (2, 0), (0, 1), (1, 1)],
                _ => &[(1, 0), (1, 1), (2, 1), (2, 2)],
            },
            Kind::Z => match rot % 4 {
                0 | 2 => &[(0, 0), (1, 0), (1, 1), (2, 1)],
                _ => &[(2, 0), (1, 1), (2, 1), (1, 2)],
            },
            Kind::J => match rot % 4 {
                0 => &[(0, 0), (0, 1), (1, 1), (2, 1)],
                1 => &[(1, 0), (2, 0), (1, 1), (1, 2)],
                2 => &[(0, 1), (1, 1), (2, 1), (2, 2)],
                _ => &[(1, 0), (1, 1), (0, 2), (1, 2)],
            },
            Kind::L => match rot % 4 {
                0 => &[(2, 0), (0, 1), (1, 1), (2, 1)],
                1 => &[(1, 0), (1, 1), (1, 2), (2, 2)],
                2 => &[(0, 1), (1, 1), (2, 1), (0, 2)],
                _ => &[(0, 0), (1, 0), (1, 1), (1, 2)],
            },
        }
    }
}

struct Active {
    kind: Kind,
    rot: usize,
    /// 좌상단 기준 앵커 보드 좌표
    x: i32,
    y: i32,
}

impl Active {
    /// 보드 안에 있는 칸만 (고정·그리기용). 벽 밖 칸은 제외한다.
    fn occupied(&self) -> Vec<(usize, usize)> {
        let w = BOARD_W as i32;
        let h = BOARD_H as i32;
        self.kind
            .cells(self.rot)
            .iter()
            .map(|(dx, dy)| (self.x + dx, self.y + dy))
            .filter(|&(x, y)| x >= 0 && x < w && y >= 0 && y < h)
            .map(|(x, y)| (x as usize, y as usize))
            .collect()
    }
}

fn valid_placement(board: &[[Option<Kind>; BOARD_W]; BOARD_H], piece: &Active) -> bool {
    let w = BOARD_W as i32;
    let h = BOARD_H as i32;
    // 네 칸 모두 검사해야 함. occupied()처럼 벽 밖 칸을 건너뛰면 좌우 이탈이 통과되는 버그가 난다.
    for &(dx, dy) in piece.kind.cells(piece.rot) {
        let x = piece.x + dx;
        let y = piece.y + dy;
        if x < 0 || x >= w || y < 0 || y >= h {
            return false;
        }
        if board[y as usize][x as usize].is_some() {
            return false;
        }
    }
    true
}

fn merge(board: &mut [[Option<Kind>; BOARD_W]; BOARD_H], piece: &Active) {
    for &(x, y) in &piece.occupied() {
        if y < BOARD_H && x < BOARD_W {
            board[y][x] = Some(piece.kind);
        }
    }
}

fn clear_lines(board: &mut [[Option<Kind>; BOARD_W]; BOARD_H]) -> usize {
    let mut cleared = 0;
    let mut y = BOARD_H;
    while y > 0 {
        y -= 1;
        if (0..BOARD_W).all(|x| board[y][x].is_some()) {
            cleared += 1;
            for yy in (1..=y).rev() {
                for x in 0..BOARD_W {
                    board[yy][x] = board[yy - 1][x];
                }
            }
            for x in 0..BOARD_W {
                board[0][x] = None;
            }
            y += 1;
        }
    }
    cleared
}

fn draw_board(
    ctx: &mut BTerm,
    board: &[[Option<Kind>; BOARD_W]; BOARD_H],
    origin_x: i32,
    origin_y: i32,
) {
    for y in 0..BOARD_H {
        for x in 0..BOARD_W {
            let px = origin_x + 1 + (x * CELL_W) as i32;
            let py = origin_y + 1 + y as i32;
            if let Some(k) = board[y][x] {
                let (r, g, b) = k.color();
                ctx.set_bg(px, py, RGB::named(rltk::BLACK));
                ctx.set(px, py, RGB::from_u8(r, g, b), RGB::from_u8(r, g, b), to_cp437(' '));
                ctx.set(px + 1, py, RGB::from_u8(r, g, b), RGB::from_u8(r, g, b), to_cp437(' '));
            } else {
                ctx.set_bg(px, py, RGB::named(rltk::BLACK));
                ctx.set_bg(px + 1, py, RGB::named(rltk::BLACK));
            }
        }
    }
}

fn draw_piece_at(
    ctx: &mut BTerm,
    kind: Kind,
    rot: usize,
    anchor_x: i32,
    anchor_y: i32,
    ghost: bool,
    origin_x: i32,
    origin_y: i32,
) {
    let (r, g, b) = kind.color();
    let fg = if ghost {
        RGB::from_f32(r as f32 / 512.0, g as f32 / 512.0, b as f32 / 512.0)
    } else {
        RGB::from_u8(r, g, b)
    };
    for &(dx, dy) in kind.cells(rot) {
        let bx = anchor_x + dx;
        let by = anchor_y + dy;
        if bx < 0
            || by < 0
            || bx >= BOARD_W as i32
            || by >= BOARD_H as i32
        {
            continue;
        }
        let px = origin_x + 1 + (bx as usize * CELL_W) as i32;
        let py = origin_y + 1 + by;
        ctx.set(px, py, fg, fg, to_cp437(' '));
        ctx.set(px + 1, py, fg, fg, to_cp437(' '));
    }
}

fn draw_next_preview(ctx: &mut BTerm, kind: Kind, base_x: i32, base_y: i32) {
    let (r, g, b) = kind.color();
    let fg = RGB::from_u8(r, g, b);
    for &(dx, dy) in kind.cells(0) {
        let px = base_x + dx * 2;
        let py = base_y + dy;
        ctx.set(px, py, fg, fg, to_cp437(' '));
        ctx.set(px + 1, py, fg, fg, to_cp437(' '));
    }
}

struct Game {
    /// 화면 기준 보드 좌상단(왼쪽 `#` 열)
    origin_x: i32,
    origin_y: i32,
    board: [[Option<Kind>; BOARD_W]; BOARD_H],
    active: Option<Active>,
    next: Kind,
    rng: RandomNumberGenerator,
    fall_accum_ms: f32,
    fall_interval_ms: f32,
    score: u32,
    game_over: bool,
    /// 키 반복 방지
    key_left: bool,
    key_right: bool,
    key_down: bool,
    key_up: bool,
    key_space: bool,
}

impl Game {
    fn new(origin_x: i32, origin_y: i32) -> Self {
        let mut rng = RandomNumberGenerator::new();
        let next = Kind::random(&mut rng);
        Self {
            origin_x,
            origin_y,
            board: [[None; BOARD_W]; BOARD_H],
            active: None,
            next,
            rng,
            fall_accum_ms: 0.0,
            fall_interval_ms: 550.0,
            score: 0,
            game_over: false,
            key_left: false,
            key_right: false,
            key_down: false,
            key_up: false,
            key_space: false,
        }
    }

    fn panel_x(&self) -> i32 {
        self.origin_x + BOARD_PX_W + 2
    }

    fn spawn(&mut self) {
        let kind = self.next;
        self.next = Kind::random(&mut self.rng);
        let piece = Active {
            kind,
            rot: 0,
            x: 3,
            y: 0,
        };
        if !valid_placement(&self.board, &piece) {
            self.game_over = true;
            self.active = None;
            return;
        }
        self.active = Some(piece);
    }

    fn try_move(&mut self, dx: i32, dy: i32) -> bool {
        let Some(ref mut p) = self.active else {
            return false;
        };
        let np = Active {
            kind: p.kind,
            rot: p.rot,
            x: p.x + dx,
            y: p.y + dy,
        };
        if valid_placement(&self.board, &np) {
            *p = np;
            true
        } else {
            false
        }
    }

    fn try_rotate(&mut self) -> bool {
        let Some(ref mut p) = self.active else {
            return false;
        };
        let new_rot = (p.rot + 1) % 4;
        let np = Active {
            kind: p.kind,
            rot: new_rot,
            x: p.x,
            y: p.y,
        };
        if valid_placement(&self.board, &np) {
            p.rot = new_rot;
            return true;
        }
        for kick in [-1, 1, -2, 2] {
            let np = Active {
                kind: p.kind,
                rot: new_rot,
                x: p.x + kick,
                y: p.y,
            };
            if valid_placement(&self.board, &np) {
                *p = np;
                return true;
            }
        }
        false
    }

    /// 스페이스: 한 번에 바닥까지 낙한 뒤 고정
    fn hard_drop(&mut self) {
        let mut dropped = 0u32;
        while self.try_move(0, 1) {
            dropped += 1;
        }
        self.score += dropped.saturating_mul(2);
        self.fall_accum_ms = 0.0;
        self.lock_and_spawn();
    }

    fn lock_and_spawn(&mut self) {
        if let Some(p) = self.active.take() {
            merge(&mut self.board, &p);
            let n = clear_lines(&mut self.board);
            self.score += n as u32 * 100;
            self.fall_interval_ms = (550.0 - (self.score as f32 / 50.0).min(350.0)).max(120.0);
        }
        if !self.game_over {
            self.spawn();
        }
    }

    fn tick_gravity(&mut self, dt_ms: f32) {
        if self.game_over || self.active.is_none() {
            if self.active.is_none() && !self.game_over {
                self.spawn();
            }
            return;
        }
        self.fall_accum_ms += dt_ms;
        if self.fall_accum_ms >= self.fall_interval_ms {
            self.fall_accum_ms -= self.fall_interval_ms;
            if !self.try_move(0, 1) {
                self.lock_and_spawn();
            }
        }
    }
}

impl GameState for Game {
    fn tick(&mut self, ctx: &mut BTerm) {
        let dt = ctx.frame_time_ms;
        if self.game_over {
            ctx.cls();
            ctx.print_color_centered(
                self.origin_y + BOARD_PX_H / 2,
                RGB::named(rltk::YELLOW),
                RGB::named(rltk::BLACK),
                "게임 오버 — ESC로 종료",
            );
            if ctx.key == Some(VirtualKeyCode::Escape) {
                ctx.quitting = true;
            }
            return;
        }

        if self.active.is_none() {
            self.spawn();
        }

        // 입력
        if let Some(key) = ctx.key {
            match key {
                VirtualKeyCode::Left => {
                    if !self.key_left {
                        self.key_left = true;
                        self.try_move(-1, 0);
                    }
                }
                VirtualKeyCode::Right => {
                    if !self.key_right {
                        self.key_right = true;
                        self.try_move(1, 0);
                    }
                }
                VirtualKeyCode::Down => {
                    if !self.key_down {
                        self.key_down = true;
                        if self.try_move(0, 1) {
                            self.score += 1;
                        }
                    }
                }
                VirtualKeyCode::Up => {
                    if !self.key_up {
                        self.key_up = true;
                        self.try_rotate();
                    }
                }
                VirtualKeyCode::Space => {
                    if !self.key_space {
                        self.key_space = true;
                        if self.active.is_some() {
                            self.hard_drop();
                        }
                    }
                }
                VirtualKeyCode::Escape => {
                    ctx.quitting = true;
                }
                _ => {}
            }
        } else {
            self.key_left = false;
            self.key_right = false;
            self.key_down = false;
            self.key_up = false;
            self.key_space = false;
        }

        self.tick_gravity(dt);

        // 그리기
        ctx.cls();
        // 테두리
        for x in 0..BOARD_PX_W {
            ctx.set(
                self.origin_x + x,
                self.origin_y,
                RGB::named(rltk::GRAY),
                RGB::named(rltk::BLACK),
                to_cp437('#'),
            );
            ctx.set(
                self.origin_x + x,
                self.origin_y + BOARD_PX_H - 1,
                RGB::named(rltk::GRAY),
                RGB::named(rltk::BLACK),
                to_cp437('#'),
            );
        }
        for y in 0..BOARD_PX_H {
            ctx.set(
                self.origin_x,
                self.origin_y + y,
                RGB::named(rltk::GRAY),
                RGB::named(rltk::BLACK),
                to_cp437('#'),
            );
            ctx.set(
                self.origin_x + BOARD_PX_W - 1,
                self.origin_y + y,
                RGB::named(rltk::GRAY),
                RGB::named(rltk::BLACK),
                to_cp437('#'),
            );
        }

        draw_board(ctx, &self.board, self.origin_x, self.origin_y);

        if let Some(ref p) = self.active {
            let mut gy = p.y;
            loop {
                let trial = Active {
                    kind: p.kind,
                    rot: p.rot,
                    x: p.x,
                    y: gy + 1,
                };
                if valid_placement(&self.board, &trial) {
                    gy += 1;
                } else {
                    break;
                }
            }
            draw_piece_at(
                ctx,
                p.kind,
                p.rot,
                p.x,
                gy,
                true,
                self.origin_x,
                self.origin_y,
            );
            draw_piece_at(
                ctx,
                p.kind,
                p.rot,
                p.x,
                p.y,
                false,
                self.origin_x,
                self.origin_y,
            );
        }

        let px = self.panel_x();
        // 우측 위: 다음 블록
        ctx.print_color(
            px,
            self.origin_y,
            RGB::named(rltk::WHITE),
            RGB::named(rltk::BLACK),
            "다음 블록",
        );
        draw_next_preview(ctx, self.next, px, self.origin_y + 2);

        let help_y = self.origin_y + BOARD_PX_H - 7;
        ctx.print_color(px, help_y, RGB::named(rltk::GRAY), RGB::named(rltk::BLACK), "조작 안내");
        ctx.print_color(
            px,
            help_y + 1,
            RGB::named(rltk::WHITE),
            RGB::named(rltk::BLACK),
            "방향키로 움직입니다.",
        );
        ctx.print_color(px, help_y + 2, RGB::named(rltk::GRAY), RGB::named(rltk::BLACK), "← → 이동");
        ctx.print_color(px, help_y + 3, RGB::named(rltk::GRAY), RGB::named(rltk::BLACK), "↓ 빠른 낙하");
        ctx.print_color(px, help_y + 4, RGB::named(rltk::GRAY), RGB::named(rltk::BLACK), "↑ 회전");
        ctx.print_color(
            px,
            help_y + 5,
            RGB::named(rltk::GRAY),
            RGB::named(rltk::BLACK),
            "스페이스 바닥까지",
        );

        ctx.print_color(
            self.origin_x,
            self.origin_y + BOARD_PX_H + 1,
            RGB::named(rltk::CYAN),
            RGB::named(rltk::BLACK),
            &format!("점수: {}  ESC 종료", self.score),
        );
    }
}

fn main() -> BError {
    let w = MIN_VIEW_W.max(CLUSTER_W.max(1) as u32 + 8);
    let h = MIN_VIEW_H.max(CLUSTER_H.max(1) as u32 + 6);
    let origin_x = (w as i32 - CLUSTER_W) / 2;
    let origin_y = (h as i32 - CLUSTER_H) / 2;

    // `resources/vga8x16.png` 사용 (저장소에 포함됨)
    let context = BTermBuilder::vga(w, h)
        .with_title("Tui Tetris")
        .build()?;

    let gs = Game::new(origin_x.max(0), origin_y.max(0));
    main_loop(context, gs)
}
