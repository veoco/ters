use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use super::views;
use crate::AppState;

pub fn comments_routers() -> Router<Arc<AppState>> {
    let comments_route = Router::new()
        .route("/api/comments/", get(views::list_comments))
        .route(
            "/api/pages/:slug/comments/",
            post(views::create_comment).get(views::list_content_comments_by_slug),
        )
        .route(
            "/api/posts/:slug/comments/",
            post(views::create_comment).get(views::list_content_comments_by_slug),
        );
    comments_route
}
