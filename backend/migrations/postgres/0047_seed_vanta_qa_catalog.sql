INSERT INTO categories (slug, name, cover_image, live_viewers, live_channels, tags_json) VALUES
('cinematic-tech', 'Cinematic Tech', 'https://images.unsplash.com/photo-1518709268805-4e9042af2176?auto=format&fit=crop&w=1400&q=80', 0, 0, '["ai","workflow","production"]'),
('documentary', 'Documentary', 'https://images.unsplash.com/photo-1500530855697-b586d89ba3ee?auto=format&fit=crop&w=1400&q=80', 0, 0, '["field","culture","craft"]'),
('music', 'Music', 'https://images.unsplash.com/photo-1516280440614-37939bbacd81?auto=format&fit=crop&w=1400&q=80', 0, 0, '["studio","performance","mix"]'),
('science-fiction', 'Science Fiction', 'https://images.unsplash.com/photo-1446776811953-b23d57bd21aa?auto=format&fit=crop&w=1400&q=80', 0, 0, '["future","space","systems"]')
ON CONFLICT (slug) DO UPDATE
SET name = EXCLUDED.name,
    cover_image = EXCLUDED.cover_image,
    live_viewers = EXCLUDED.live_viewers,
    live_channels = EXCLUDED.live_channels,
    tags_json = EXCLUDED.tags_json;

INSERT INTO streamers (id, handle, display_name, avatar, bio, followers, is_partner, is_live) VALUES
('str-aria-labs', 'arialabs', 'Aria Labs', 'https://images.unsplash.com/photo-1494790108377-be9c29b29330?auto=format&fit=crop&w=320&q=80', 'Realtime filmmaking, model tooling, and production experiments.', 184200, 1, 1),
('str-noor-frame', 'noorframe', 'Noor Frame', 'https://images.unsplash.com/photo-1500648767791-00dcc994a43e?auto=format&fit=crop&w=320&q=80', 'Documentary director building polished field journals live.', 98200, 1, 1),
('str-kai-signal', 'kaisignal', 'Kai Signal', 'https://images.unsplash.com/photo-1534528741775-53994a69daeb?auto=format&fit=crop&w=320&q=80', 'Sound design, score sessions, and late-night mix breakdowns.', 76300, 1, 1)
ON CONFLICT (id) DO UPDATE
SET handle = EXCLUDED.handle,
    display_name = EXCLUDED.display_name,
    avatar = EXCLUDED.avatar,
    bio = EXCLUDED.bio,
    followers = EXCLUDED.followers,
    is_partner = EXCLUDED.is_partner,
    is_live = EXCLUDED.is_live;

INSERT INTO live_streams (id, slug, title, category, tags_json, streamer_id, viewers, started_at, thumbnail, language, is_mature) VALUES
('live-shader-wall', 'shader-wall-build', 'Building a Realtime Shader Wall', 'Cinematic Tech', '["graphics","tools","live-build"]', 'str-aria-labs', 12840, '2026-08-24T00:18:00Z', 'https://images.unsplash.com/photo-1550745165-9bc0b252726f?auto=format&fit=crop&w=1400&q=80', 'en', 0),
('live-field-cut', 'field-cut-room', 'Field Cut Room: Coastline Edit', 'Documentary', '["edit","field","review"]', 'str-noor-frame', 7310, '2026-08-24T00:02:00Z', 'https://images.unsplash.com/photo-1492691527719-9d1e07e534b4?auto=format&fit=crop&w=1400&q=80', 'en', 0),
('live-score-pass', 'score-pass-afterhours', 'Afterhours Score Pass', 'Music', '["score","synth","mix"]', 'str-kai-signal', 5124, '2026-08-23T23:44:00Z', 'https://images.unsplash.com/photo-1511379938547-c1f69419868d?auto=format&fit=crop&w=1400&q=80', 'en', 0)
ON CONFLICT (id) DO UPDATE
SET slug = EXCLUDED.slug,
    title = EXCLUDED.title,
    category = EXCLUDED.category,
    tags_json = EXCLUDED.tags_json,
    streamer_id = EXCLUDED.streamer_id,
    viewers = EXCLUDED.viewers,
    started_at = EXCLUDED.started_at,
    thumbnail = EXCLUDED.thumbnail,
    language = EXCLUDED.language,
    is_mature = EXCLUDED.is_mature;

INSERT INTO series (id, slug, title, tagline, synopsis, year, rating, genres_json, images_json, credits_json, score, is_original, trending, hero_color, status, total_episodes) VALUES
('ser-northlight', 'northlight', 'Northlight', 'A quiet signal at the edge of the grid.', 'A systems engineer follows an unexplained aurora across remote observatories and discovers a network waking up beneath the ice.', 2026, 'TV-14', '["Science Fiction","Cinematic Tech"]', '{"poster":"https://images.unsplash.com/photo-1483347756197-71ef80e95f73?auto=format&fit=crop&w=900&q=80","backdrop":"https://images.unsplash.com/photo-1500534314209-a25ddb2bd429?auto=format&fit=crop&w=1600&q=80","thumbnail":"https://images.unsplash.com/photo-1446776811953-b23d57bd21aa?auto=format&fit=crop&w=900&q=80","logo":null}', '[{"id":"cr-northlight-1","name":"Mara Vale","role":"Creator","character":null},{"id":"cr-northlight-2","name":"Ilya Ren","role":"Director","character":null}]', 98, 1, 1, '#b8e7ff', 'ongoing', 6),
('ser-cutline', 'cutline', 'Cutline', 'Every frame has a witness.', 'A documentary unit rebuilds decisive moments from raw footage, production notes, and the people who were almost left out of the final edit.', 2025, 'TV-PG', '["Documentary","Cinematic Tech"]', '{"poster":"https://images.unsplash.com/photo-1485846234645-a62644f84728?auto=format&fit=crop&w=900&q=80","backdrop":"https://images.unsplash.com/photo-1492691527719-9d1e07e534b4?auto=format&fit=crop&w=1600&q=80","thumbnail":"https://images.unsplash.com/photo-1485846234645-a62644f84728?auto=format&fit=crop&w=900&q=80","logo":null}', '[{"id":"cr-cutline-1","name":"Noor Frame","role":"Creator","character":null},{"id":"cr-cutline-2","name":"Ada Chen","role":"Producer","character":null}]', 93, 1, 1, '#f4d49a', 'ongoing', 4),
('ser-signal-room', 'signal-room', 'Signal Room', 'Live tools, finished taste.', 'Builders, editors, and performers use live production systems to ship polished creative work under real constraints.', 2026, 'TV-PG', '["Cinematic Tech","Music"]', '{"poster":"https://images.unsplash.com/photo-1516280440614-37939bbacd81?auto=format&fit=crop&w=900&q=80","backdrop":"https://images.unsplash.com/photo-1511379938547-c1f69419868d?auto=format&fit=crop&w=1600&q=80","thumbnail":"https://images.unsplash.com/photo-1550745165-9bc0b252726f?auto=format&fit=crop&w=900&q=80","logo":null}', '[{"id":"cr-signal-1","name":"Kai Signal","role":"Host","character":null},{"id":"cr-signal-2","name":"Aria Labs","role":"Production","character":null}]', 90, 1, 1, '#9cffd6', 'ongoing', 5)
ON CONFLICT (id) DO UPDATE
SET slug = EXCLUDED.slug,
    title = EXCLUDED.title,
    tagline = EXCLUDED.tagline,
    synopsis = EXCLUDED.synopsis,
    year = EXCLUDED.year,
    rating = EXCLUDED.rating,
    genres_json = EXCLUDED.genres_json,
    images_json = EXCLUDED.images_json,
    credits_json = EXCLUDED.credits_json,
    score = EXCLUDED.score,
    is_original = EXCLUDED.is_original,
    trending = EXCLUDED.trending,
    hero_color = EXCLUDED.hero_color,
    status = EXCLUDED.status,
    total_episodes = EXCLUDED.total_episodes;

INSERT INTO seasons (series_id, season_number, title) VALUES
('ser-northlight', 1, 'Signal'),
('ser-northlight', 2, 'Whiteout'),
('ser-cutline', 1, 'Assembly'),
('ser-signal-room', 1, 'Live Systems')
ON CONFLICT (series_id, season_number) DO UPDATE
SET title = EXCLUDED.title;

INSERT INTO episodes (id, series_id, season_number, episode_number, title, synopsis, duration_sec, aired_at, thumbnail) VALUES
('ser-northlight-s1e1', 'ser-northlight', 1, 1, 'Aurora Index', 'Mara finds a signal hidden in a weather archive and loses contact with the first relay station.', 3040, '2026-05-14T00:00:00Z', 'https://images.unsplash.com/photo-1446776811953-b23d57bd21aa?auto=format&fit=crop&w=900&q=80'),
('ser-northlight-s1e2', 'ser-northlight', 1, 2, 'Relay Drift', 'The team follows the interference north as the network starts predicting their route.', 3180, '2026-05-21T00:00:00Z', 'https://images.unsplash.com/photo-1473923377535-0002805f57e8?auto=format&fit=crop&w=900&q=80'),
('ser-northlight-s1e3', 'ser-northlight', 1, 3, 'Cold Boot', 'An offline observatory powers itself on and reveals a map no one built.', 3220, '2026-05-28T00:00:00Z', 'https://images.unsplash.com/photo-1519681393784-d120267933ba?auto=format&fit=crop&w=900&q=80'),
('ser-northlight-s2e1', 'ser-northlight', 2, 1, 'Under Ice', 'Season two opens below the shelf, where the signal has a physical address.', 3100, '2026-07-09T00:00:00Z', 'https://images.unsplash.com/photo-1482192505345-5655af888cc4?auto=format&fit=crop&w=900&q=80'),
('ser-northlight-s2e2', 'ser-northlight', 2, 2, 'Low Sun', 'A supply pilot brings back impossible footage from a station that should not exist.', 3090, '2026-07-16T00:00:00Z', 'https://images.unsplash.com/photo-1500534314209-a25ddb2bd429?auto=format&fit=crop&w=900&q=80'),
('ser-northlight-s2e3', 'ser-northlight', 2, 3, 'Blue Return', 'The network answers in daylight and forces Mara to choose between proof and rescue.', 3120, '2026-07-23T00:00:00Z', 'https://images.unsplash.com/photo-1500530855697-b586d89ba3ee?auto=format&fit=crop&w=900&q=80'),
('ser-cutline-s1e1', 'ser-cutline', 1, 1, 'The Missing Slate', 'A single missing slate changes how a coastal story is remembered.', 2860, '2026-03-11T00:00:00Z', 'https://images.unsplash.com/photo-1492691527719-9d1e07e534b4?auto=format&fit=crop&w=900&q=80'),
('ser-cutline-s1e2', 'ser-cutline', 1, 2, 'Room Tone', 'Editors return to field audio and find the scene that makes the film honest.', 2920, '2026-03-18T00:00:00Z', 'https://images.unsplash.com/photo-1485846234645-a62644f84728?auto=format&fit=crop&w=900&q=80'),
('ser-signal-room-s1e1', 'ser-signal-room', 1, 1, 'Switching Live', 'The crew builds a compact command room for a live cinematic stream.', 2440, '2026-06-01T00:00:00Z', 'https://images.unsplash.com/photo-1550745165-9bc0b252726f?auto=format&fit=crop&w=900&q=80'),
('ser-signal-room-s1e2', 'ser-signal-room', 1, 2, 'Score Bus', 'A live score session routes synths, stems, and audience cues into one timeline.', 2520, '2026-06-08T00:00:00Z', 'https://images.unsplash.com/photo-1511379938547-c1f69419868d?auto=format&fit=crop&w=900&q=80')
ON CONFLICT (id) DO UPDATE
SET series_id = EXCLUDED.series_id,
    season_number = EXCLUDED.season_number,
    episode_number = EXCLUDED.episode_number,
    title = EXCLUDED.title,
    synopsis = EXCLUDED.synopsis,
    duration_sec = EXCLUDED.duration_sec,
    aired_at = EXCLUDED.aired_at,
    thumbnail = EXCLUDED.thumbnail;

INSERT INTO films (id, slug, title, tagline, synopsis, year, rating, genres_json, images_json, credits_json, score, is_original, trending, hero_color, duration_sec) VALUES
('film-ghost-standard', 'ghost-standard', 'Ghost Standard', 'A protocol no one remembers keeps passing audit.', 'When an old broadcast standard begins authenticating impossible footage, a compliance engineer tracks the source through a maze of archives, live rooms, and forgotten hardware.', 2026, 'PG-13', '["Science Fiction","Cinematic Tech"]', '{"poster":"https://images.unsplash.com/photo-1518709268805-4e9042af2176?auto=format&fit=crop&w=900&q=80","backdrop":"https://images.unsplash.com/photo-1517976487492-5750f3195933?auto=format&fit=crop&w=1600&q=80","thumbnail":"https://images.unsplash.com/photo-1518709268805-4e9042af2176?auto=format&fit=crop&w=900&q=80","logo":null}', '[{"id":"cr-ghost-1","name":"Rin Vale","role":"Writer","character":null},{"id":"cr-ghost-2","name":"Aria Labs","role":"Studio","character":null}]', 96, 1, 1, '#c7f5ff', 6420),
('film-after-the-cut', 'after-the-cut', 'After the Cut', 'The last edit is never the last story.', 'A feature documentary follows three editors racing to finish a premiere while the footage keeps changing what the film wants to be.', 2025, 'PG', '["Documentary"]', '{"poster":"https://images.unsplash.com/photo-1492691527719-9d1e07e534b4?auto=format&fit=crop&w=900&q=80","backdrop":"https://images.unsplash.com/photo-1485846234645-a62644f84728?auto=format&fit=crop&w=1600&q=80","thumbnail":"https://images.unsplash.com/photo-1492691527719-9d1e07e534b4?auto=format&fit=crop&w=900&q=80","logo":null}', '[{"id":"cr-after-1","name":"Noor Frame","role":"Director","character":null},{"id":"cr-after-2","name":"Mina Hart","role":"Editor","character":null}]', 91, 1, 1, '#f7d8ac', 5340),
('film-night-mix', 'night-mix', 'Night Mix', 'A city heard through one room.', 'A music film captures one overnight studio session as a score evolves from loose sketches into a finished performance.', 2026, 'PG', '["Music","Documentary"]', '{"poster":"https://images.unsplash.com/photo-1516280440614-37939bbacd81?auto=format&fit=crop&w=900&q=80","backdrop":"https://images.unsplash.com/photo-1511379938547-c1f69419868d?auto=format&fit=crop&w=1600&q=80","thumbnail":"https://images.unsplash.com/photo-1516280440614-37939bbacd81?auto=format&fit=crop&w=900&q=80","logo":null}', '[{"id":"cr-night-1","name":"Kai Signal","role":"Composer","character":null},{"id":"cr-night-2","name":"June Park","role":"Director","character":null}]', 88, 1, 0, '#adffda', 4980)
ON CONFLICT (id) DO UPDATE
SET slug = EXCLUDED.slug,
    title = EXCLUDED.title,
    tagline = EXCLUDED.tagline,
    synopsis = EXCLUDED.synopsis,
    year = EXCLUDED.year,
    rating = EXCLUDED.rating,
    genres_json = EXCLUDED.genres_json,
    images_json = EXCLUDED.images_json,
    credits_json = EXCLUDED.credits_json,
    score = EXCLUDED.score,
    is_original = EXCLUDED.is_original,
    trending = EXCLUDED.trending,
    hero_color = EXCLUDED.hero_color,
    duration_sec = EXCLUDED.duration_sec;
