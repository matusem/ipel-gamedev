use dioxus::prelude::*;
use shared_types::Player;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let sample = format!("{:?}", Player::Player1);
    rsx! {
        main {
            h1 { "Tic-Tac-Toe (3x3)" }
            p { "Starter UI scaffolded by gamedev-cli." }
            p { "Shared type sample: {sample}" }
        }
    }
}
