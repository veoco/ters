#![allow(non_snake_case)]
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use validator::Validate;

use super::{de::from_str, errors::Error};

#[derive(Serialize, Deserialize, FromRow)]
pub struct Attachment {
    pub cid: u32,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub created: u32,
    pub modified: u32,
    pub text: String,
    pub order: u32,
    pub authorId: u32,
    pub template: Option<String>,
    pub r#type: String,
    pub status: String,
    pub password: Option<String>,
    pub commentsNum: u32,
    pub allowComment: String,
    pub allowPing: String,
    pub allowFeed: String,
    pub parent: u32,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct AttachmentText {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub r#type: String,
    pub mime: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct AttachmentInfo {
    pub cid: u32,
    pub created: u32,
    pub modified: u32,
    pub name: String,
    pub path: String,
    pub size: u64,
    pub r#type: String,
    pub mime: String,
}

impl AttachmentInfo {
    pub fn from_attachment_text(at: AttachmentText, cid: u32, created: u32, modified: u32) -> Self {
        AttachmentInfo {
            cid,
            created,
            modified,
            name: at.name,
            path: at.path,
            size: at.size,
            r#type: at.r#type,
            mime: at.mime,
        }
    }

    pub fn from_attachment(attachment: Attachment) -> Result<Self, Error> {
        let at = from_str::<AttachmentText>(&attachment.text)?;
        Ok(AttachmentInfo {
            cid: attachment.cid,
            created: attachment.created,
            modified: attachment.modified,
            name: at.name,
            path: at.path,
            size: at.size,
            r#type: at.r#type,
            mime: at.mime,
        })
    }
}

#[derive(Serialize, Deserialize, Validate)]
pub struct AttachmentsQuery {
    #[validate(range(min = 1, message = "page must greater than 1"))]
    pub page: Option<u32>,
    #[validate(range(min = 1, message = "page_size must greater than 1"))]
    pub page_size: Option<u32>,
    #[validate(length(min = 1, max = 13, message = "order_by length must greater than 1"))]
    pub order_by: Option<String>,
    pub private: Option<bool>,
}