use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{Conversation, ConversationThread, Message, ModelMember, ProviderKind};

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("database migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("stored provider is invalid: {0}")]
    InvalidProvider(String),
}

#[derive(Clone)]
pub struct WorkspaceRepository {
    pool: SqlitePool,
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

    async fn load_conversation(&self, id: &str) -> Result<Conversation, RepositoryError> {
        let row = sqlx::query("SELECT id, title, created_at FROM conversations WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(Conversation {
            id: row.try_get("id")?,
            title: row.try_get("title")?,
            created_at: row.try_get("created_at")?,
        })
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
