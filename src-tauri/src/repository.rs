use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{
    Conversation, ConversationThread, DiscussionRecord, Message, ModelMember, ProviderKind,
    ReviewItem, ReviewStatus, SourceRecord, WikiRevision,
};

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("database migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("stored provider is invalid: {0}")]
    InvalidProvider(String),
    #[error("stored review status is invalid: {0}")]
    InvalidReviewStatus(String),
    #[error("stored JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct WorkspaceRepository {
    pool: SqlitePool,
}

pub struct NewSource<'a> {
    pub kind: &'a str,
    pub title: &'a str,
    pub source_uri: &'a str,
    pub raw_path: &'a str,
    pub content_hash: &'a str,
    pub extracted_text: Option<&'a str>,
    pub extraction_error: Option<&'a str>,
}

pub struct NewWikiRevision<'a> {
    pub relative_path: &'a str,
    pub before_content: Option<&'a str>,
    pub after_content: &'a str,
    pub before_hash: Option<&'a str>,
    pub after_hash: &'a str,
    pub source_ids: &'a [String],
    pub reason: &'a str,
}

impl WorkspaceRepository {
    pub async fn open(path: &Path) -> Result<Self, RepositoryError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        Self::from_pool(pool).await
    }

    pub async fn in_memory() -> Result<Self, RepositoryError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        Self::from_pool(pool).await
    }

    async fn from_pool(pool: SqlitePool) -> Result<Self, RepositoryError> {
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn create_conversation(&self, title: &str) -> Result<Conversation, RepositoryError> {
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO conversations (id, title) VALUES (?, ?)")
            .bind(&id)
            .bind(title)
            .execute(&self.pool)
            .await?;
        self.load_conversation(&id).await
    }

    pub async fn list_conversations(&self) -> Result<Vec<Conversation>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT id, title, created_at FROM conversations ORDER BY created_at DESC, rowid DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(conversation_from_row)
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub async fn add_member(
        &self,
        conversation_id: &str,
        provider: ProviderKind,
        model: &str,
        role_name: &str,
        role_instruction: &str,
    ) -> Result<ModelMember, RepositoryError> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO members (id, conversation_id, provider, model, role_name, role_instruction) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(conversation_id)
        .bind(provider.as_str())
        .bind(model)
        .bind(role_name)
        .bind(role_instruction)
        .execute(&self.pool)
        .await?;

        Ok(ModelMember {
            id,
            conversation_id: conversation_id.to_owned(),
            provider,
            model: model.to_owned(),
            role_name: role_name.to_owned(),
            role_instruction: role_instruction.to_owned(),
        })
    }

    pub async fn append_message(
        &self,
        conversation_id: &str,
        author_kind: &str,
        author_id: Option<&str>,
        content: &str,
    ) -> Result<Message, RepositoryError> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO messages (id, conversation_id, author_kind, author_id, content) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(conversation_id)
        .bind(author_kind)
        .bind(author_id)
        .bind(content)
        .execute(&self.pool)
        .await?;

        self.load_message(&id).await
    }

    pub async fn record_event(
        &self,
        conversation_id: &str,
        trigger_message_id: &str,
        member_id: &str,
        kind: &str,
        status: &str,
        public_reason: Option<&str>,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO events (id, conversation_id, trigger_message_id, member_id, kind, status, public_reason) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(conversation_id)
        .bind(trigger_message_id)
        .bind(member_id)
        .bind(kind)
        .bind(status)
        .bind(public_reason)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_thread(
        &self,
        conversation_id: &str,
    ) -> Result<ConversationThread, RepositoryError> {
        let conversation = self.load_conversation(conversation_id).await?;
        let member_rows = sqlx::query(
            "SELECT id, conversation_id, provider, model, role_name, role_instruction FROM members WHERE conversation_id = ? ORDER BY rowid",
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await?;
        let mut members = Vec::with_capacity(member_rows.len());
        for row in member_rows {
            let stored_provider: String = row.try_get("provider")?;
            let provider = ProviderKind::from_storage(&stored_provider)
                .ok_or_else(|| RepositoryError::InvalidProvider(stored_provider.clone()))?;
            members.push(ModelMember {
                id: row.try_get("id")?,
                conversation_id: row.try_get("conversation_id")?,
                provider,
                model: row.try_get("model")?,
                role_name: row.try_get("role_name")?,
                role_instruction: row.try_get("role_instruction")?,
            });
        }

        let message_rows = sqlx::query(
            "SELECT id, conversation_id, author_kind, author_id, content, created_at FROM messages WHERE conversation_id = ? ORDER BY created_at, rowid",
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await?;
        let messages = message_rows
            .into_iter()
            .map(message_from_row)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ConversationThread {
            conversation,
            members,
            messages,
        })
    }

    pub async fn load_events(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<DiscussionRecord>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT id, conversation_id, trigger_message_id, member_id, kind, status, public_reason, created_at FROM events WHERE conversation_id = ? ORDER BY created_at, rowid",
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(discussion_record_from_row)
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub async fn save_source(
        &self,
        source: NewSource<'_>,
    ) -> Result<SourceRecord, RepositoryError> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO sources (id, kind, title, source_uri, raw_path, content_hash, extracted_text, extraction_error) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(source.kind)
        .bind(source.title)
        .bind(source.source_uri)
        .bind(source.raw_path)
        .bind(source.content_hash)
        .bind(source.extracted_text)
        .bind(source.extraction_error)
        .execute(&self.pool)
        .await?;
        self.load_source(&id).await
    }

    pub async fn find_source_by_raw_path(
        &self,
        raw_path: &str,
    ) -> Result<Option<SourceRecord>, RepositoryError> {
        let row = sqlx::query(
            "SELECT id, kind, title, source_uri, raw_path, content_hash, extracted_text, extraction_error, created_at FROM sources WHERE raw_path = ?",
        )
        .bind(raw_path)
        .fetch_optional(&self.pool)
        .await?;
        row.map(source_from_row).transpose().map_err(Into::into)
    }

    pub async fn load_source(&self, id: &str) -> Result<SourceRecord, RepositoryError> {
        let row = sqlx::query(
            "SELECT id, kind, title, source_uri, raw_path, content_hash, extracted_text, extraction_error, created_at FROM sources WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(source_from_row(row)?)
    }

    pub async fn attach_source(
        &self,
        conversation_id: &str,
        source_id: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT OR IGNORE INTO conversation_sources (conversation_id, source_id) VALUES (?, ?)",
        )
        .bind(conversation_id)
        .bind(source_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_sources(
        &self,
        conversation_id: Option<&str>,
    ) -> Result<Vec<SourceRecord>, RepositoryError> {
        let rows = if let Some(conversation_id) = conversation_id {
            sqlx::query(
                "SELECT s.id, s.kind, s.title, s.source_uri, s.raw_path, s.content_hash, s.extracted_text, s.extraction_error, s.created_at FROM sources s JOIN conversation_sources cs ON cs.source_id = s.id WHERE cs.conversation_id = ? ORDER BY cs.created_at, cs.rowid",
            )
            .bind(conversation_id)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, kind, title, source_uri, raw_path, content_hash, extracted_text, extraction_error, created_at FROM sources ORDER BY created_at DESC, rowid DESC",
            )
            .fetch_all(&self.pool)
            .await?
        };
        rows.into_iter()
            .map(source_from_row)
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub async fn create_wiki_revision(
        &self,
        revision: NewWikiRevision<'_>,
    ) -> Result<WikiRevision, RepositoryError> {
        let revision_id = Uuid::new_v4().to_string();
        let review_id = Uuid::new_v4().to_string();
        let source_ids_json = serde_json::to_string(revision.source_ids)?;
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO wiki_revisions (id, relative_path, before_content, after_content, before_hash, after_hash, source_ids_json, reason) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&revision_id)
        .bind(revision.relative_path)
        .bind(revision.before_content)
        .bind(revision.after_content)
        .bind(revision.before_hash)
        .bind(revision.after_hash)
        .bind(source_ids_json)
        .bind(revision.reason)
        .execute(&mut *transaction)
        .await?;
        sqlx::query("INSERT INTO review_items (id, revision_id) VALUES (?, ?)")
            .bind(review_id)
            .bind(&revision_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        self.load_wiki_revision(&revision_id).await
    }

    pub async fn load_wiki_revision(
        &self,
        revision_id: &str,
    ) -> Result<WikiRevision, RepositoryError> {
        let row = sqlx::query(
            "SELECT w.id, w.relative_path, w.before_content, w.after_content, w.before_hash, w.after_hash, w.source_ids_json, w.reason, w.created_at, r.status FROM wiki_revisions w JOIN review_items r ON r.revision_id = w.id WHERE w.id = ?",
        )
        .bind(revision_id)
        .fetch_one(&self.pool)
        .await?;
        wiki_revision_from_row(row)
    }

    pub async fn list_review_items(&self) -> Result<Vec<ReviewItem>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT r.id, r.revision_id, r.status, r.created_at, r.reviewed_at, w.relative_path, w.reason, w.source_ids_json, w.before_content, w.after_content FROM review_items r JOIN wiki_revisions w ON w.id = r.revision_id ORDER BY r.created_at, r.rowid",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(review_item_from_row).collect()
    }

    pub async fn set_review_status(
        &self,
        revision_id: &str,
        status: ReviewStatus,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query(
            "UPDATE review_items SET status = ?, reviewed_at = CURRENT_TIMESTAMP WHERE revision_id = ?",
        )
        .bind(status.as_storage())
        .bind(revision_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::Database(sqlx::Error::RowNotFound));
        }
        Ok(())
    }

    async fn load_conversation(&self, id: &str) -> Result<Conversation, RepositoryError> {
        let row = sqlx::query("SELECT id, title, created_at FROM conversations WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(conversation_from_row(row)?)
    }

    async fn load_message(&self, id: &str) -> Result<Message, RepositoryError> {
        let row = sqlx::query(
            "SELECT id, conversation_id, author_kind, author_id, content, created_at FROM messages WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(message_from_row(row)?)
    }
}

fn conversation_from_row(row: sqlx::sqlite::SqliteRow) -> Result<Conversation, sqlx::Error> {
    Ok(Conversation {
        id: row.try_get("id")?,
        title: row.try_get("title")?,
        created_at: row.try_get("created_at")?,
    })
}

fn message_from_row(row: sqlx::sqlite::SqliteRow) -> Result<Message, sqlx::Error> {
    Ok(Message {
        id: row.try_get("id")?,
        conversation_id: row.try_get("conversation_id")?,
        author_kind: row.try_get("author_kind")?,
        author_id: row.try_get("author_id")?,
        content: row.try_get("content")?,
        created_at: row.try_get("created_at")?,
    })
}

fn source_from_row(row: sqlx::sqlite::SqliteRow) -> Result<SourceRecord, sqlx::Error> {
    Ok(SourceRecord {
        id: row.try_get("id")?,
        kind: row.try_get("kind")?,
        title: row.try_get("title")?,
        source_uri: row.try_get("source_uri")?,
        raw_path: row.try_get("raw_path")?,
        content_hash: row.try_get("content_hash")?,
        extracted_text: row.try_get("extracted_text")?,
        extraction_error: row.try_get("extraction_error")?,
        created_at: row.try_get("created_at")?,
    })
}

fn discussion_record_from_row(
    row: sqlx::sqlite::SqliteRow,
) -> Result<DiscussionRecord, sqlx::Error> {
    Ok(DiscussionRecord {
        id: row.try_get("id")?,
        conversation_id: row.try_get("conversation_id")?,
        trigger_message_id: row.try_get("trigger_message_id")?,
        member_id: row.try_get("member_id")?,
        kind: row.try_get("kind")?,
        status: row.try_get("status")?,
        public_reason: row.try_get("public_reason")?,
        created_at: row.try_get("created_at")?,
    })
}

fn wiki_revision_from_row(row: sqlx::sqlite::SqliteRow) -> Result<WikiRevision, RepositoryError> {
    let status: String = row.try_get("status")?;
    let source_ids_json: String = row.try_get("source_ids_json")?;
    Ok(WikiRevision {
        id: row.try_get("id")?,
        relative_path: row.try_get("relative_path")?,
        before_content: row.try_get("before_content")?,
        after_content: row.try_get("after_content")?,
        before_hash: row.try_get("before_hash")?,
        after_hash: row.try_get("after_hash")?,
        source_ids: serde_json::from_str(&source_ids_json)?,
        reason: row.try_get("reason")?,
        created_at: row.try_get("created_at")?,
        review_pending: status == ReviewStatus::Pending.as_storage(),
    })
}

fn review_item_from_row(row: sqlx::sqlite::SqliteRow) -> Result<ReviewItem, RepositoryError> {
    let stored_status: String = row.try_get("status")?;
    let status = ReviewStatus::from_storage(&stored_status)
        .ok_or_else(|| RepositoryError::InvalidReviewStatus(stored_status.clone()))?;
    let source_ids_json: String = row.try_get("source_ids_json")?;
    Ok(ReviewItem {
        id: row.try_get("id")?,
        revision_id: row.try_get("revision_id")?,
        path: row.try_get("relative_path")?,
        reason: row.try_get("reason")?,
        status,
        source_ids: serde_json::from_str(&source_ids_json)?,
        before_content: row.try_get("before_content")?,
        after_content: row.try_get("after_content")?,
        created_at: row.try_get("created_at")?,
        reviewed_at: row.try_get("reviewed_at")?,
    })
}
