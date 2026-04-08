#[test]
fn tictactoe_turn_order_example() {
    let mut next = "Player1";
    for _ in 0..2 {
        next = if next == "Player1" { "Player2" } else { "Player1" };
    }
    assert_eq!(next, "Player1");
}
