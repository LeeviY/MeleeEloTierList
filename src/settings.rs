const P1: &str = "LY＃863";
const P2: &str = "KEKW＃849";

pub fn match_player_code(code: &str) -> bool {
    match code {
        P1 => true,
        P2 => true,
        _ => false,
    }
}

pub fn r_presser(is_max: bool) -> &'static str {
    return if is_max { P1 } else { P2 };
}

pub fn is_player1(id: &str) -> bool {
    id == P1
}
