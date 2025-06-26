const P1: &str = "LY＃863";
const P2: &str = "KEKW＃849";

pub const MIN_FRAMES: usize = 30 * 60; // 30s * 60fps

pub fn match_player_code(code: &str) -> bool {
    matches!(code, P1 | P2)
}

pub fn r_presser(is_max: bool) -> &'static str {
    if is_max { P1 } else { P2 }
}

pub fn is_player1(id: &str) -> bool {
    id == P1
}
