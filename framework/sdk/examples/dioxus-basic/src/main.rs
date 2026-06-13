use dioxus::prelude::*;
use upjs_gdd_dioxus::use_realtime_controller;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let _rt = use_realtime_controller(
        "ws://localhost:8080/graphql".to_string(),
        "dev-token".to_string(),
    );
    rsx! { h1 { "Dioxus SDK example" } }
}
