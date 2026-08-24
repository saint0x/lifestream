ALTER TABLE chat_messages
ADD COLUMN user_id TEXT REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE chat_messages
ADD COLUMN creator_id TEXT REFERENCES creator_profiles(id) ON DELETE SET NULL;

UPDATE chat_messages
SET user_id = (
    SELECT users.id
    FROM users
    WHERE users.handle = chat_messages.user_handle
)
WHERE user_id IS NULL;

UPDATE chat_messages
SET creator_id = (
    SELECT creator_profiles.id
    FROM creator_profiles
    JOIN users ON users.id = creator_profiles.user_id
    WHERE users.id = chat_messages.user_id
)
WHERE creator_id IS NULL AND user_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_chat_messages_stream_user_sent_at
ON chat_messages(stream_id, user_id, sent_at DESC)
WHERE user_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_chat_messages_stream_creator_sent_at
ON chat_messages(stream_id, creator_id, sent_at DESC)
WHERE creator_id IS NOT NULL;
