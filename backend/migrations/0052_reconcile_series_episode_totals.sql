UPDATE series
SET total_episodes = (
    SELECT COUNT(*)
    FROM episodes
    WHERE episodes.series_id = series.id
);

CREATE TRIGGER IF NOT EXISTS trg_episodes_total_after_insert
AFTER INSERT ON episodes
BEGIN
    UPDATE series
    SET total_episodes = (
        SELECT COUNT(*)
        FROM episodes
        WHERE series_id = NEW.series_id
    )
    WHERE id = NEW.series_id;
END;

CREATE TRIGGER IF NOT EXISTS trg_episodes_total_after_delete
AFTER DELETE ON episodes
BEGIN
    UPDATE series
    SET total_episodes = (
        SELECT COUNT(*)
        FROM episodes
        WHERE series_id = OLD.series_id
    )
    WHERE id = OLD.series_id;
END;

CREATE TRIGGER IF NOT EXISTS trg_episodes_total_after_update_series
AFTER UPDATE OF series_id ON episodes
BEGIN
    UPDATE series
    SET total_episodes = (
        SELECT COUNT(*)
        FROM episodes
        WHERE series_id = OLD.series_id
    )
    WHERE id = OLD.series_id;

    UPDATE series
    SET total_episodes = (
        SELECT COUNT(*)
        FROM episodes
        WHERE series_id = NEW.series_id
    )
    WHERE id = NEW.series_id;
END;
