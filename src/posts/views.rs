use axum::extract::{Path, State};
use axum::response::Json;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::SystemTime;

use super::models::{Post, PostCreate, PostQuery, PostWithMeta, PostsQuery};
use crate::users::errors::FieldError;
use crate::users::extractors::{PMContributor, ValidatedJson, ValidatedQuery};
use crate::AppState;

pub async fn create_post(
    State(state): State<Arc<AppState>>,
    PMContributor(user): PMContributor,
    ValidatedJson(post_create): ValidatedJson<PostCreate>,
) -> Result<Json<Value>, FieldError> {
    let exist = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (SELECT 1 FROM typecho_contents WHERE slug == ?1)
        "#,
    )
    .bind(&post_create.slug)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    if exist {
        return Err(FieldError::AlreadyExist("slug".to_owned()));
    }

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;

    if let Ok(r) = sqlx::query(
        r#"
        INSERT INTO typecho_contents (type, title, slug, created, modified, text, authorId, template, status, password, allowComment, allowPing, allowFeed)
        VALUES ("post", ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"#,
    )
    .bind(post_create.title)
    .bind(post_create.slug)
    .bind(post_create.created)
    .bind(now)
    .bind(post_create.text)
    .bind(user.uid)
    .bind(post_create.template)
    .bind(post_create.status)
    .bind(post_create.password)
    .bind(post_create.allowComment)
    .bind(post_create.allowPing)
    .bind(post_create.allowFeed)
    .execute(&state.pool)
    .await
    {
        return Ok(Json(json!({"id": r.last_insert_rowid()})));
    }
    Err(FieldError::AlreadyExist("slug".to_owned()))
}

pub async fn list_posts(
    State(state): State<Arc<AppState>>,
    ValidatedQuery(q): ValidatedQuery<PostsQuery>,
) -> Json<Value> {
    let all_count = sqlx::query_scalar::<_, i32>(
        r#"
        SELECT COUNT(*)
        FROM typecho_contents
        WHERE type == post
        "#,
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let offset = (q.page - 1) * q.page_size;
    let order_by = match q.order_by.as_str() {
        "-cid" => "cid DESC",
        "slug" => "slug",
        "-slug" => "slug DESC",
        _ => "cid",
    };

    if q.with_meta.unwrap_or(false) {
        let sql = format!(
            r#"
            WITH categories_json AS (
                SELECT typecho_contents.cid,
                    json_group_array(json_object(
                        'mid', typecho_metas.mid,
                        'slug', typecho_metas.slug,
                        'type', 'category',
                        'name', typecho_metas.name,
                        'description', typecho_metas.description,
                        'count', typecho_metas."count",
                        'order', typecho_metas."order",
                        'parent', typecho_metas.parent
                    )) AS categories
                FROM typecho_contents
                JOIN typecho_relationships ON typecho_contents.cid == typecho_relationships.cid
                JOIN typecho_metas ON typecho_relationships.mid == typecho_metas.mid
                WHERE typecho_contents."type" == "post" AND typecho_metas."type" == "category"
                GROUP BY typecho_contents.cid
                ORDER BY typecho_contents.cid
            ), tags_json AS (
                SELECT typecho_contents.cid,
                    json_group_array(json_object(
                        'mid', typecho_metas.mid,
                        'slug', typecho_metas.slug,
                        'type', 'tag',
                        'name', typecho_metas.name,
                        'description', typecho_metas.description,
                        'count', typecho_metas."count",
                        'order', typecho_metas."order",
                        'parent', typecho_metas.parent
                    )) AS tags
                FROM typecho_contents
                JOIN typecho_relationships ON typecho_contents.cid == typecho_relationships.cid
                JOIN typecho_metas ON typecho_relationships.mid == typecho_metas.mid
                WHERE typecho_contents."type" == "post" AND typecho_metas."type" == "tag"
                GROUP BY typecho_contents.cid
                ORDER BY typecho_contents.cid
            )
            
            SELECT *
            FROM typecho_contents
            JOIN categories_json ON typecho_contents.cid == categories_json.cid
            JOIN tags_json ON typecho_contents.cid == tags_json.cid
            WHERE typecho_contents."type" == "post"
            GROUP BY typecho_contents.cid
            ORDER BY {}
            LIMIT ?1 OFFSET ?2"#,
            order_by
        );

        if let Ok(posts) = sqlx::query_as::<_, PostWithMeta>(&sql)
            .bind(q.page_size)
            .bind(offset)
            .fetch_all(&state.pool)
            .await
        {
            return Json(json!({
                "page": q.page,
                "page_size": q.page_size,
                "all_count": all_count,
                "count": posts.len(),
                "results": posts
            }));
        }
    } else {
        let sql = format!(
            r#"
        SELECT *
        FROM typecho_contents
        WHERE type == "post"
        ORDER BY {}
        LIMIT ?1 OFFSET ?2"#,
            order_by
        );
        if let Ok(posts) = sqlx::query_as::<_, Post>(&sql)
            .bind(q.page_size)
            .bind(offset)
            .fetch_all(&state.pool)
            .await
        {
            return Json(json!({
                "page": q.page,
                "page_size": q.page_size,
                "all_count": all_count,
                "count": posts.len(),
                "results": posts
            }));
        }
    };
    Json(json!({
        "page": q.page,
        "page_size": q.page_size,
        "all_count": all_count,
        "count": 0,
        "results": []
    }))
}

pub async fn get_post_by_slug(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    ValidatedQuery(q): ValidatedQuery<PostQuery>,
) -> Result<Json<Value>, FieldError> {
    if q.with_meta.unwrap_or(false) {
        if let Ok(target_post) = sqlx::query_as::<_, PostWithMeta>(
            r#"
            WITH categories_json AS (
                SELECT typecho_contents.cid,
                    json_group_array(json_object(
                        'mid', typecho_metas.mid,
                        'slug', typecho_metas.slug,
                        'type', 'category',
                        'name', typecho_metas.name,
                        'description', typecho_metas.description,
                        'count', typecho_metas."count",
                        'order', typecho_metas."order",
                        'parent', typecho_metas.parent
                    )) AS categories
                FROM typecho_contents
                JOIN typecho_relationships ON typecho_contents.cid == typecho_relationships.cid
                JOIN typecho_metas ON typecho_relationships.mid == typecho_metas.mid
                WHERE typecho_contents."type" == "post" AND typecho_metas."type" == "category"
                GROUP BY typecho_contents.cid
                ORDER BY typecho_contents.cid
            ), tags_json AS (
                SELECT typecho_contents.cid,
                    json_group_array(json_object(
                        'mid', typecho_metas.mid,
                        'slug', typecho_metas.slug,
                        'type', 'tag',
                        'name', typecho_metas.name,
                        'description', typecho_metas.description,
                        'count', typecho_metas."count",
                        'order', typecho_metas."order",
                        'parent', typecho_metas.parent
                    )) AS tags
                FROM typecho_contents
                JOIN typecho_relationships ON typecho_contents.cid == typecho_relationships.cid
                JOIN typecho_metas ON typecho_relationships.mid == typecho_metas.mid
                WHERE typecho_contents."type" == "post" AND typecho_metas."type" == "tag"
                GROUP BY typecho_contents.cid
                ORDER BY typecho_contents.cid
            )

            SELECT *
            FROM typecho_contents
            JOIN categories_json ON typecho_contents.cid == categories_json.cid
            JOIN tags_json ON typecho_contents.cid == tags_json.cid
            WHERE typecho_contents."type" == "post" AND slug == ?1"#,
        )
        .bind(slug)
        .fetch_one(&state.pool)
        .await
        {
            return Ok(Json(json!(target_post)));
        }
    } else {
        if let Ok(target_post) = sqlx::query_as::<_, Post>(
            r#"
            SELECT *
            FROM typecho_contents
            WHERE type == "post" AND slug == ?1"#,
        )
        .bind(slug)
        .fetch_one(&state.pool)
        .await
        {
            return Ok(Json(json!(target_post)));
        }
    }

    Err(FieldError::PermissionDeny)
}
