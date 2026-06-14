use server::test_support::TestEnv;

fn gql_errors(resp: &async_graphql::Response) -> String {
    resp.errors
        .iter()
        .map(|e| e.message.clone())
        .collect::<Vec<_>>()
        .join("; ")
}

#[tokio::test]
async fn is_superadmin_false_by_default() {
    let env = TestEnv::new().await;
    let user = env.register_user("alice").await;
    let resp = env.gql("query { isSuperadmin }", Some(&user)).await;
    server::test_support::TestEnv::assert_no_errors(&resp);
    let v = TestEnv::data_path(&resp, &["isSuperadmin"]).unwrap();
    assert_eq!(v, async_graphql::Value::from(false));
}

#[tokio::test]
async fn superadmin_env_grants_access() {
    let env = TestEnv::new().await;
    let user = env.register_user("bootstrap").await;
    unsafe { std::env::set_var("SUPERADMIN_USER_IDS", user.id.to_string()) };

    let resp = env.gql("query { isSuperadmin }", Some(&user)).await;
    TestEnv::assert_no_errors(&resp);
    let v = TestEnv::data_path(&resp, &["isSuperadmin"]).unwrap();
    assert_eq!(v, async_graphql::Value::from(true));

    let resp = env
        .gql("query { adminPlatformOverview { userCount } }", Some(&user))
        .await;
    TestEnv::assert_no_errors(&resp);

    unsafe { std::env::remove_var("SUPERADMIN_USER_IDS") };
}

#[tokio::test]
async fn persisted_superadmin_role_grants_access() {
    let env = TestEnv::new().await;
    let admin = env.register_user("admin").await;
    let other = env.register_user("bob").await;

    unsafe { std::env::set_var("SUPERADMIN_USER_IDS", admin.id.to_string()) };
    let grant = env
        .gql(
            &format!(
                r#"mutation {{ adminGrantRole(userId: "{}", role: "superadmin") }}"#,
                other.id
            ),
            Some(&admin),
        )
        .await;
    TestEnv::assert_no_errors(&grant);
    unsafe { std::env::remove_var("SUPERADMIN_USER_IDS") };

    let resp = env.gql("query { isSuperadmin }", Some(&other)).await;
    TestEnv::assert_no_errors(&resp);
    let v = TestEnv::data_path(&resp, &["isSuperadmin"]).unwrap();
    assert_eq!(v, async_graphql::Value::from(true));
}

#[tokio::test]
async fn normal_user_cannot_call_admin_queries() {
    let env = TestEnv::new().await;
    let user = env.register_user("regular").await;
    let resp = env
        .gql("query { adminUsers(limit: 5) { id } }", Some(&user))
        .await;
    assert!(!resp.errors.is_empty(), "expected auth error");
    assert!(gql_errors(&resp).contains("superadmin"));
}

#[tokio::test]
async fn superadmin_is_developer() {
    let env = TestEnv::new().await;
    let user = env.register_user("dev-admin").await;
    unsafe { std::env::set_var("SUPERADMIN_USER_IDS", user.id.to_string()) };

    let resp = env.gql("query { isDeveloper }", Some(&user)).await;
    TestEnv::assert_no_errors(&resp);
    let v = TestEnv::data_path(&resp, &["isDeveloper"]).unwrap();
    assert_eq!(v, async_graphql::Value::from(true));

    unsafe { std::env::remove_var("SUPERADMIN_USER_IDS") };
}
