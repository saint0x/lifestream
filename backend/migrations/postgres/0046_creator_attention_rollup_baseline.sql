ALTER TABLE creator_attention_daily
ADD COLUMN baseline_value_per_qualified_viewer REAL NOT NULL DEFAULT 0.05;
