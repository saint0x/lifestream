# Database Provider Boundary

This backend now treats `Database` as the provider entry point. Provider-neutral behavior belongs on
`Database`; provider-specific SQL belongs behind repository adapters and must not be introduced in HTTP
handler code as an accidental dependency.

## Active Providers

- `sqlite`: local and development provider. It owns SQLite PRAGMAs, the SQLite migration set, FTS5 search,
  SQLite JSON expressions, and SQLite date/time expressions.
- `postgres`: production provider. It is selected with `VANTA_DATABASE_KIND=postgres` and must use
  Postgres-specific repository implementations instead of the SQLite adapter.

## Compatibility Plan

- Search: SQLite FTS5 queries stay in the SQLite catalog adapter. Postgres search should use `tsvector`,
  `websearch_to_tsquery`, weighted ranks, and trigram fallback for short terms.
- JSON querying: SQLite `json_extract` and text JSON payload reads stay in SQLite adapters. Postgres
  equivalents should use `jsonb`, `->`, `->>`, containment operators, and expression indexes.
- Time math: SQLite `datetime`, `strftime`, and text timestamp comparisons stay in SQLite adapters.
  Postgres equivalents should use `timestamptz`, intervals, and clock source boundaries passed from
  application code when deterministic tests require fixed time.
- Conflict handling: SQLite `INSERT OR IGNORE` and `ON CONFLICT` forms stay in SQLite adapters. Postgres
  equivalents should use explicit conflict targets and `RETURNING` where handlers need the authoritative row.
- Row models: `SqliteRow` mapping belongs in SQLite adapter code. HTTP handlers should consume domain
  structs and repository/service methods.

## Domain Interfaces To Own Before Postgres Expansion

- Auth and sessions: already backed by provider-aware `Database` methods.
- Viewer profile, settings, watchlist, following, notifications, and playback sessions: already started on
  provider-aware `Database` methods and should remain there.
- Catalog and discovery: route modules should call catalog repository/service methods; provider-specific
  FTS and discovery joins stay in provider adapters.
- Creator live/control plane: route modules should call creator/control services; provider-specific telemetry,
  socket presence, and reconciliation SQL stay in provider adapters.
- Uploads and media pipeline: job lifecycle and publication state should be exposed through media repository
  methods; file/object ownership remains in `Storage`.
- Playback grants and sessions: grant issuance is provider-neutral at the service boundary; target lookup,
  entitlement checks, and session persistence belong in database adapters.
- Moderation, collaboration, notifications, and reconciliation: route handlers should orchestrate identity,
  validation, and response shape while provider adapters own SQL and row mapping.

## Current SQLite Adapter Inventory

The current codebase still contains legacy SQLite adapter usage while the migration proceeds. The
inventory excludes tests, `db.rs`, and `main.rs`; `main.rs` is the provider bootstrap entry point
that selects and initializes the active database provider.

- SQLite adapter call sites in backend source: 656.
- `SqlitePool`/`SqliteRow` references in backend source: 395.

Those counts are guarded by `provider_boundary_audit_stays_in_sync`. Lowering them is always allowed;
raising them requires either migrating the new logic behind `Database` or intentionally updating this
document and the audit test in the same change.
