CREATE OR REPLACE FUNCTION refresh_series_total_episodes(target_series_id TEXT)
RETURNS VOID AS $$
BEGIN
    UPDATE series
    SET total_episodes = (
        SELECT COUNT(*)::INTEGER
        FROM episodes
        WHERE series_id = target_series_id
    )
    WHERE id = target_series_id;
END;
$$ LANGUAGE plpgsql;

UPDATE series
SET total_episodes = episode_counts.persisted_episodes
FROM (
    SELECT s.id, COUNT(e.id)::INTEGER AS persisted_episodes
    FROM series s
    LEFT JOIN episodes e ON e.series_id = s.id
    GROUP BY s.id
) AS episode_counts
WHERE series.id = episode_counts.id;

CREATE OR REPLACE FUNCTION maintain_series_total_episodes()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM refresh_series_total_episodes(NEW.series_id);
        RETURN NEW;
    END IF;

    IF TG_OP = 'DELETE' THEN
        PERFORM refresh_series_total_episodes(OLD.series_id);
        RETURN OLD;
    END IF;

    IF OLD.series_id IS DISTINCT FROM NEW.series_id THEN
        PERFORM refresh_series_total_episodes(OLD.series_id);
        PERFORM refresh_series_total_episodes(NEW.series_id);
    ELSE
        PERFORM refresh_series_total_episodes(NEW.series_id);
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_episodes_maintain_series_total ON episodes;

CREATE TRIGGER trg_episodes_maintain_series_total
AFTER INSERT OR UPDATE OF series_id OR DELETE ON episodes
FOR EACH ROW
EXECUTE FUNCTION maintain_series_total_episodes();
