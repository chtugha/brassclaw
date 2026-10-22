use std::collections::BTreeMap;

use async_trait::async_trait;
use brassclaw_host_api::VirtualPath;

use crate::backend::EventRecord;
use crate::db::{
    child_path_like_pattern, db_error, direct_children, directory_append_error,
    directory_write_error, escape_like_literal, escape_like_with_trailing_wildcard,
    infrastructure_error, infrastructure_pg_error, is_not_found, not_found, page_offset_to_i64,
    record_version_from_i64, record_version_to_i64, sql_index_name, system_time_from_unix_seconds,
};
use crate::vector::{cosine_similarity, decode_embedding_blob};
use crate::{
    BackendCapabilities, Capability, CasExpectation, ContentType, DirEntry, Entry,
    EntryIndexMetadata, EntryMetadata, FileStat, FileType, FilesystemError, FilesystemOperation,
    Filter, IndexKey, IndexKind, IndexSpec, IndexValue, Page, RecordKind, RecordVersion,
    RootFilesystem, SeqNo, VersionedEntry,
};

#[cfg(feature = "postgres")]
/// PostgreSQL-backed [`RootFilesystem`] storing file contents by virtual path.
pub struct PostgresRootFilesystem {
    pool: deadpool_postgres::Pool,
}

#[cfg(feature = "postgres")]
fn decode_metadata(
    requested: &VirtualPath,
    row: &tokio_postgres::Row,
    operation: FilesystemOperation,
) -> Result<EntryMetadata, FilesystemError> {
    let invalid = || FilesystemError::Backend {
        path: requested.clone(),
        operation,
        reason: "stored entry metadata is invalid".into(),
    };
    let path: String = row.get("path");
    let kind: Option<String> = row.get("kind");
    Ok(EntryMetadata {
        path: VirtualPath::new(path).map_err(|_| invalid())?,
        kind: kind
            .map(RecordKind::new)
            .transpose()
            .map_err(|_| invalid())?,
    })
}

#[cfg(feature = "postgres")]
impl PostgresRootFilesystem {
    async fn put_entry(
        &self,
        path: &VirtualPath,
        entry: Entry,
        cas: CasExpectation,
        opaque_only: bool,
    ) -> Result<RecordVersion, FilesystemError> {
        let indexed_json = serde_json::to_value(&entry.indexed).map_err(|_| {
            FilesystemError::SerializeIndexed {
                path: path.clone(),
                operation: FilesystemOperation::WriteFile,
            }
        })?;
        let kind_str = entry.kind.as_ref().map(|k| k.as_str().to_string());
        let content_type_str = entry.content_type.as_str().to_string();
        let body = entry.body;
        let path_str = path.as_str();

        let mut client = self.client().await?;
        let transaction = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::WriteFile, error))?;
        Self::validate_file_namespace(&transaction, path, FilesystemOperation::WriteFile).await?;

        let version = match cas {
            CasExpectation::Absent => {
                let rows = transaction
                    .execute(
                        r#"
                        INSERT INTO root_filesystem_entries
                            (path, contents, is_dir, content_type, kind, indexed, version)
                        VALUES ($1, $2, FALSE, $3, $4, $5, 1)
                        ON CONFLICT (path) DO NOTHING
                        "#,
                        &[
                            &path_str,
                            &body,
                            &content_type_str,
                            &kind_str,
                            &indexed_json,
                        ],
                    )
                    .await
                    .map_err(|error| {
                        db_error(path.clone(), FilesystemOperation::WriteFile, error)
                    })?;
                if rows == 0 {
                    let found = self
                        .current_version_with_transaction(&transaction, path)
                        .await?;
                    return Err(FilesystemError::VersionMismatch {
                        path: path.clone(),
                        expected: None,
                        found,
                    });
                }
                Ok(RecordVersion::from_backend(1))
            }
            CasExpectation::Version(expected) => {
                let expected_raw = record_version_to_i64(path, expected)?;
                let rows = transaction
                    .execute(
                        r#"
                        UPDATE root_filesystem_entries
                        SET contents = $1,
                            content_type = $2,
                            kind = $3,
                            indexed = $4,
                            version = version + 1,
                            updated_at = NOW()
                        WHERE path = $5 AND is_dir = FALSE AND version = $6
                            AND (NOT $7 OR kind IS NULL)
                        "#,
                        &[
                            &body,
                            &content_type_str,
                            &kind_str,
                            &indexed_json,
                            &path_str,
                            &expected_raw,
                            &opaque_only,
                        ],
                    )
                    .await
                    .map_err(|error| {
                        db_error(path.clone(), FilesystemOperation::WriteFile, error)
                    })?;
                if rows == 0 {
                    let found = self
                        .current_version_with_transaction(&transaction, path)
                        .await?;
                    return Err(FilesystemError::VersionMismatch {
                        path: path.clone(),
                        expected: Some(expected),
                        found,
                    });
                }
                expected
                    .checked_next()
                    .ok_or_else(|| FilesystemError::Backend {
                        path: path.clone(),
                        operation: FilesystemOperation::WriteFile,
                        reason: "entry version representation exhausted".into(),
                    })
            }
            CasExpectation::Any => {
                let row = transaction
                    .query_opt(
                        r#"
                        INSERT INTO root_filesystem_entries
                            (path, contents, is_dir, content_type, kind, indexed, version)
                        VALUES ($1, $2, FALSE, $3, $4, $5, 1)
                        ON CONFLICT (path) DO UPDATE SET
                            contents = EXCLUDED.contents,
                            content_type = EXCLUDED.content_type,
                            kind = EXCLUDED.kind,
                            indexed = EXCLUDED.indexed,
                            version = root_filesystem_entries.version + 1,
                            updated_at = NOW()
                        WHERE root_filesystem_entries.is_dir = FALSE
                            AND (NOT $6 OR root_filesystem_entries.kind IS NULL)
                        RETURNING version
                        "#,
                        &[
                            &path_str,
                            &body,
                            &content_type_str,
                            &kind_str,
                            &indexed_json,
                            &opaque_only,
                        ],
                    )
                    .await
                    .map_err(|error| {
                        db_error(path.clone(), FilesystemOperation::WriteFile, error)
                    })?;
                let Some(row) = row else {
                    return Err(if opaque_only {
                        FilesystemError::Unsupported {
                            path: path.clone(),
                            operation: FilesystemOperation::WriteFile,
                        }
                    } else {
                        directory_write_error(path.clone())
                    });
                };
                let version: i64 = row.get("version");
                record_version_from_i64(path, version)
            }
        }?;
        transaction
            .commit()
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::WriteFile, error))?;
        Ok(version)
    }

    pub fn new(pool: deadpool_postgres::Pool) -> Self {
        Self { pool }
    }

    pub async fn run_migrations(&self) -> Result<(), FilesystemError> {
        let client = self.client().await?;
        client
            .batch_execute(POSTGRES_ROOT_FILESYSTEM_SCHEMA)
            .await
            .map_err(|error| infrastructure_pg_error(FilesystemOperation::CreateDirAll, error))
    }

    async fn client(&self) -> Result<deadpool_postgres::Object, FilesystemError> {
        self.pool
            .get()
            .await
            .map_err(|error| infrastructure_error(FilesystemOperation::Stat, error.to_string()))
    }
}

#[cfg(feature = "postgres")]
#[async_trait]
impl RootFilesystem for PostgresRootFilesystem {
    fn capabilities(&self) -> BackendCapabilities {
        // sql_typical: read/write/append/list/stat/delete/records/query/
        // IndexExact/IndexPrefix/CAS. Events join the set with the V30
        // append/tail backing table. Postgres has native `tsvector` /
        // `plainto_tsquery` so we advertise IndexFts. Vector indexing is
        // currently a brute-force cosine ranker against `indexed->>'key'`
        // values stored as IndexValue::Bytes; we advertise IndexVector but
        // do not require pgvector.
        BackendCapabilities::sql_typical()
            .with(Capability::Events)
            .with(Capability::EntryMetadata)
            .with(Capability::EntryIndexMetadata)
            .with(Capability::OpaqueWrite)
            .with(Capability::IndexFts)
            .with(Capability::IndexVector)
    }

    async fn put(
        &self,
        path: &VirtualPath,
        entry: Entry,
        cas: CasExpectation,
    ) -> Result<RecordVersion, FilesystemError> {
        self.put_entry(path, entry, cas, false).await
    }

    async fn put_opaque(
        &self,
        path: &VirtualPath,
        body: Vec<u8>,
        cas: CasExpectation,
    ) -> Result<RecordVersion, FilesystemError> {
        self.put_entry(path, Entry::bytes(body), cas, true).await
    }

    async fn get(&self, path: &VirtualPath) -> Result<Option<VersionedEntry>, FilesystemError> {
        let client = self.client().await?;
        let row = client
            .query_opt(
                r#"
                SELECT contents, is_dir, content_type, kind, indexed, version
                FROM root_filesystem_entries
                WHERE path = $1
                "#,
                &[&path.as_str()],
            )
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::ReadFile, error))?;
        let Some(row) = row else {
            return Ok(None);
        };
        let is_dir: bool = row.get("is_dir");
        if is_dir {
            return Ok(None);
        }
        let body: Vec<u8> = row.get("contents");
        let content_type_raw: String = row.get("content_type");
        let kind_raw: Option<String> = row.get("kind");
        let indexed_value: serde_json::Value = row.get("indexed");
        let version_raw: i64 = row.get("version");
        let entry = build_entry(path, body, content_type_raw, kind_raw, indexed_value)?;
        Ok(Some(VersionedEntry {
            path: path.clone(),
            entry,
            version: record_version_from_i64(path, version_raw)?,
        }))
    }

    async fn ensure_index(
        &self,
        path: &VirtualPath,
        spec: &IndexSpec,
    ) -> Result<(), FilesystemError> {
        let kind_str = match &spec.kind {
            IndexKind::Exact => "exact".to_string(),
            IndexKind::Prefix => "prefix".to_string(),
            IndexKind::Fts => "fts".to_string(),
            IndexKind::Vector { dim } => format!("vector:{dim}"),
        };
        if spec.keys.is_empty() {
            return Err(FilesystemError::IndexConflict {
                path: path.clone(),
                name: spec.name.clone(),
                reason: crate::IndexConflictReason::EmptyKeys,
            });
        }
        let keys_json = serde_json::to_value(
            spec.keys
                .iter()
                .map(|k| k.as_str().to_string())
                .collect::<Vec<_>>(),
        )
        .map_err(|_| FilesystemError::SerializeIndexed {
            path: path.clone(),
            operation: FilesystemOperation::EnsureIndex,
        })?;

        let client = self.client().await?;
        // PR #3661 reviewer fix: race-idempotent declaration. Single
        // INSERT ... ON CONFLICT DO NOTHING followed by a read-back +
        // canonical-spec equality check. Two concurrent declarers of the
        // same spec both succeed; declarers of conflicting specs see
        // IndexConflict deterministically.
        client
            .execute(
                "INSERT INTO root_filesystem_index_specs (prefix, name, keys, kind) \
                 VALUES ($1, $2, $3, $4) \
                 ON CONFLICT (prefix, name) DO NOTHING",
                &[&path.as_str(), &spec.name.as_str(), &keys_json, &kind_str],
            )
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::EnsureIndex, error))?;

        let row = client
            .query_opt(
                "SELECT keys, kind FROM root_filesystem_index_specs WHERE prefix = $1 AND name = $2",
                &[&path.as_str(), &spec.name.as_str()],
            )
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::EnsureIndex, error))?
            .ok_or_else(|| FilesystemError::IndexSpecMissingAfterUpsert {
                path: path.clone(),
                name: spec.name.clone(),
            })?;
        let existing_keys: serde_json::Value = row.get("keys");
        let existing_kind: String = row.get("kind");
        if existing_keys != keys_json || existing_kind != kind_str {
            return Err(FilesystemError::IndexConflict {
                path: path.clone(),
                name: spec.name.clone(),
                reason: crate::IndexConflictReason::SpecMismatch,
            });
        }

        let index_name = sql_index_name(path.as_str(), spec.name.as_str());
        match &spec.kind {
            IndexKind::Exact | IndexKind::Prefix => {
                let expressions: Vec<String> = spec
                    .keys
                    .iter()
                    .map(|k| format!("((indexed->>'{}'))", k.as_str()))
                    .collect();
                let ddl = format!(
                    "CREATE INDEX IF NOT EXISTS {index_name} ON root_filesystem_entries ({})",
                    expressions.join(", ")
                );
                client.batch_execute(&ddl).await.map_err(|error| {
                    db_error(path.clone(), FilesystemOperation::EnsureIndex, error)
                })?;
            }
            IndexKind::Fts => {
                if spec.keys.len() != 1 {
                    return Err(FilesystemError::IndexConflict {
                        path: path.clone(),
                        name: spec.name.clone(),
                        reason: crate::IndexConflictReason::SpecMismatch,
                    });
                }
                let fts_key = spec.keys[0].as_str();
                // GIN expression index on a `to_tsvector(...)` over the
                // indexed JSON projection. Postgres `to_tsvector` returns
                // `tsvector`, which GIN indexes natively. The matching
                // predicate at query time uses `@@ plainto_tsquery`.
                //
                // Audit finding F4: libsql FTS5 virtual tables are
                // declared per-mount-prefix (one vtable per
                // `ensure_index(prefix, ...)`), so a query at one prefix
                // can't accidentally match indexed rows under a sibling
                // prefix. The Postgres GIN index, by contrast, used to
                // be global over `root_filesystem_entries` regardless of
                // which prefix declared it — fine for correctness (the
                // query path still scopes by `path LIKE prefix/%`) but
                // it breaks parity in two ways: a search at `/memory/a`
                // would have its planner consider postings from
                // `/memory/b` before filtering, and `DROP INDEX` for a
                // prefix-scoped tear-down would impact other prefixes.
                // Make the index a partial index gated by the prefix to
                // restore parity.
                let prefix_pattern =
                    escape_like_with_trailing_wildcard(&format!("{}/%", path.as_str()));
                let prefix_literal = prefix_pattern.replace('\'', "''");
                let ddl = format!(
                    "CREATE INDEX IF NOT EXISTS {index_name} ON root_filesystem_entries \
                     USING GIN (to_tsvector('english', COALESCE(indexed->>'{fts_key}', ''))) \
                     WHERE path = '{path_literal}' OR path LIKE '{prefix_literal}' ESCAPE '!'",
                    path_literal = path.as_str().replace('\'', "''"),
                );
                client.batch_execute(&ddl).await.map_err(|error| {
                    db_error(path.clone(), FilesystemOperation::EnsureIndex, error)
                })?;
            }
            IndexKind::Vector { dim } => {
                if *dim == 0 {
                    return Err(FilesystemError::IndexConflict {
                        path: path.clone(),
                        name: spec.name.clone(),
                        reason: crate::IndexConflictReason::SpecMismatch,
                    });
                }
                // Vector storage = IndexValue::Bytes in the indexed JSON
                // projection. No per-row table or DDL is required; queries
                // brute-force cosine over the candidate set. pgvector
                // support could be layered in later via a dialect probe
                // (`SELECT * FROM pg_extension WHERE extname='vector'`)
                // without changing this trait surface.
            }
        }
        Ok(())
    }

    async fn get_metadata(
        &self,
        path: &VirtualPath,
    ) -> Result<Option<EntryMetadata>, FilesystemError> {
        let client = self.client().await?;
        client
            .query_opt(
                "SELECT path,kind FROM root_filesystem_entries WHERE path=$1 AND NOT is_dir",
                &[&path.as_str()],
            )
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::ReadFile, error))?
            .map(|row| decode_metadata(path, &row, FilesystemOperation::ReadFile))
            .transpose()
    }

    async fn query_metadata(
        &self,
        path: &VirtualPath,
        page: Page,
    ) -> Result<Vec<EntryMetadata>, FilesystemError> {
        let offset = page_offset_to_i64(path, page.offset)?;
        let limit = i64::from(page.limit.min(Page::MAX_LIMIT));
        let prefix = child_path_like_pattern(path);
        let client = self.client().await?;
        client.query(r#"SELECT path,kind FROM root_filesystem_entries WHERE NOT is_dir AND (path=$1 OR path LIKE $2 ESCAPE '!') ORDER BY path COLLATE "C" LIMIT $3 OFFSET $4"#, &[&path.as_str(), &prefix, &limit, &offset]).await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::Query, error))?
            .into_iter().map(|row| decode_metadata(path, &row, FilesystemOperation::Query)).collect()
    }

    async fn query_index_metadata(
        &self,
        path: &VirtualPath,
        key: &IndexKey,
        page: Page,
    ) -> Result<Vec<EntryIndexMetadata>, FilesystemError> {
        let offset = page_offset_to_i64(path, page.offset)?;
        let limit = i64::from(page.limit.min(Page::MAX_LIMIT));
        let prefix = child_path_like_pattern(path);
        let client = self.client().await?;
        let rows = client.query(
            r#"SELECT path,kind,jsonb_typeof(indexed)='object' AS index_object,indexed->($3::text) AS selected_value FROM root_filesystem_entries WHERE NOT is_dir AND (path=$1 OR path LIKE $2 ESCAPE '!') ORDER BY path COLLATE "C" LIMIT $4 OFFSET $5"#,
            &[&path.as_str(), &prefix, &key.as_str(), &limit, &offset],
        ).await.map_err(|error| db_error(path.clone(), FilesystemOperation::Query, error))?;
        rows.into_iter()
            .map(|row| {
                let invalid = || FilesystemError::Backend {
                    path: path.clone(),
                    operation: FilesystemOperation::Query,
                    reason: "stored selected index metadata is invalid".into(),
                };
                if !row.get::<_, bool>("index_object") {
                    return Err(invalid());
                }
                let selected: Option<serde_json::Value> = row.get("selected_value");
                Ok(EntryIndexMetadata {
                    metadata: decode_metadata(path, &row, FilesystemOperation::Query)?,
                    value: selected
                        .map(serde_json::from_value::<IndexValue>)
                        .transpose()
                        .map_err(|_| invalid())?,
                })
            })
            .collect()
    }

    async fn query(
        &self,
        path: &VirtualPath,
        filter: &Filter,
        page: Page,
    ) -> Result<Vec<VersionedEntry>, FilesystemError> {
        // Vector-nearest is evaluated by ranking the candidate set in Rust.
        if let Filter::VectorNearest {
            key,
            embedding,
            limit,
        } = filter
        {
            return self
                .vector_nearest_query(path, key, embedding, *limit)
                .await;
        }
        let mut params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>> = Vec::new();
        let path_str = path.as_str().to_string();
        let prefix_pattern = escape_like_with_trailing_wildcard(&format!("{}/%", path.as_str()));
        params.push(Box::new(path_str));
        params.push(Box::new(prefix_pattern));

        let mut conditions = String::new();
        translate_filter(path, filter, &mut conditions, &mut params)?;

        let mut sql = String::from(
            "SELECT path, contents, content_type, kind, indexed, version \
             FROM root_filesystem_entries \
             WHERE is_dir = FALSE AND (path = $1 OR path LIKE $2 ESCAPE '!')",
        );
        if !conditions.is_empty() {
            sql.push_str(" AND ");
            sql.push_str(&conditions);
        }
        sql.push_str(&format!(
            " ORDER BY path LIMIT ${} OFFSET ${}",
            params.len() + 1,
            params.len() + 2
        ));
        // `page.limit` is `u32` and clamped to `Page::MAX_LIMIT`, so the
        // `i64::from` is safe by construction. `page.offset` is `u64` and
        // user-supplied — guard with `try_from` so values ≥ 2^63 surface
        // a typed `Backend` error instead of wrapping to a negative
        // OFFSET. (Audit finding F6.)
        params.push(Box::new(i64::from(page.limit.min(Page::MAX_LIMIT))));
        params.push(Box::new(page_offset_to_i64(path, page.offset)?));

        let client = self.client().await?;
        let params_ref: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            params.iter().map(|p| p.as_ref() as _).collect();
        let rows = client
            .query(sql.as_str(), &params_ref[..])
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::Query, error))?;
        rows.into_iter()
            .map(|row| {
                let row_path: String = row.get("path");
                let row_path = VirtualPath::new(row_path)?;
                let body: Vec<u8> = row.get("contents");
                let content_type_raw: String = row.get("content_type");
                let kind_raw: Option<String> = row.get("kind");
                let indexed_value: serde_json::Value = row.get("indexed");
                let version_raw: i64 = row.get("version");
                let entry =
                    build_entry(&row_path, body, content_type_raw, kind_raw, indexed_value)?;
                let version = record_version_from_i64(&row_path, version_raw)?;
                Ok(VersionedEntry {
                    path: row_path,
                    entry,
                    version,
                })
            })
            .collect()
    }

    async fn read_file(&self, path: &VirtualPath) -> Result<Vec<u8>, FilesystemError> {
        let client = self.client().await?;
        let row = client
            .query_opt(
                "SELECT contents, is_dir FROM root_filesystem_entries WHERE path = $1",
                &[&path.as_str()],
            )
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::ReadFile, error))?;
        let Some(row) = row else {
            return Err(not_found(path.clone(), FilesystemOperation::ReadFile));
        };
        let is_dir: bool = row.get("is_dir");
        if is_dir {
            return Err(FilesystemError::Backend {
                path: path.clone(),
                operation: FilesystemOperation::ReadFile,
                reason: "is a directory".to_string(),
            });
        }
        Ok(row.get("contents"))
    }

    async fn read_file_bounded(
        &self,
        path: &VirtualPath,
        max_bytes: usize,
    ) -> Result<Option<Vec<u8>>, FilesystemError> {
        let client = self.client().await?;
        // All PostgreSQL lengths fit i64. A larger caller ceiling permits
        // every representable body; never wrap that ceiling into a negative
        // SQL parameter on 64-bit hosts.
        let max_bytes = max_bytes.min(i64::MAX as usize) as i64;
        let row = client
            .query_opt(
                r#"
                SELECT
                    CASE
                        WHEN kind IS NOT NULL THEN NULL
                        WHEN octet_length(contents)::BIGINT <= $2 THEN contents
                        ELSE NULL
                    END AS contents,
                    octet_length(contents)::BIGINT AS len,
                    kind IS NOT NULL AS sensitive,
                    is_dir
                FROM root_filesystem_entries
                WHERE path = $1
                "#,
                &[&path.as_str(), &max_bytes],
            )
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::ReadFile, error))?;
        let Some(row) = row else {
            return Err(not_found(path.clone(), FilesystemOperation::ReadFile));
        };
        if row.get::<_, bool>("sensitive") {
            return Err(FilesystemError::Unsupported {
                path: path.clone(),
                operation: FilesystemOperation::ReadFile,
            });
        }
        let is_dir: bool = row.get("is_dir");
        if is_dir {
            return Err(FilesystemError::Backend {
                path: path.clone(),
                operation: FilesystemOperation::ReadFile,
                reason: "is a directory".to_string(),
            });
        }
        let len: i64 = row.get("len");
        if len > max_bytes {
            return Ok(None);
        }
        Ok(Some(row.get("contents")))
    }

    async fn write_file(&self, path: &VirtualPath, bytes: &[u8]) -> Result<(), FilesystemError> {
        // The unified mutation preserves opaque-write metadata reset and uses
        // the same atomic namespace checks as record-shaped put.
        self.put(path, Entry::bytes(bytes.to_vec()), CasExpectation::Any)
            .await
            .map(|_| ())
    }

    async fn append_file(&self, path: &VirtualPath, bytes: &[u8]) -> Result<(), FilesystemError> {
        let mut client = self.client().await?;
        let transaction = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::AppendFile, error))?;
        Self::validate_file_namespace(&transaction, path, FilesystemOperation::AppendFile).await?;
        // PR #3660 reviewer fix: append also resets schema metadata.
        // Appending bytes onto a previously record-shaped entry was always
        // a category error; surface it by clearing the schema metadata
        // rather than leaving it stale on top of changed bytes.
        // Note: append rewrites the whole DB row. This is acceptable for
        // the legacy bytes plane (slated for removal in the consumer-
        // migration cleanup pass — see RootFilesystem::append_file's
        // deprecation note). New callers must use `append`/`tail` for
        // log-shaped mounts or `get`+`put` read-modify-write.
        transaction
            .execute(
                r#"
                INSERT INTO root_filesystem_entries
                    (path, contents, is_dir, content_type, kind, indexed, version)
                VALUES ($1, $2, FALSE, 'application/octet-stream', NULL, '{}'::jsonb, 1)
                ON CONFLICT (path) DO UPDATE SET
                    contents = root_filesystem_entries.contents || EXCLUDED.contents,
                    is_dir = FALSE,
                    content_type = EXCLUDED.content_type,
                    kind = EXCLUDED.kind,
                    indexed = EXCLUDED.indexed,
                    version = root_filesystem_entries.version + 1,
                    updated_at = NOW()
                "#,
                &[&path.as_str(), &bytes],
            )
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::AppendFile, error))?;
        transaction
            .commit()
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::AppendFile, error))?;
        Ok(())
    }

    async fn list_dir(&self, path: &VirtualPath) -> Result<Vec<DirEntry>, FilesystemError> {
        let client = self.client().await?;
        let exact_entry = self.exact_entry_with_client(&client, path).await?;
        if exact_entry
            .as_ref()
            .is_some_and(|entry| entry.file_type == FileType::File)
        {
            return Err(FilesystemError::Backend {
                path: path.clone(),
                operation: FilesystemOperation::ListDir,
                reason: "not a directory".to_string(),
            });
        }
        let rows = self
            .child_entries_with_client(&client, path, FilesystemOperation::ListDir)
            .await?;
        let children = direct_children(path, rows);
        if exact_entry
            .as_ref()
            .is_some_and(|entry| entry.file_type == FileType::Directory)
            && is_not_found(&children)
        {
            return Ok(Vec::new());
        }
        children
    }

    async fn list_dir_bounded(
        &self,
        path: &VirtualPath,
        max_entries: usize,
    ) -> Result<Vec<DirEntry>, FilesystemError> {
        let limit = i64::try_from(max_entries.max(1)).map_err(|_| FilesystemError::Backend {
            path: path.clone(),
            operation: FilesystemOperation::ListDir,
            reason: "directory limit is not representable on this backend".into(),
        })?;
        let client = self.client().await?;
        let exact = self.exact_entry_with_client(&client, path).await?;
        if exact
            .as_ref()
            .is_some_and(|entry| entry.file_type == FileType::File)
        {
            return Err(FilesystemError::Backend {
                path: path.clone(),
                operation: FilesystemOperation::ListDir,
                reason: "not a directory".into(),
            });
        }
        let prefix = format!("{}/", path.as_str().trim_end_matches('/'));
        let pattern = child_path_like_pattern(path);
        // Limit distinct DIRECT children, not descendant rows. Project no
        // payload or payload length; a directory can contain large files and
        // many descendants under just one child. The first path preserves the
        // existing direct_children classification for each selected name.
        let rows = client
            .query(
                r#"
            WITH candidates AS (
                SELECT path,is_dir,substr(path,char_length($2)+1) AS tail
                FROM root_filesystem_entries WHERE path LIKE $1 ESCAPE '!'
            ), children AS (
                SELECT path,is_dir,tail,split_part(tail,'/',1) AS name FROM candidates
            )
            SELECT DISTINCT ON (name COLLATE "C") name,
                (is_dir OR position('/' IN tail)>0) AS directory
            FROM children WHERE name<>''
            ORDER BY name COLLATE "C",path COLLATE "C" LIMIT $3
        "#,
                &[&pattern, &prefix, &limit],
            )
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::ListDir, error))?;
        if rows.is_empty() && exact.is_none() {
            return Err(not_found(path.clone(), FilesystemOperation::ListDir));
        }
        // Even a zero-sized window verifies directory existence, as the
        // compatibility implementation does, without materializing its children.
        rows.into_iter()
            .take(max_entries)
            .map(|row| {
                let name: String = row.get("name");
                Ok(DirEntry {
                    path: VirtualPath::new(format!("{prefix}{name}"))?,
                    name,
                    file_type: if row.get("directory") {
                        FileType::Directory
                    } else {
                        FileType::File
                    },
                })
            })
            .collect()
    }

    async fn stat(&self, path: &VirtualPath) -> Result<FileStat, FilesystemError> {
        let client = self.client().await?;
        if let Some(stat) = self.exact_entry_with_client(&client, path).await? {
            return Ok(stat);
        }
        if self.has_child_entry_with_client(&client, path).await? {
            return Ok(FileStat {
                path: path.clone(),
                file_type: FileType::Directory,
                len: 0,
                modified: None,
                sensitive: false,
            });
        }
        Err(not_found(path.clone(), FilesystemOperation::Stat))
    }

    async fn delete(&self, path: &VirtualPath) -> Result<(), FilesystemError> {
        let client = self.client().await?;
        let child_pattern = child_path_like_pattern(path);
        let deleted = client
            .execute(
                "DELETE FROM root_filesystem_entries WHERE path = $1 OR path LIKE $2 ESCAPE '!'",
                &[&path.as_str(), &child_pattern],
            )
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::Delete, error))?;
        if deleted == 0 {
            return Err(not_found(path.clone(), FilesystemOperation::Delete));
        }
        Ok(())
    }

    async fn append(&self, path: &VirtualPath, payload: Vec<u8>) -> Result<SeqNo, FilesystemError> {
        let client = self.client().await?;
        let row = client
            .query_one(
                r#"
                INSERT INTO root_filesystem_events (path, payload)
                VALUES ($1, $2)
                RETURNING id
                "#,
                &[&path.as_str(), &payload],
            )
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::Append, error))?;
        let id: i64 = row.get("id");
        seq_no_from_i64(path, id, FilesystemOperation::Append)
    }

    async fn tail(
        &self,
        path: &VirtualPath,
        from: SeqNo,
    ) -> Result<Vec<EventRecord>, FilesystemError> {
        let client = self.client().await?;
        let from_raw = i64::try_from(from.get()).map_err(|_| {
            backend_error(
                path.clone(),
                FilesystemOperation::Tail,
                "tail cursor exceeds i64",
            )
        })?;
        let rows = client
            .query(
                r#"
                SELECT id, payload
                FROM root_filesystem_events
                WHERE path = $1 AND id > $2
                ORDER BY id ASC
                "#,
                &[&path.as_str(), &from_raw],
            )
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::Tail, error))?;
        rows.into_iter()
            .map(|row| {
                let id: i64 = row.get("id");
                let payload: Vec<u8> = row.get("payload");
                Ok(EventRecord {
                    seq: seq_no_from_i64(path, id, FilesystemOperation::Tail)?,
                    payload,
                })
            })
            .collect()
    }

    async fn head_seq(
        &self,
        path: &VirtualPath,
        from: SeqNo,
    ) -> Result<Option<SeqNo>, FilesystemError> {
        let client = self.client().await?;
        let from_raw = i64::try_from(from.get()).map_err(|_| {
            backend_error(
                path.clone(),
                FilesystemOperation::HeadSeq,
                "head_seq cursor exceeds i64",
            )
        })?;
        let row = client
            .query_one(
                r#"
                SELECT MAX(id) AS head
                FROM root_filesystem_events
                WHERE path = $1 AND id > $2
                "#,
                &[&path.as_str(), &from_raw],
            )
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::HeadSeq, error))?;
        // `MAX(...)` over an empty match set yields SQL NULL.
        let head_raw: Option<i64> = row.get("head");
        match head_raw {
            Some(id) => Ok(Some(seq_no_from_i64(
                path,
                id,
                FilesystemOperation::HeadSeq,
            )?)),
            None => Ok(None),
        }
    }

    async fn create_dir_all(&self, path: &VirtualPath) -> Result<(), FilesystemError> {
        let mut client = self.client().await?;
        let transaction = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::ReadCommitted)
            .start()
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::CreateDirAll, error))?;
        Self::lock_namespace(&transaction, path, FilesystemOperation::CreateDirAll).await?;
        for prefix in path.prefixes() {
            let prefix = VirtualPath::new(prefix)?;
            let row = transaction
                .query_opt(
                    "SELECT is_dir FROM root_filesystem_entries WHERE path = $1",
                    &[&prefix.as_str()],
                )
                .await
                .map_err(|error| {
                    db_error(prefix.clone(), FilesystemOperation::CreateDirAll, error)
                })?;
            if row.is_some_and(|row| !row.get::<_, bool>("is_dir")) {
                return Err(FilesystemError::Backend {
                    path: prefix,
                    operation: FilesystemOperation::CreateDirAll,
                    reason: "file exists where directory is required".to_string(),
                });
            }
            transaction
                .execute(
                    r#"
                    INSERT INTO root_filesystem_entries (path, contents, is_dir)
                    VALUES ($1, ''::bytea, TRUE)
                    ON CONFLICT (path) DO NOTHING
                    "#,
                    &[&prefix.as_str()],
                )
                .await
                .map_err(|error| {
                    db_error(path.clone(), FilesystemOperation::CreateDirAll, error)
                })?;
        }
        transaction
            .commit()
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::CreateDirAll, error))?;
        Ok(())
    }
}

#[cfg(feature = "postgres")]
impl PostgresRootFilesystem {
    /// Shared ancestor locks allow unrelated leaves to proceed together. A
    /// parent writer needs its exclusive leaf lock, so it cannot race a child
    /// writer's ancestor check. Keep locks through the actual CAS and commit.
    /// ReadCommitted is explicit: the validation statement after a lock wait
    /// must see the preceding holder's committed mutation.
    async fn lock_namespace(
        transaction: &tokio_postgres::Transaction<'_>,
        path: &VirtualPath,
        operation: FilesystemOperation,
    ) -> Result<(), FilesystemError> {
        for prefix in path.prefixes() {
            let key = format!("brassclaw.root-filesystem.namespace:{prefix}");
            let leaf = prefix == path.as_str();
            let sql = if leaf {
                "SELECT pg_advisory_xact_lock(hashtextextended($1,0))"
            } else {
                "SELECT pg_advisory_xact_lock_shared(hashtextextended($1,0))"
            };
            transaction
                .query_one(sql, &[&key])
                .await
                .map_err(|error| db_error(path.clone(), operation, error))?;
            if !leaf {
                // A separate ReadCommitted statement AFTER the lock wait sees
                // the preceding writer's commit. Do not build a quadratic
                // array of all ancestor strings for one SQL parameter.
                let exists: bool = transaction.query_one(
                    "SELECT EXISTS(SELECT 1 FROM root_filesystem_entries WHERE path=$1 AND NOT is_dir)",
                    &[&prefix]).await
                    .map_err(|error| db_error(path.clone(), operation, error))?.get(0);
                if exists {
                    return Err(FilesystemError::Backend {
                        path: if operation == FilesystemOperation::CreateDirAll {
                            VirtualPath::new(prefix)?
                        } else {
                            path.clone()
                        },
                        operation,
                        reason: "file exists where an ancestor directory is required".into(),
                    });
                }
            }
        }
        Ok(())
    }

    async fn validate_file_namespace(
        transaction: &tokio_postgres::Transaction<'_>,
        path: &VirtualPath,
        operation: FilesystemOperation,
    ) -> Result<(), FilesystemError> {
        Self::lock_namespace(transaction, path, operation).await?;
        let child_pattern = child_path_like_pattern(path);
        let directory: bool = transaction.query_one(
            "SELECT EXISTS(SELECT 1 FROM root_filesystem_entries WHERE (path=$1 AND is_dir) OR path LIKE $2 ESCAPE '!')",
            &[&path.as_str(),&child_pattern]).await
            .map_err(|error| db_error(path.clone(), operation, error))?.get(0);
        if directory {
            return Err(if operation == FilesystemOperation::AppendFile {
                directory_append_error(path.clone())
            } else {
                directory_write_error(path.clone())
            });
        }
        Ok(())
    }

    async fn exact_entry_with_client(
        &self,
        client: &tokio_postgres::Client,
        path: &VirtualPath,
    ) -> Result<Option<FileStat>, FilesystemError> {
        let row = client
            .query_opt(
                "SELECT OCTET_LENGTH(contents)::bigint AS len, is_dir, kind IS NOT NULL AS sensitive, EXTRACT(EPOCH FROM updated_at)::bigint AS updated_at_epoch FROM root_filesystem_entries WHERE path = $1",
                &[&path.as_str()],
            )
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::Stat, error))?;
        Ok(row.map(|row| {
            let len: i64 = row.get("len");
            let is_dir: bool = row.get("is_dir");
            let updated_at_epoch: i64 = row.get("updated_at_epoch");
            FileStat {
                path: path.clone(),
                len: if is_dir { 0 } else { len.max(0) as u64 },
                file_type: if is_dir {
                    FileType::Directory
                } else {
                    FileType::File
                },
                modified: system_time_from_unix_seconds(updated_at_epoch),
                // Coding primitives operate on opaque files, not structured
                // application records. Match the in-memory backend's boundary.
                sensitive: row.get("sensitive"),
            }
        }))
    }

    async fn child_entries_with_client(
        &self,
        client: &tokio_postgres::Client,
        parent: &VirtualPath,
        operation: FilesystemOperation,
    ) -> Result<Vec<(VirtualPath, u64, FileType)>, FilesystemError> {
        let pattern = child_path_like_pattern(parent);
        let rows = client
            .query(
                "SELECT path, OCTET_LENGTH(contents)::bigint AS len, is_dir FROM root_filesystem_entries WHERE path LIKE $1 ESCAPE '!' ORDER BY path",
                &[&pattern],
            )
            .await
            .map_err(|error| db_error(parent.clone(), operation, error))?;
        rows.into_iter()
            .map(|row| {
                let path: String = row.get("path");
                let len: i64 = row.get("len");
                let is_dir: bool = row.get("is_dir");
                Ok((
                    VirtualPath::new(path)?,
                    if is_dir { 0 } else { len.max(0) as u64 },
                    if is_dir {
                        FileType::Directory
                    } else {
                        FileType::File
                    },
                ))
            })
            .collect()
    }

    async fn has_child_entry_with_client(
        &self,
        client: &tokio_postgres::Client,
        parent: &VirtualPath,
    ) -> Result<bool, FilesystemError> {
        let pattern = child_path_like_pattern(parent);
        let row = client
            .query_opt(
                "SELECT 1 FROM root_filesystem_entries WHERE path LIKE $1 ESCAPE '!' LIMIT 1",
                &[&pattern],
            )
            .await
            .map_err(|error| db_error(parent.clone(), FilesystemOperation::Stat, error))?;
        Ok(row.is_some())
    }

    /// Brute-force cosine ranker over candidate rows in this prefix whose
    /// indexed projection has an `IndexValue::Bytes` value at `key`. Used
    /// when the caller's filter is a top-level `VectorNearest`.
    ///
    /// Two-phase to bound memory on large prefixes (review feedback on
    /// the unified-FS rework): first SELECT `(path, indexed, version)`
    /// for every candidate, rank by cosine in Rust, then `get()` the
    /// top-k entries to materialize bodies. Rows outside the cutoff
    /// never have their `contents` bytea loaded.
    async fn vector_nearest_query(
        &self,
        path: &VirtualPath,
        key: &IndexKey,
        embedding: &[f32],
        limit: u32,
    ) -> Result<Vec<VersionedEntry>, FilesystemError> {
        let client = self.client().await?;
        let prefix_pattern = escape_like_with_trailing_wildcard(&format!("{}/%", path.as_str()));
        let rows = client
            .query(
                "SELECT path, indexed, version \
                 FROM root_filesystem_entries \
                 WHERE is_dir = FALSE AND (path = $1 OR path LIKE $2 ESCAPE '!') AND indexed ? $3",
                &[&path.as_str(), &prefix_pattern, &key.as_str()],
            )
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::Query, error))?;
        let mut ranked: Vec<(VirtualPath, RecordVersion, f32)> = Vec::new();
        for row in rows {
            let row_path: String = row.get("path");
            let row_path = VirtualPath::new(row_path)?;
            let indexed_value: serde_json::Value = row.get("indexed");
            let version_raw: i64 = row.get("version");
            let indexed: BTreeMap<IndexKey, IndexValue> = if indexed_value.is_null() {
                BTreeMap::new()
            } else {
                serde_json::from_value(indexed_value).map_err(|_| {
                    FilesystemError::DeserializeIndexed {
                        path: row_path.clone(),
                        operation: FilesystemOperation::Query,
                    }
                })?
            };
            let Some(IndexValue::Bytes(bytes)) = indexed.get(key) else {
                continue;
            };
            let Some(vec) = decode_embedding_blob(bytes) else {
                continue;
            };
            let Some(score) = cosine_similarity(embedding, &vec) else {
                continue;
            };
            let version = record_version_from_i64(&row_path, version_raw)?;
            ranked.push((row_path, version, score));
        }
        // Sort by descending cosine score, then ascending path for a stable
        // tie-breaker so equal-score rows truncate deterministically across
        // runs and across backends. The in-memory reference uses the same
        // tie-breaker; this keeps cross-backend behavior aligned.
        ranked.sort_by(|a, b| {
            b.2.partial_cmp(&a.2)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.as_str().cmp(b.0.as_str()))
        });
        ranked.truncate(limit as usize);
        // Release the client so each `get()` below claims its own
        // pooled connection rather than serializing through one.
        drop(client);
        self.materialize_ranked(ranked).await
    }

    /// Phase-2 of [`vector_nearest_query`]: load full [`VersionedEntry`]
    /// bodies for the ranked-and-truncated candidate set. Mirrors the
    /// libSQL backend's `materialize_ranked`, including the silently-skip
    /// behaviour when a candidate path disappears between phase-1
    /// ranking and the phase-2 `get`. Pulled out so the concurrent-delete
    /// branch has a deterministic test seam.
    pub(crate) async fn materialize_ranked(
        &self,
        ranked: Vec<(VirtualPath, RecordVersion, f32)>,
    ) -> Result<Vec<VersionedEntry>, FilesystemError> {
        let mut out = Vec::with_capacity(ranked.len());
        for (row_path, _version, _score) in ranked {
            let Some(versioned) = self.get(&row_path).await? else {
                // Concurrent delete between the ranking SELECT and
                // the body fetch — skip rather than error so the
                // search doesn't blow up on a race.
                continue;
            };
            out.push(versioned);
        }
        Ok(out)
    }

    async fn current_version_with_transaction(
        &self,
        client: &tokio_postgres::Transaction<'_>,
        path: &VirtualPath,
    ) -> Result<Option<RecordVersion>, FilesystemError> {
        let row = client
            .query_opt(
                "SELECT version FROM root_filesystem_entries WHERE path = $1 AND is_dir = FALSE",
                &[&path.as_str()],
            )
            .await
            .map_err(|error| db_error(path.clone(), FilesystemOperation::ReadFile, error))?;
        row.map(|row| {
            let version: i64 = row.get("version");
            record_version_from_i64(path, version)
        })
        .transpose()
    }
}

/// Translate a [`Filter`] tree into a postgres WHERE-clause fragment.
/// Bound parameters use `$N` placeholders sized from `params.len() + 1`.
///
/// PR #3661 fixes carried over from the libsql translator:
/// - `Filter::All` emits `TRUE`; empty `And` → `TRUE`, empty `Or` →
///   `FALSE` (matching in-memory `all`/`any` semantics).
/// - `Filter::Range` on `IndexValue::I64` bounds casts both sides to
///   `BIGINT` so the comparison is numeric, not lexicographic on text.
#[cfg(feature = "postgres")]
fn translate_filter(
    path: &VirtualPath,
    filter: &Filter,
    out: &mut String,
    params: &mut Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>>,
) -> Result<(), FilesystemError> {
    match filter {
        Filter::All => {
            out.push_str("TRUE");
            Ok(())
        }
        Filter::Eq { key, value } => {
            let placeholder = bind_index_value(path, value, params)?;
            out.push_str(&format!("(indexed->>'{}' = ${placeholder})", key.as_str()));
            Ok(())
        }
        Filter::PrefixOn { key, value } => {
            let IndexValue::Text(prefix_value) = value else {
                return Err(FilesystemError::Unsupported {
                    path: path.clone(),
                    operation: FilesystemOperation::Query,
                });
            };
            let escaped = escape_like_literal(prefix_value);
            params.push(Box::new(format!("{escaped}%")));
            out.push_str(&format!(
                "(indexed->>'{}' LIKE ${} ESCAPE '!')",
                key.as_str(),
                params.len()
            ));
            Ok(())
        }
        Filter::Range { key, lo, hi } => {
            // Mixed-variant bounds have no meaningful BETWEEN. Reject rather
            // than fall through to a lexicographic-on-text comparison that
            // silently produces wrong results. Matches the in-memory
            // backend's `discriminant(lo) == discriminant(hi)` requirement.
            if std::mem::discriminant(lo) != std::mem::discriminant(hi) {
                return Err(FilesystemError::Unsupported {
                    path: path.clone(),
                    operation: FilesystemOperation::Query,
                });
            }
            // PR #3661 reviewer fix: when both bounds are `I64`, cast both
            // the extracted JSON text and bound params to `BIGINT` so the
            // BETWEEN comparison is numeric. Otherwise `'2' BETWEEN '10'
            // AND '99'` would compare lexicographically and miss values.
            //
            // PR #3659 review fix: guard each cast with a `jsonb_typeof`
            // check so a row whose stored value at `'{key}'` is a different
            // variant (e.g. text under a numeric range) is filtered out
            // BEFORE the cast — otherwise one stored text value can fail
            // the whole query with a `bigint` cast error.
            match (lo, hi) {
                (IndexValue::I64(lo_val), IndexValue::I64(hi_val)) => {
                    params.push(Box::new(*lo_val));
                    let lo_idx = params.len();
                    params.push(Box::new(*hi_val));
                    let hi_idx = params.len();
                    out.push_str(&format!(
                        "(jsonb_typeof(indexed->'{}') = 'number' \
                         AND (indexed->>'{}')::bigint BETWEEN ${lo_idx} AND ${hi_idx})",
                        key.as_str(),
                        key.as_str(),
                    ));
                }
                _ => {
                    let lo_idx = bind_index_value(path, lo, params)?;
                    let hi_idx = bind_index_value(path, hi, params)?;
                    let expected_json_type = index_value_jsonb_typeof(lo);
                    out.push_str(&format!(
                        "(jsonb_typeof(indexed->'{}') = '{expected_json_type}' \
                         AND indexed->>'{}' BETWEEN ${lo_idx} AND ${hi_idx})",
                        key.as_str(),
                        key.as_str(),
                    ));
                }
            }
            Ok(())
        }
        Filter::Fts { key, query } => {
            // `plainto_tsquery` is the user-input-safe parser; we never
            // splice the user query into SQL. Match against an expression
            // identical to the `to_tsvector(...)` used by the GIN index in
            // ensure_index so the planner can use it.
            params.push(Box::new(query.clone()));
            out.push_str(&format!(
                "(to_tsvector('english', COALESCE(indexed->>'{}', '')) @@ plainto_tsquery('english', ${}))",
                key.as_str(),
                params.len()
            ));
            Ok(())
        }
        Filter::VectorNearest { .. } => Err(FilesystemError::Unsupported {
            // Same reason as libsql: VectorNearest is a ranking operation
            // and is evaluated at the top-level `query` method, not as a
            // WHERE-clause predicate. Nested usage is unsupported.
            path: path.clone(),
            operation: FilesystemOperation::Query,
        }),
        Filter::And(children) => translate_compound(path, children, " AND ", "TRUE", out, params),
        Filter::Or(children) => translate_compound(path, children, " OR ", "FALSE", out, params),
    }
}

#[cfg(feature = "postgres")]
fn translate_compound(
    path: &VirtualPath,
    children: &[Filter],
    joiner: &str,
    empty_identity: &str,
    out: &mut String,
    params: &mut Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>>,
) -> Result<(), FilesystemError> {
    if children.is_empty() {
        out.push_str(empty_identity);
        return Ok(());
    }
    out.push('(');
    for (i, child) in children.iter().enumerate() {
        if i > 0 {
            out.push_str(joiner);
        }
        translate_filter(path, child, out, params)?;
    }
    out.push(')');
    Ok(())
}

/// Maps an [`IndexValue`] variant to its Postgres `jsonb_typeof` string.
/// Used to guard `Filter::Range` so cross-variant stored values are filtered
/// out before any cast/comparison (PR #3659 review fix). Postgres returns:
/// `"string"` / `"number"` / `"boolean"` / `"null"` / `"object"` / `"array"`.
#[cfg(feature = "postgres")]
fn index_value_jsonb_typeof(value: &IndexValue) -> &'static str {
    match value {
        IndexValue::Text(_) | IndexValue::Bytes(_) => "string",
        IndexValue::I64(_) => "number",
        IndexValue::Bool(_) => "boolean",
    }
}

#[cfg(feature = "postgres")]
fn bind_index_value(
    path: &VirtualPath,
    value: &IndexValue,
    params: &mut Vec<Box<dyn tokio_postgres::types::ToSql + Sync + Send>>,
) -> Result<usize, FilesystemError> {
    // `indexed->>'key'` returns text regardless of the underlying JSON type,
    // so we bind every supported variant as text. This keeps the index
    // (which is also an expression on the text form) usable for all three
    // variants without dialect branches.
    let bound: Box<dyn tokio_postgres::types::ToSql + Sync + Send> = match value {
        IndexValue::Text(s) => Box::new(s.clone()),
        IndexValue::I64(n) => Box::new(n.to_string()),
        IndexValue::Bool(b) => Box::new(if *b {
            "true".to_string()
        } else {
            "false".to_string()
        }),
        IndexValue::Bytes(_) => {
            return Err(FilesystemError::Unsupported {
                path: path.clone(),
                operation: FilesystemOperation::Query,
            });
        }
    };
    params.push(bound);
    Ok(params.len())
}

#[cfg(feature = "postgres")]
fn build_entry(
    path: &VirtualPath,
    body: Vec<u8>,
    content_type_raw: String,
    kind_raw: Option<String>,
    indexed_value: serde_json::Value,
) -> Result<Entry, FilesystemError> {
    let content_type = ContentType::new(content_type_raw).map_err(FilesystemError::Contract)?;
    let kind = kind_raw
        .map(RecordKind::new)
        .transpose()
        .map_err(FilesystemError::Contract)?;
    let indexed: BTreeMap<IndexKey, IndexValue> = if indexed_value.is_null() {
        BTreeMap::new()
    } else {
        serde_json::from_value(indexed_value).map_err(|_| FilesystemError::DeserializeIndexed {
            path: path.clone(),
            operation: FilesystemOperation::ReadFile,
        })?
    };
    Ok(Entry {
        body,
        content_type,
        kind,
        indexed,
    })
}

#[cfg(feature = "postgres")]
fn seq_no_from_i64(
    path: &VirtualPath,
    raw: i64,
    operation: FilesystemOperation,
) -> Result<SeqNo, FilesystemError> {
    u64::try_from(raw)
        .map(SeqNo::from_backend)
        .map_err(|_| FilesystemError::Backend {
            path: path.clone(),
            operation,
            reason: format!("event seq {raw} is not representable"),
        })
}

#[cfg(feature = "postgres")]
fn backend_error(
    path: VirtualPath,
    operation: FilesystemOperation,
    reason: impl Into<String>,
) -> FilesystemError {
    FilesystemError::Backend {
        path,
        operation,
        reason: reason.into(),
    }
}

#[cfg(feature = "postgres")]
const POSTGRES_ROOT_FILESYSTEM_SCHEMA: &str = concat!(
    include_str!("../../../migrations/V26__root_filesystem_entries.sql"),
    "\n",
    include_str!("../../../migrations/V27__root_filesystem_entries_directories.sql"),
    "\n",
    include_str!("../../../migrations/V28__root_filesystem_records.sql"),
    "\n",
    include_str!("../../../migrations/V29__root_filesystem_index_specs.sql"),
    "\n",
    include_str!("../../../migrations/V30__root_filesystem_events.sql"),
);
