PRAGMA foreign_keys = ON;

CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE members (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    role_name TEXT NOT NULL,
    role_instruction TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX members_conversation_idx ON members(conversation_id, created_at);

CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    author_kind TEXT NOT NULL CHECK(author_kind IN ('user', 'model', 'system')),
    author_id TEXT,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX messages_conversation_idx ON messages(conversation_id, created_at);

CREATE TABLE events (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    trigger_message_id TEXT REFERENCES messages(id) ON DELETE SET NULL,
    member_id TEXT REFERENCES members(id) ON DELETE SET NULL,
    kind TEXT NOT NULL,
    status TEXT NOT NULL,
    public_reason TEXT,
    payload_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX events_conversation_idx ON events(conversation_id, created_at);

CREATE TABLE sources (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    raw_path TEXT NOT NULL UNIQUE,
    content_hash TEXT NOT NULL,
    extracted_text TEXT,
    extraction_error TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE citations (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    excerpt TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX citations_message_idx ON citations(message_id);
CREATE INDEX citations_source_idx ON citations(source_id);

CREATE TABLE wiki_revisions (
    id TEXT PRIMARY KEY,
    relative_path TEXT NOT NULL,
    before_content TEXT,
    after_content TEXT NOT NULL,
    before_hash TEXT,
    after_hash TEXT NOT NULL,
    source_ids_json TEXT NOT NULL DEFAULT '[]',
    reason TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX wiki_revisions_path_idx ON wiki_revisions(relative_path, created_at);

CREATE TABLE review_items (
    id TEXT PRIMARY KEY,
    revision_id TEXT NOT NULL UNIQUE REFERENCES wiki_revisions(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'accepted', 'incorrect', 'rolled_back')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    reviewed_at TEXT
);

CREATE INDEX review_items_status_idx ON review_items(status, created_at);
