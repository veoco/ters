use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use hmac::{Hmac, Mac};
use jwt::SignWithKey;
use serde_json::{json, Value};
use sha2::Sha256;
use std::sync::Arc;
use std::time::SystemTime;

use super::errors::{AuthError, FieldError};
use super::extractors::{PMAdministrator, PMSubscriber, ValidatedJson, ValidatedQuery};
use super::models::{TokenData, User, UserLogin, UserModify, UserRegister, UsersQuery};
use super::utils::{authenticate_user, hash};
use crate::AppState;

pub async fn login_for_access_token(
    State(state): State<Arc<AppState>>,
    ValidatedJson(user_login): ValidatedJson<UserLogin>,
) -> Result<Json<Value>, AuthError> {
    if let Some(user) = authenticate_user(&state, &user_login).await {
        let key: Hmac<Sha256> = Hmac::new_from_slice(state.secret_key.as_bytes()).unwrap();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let token_data = TokenData {
            sub: format!("{}", user.uid),
            exp: now + state.access_token_expire_secondes,
        };
        let access_token = token_data.sign_with_key(&key).unwrap();

        let update_sql = format!(
            r#"
            UPDATE {users_table}
            SET "activated" = ?1, "logged" = ?1
            WHERE {users_table}."uid" = ?2
            "#,
            users_table = &state.users_table
        );
        let _ = sqlx::query(&update_sql)
            .bind(now as u32)
            .bind(user.uid)
            .execute(&state.pool)
            .await;

        return Ok(Json(
            json!({"access_token": access_token, "token_type": "Bearer"}),
        ));
    }
    Err(AuthError::WrongCredentials)
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    ValidatedJson(user_register): ValidatedJson<UserRegister>,
) -> Result<(StatusCode, Json<Value>), FieldError> {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;
    let hashed_password = hash(&user_register.password);

    let insert_sql = format!(
        r#"
        INSERT INTO {users_table} ("name", "mail", "url", "screenName", "password", "created", "group")
        VALUES (?1, ?2, ?3, ?1, ?4, ?5, 'subscriber')
        "#,
        users_table = &state.users_table
    );
    if let Ok(r) = sqlx::query(&insert_sql)
        .bind(user_register.name)
        .bind(user_register.mail)
        .bind(user_register.url)
        .bind(hashed_password)
        .bind(now)
        .execute(&state.pool)
        .await
    {
        return Ok((
            StatusCode::CREATED,
            Json(json!({"id": r.last_insert_rowid()})),
        ));
    }
    Err(FieldError::AlreadyExist("name or mail".to_owned()))
}

pub async fn list_users(
    State(state): State<Arc<AppState>>,
    PMAdministrator(_): PMAdministrator,
    ValidatedQuery(q): ValidatedQuery<UsersQuery>,
) -> Result<Json<Value>, FieldError> {
    let all_sql = format!(
        r#"
        SELECT COUNT(*)
        FROM {users_table};
        "#,
        users_table = &state.users_table
    );
    let all_count = sqlx::query_scalar::<_, i32>(&all_sql)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);

    let page = q.page.unwrap_or(1);
    let page_size = q.page_size.unwrap_or(10);
    let order_by = q.order_by.unwrap_or("-uid".to_string());

    let offset = (page - 1) * page_size;
    let order_by = match order_by.as_str() {
        "uid" => "uid",
        "-uid" => "uid DESC",
        "name" => "name",
        "-name" => "name DESC",
        "mail" => "mail",
        "-mail" => "mail DESC",
        f => return Err(FieldError::InvalidParams(f.to_string())),
    };
    let sql = format!(
        r#"
        SELECT *
        FROM {users_table}
        ORDER BY {users_table}.{}
        LIMIT ?1 OFFSET ?2"#,
        order_by,
        users_table = &state.users_table
    );

    match sqlx::query_as::<_, User>(&sql)
        .bind(page_size)
        .bind(offset)
        .fetch_all(&state.pool)
        .await
    {
        Ok(users) => {
            return Ok(Json(json!({
                "page": page,
                "page_size": page_size,
                "all_count": all_count,
                "count": users.len(),
                "results": users
            })));
        }
        Err(e) => return Err(FieldError::DatabaseFailed(e.to_string())),
    }
}

pub async fn get_user_by_id(
    State(state): State<Arc<AppState>>,
    PMSubscriber(user): PMSubscriber,
    Path(uid): Path<u32>,
) -> Result<Json<Value>, FieldError> {
    if user.uid == uid {
        return Ok(Json(json!(user)));
    }

    if user.group == "administrator" {
        let user_sql = format!(
            r#"
            SELECT *
            FROM {users_table}
            WHERE {users_table}."uid" == ?1
            "#,
            users_table = &state.users_table
        );
        if let Ok(mut target_user) = sqlx::query_as::<_, User>(&user_sql)
            .bind(uid)
            .fetch_one(&state.pool)
            .await
        {
            target_user.password = None;
            return Ok(Json(json!(target_user)));
        }
        return Err(FieldError::InvalidParams("uid".to_string()));
    }
    Err(FieldError::PermissionDeny)
}

pub async fn modify_user_by_id(
    State(state): State<Arc<AppState>>,
    PMSubscriber(user): PMSubscriber,
    Path(uid): Path<u32>,
    ValidatedJson(user_modify): ValidatedJson<UserModify>,
) -> Result<Json<Value>, FieldError> {
    if (user.uid == uid && user.group == user_modify.group) || user.group == "administrator" {
        match user_modify.group.as_str() {
            "subscriber" | "contributor" | "editor" | "administrator" => {}
            _ => return Err(FieldError::InvalidParams("group".to_string())),
        }

        let exist_sql = format!(
            r#"
            SELECT EXISTS (SELECT 1 FROM {users_table} WHERE {users_table}."uid" == ?1)
            "#,
            users_table = &state.users_table
        );
        let exist = sqlx::query_scalar::<_, bool>(&exist_sql)
            .bind(uid)
            .fetch_one(&state.pool)
            .await
            .unwrap_or(false);

        if exist {
            if user_modify.password.is_none() {
                let update_sql = format!(
                    r#"
                    UPDATE {users_table} SET "name" = ?1, "mail" = ?2, url = ?3, "screenName" = ?4, "group" = ?5 WHERE {users_table}."uid" = ?6
                    "#,
                    users_table = &state.users_table
                );
                if let Ok(r) = sqlx::query(&update_sql)
                    .bind(user_modify.name)
                    .bind(user_modify.mail)
                    .bind(user_modify.url)
                    .bind(user_modify.screenName)
                    .bind(user_modify.group)
                    .bind(uid)
                    .execute(&state.pool)
                    .await
                {
                    return Ok(Json(json!({
                        "msg": format!("{} infomation changed", r.last_insert_rowid())
                    })));
                }
            } else {
                let password = user_modify.password.unwrap();
                let hashed_password = hash(&password);
                let update_sql = format!(
                    r#"
                    UPDATE {users_table} SET {users_table}."password" = ?1 WHERE {users_table}."uid" = ?2
                    "#,
                    users_table = &state.users_table
                );
                if let Ok(r) = sqlx::query(&update_sql)
                    .bind(hashed_password)
                    .bind(uid)
                    .execute(&state.pool)
                    .await
                {
                    return Ok(Json(json!({
                        "msg": format!("{} password changed", r.last_insert_rowid())
                    })));
                }
            }
        }

        return Err(FieldError::InvalidParams("uid".to_string()));
    }
    Err(FieldError::PermissionDeny)
}
