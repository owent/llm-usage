CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    parent_session_id TEXT,
    title TEXT NOT NULL,
    message_count INTEGER NOT NULL DEFAULT 0 CHECK (message_count >= 0),
    prompt_tokens  INTEGER NOT NULL DEFAULT 0 CHECK (prompt_tokens >= 0),
    completion_tokens  INTEGER NOT NULL DEFAULT 0 CHECK (completion_tokens>= 0),
    cost REAL NOT NULL DEFAULT 0.0 CHECK (cost >= 0.0),
    updated_at INTEGER NOT NULL,  -- Unix timestamp in seconds
    created_at INTEGER NOT NULL   -- Unix timestamp in seconds
, summary_message_id TEXT, todos TEXT, channel TEXT);
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL,
    parts TEXT NOT NULL default '[]',
    model TEXT,
    created_at INTEGER NOT NULL,  -- Unix timestamp in seconds
    updated_at INTEGER NOT NULL,  -- Unix timestamp in seconds
    finished_at INTEGER, provider TEXT, is_summary_message INTEGER DEFAULT 0 NOT NULL, prism_model_id TEXT, prism_model_name TEXT, prism_hypercredit_savings REAL, prism_dollar_savings REAL,  -- Unix timestamp in seconds
    FOREIGN KEY (session_id) REFERENCES sessions (id) ON DELETE CASCADE
);
INSERT INTO sessions ("id","parent_session_id","title","message_count","prompt_tokens","completion_tokens","cost","updated_at","created_at","summary_message_id","todos","channel") VALUES ('11111111-2222-4333-8444-555555555555',NULL,'REDACTED',2,4899,2,0.005059,1791273572,1791273568,NULL,NULL,NULL);
INSERT INTO messages ("id","session_id","role","model","provider","created_at","updated_at","finished_at","is_summary_message","parts") VALUES ('00000000-0000-4000-8000-000000000001','11111111-2222-4333-8444-555555555555','assistant','qwen2.5-0.5b-local','local-model',1791273568,1791273572,1791273572,0,'[]');
INSERT INTO messages ("id","session_id","role","model","provider","created_at","updated_at","finished_at","is_summary_message","parts") VALUES ('00000000-0000-4000-8000-000000000002','11111111-2222-4333-8444-555555555555','user','',NULL,1791273568,1791273568,NULL,0,'[]');
