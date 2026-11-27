#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset, NaiveDate};
    use sea_orm::ActiveValue::Set;
    use sea_orm::EntityTrait;
    use uuid::Uuid;

    use super::super::ToolOutput;
    use super::super::dto::{
        DocumentChunkDto, DocumentDto, DocumentMentionDto, DocumentVersionReferenceDto,
        GetDocumentChunkParams, GetDocumentChunkResult, SearchDocumentItemDto,
        SearchDocumentsParams, SearchDocumentsResult,
    };
    use super::super::tests_common::{build_server, insert_strategy};
    use core_application::document_storage::{
        DocumentChunkInput, DocumentMetadata, DocumentStorageRepository,
    };
    use core_application::unit_of_work::UnitOfWork;
    use gateway_postgres::entities::{document_chunk_mention, stock};
    use gateway_postgres::{DatabaseHandle, PostgresDocumentStorageRepository, PostgresUnitOfWork};
    use rmcp::model::ErrorCode;

    fn timestamp(day: u32) -> DateTime<FixedOffset> {
        NaiveDate::from_ymd_opt(2026, 6, day)
            .expect("date is valid")
            .and_hms_opt(12, 0, 0)
            .expect("time is valid")
            .and_utc()
            .fixed_offset()
    }

    async fn insert_stock(db: &DatabaseHandle, id: &str, name: &str) {
        stock::Entity::insert(stock::ActiveModel {
            id: Set(id.into()),
            name: Set(name.into()),
            market: Set(None),
            product_category: Set(None),
            cik: Set(None),
            created_at: Default::default(),
            updated_at: Default::default(),
        })
        .exec_without_returning(db)
        .await
        .expect("insert synthetic stock");
    }

    async fn save_document(
        db: &DatabaseHandle,
        source_document_id: &str,
        published_at: DateTime<FixedOffset>,
        retrieved_at: DateTime<FixedOffset>,
    ) -> (Uuid, String, usize, usize) {
        let body = "Before section Synthetic Mentioned Co after section".to_owned();
        let target = "Synthetic Mentioned Co";
        let start = body.find(target).expect("target is in body");
        let end = start + target.len();
        let offset = |value: usize| i32::try_from(value).expect("synthetic offset fits in i32");
        let chunks = vec![
            DocumentChunkInput {
                sequence: 0,
                start_offset: 0,
                end_offset: offset(start),
                body: body[..start].to_owned(),
            },
            DocumentChunkInput {
                sequence: 1,
                start_offset: offset(start),
                end_offset: offset(end),
                body: target.to_owned(),
            },
            DocumentChunkInput {
                sequence: 2,
                start_offset: offset(end),
                end_offset: offset(body.len()),
                body: body[end..].to_owned(),
            },
        ];
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let repository = PostgresDocumentStorageRepository::new(db.clone());
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        let snapshot = repository
            .save(
                &transaction,
                DocumentMetadata {
                    source: "synthetic-archive".into(),
                    source_document_id: source_document_id.into(),
                    stock_id: Some("demo-issuer".into()),
                    source_issuer_code: "ISSUER-TEST-1".into(),
                    title: "Synthetic notice".into(),
                    published_at,
                },
                "a".repeat(64),
                retrieved_at,
                body.clone(),
                chunks,
            )
            .await
            .expect("document saves");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");
        (snapshot.chunks[1].id, body, start, end)
    }

    fn normalize_search(output: ToolOutput<SearchDocumentsResult>) -> SearchDocumentsResult {
        output
            .normalize_json(|value| {
                for item in value["items"].as_array_mut().expect("items array") {
                    item["document"]["id"] = serde_json::json!(Uuid::nil());
                    item["version"]["id"] = serde_json::json!(Uuid::nil());
                    item["chunk"]["id"] = serde_json::json!(Uuid::nil());
                }
            })
            .into_value()
    }

    fn normalize_read(output: ToolOutput<GetDocumentChunkResult>) -> GetDocumentChunkResult {
        output
            .normalize_json(|value| {
                value["document"]["id"] = serde_json::json!(Uuid::nil());
                value["version"]["id"] = serde_json::json!(Uuid::nil());
                value["previous_chunk"]["id"] = serde_json::json!(Uuid::nil());
                value["chunk"]["id"] = serde_json::json!(Uuid::nil());
                value["next_chunk"]["id"] = serde_json::json!(Uuid::nil());
            })
            .into_value()
    }

    #[backend_test_macros::database_test]
    async fn search_and_read_document_chunks_include_mentions_and_source_context(
        db: DatabaseHandle,
    ) {
        let server = build_server(db.clone());
        let strategy_id = insert_strategy(&db, "synthetic").await;
        insert_stock(&db, "demo-issuer", "Synthetic Issuer").await;
        insert_stock(&db, "demo-mentioned", "Synthetic Mentioned Co").await;
        let (chunk_id, body, start, end) =
            save_document(&db, "DOC-TEST-1", timestamp(1), timestamp(2)).await;
        document_chunk_mention::Entity::insert(document_chunk_mention::ActiveModel {
            id: Set(Uuid::new_v4()),
            chunk_id: Set(chunk_id),
            stock_id: Set("demo-mentioned".into()),
            start_offset: Set(i32::try_from(start).expect("offset fits in i32")),
            end_offset: Set(i32::try_from(end).expect("offset fits in i32")),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert synthetic mention");

        let searched = server
            .search_documents(
                strategy_id,
                SearchDocumentsParams {
                    query: "synthetic mentioned".into(),
                    as_of: Some(timestamp(3)),
                    stock_id: Some("demo-issuer".into()),
                    limit: Some(1),
                },
            )
            .await
            .expect("search documents");
        let searched_chunk_id = searched
            .items
            .first()
            .map(|item| item.chunk.id)
            .expect("search returns a matching chunk");
        let read = server
            .get_document_chunk(
                strategy_id,
                GetDocumentChunkParams {
                    id: searched_chunk_id,
                    as_of: Some(timestamp(3)),
                },
            )
            .await
            .expect("read document chunk");

        let chunk_start = i32::try_from(start).expect("offset fits in i32");
        let chunk_end = i32::try_from(end).expect("offset fits in i32");
        assert_eq!(
            (
                searched_chunk_id,
                normalize_search(searched),
                normalize_read(read),
            ),
            (
                chunk_id,
                SearchDocumentsResult {
                    items: vec![SearchDocumentItemDto {
                        document: DocumentDto {
                            id: Uuid::nil(),
                            source: "synthetic-archive".into(),
                            source_document_id: "DOC-TEST-1".into(),
                            stock_id: Some("demo-issuer".into()),
                            source_issuer_code: "ISSUER-TEST-1".into(),
                            title: "Synthetic notice".into(),
                            published_at: timestamp(1),
                        },
                        version: DocumentVersionReferenceDto {
                            id: Uuid::nil(),
                            content_hash: "a".repeat(64),
                            retrieved_at: timestamp(2),
                        },
                        chunk: DocumentChunkDto {
                            id: Uuid::nil(),
                            sequence: 1,
                            start_offset: chunk_start,
                            end_offset: chunk_end,
                            body: "Synthetic Mentioned Co".into(),
                        },
                        mentions: vec![DocumentMentionDto {
                            stock_id: "demo-mentioned".into(),
                            start_offset: chunk_start,
                            end_offset: chunk_end,
                        }],
                    }],
                },
                GetDocumentChunkResult {
                    document: DocumentDto {
                        id: Uuid::nil(),
                        source: "synthetic-archive".into(),
                        source_document_id: "DOC-TEST-1".into(),
                        stock_id: Some("demo-issuer".into()),
                        source_issuer_code: "ISSUER-TEST-1".into(),
                        title: "Synthetic notice".into(),
                        published_at: timestamp(1),
                    },
                    version: DocumentVersionReferenceDto {
                        id: Uuid::nil(),
                        content_hash: "a".repeat(64),
                        retrieved_at: timestamp(2),
                    },
                    previous_chunk: Some(DocumentChunkDto {
                        id: Uuid::nil(),
                        sequence: 0,
                        start_offset: 0,
                        end_offset: chunk_start,
                        body: body[..start].to_owned(),
                    }),
                    chunk: DocumentChunkDto {
                        id: Uuid::nil(),
                        sequence: 1,
                        start_offset: chunk_start,
                        end_offset: chunk_end,
                        body: "Synthetic Mentioned Co".into(),
                    },
                    next_chunk: Some(DocumentChunkDto {
                        id: Uuid::nil(),
                        sequence: 2,
                        start_offset: chunk_end,
                        end_offset: i32::try_from(body.len()).expect("body length fits in i32"),
                        body: body[end..].to_owned(),
                    }),
                    mentions: vec![DocumentMentionDto {
                        stock_id: "demo-mentioned".into(),
                        start_offset: chunk_start,
                        end_offset: chunk_end,
                    }],
                },
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn document_chunk_read_hides_later_retrievals_at_as_of_time(db: DatabaseHandle) {
        let server = build_server(db.clone());
        let strategy_id = insert_strategy(&db, "synthetic").await;
        insert_stock(&db, "demo-issuer", "Synthetic Issuer").await;
        insert_stock(&db, "demo-mentioned", "Synthetic Mentioned Co").await;
        let (chunk_id, body, start, end) =
            save_document(&db, "DOC-TEST-1", timestamp(1), timestamp(2)).await;
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let repository = PostgresDocumentStorageRepository::new(db.clone());
        let transaction = unit_of_work.begin().await.expect("transaction begins");
        let later_body = "Synthetic changed version".to_owned();
        let later_end = i32::try_from(later_body.len()).expect("synthetic body length fits in i32");
        let later = repository
            .save(
                &transaction,
                DocumentMetadata {
                    source: "synthetic-archive".into(),
                    source_document_id: "DOC-TEST-1".into(),
                    stock_id: Some("demo-issuer".into()),
                    source_issuer_code: "ISSUER-TEST-1".into(),
                    title: "Synthetic notice".into(),
                    published_at: timestamp(1),
                },
                "b".repeat(64),
                timestamp(4),
                later_body.clone(),
                vec![DocumentChunkInput {
                    sequence: 0,
                    start_offset: 0,
                    end_offset: later_end,
                    body: later_body,
                }],
            )
            .await
            .expect("later version saves");
        unit_of_work
            .commit(transaction)
            .await
            .expect("transaction commits");
        let (future_publication_chunk_id, _, _, _) =
            save_document(&db, "DOC-TEST-2", timestamp(4), timestamp(2)).await;

        let visible = server
            .get_document_chunk(
                strategy_id,
                GetDocumentChunkParams {
                    id: chunk_id,
                    as_of: Some(timestamp(3)),
                },
            )
            .await
            .map(normalize_read)
            .map_err(|error| (error.code, error.message));
        let hidden = server
            .get_document_chunk(
                strategy_id,
                GetDocumentChunkParams {
                    id: later.chunks[0].id,
                    as_of: Some(timestamp(3)),
                },
            )
            .await
            .map(normalize_read)
            .map_err(|error| (error.code, error.message));
        let future_publication_hidden = server
            .get_document_chunk(
                strategy_id,
                GetDocumentChunkParams {
                    id: future_publication_chunk_id,
                    as_of: Some(timestamp(3)),
                },
            )
            .await
            .map(normalize_read)
            .map_err(|error| (error.code, error.message));

        let chunk_start = i32::try_from(start).expect("synthetic offset fits in i32");
        let chunk_end = i32::try_from(end).expect("synthetic offset fits in i32");

        assert_eq!(
            (visible, hidden, future_publication_hidden,),
            (
                Ok(GetDocumentChunkResult {
                    document: DocumentDto {
                        id: Uuid::nil(),
                        source: "synthetic-archive".into(),
                        source_document_id: "DOC-TEST-1".into(),
                        stock_id: Some("demo-issuer".into()),
                        source_issuer_code: "ISSUER-TEST-1".into(),
                        title: "Synthetic notice".into(),
                        published_at: timestamp(1),
                    },
                    version: DocumentVersionReferenceDto {
                        id: Uuid::nil(),
                        content_hash: "a".repeat(64),
                        retrieved_at: timestamp(2),
                    },
                    previous_chunk: Some(DocumentChunkDto {
                        id: Uuid::nil(),
                        sequence: 0,
                        start_offset: 0,
                        end_offset: chunk_start,
                        body: body[..start].to_owned(),
                    }),
                    chunk: DocumentChunkDto {
                        id: Uuid::nil(),
                        sequence: 1,
                        start_offset: chunk_start,
                        end_offset: chunk_end,
                        body: "Synthetic Mentioned Co".into(),
                    },
                    next_chunk: Some(DocumentChunkDto {
                        id: Uuid::nil(),
                        sequence: 2,
                        start_offset: chunk_end,
                        end_offset: i32::try_from(body.len())
                            .expect("synthetic body length fits in i32"),
                        body: body[end..].to_owned(),
                    }),
                    mentions: Vec::new(),
                }),
                Err((
                    ErrorCode::RESOURCE_NOT_FOUND,
                    "document chunk not found".into(),
                )),
                Err((
                    ErrorCode::RESOURCE_NOT_FOUND,
                    "document chunk not found".into(),
                )),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn invalid_document_search_input_returns_correctable_mcp_error(db: DatabaseHandle) {
        let server = build_server(db.clone());
        let strategy_id = insert_strategy(&db, "synthetic").await;
        let short_query_error = server
            .search_documents(
                strategy_id,
                SearchDocumentsParams {
                    query: "x".into(),
                    as_of: None,
                    stock_id: None,
                    limit: Some(10),
                },
            )
            .await
            .expect_err("short query is rejected");
        let invalid_limit_error = server
            .search_documents(
                strategy_id,
                SearchDocumentsParams {
                    query: "synthetic phrase".into(),
                    as_of: None,
                    stock_id: None,
                    limit: Some(101),
                },
            )
            .await
            .expect_err("invalid limit is rejected");

        assert_eq!(
            (
                (short_query_error.code, short_query_error.message),
                (invalid_limit_error.code, invalid_limit_error.message),
            ),
            (
                (
                    ErrorCode::INVALID_PARAMS,
                    "query must contain at least one word, with every word at least 2 characters long".into(),
                ),
                (
                    ErrorCode::INVALID_PARAMS,
                    "limit must be between 1 and 100".into(),
                ),
            ),
        );
    }
}
