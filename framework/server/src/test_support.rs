//! GraphQL integration test harness (in-memory SQLite + schema context).
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use async_graphql::{Request, Value, Variables};
use sqlx::SqlitePool;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::component_db::ComponentDb;
use crate::db::{self, GameInstanceStore};
use crate::game_db::GameDb;
use crate::game_registry::GameRegistry;
use crate::graphql::{build_schema, AppSchema, DraftsDir, GamesDir, RequestUser};
use crate::lobby_db::LobbyListNotify;

pub struct TestEnv {
    pub pool: SqlitePool,
    pub schema: AppSchema,
    pub game_db: GameDb,
    pub component_db: ComponentDb,
    pub registry: Arc<RwLock<GameRegistry>>,
    pub game_store: Arc<GameInstanceStore>,
    pub lobby_notify: LobbyListNotify,
    pub games_dir: PathBuf,
    pub drafts_dir: PathBuf,
}

impl TestEnv {
    pub async fn new() -> Self {
        let pool = db::connect_and_migrate("sqlite::memory:")
            .await
            .expect("in-memory migrate");
        let (list_tx, _list_rx) = broadcast::channel::<()>(16);
        let game_db = GameDb::new(Some(list_tx));
        let (lobby_tx, _lobby_rx) = broadcast::channel::<()>(16);
        let lobby_notify = LobbyListNotify { tx: lobby_tx };
        let game_store = Arc::new(GameInstanceStore::new(pool.clone()));
        let component_db = ComponentDb::new();
        let games_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/games");
        let drafts_dir = std::env::temp_dir().join("upjs-gdd-test-drafts");
        let registry = Arc::new(RwLock::new(GameRegistry::load(&games_dir, &component_db)));
        let schema = build_schema();

        Self {
            pool,
            schema,
            game_db,
            component_db,
            registry,
            game_store,
            lobby_notify,
            games_dir,
            drafts_dir,
        }
    }

    pub async fn register_user(&self, display_name: &str) -> Uuid {
        let (id, _, _) = db::register_user(&self.pool, display_name)
            .await
            .expect("register user");
        id
    }

    pub async fn gql(&self, query: &str, user_id: Option<Uuid>) -> async_graphql::Response {
        self.gql_with_vars(query, user_id, Variables::default()).await
    }

    pub async fn gql_with_vars(
        &self,
        query: &str,
        user_id: Option<Uuid>,
        variables: Variables,
    ) -> async_graphql::Response {
        let auth = RequestUser(user_id.map(|u| u.to_string()));
        let req = Request::new(query)
            .variables(variables)
            .data(self.pool.clone())
            .data(self.game_db.clone())
            .data(self.registry.clone())
            .data(self.component_db.clone())
            .data(self.game_store.clone())
            .data(self.lobby_notify.clone())
            .data(GamesDir(self.games_dir.clone()))
            .data(DraftsDir(self.drafts_dir.clone()))
            .data(auth);
        self.schema.execute(req).await
    }

    pub fn data_path(response: &async_graphql::Response, path: &[&str]) -> Option<Value> {
        let mut cur = response.data.clone();
        for key in path {
            let Value::Object(obj) = cur else {
                return None;
            };
            cur = obj.get(*key)?.clone();
        }
        Some(cur)
    }

    pub fn assert_no_errors(response: &async_graphql::Response) {
        assert!(
            response.errors.is_empty(),
            "GraphQL errors: {:?}, data: {:?}",
            response.errors,
            response.data
        );
    }

    pub fn value_string(value: Value) -> Option<String> {
        match value {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
}
