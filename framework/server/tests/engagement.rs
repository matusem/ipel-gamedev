use server::test_support::TestEnv;
use server::user_engagement;

#[tokio::test]
async fn register_user_gets_welcome_notification() {
    let env = TestEnv::new().await;
    let resp = env
        .gql(
            r#"mutation { registerUser(displayName: "BadgeTester") { id } }"#,
            None,
        )
        .await;
    TestEnv::assert_no_errors(&resp);
    let user_id = TestEnv::data_path(&resp, &["registerUser", "id"])
        .and_then(TestEnv::value_string)
        .expect("user id");
    let uid = uuid::Uuid::parse_str(&user_id).unwrap();

    let notes = user_engagement::list_notifications(&env.pool, uid, 10)
        .await
        .unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].title, "Welcome to UPJŠ GDD Platform");
    assert!(notes[0].unread);
}

#[tokio::test]
async fn my_badges_returns_catalog_with_locked_state() {
    let env = TestEnv::new().await;
    let uid = env.register_user("Achiever").await;

    let resp = env
        .gql(
            r#"query { myBadges { id label tier locked earnedAt } }"#,
            Some(uid),
        )
        .await;
    TestEnv::assert_no_errors(&resp);
    let badges = TestEnv::data_path(&resp, &["myBadges"])
        .and_then(|v| match v {
            async_graphql::Value::List(items) => Some(items.len()),
            _ => None,
        })
        .expect("badge list");
    assert_eq!(badges, 6);
}

#[tokio::test]
async fn mark_all_notifications_read_clears_unread_count() {
    let env = TestEnv::new().await;
    let uid = env.register_user("Reader").await;

    let before = env
        .gql("query { unreadNotificationCount }", Some(uid))
        .await;
    TestEnv::assert_no_errors(&before);

    let mark = env
        .gql("mutation { markAllNotificationsRead }", Some(uid))
        .await;
    TestEnv::assert_no_errors(&mark);

    let after = env
        .gql("query { unreadNotificationCount }", Some(uid))
        .await;
    TestEnv::assert_no_errors(&after);
    let count = TestEnv::data_path(&after, &["unreadNotificationCount"]).and_then(|v| match v {
        async_graphql::Value::Number(n) => n.as_i64(),
        _ => None,
    });
    assert_eq!(count, Some(0));
}
