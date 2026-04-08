use dioxus::prelude::*;
use shared_types::Player;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let board = vec![" ", " ", " ", " ", " ", " ", " ", " ", " "];
    rsx! {
        main {
            h1 { "Tic-Tac-Toe (3x3)" }
            p { "Simple starter UI scaffolded by gamedev-cli." }
            p { "Shared type sample: {format!(\"{:?}\", Player::Player1)}" }
            pre { "{board[0]}|{board[1]}|{board[2]}\n-----\n{board[3]}|{board[4]}|{board[5]}\n-----\n{board[6]}|{board[7]}|{board[8]}" }
        }
    }
}
