//! GraphQL integration tests: lobby create → guest join → ready → start guardrails.

use server::lobby_db;
use server::test_support::TestEnv;
use uuid::Uuid;

#[tokio::test]
async fn register_user_mutation_returns_profile() {
    let env = TestEnv::new().await;
    let resp = env
        .gql(
            r#"mutation { registerUser(displayName: "Alice") { user { id displayName } } }"#,
            None,
        )
        .await;
    TestEnv::assert_no_errors(&resp);
    let name = TestEnv::data_path(&resp, &["registerUser", "user", "displayName"])
        .and_then(TestEnv::value_string);
    assert_eq!(name.as_deref(), Some("Alice"));
}

#[tokio::test]
async fn create_lobby_returns_configuring_room() {
    let env = TestEnv::new().await;
    let owner = env.register_user("Owner").await;

    let resp = env
        .gql(
            r#"mutation { createLobby { id status ownerDisplayName seats { seatIndex } } }"#,
            Some(&owner),
        )
        .await;
    TestEnv::assert_no_errors(&resp);
    assert_eq!(
        TestEnv::data_path(&resp, &["createLobby", "status"])
            .and_then(TestEnv::value_string)
            .as_deref(),
        Some("configuring")
    );
    assert_eq!(
        TestEnv::data_path(&resp, &["createLobby", "ownerDisplayName"])
            .and_then(TestEnv::value_string)
            .as_deref(),
        Some("Owner")
    );
}

#[tokio::test]
async fn guest_joins_claimed_seat_in_waiting_lobby() {
    let env = TestEnv::new().await;
    let owner = env.register_user("Host").await;
    let guest = env.register_user("Guest").await;

    let create = env
        .gql(r#"mutation { createLobby { id } }"#, Some(&owner))
        .await;
    TestEnv::assert_no_errors(&create);
    let lobby_id = TestEnv::data_path(&create, &["createLobby", "id"])
        .and_then(TestEnv::value_string)
        .expect("lobby id");
    let lid = Uuid::parse_str(&lobby_id).unwrap();

    lobby_db::owner_replace_game_type_and_seats(
        &env.pool,
        lid,
        owner.id,
        "tic_tac_toe",
        &["p1".into(), "p2".into()],
        false,
    )
    .await
    .expect("seed seats");

    let join = env
        .gql(
            &format!(
                r#"mutation {{
                    joinLobby(lobbyId: "{lobby_id}", seatIndex: 1) {{
                        id status
                        seats {{ seatIndex claimedDisplayName ready }}
                    }}
                }}"#
            ),
            Some(&guest),
        )
        .await;
    TestEnv::assert_no_errors(&join);

    let seats = TestEnv::data_path(&join, &["joinLobby", "seats"])
        .and_then(|v| match v {
            async_graphql::Value::List(items) => Some(items),
            _ => None,
        })
        .expect("seats array");
    let guest_seat = seats.iter().find(|s| {
        matches!(
            s,
            async_graphql::Value::Object(obj)
                if matches!(obj.get("seatIndex"), Some(async_graphql::Value::Number(n)) if n.as_i64() == Some(1))
        )
    }).expect("seat 1");
    let claimed = match guest_seat {
        async_graphql::Value::Object(obj) => obj
            .get("claimedDisplayName")
            .cloned()
            .and_then(TestEnv::value_string),
        _ => None,
    };
    assert_eq!(claimed.as_deref(), Some("Guest"));
    assert_eq!(
        TestEnv::data_path(&join, &["joinLobby", "status"])
            .and_then(TestEnv::value_string)
            .as_deref(),
        Some("waiting")
    );
}

#[tokio::test]
async fn owner_sets_ready_and_start_blocked_without_all_seats() {
    let env = TestEnv::new().await;
    let owner = env.register_user("Host").await;

    let create = env
        .gql(r#"mutation { createLobby { id } }"#, Some(&owner))
        .await;
    TestEnv::assert_no_errors(&create);
    let lobby_id = TestEnv::data_path(&create, &["createLobby", "id"])
        .and_then(TestEnv::value_string)
        .unwrap();
    let lid = Uuid::parse_str(&lobby_id).unwrap();

    lobby_db::owner_replace_game_type_and_seats(
        &env.pool,
        lid,
        owner.id,
        "tic_tac_toe",
        &["p1".into(), "p2".into()],
        false,
    )
    .await
    .unwrap();

    let claim = env
        .gql(
            &format!(r#"mutation {{ joinLobby(lobbyId: "{lobby_id}", seatIndex: 0) {{ id }} }}"#),
            Some(&owner),
        )
        .await;
    TestEnv::assert_no_errors(&claim);

    let ready = env
        .gql(
            &format!(
                r#"mutation {{ setLobbySeatReady(lobbyId: "{lobby_id}", ready: true) {{ id }} }}"#
            ),
            Some(&owner),
        )
        .await;
    TestEnv::assert_no_errors(&ready);

    let start = env
        .gql(
            &format!(r#"mutation {{ startLobby(lobbyId: "{lobby_id}") }}"#),
            Some(&owner),
        )
        .await;
    assert!(
        !start.errors.is_empty(),
        "start should fail when not all seats claimed/ready"
    );
}

#[tokio::test]
async fn owner_transfers_lobby_to_seated_guest() {
    let env = TestEnv::new().await;
    let owner = env.register_user("Host").await;
    let guest = env.register_user("Guest").await;

    let create = env
        .gql(r#"mutation { createLobby { id } }"#, Some(&owner))
        .await;
    TestEnv::assert_no_errors(&create);
    let lobby_id = TestEnv::data_path(&create, &["createLobby", "id"])
        .and_then(TestEnv::value_string)
        .expect("lobby id");
    let lid = Uuid::parse_str(&lobby_id).unwrap();

    lobby_db::owner_replace_game_type_and_seats(
        &env.pool,
        lid,
        owner.id,
        "tic_tac_toe",
        &["p1".into(), "p2".into()],
        false,
    )
    .await
    .expect("seed seats");

    let join = env
        .gql(
            &format!(r#"mutation {{ joinLobby(lobbyId: "{lobby_id}", seatIndex: 1) {{ id }} }}"#),
            Some(&guest),
        )
        .await;
    TestEnv::assert_no_errors(&join);

    let guest_id = guest.id;
    let transfer = env
        .gql(
            &format!(
                r#"mutation {{
                    transferLobbyOwnership(lobbyId: "{lobby_id}", newOwnerUserId: "{guest_id}") {{
                        ownerUserId ownerDisplayName
                    }}
                }}"#
            ),
            Some(&owner),
        )
        .await;
    TestEnv::assert_no_errors(&transfer);
    assert_eq!(
        TestEnv::data_path(&transfer, &["transferLobbyOwnership", "ownerUserId"])
            .and_then(TestEnv::value_string)
            .as_deref(),
        Some(guest_id.to_string().as_str())
    );
    assert_eq!(
        TestEnv::data_path(&transfer, &["transferLobbyOwnership", "ownerDisplayName"])
            .and_then(TestEnv::value_string)
            .as_deref(),
        Some("Guest")
    );

    let detail = lobby_db::get_lobby(&env.pool, lid).await.unwrap().unwrap();
    assert_eq!(detail.owner_user_id, guest_id);
}

#[tokio::test]
async fn list_lobbies_includes_active_room() {
    let env = TestEnv::new().await;
    let owner = env.register_user("Lister").await;
    let _ = env
        .gql(r#"mutation { createLobby { id } }"#, Some(&owner))
        .await;

    let list = env
        .gql(
            r#"query { lobbies { id ownerDisplayName status } }"#,
            Some(&owner),
        )
        .await;
    TestEnv::assert_no_errors(&list);
    let lobbies = TestEnv::data_path(&list, &["lobbies"])
        .and_then(|v| match v {
            async_graphql::Value::List(items) => Some(items),
            _ => None,
        })
        .expect("lobbies");
    assert!(!lobbies.is_empty());
}
