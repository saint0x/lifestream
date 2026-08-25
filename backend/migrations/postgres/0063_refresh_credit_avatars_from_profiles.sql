UPDATE series
SET credits_json = (
    SELECT json_agg(json_build_object(
        'id', cc.id,
        'personId', p.id,
        'personSlug', p.slug,
        'name', p.display_name,
        'role', cc.role,
        'character', cc.character,
        'avatar', COALESCE(NULLIF(cp.avatar, ''), NULLIF(u.avatar, ''), NULLIF(p.avatar, ''), '')
    ) ORDER BY cc.credit_order)::TEXT
    FROM content_credits cc
    JOIN person_profiles p ON p.id = cc.person_id
    LEFT JOIN users u ON u.id = p.user_id
    LEFT JOIN creator_profiles cp ON cp.user_id = p.user_id
    WHERE cc.content_kind = 'series' AND cc.content_id = series.id
)
WHERE EXISTS (
    SELECT 1
    FROM content_credits cc
    WHERE cc.content_kind = 'series' AND cc.content_id = series.id
);

UPDATE films
SET credits_json = (
    SELECT json_agg(json_build_object(
        'id', cc.id,
        'personId', p.id,
        'personSlug', p.slug,
        'name', p.display_name,
        'role', cc.role,
        'character', cc.character,
        'avatar', COALESCE(NULLIF(cp.avatar, ''), NULLIF(u.avatar, ''), NULLIF(p.avatar, ''), '')
    ) ORDER BY cc.credit_order)::TEXT
    FROM content_credits cc
    JOIN person_profiles p ON p.id = cc.person_id
    LEFT JOIN users u ON u.id = p.user_id
    LEFT JOIN creator_profiles cp ON cp.user_id = p.user_id
    WHERE cc.content_kind = 'film' AND cc.content_id = films.id
)
WHERE EXISTS (
    SELECT 1
    FROM content_credits cc
    WHERE cc.content_kind = 'film' AND cc.content_id = films.id
);
