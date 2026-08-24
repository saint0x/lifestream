ALTER TABLE chat_messages
ADD COLUMN sequence INTEGER NOT NULL DEFAULT 0;

UPDATE chat_messages AS target
SET sequence = (
    SELECT COUNT(*)
    FROM chat_messages AS prior
    WHERE prior.stream_id = target.stream_id
      AND (
        prior.sent_at < target.sent_at
        OR (prior.sent_at = target.sent_at AND prior.id <= target.id)
      )
)
WHERE target.sequence = 0;

CREATE UNIQUE INDEX IF NOT EXISTS idx_chat_messages_stream_sequence
ON chat_messages(stream_id, sequence);

CREATE TABLE IF NOT EXISTS chat_stream_cursors (
    stream_id TEXT PRIMARY KEY,
    last_sequence INTEGER NOT NULL,
    FOREIGN KEY (stream_id) REFERENCES live_streams(id) ON DELETE CASCADE
);

INSERT INTO chat_stream_cursors (stream_id, last_sequence)
SELECT stream_id, MAX(sequence)
FROM chat_messages
GROUP BY stream_id
ON CONFLICT(stream_id) DO UPDATE SET
    last_sequence = excluded.last_sequence;
