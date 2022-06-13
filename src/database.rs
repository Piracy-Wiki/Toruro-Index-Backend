use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use crate::models::user::User;
use crate::errors::ServiceError;
use crate::models::torrent::TorrentListing;
use crate::utils::time::current_time;
use crate::models::tracker_key::TrackerKey;
use serde::Serialize;
use crate::models::page::Page;

#[derive(Debug, Serialize)]
pub struct TorrentCompact {
    pub torrent_id: i64,
    pub info_hash: String,
}

pub struct Database {
    pub pool: SqlitePool
}

pub struct Category {
    pub name: String
}

impl Database {
    pub async fn new(database_url: &str) -> Database {
        let db = SqlitePoolOptions::new()
            .connect(database_url)
            .await
            .expect("Unable to create database pool");

        Database {
            pool: db
        }
    }

    pub async fn get_user_with_username(&self, username: &str) -> Option<User> {
        let res = sqlx::query_as!(
            User,
            "SELECT * FROM torrust_users WHERE username = ?",
            username,
        )
            .fetch_one(&self.pool)
            .await;

        match res {
            Ok(user) => Some(user),
            _ => None
        }
    }

    pub async fn get_user_with_email(&self, email: &str) -> Option<User> {
        let res = sqlx::query_as!(
            User,
            "SELECT * FROM torrust_users WHERE email = ?",
            email,
        )
            .fetch_one(&self.pool)
            .await;

        match res {
            Ok(user) => Some(user),
            _ => None
        }
    }

    pub async fn delete_user(&self, user_id: i64) -> Result<(), sqlx::Error> {
        let _res = sqlx::query!(
            "DELETE FROM torrust_users WHERE rowid = ?",
            user_id
        )
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn insert_torrent_and_get_id(&self, username: String, info_hash: String, title: String, category_id: i64, description: String, file_size: i64, seeders: i64, leechers: i64) -> Result<i64, sqlx::Error> {
        let current_time = current_time() as i64;

        let res = sqlx::query!(
            r#"INSERT INTO torrust_torrents (uploader, info_hash, title, category_id, description, upload_date, file_size, seeders, leechers)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING torrent_id as "torrent_id: i64""#,
            username,
            info_hash,
            title,
            category_id,
            description,
            current_time,
            file_size,
            seeders,
            leechers
        )
            .fetch_one(&self.pool)
            .await?;

        Ok(res.torrent_id)
    }

    pub async fn get_torrent_by_id(&self, torrent_id: i64) -> Result<TorrentListing, ServiceError> {
        let res = sqlx::query_as!(
            TorrentListing,
            r#"SELECT * FROM torrust_torrents
               WHERE torrent_id = ?"#,
            torrent_id
        )
            .fetch_one(&self.pool)
            .await;

        match res {
            Ok(torrent) => Ok(torrent),
            _ => Err(ServiceError::TorrentNotFound)
        }
    }

    pub async fn get_all_torrent_ids(&self) -> Result<Vec<TorrentCompact>, ()> {
        let res = sqlx::query_as!(
            TorrentCompact,
            r#"SELECT torrent_id, info_hash FROM torrust_torrents"#
        )
            .fetch_all(&self.pool)
            .await;

        match res {
            Ok(torrents) => Ok(torrents),
            Err(e) => {
                println!("{:?}", e);
                Err(())
            }
        }
    }

    pub async fn update_tracker_info(&self, info_hash: &str, seeders: i64, leechers: i64) -> Result<(), ()> {
        let res = sqlx::query!(
            "UPDATE torrust_torrents SET seeders = $1, leechers = $2 WHERE info_hash = $3",
            seeders,
            leechers,
            info_hash
        )
            .execute(&self.pool)
            .await;

        match res {
            Ok(_) => Ok(()),
            _ => Err(())
        }
    }

    pub async fn get_valid_tracker_key(&self, user_id: i64) -> Option<TrackerKey> {
        const WEEK: i64 = 604_800;
        let current_time_plus_week = (current_time() as i64) + WEEK;

        let res = sqlx::query_as!(
            TrackerKey,
            r#"SELECT key, valid_until FROM torrust_tracker_keys
               WHERE user_id = $1 AND valid_until > $2"#,
            user_id,
            current_time_plus_week
        )
            .fetch_one(&self.pool)
            .await;

        match res {
            Ok(tracker_key) => Some(tracker_key),
            _ => None
        }
    }

    pub async fn issue_tracker_key(&self, tracker_key: &TrackerKey, user_id: i64) -> Result<(), ServiceError> {
        let res = sqlx::query!(
            "INSERT INTO torrust_tracker_keys (user_id, key, valid_until) VALUES ($1, $2, $3)",
            user_id,
            tracker_key.key,
            tracker_key.valid_until,
        )
            .execute(&self.pool)
            .await;

        match res {
            Ok(_) => Ok(()),
            Err(_) => Err(ServiceError::InternalServerError)
        }
    }

    pub async fn verify_category(&self, category: &str) -> Option<String> {
        let res = sqlx::query_as!(
            Category,
            "SELECT name FROM torrust_categories WHERE name = ?",
            category
        )
            .fetch_one(&self.pool)
            .await;

        match res {
            Ok(v) => Some(v.name),
            Err(_) => None
        }
    }

    pub async fn get_pages(&self) -> Option<Vec<Page>> {
        let res = sqlx::query_as!(Page, "SELECT * FROM torrust_pages")
            .fetch_all(&self.pool)
            .await;

        match res {
            Ok(v) => Some(v),
            Err(_) => None,
        }
    }

    pub async fn get_page_by_route(&self, route: &str) -> Option<Page> {
        let res = sqlx::query_as!(Page, "SELECT * FROM torrust_pages WHERE route=?", route)
            .fetch_one(&self.pool)
            .await;

        match res {
            Ok(v) => Some(v),
            Err(g) => {
                return None;
            }
        }
    }

    pub async fn insert_page(&self, route: &str, title: &str, description: &Option<String>) -> Result<(), ServiceError> {

        if let Some(pages) = self.get_pages().await {
            if pages.iter().any(|i| i.route == route) {
                return Err(ServiceError::PageAlreadyExists);
            }
        }
        let current_time = current_time() as i64;
        let res = sqlx::query!(
            "INSERT INTO torrust_pages (route, title, description, creation_date)
                    VALUES ($1, $2, $3, $4)",
            route,
            title,
            description,
            current_time
        )
        .execute(&self.pool)
        .await;
        match res {
            Ok(_) => Ok(()),
            Err(_) => Err(ServiceError::InternalServerError),
        }
    }
}
