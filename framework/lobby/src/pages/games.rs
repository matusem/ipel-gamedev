use crate::api::graphql_post;
use crate::components::game::GameTypeCatalogCard;
use crate::components::ui::*;
use crate::components::SearchContext;
use crate::LobbyRoute;
use crate::models::GameTypeInfo;
use dioxus::prelude::*;
use serde::Deserialize;

#[component]
pub fn GamesListPage() -> Element {
    let nav = use_navigator();
    let search_ctx = use_context::<SearchContext>();
    let game_types: Signal<Vec<GameTypeInfo>> = use_signal(Vec::new);
    let loading = use_signal(|| true);
    let mut error_msg = use_signal(|| None::<String>);
    let mut local_search = use_signal(String::new);

    use_hook(move || {
        let mut game_types = game_types;
        let mut loading = loading;
        let mut error_msg = error_msg;
        spawn(async move {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Data { game_types: Vec<GameTypeInfo> }
            let q = format!(
                "query {{ gameTypes {{ {} }} }}",
                crate::models::GAME_TYPES_GQL_FIELDS
            );
            match graphql_post::<Data>(&q).await {
                Ok(d) => game_types.set(d.game_types),
                Err(e) => error_msg.set(Some(e)),
            }
            loading.set(false);
        });
    });

    let query = {
        let header = (search_ctx.query)().to_lowercase();
        let local = local_search().to_lowercase();
        if !header.is_empty() { header } else { local }
    };
    let filtered: Vec<GameTypeInfo> = game_types()
        .into_iter()
        .filter(|g| {
            query.is_empty()
                || g.display_name.to_lowercase().contains(&query)
                || g.name.to_lowercase().contains(&query)
        })
        .collect();

    rsx! {
        div { class: "page-stack",
            PageHeader {
                title: "Games".to_string(),
                subtitle: Some("Browse published game types on this server.".to_string()),
                badge: None,
                children: Some(rsx! {
                    SearchInput {
                        placeholder: "Filter games…",
                        value: local_search(),
                        width_class: "w-44",
                        oninput: move |val| local_search.set(val),
                    }
                }),
            }
            if let Some(err) = error_msg() {
                ErrorBanner { message: err }
            }
            if loading() {
                div { class: "grid gap-6 sm:grid-cols-2 lg:grid-cols-3",
                    SkeletonCard {}
                    SkeletonCard {}
                    SkeletonCard {}
                }
            } else if filtered.is_empty() {
                EmptyState {
                    icon: "sports_esports",
                    title: "No games match".to_string(),
                    description: "Try a different search or upload a game.".to_string(),
                    cta_label: Some("Developer Hub".to_string()),
                    on_cta: Some(EventHandler::new(move |_| {
                        nav.push(LobbyRoute::DeveloperUploads {});
                    })),
                }
            } else {
                div { class: "grid gap-6 sm:grid-cols-2 lg:grid-cols-3",
                    for gt in filtered {
                        GameTypeCatalogCard { gt }
                    }
                }
            }
        }
    }
}
