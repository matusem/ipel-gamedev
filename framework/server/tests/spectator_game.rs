//! Spectator registration on a live game instance (requires tic_tac_toe fixture wasm).

use server::game_service::create_and_spawn_game;
use server::test_support::TestEnv;

#[tokio::test]
async fn register_spectator_returns_initial_public_state() {
    let env = TestEnv::new().await;
    if env.registry.read().unwrap().game_types().is_empty() {
        eprintln!("skip spectator_game: no game fixtures under server/tests/fixtures/games");
        return;
    }

    let game_id = create_and_spawn_game(
        &env.component_db,
        &env.game_db,
        env.game_store.clone(),
        "tic_tac_toe".into(),
        b"null".to_vec(),
        None,
        env.pool.clone(),
        env.lobby_notify.clone(),
        vec![],
    )
    .await
    .expect("spawn tic_tac_toe");

    let mut instance = env.game_db.get_game(game_id).expect("game instance");
    let (spectator_state, _channel, _id) =
        instance.register_spectator().expect("register spectator");
    assert!(
        !spectator_state.is_empty(),
        "spectator initial state should be serialized JSON bytes"
    );
}
