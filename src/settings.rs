pub fn match_player_code(code: &str) -> bool {
    match code {
        "LY＃863" | "KEKW＃849" => true,
        _ => false,
    }
}

pub fn r_presser(is_max: bool) -> &'static str {
    return if is_max { "LY＃863" } else { "KEKW＃849" };
}
