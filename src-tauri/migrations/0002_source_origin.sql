ALTER TABLE sources ADD COLUMN source_uri TEXT NOT NULL DEFAULT '';

CREATE INDEX sources_content_hash_idx ON sources(content_hash);
