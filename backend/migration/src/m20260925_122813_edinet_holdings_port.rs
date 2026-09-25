use sea_orm_migration::prelude::*;

#[derive(DeriveIden, Clone, Copy)]
enum LargeVolumeDocuments {
    #[sea_orm(iden = "large_volume_shareholding_documents")]
    Table,
    DocumentId,
    StockCode,
    FilerCode,
    SubmittedOn,
    Details,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
enum MajorShareholderDocuments {
    #[sea_orm(iden = "major_shareholder_documents")]
    Table,
    DocumentId,
    StockCode,
    FilerCode,
    SubmittedOn,
    Details,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
enum CrossShareholdingDocuments {
    #[sea_orm(iden = "cross_shareholding_documents")]
    Table,
    DocumentId,
    StockCode,
    FilerCode,
    SubmittedOn,
    Details,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
enum LegacyLargeVolumeDocuments {
    #[sea_orm(iden = "edinet_large_volume_shareholdings")]
    Table,
    DocId,
    Code,
    EdinetCode,
    SubDate,
    Document,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
enum LegacyMajorShareholderDocuments {
    #[sea_orm(iden = "edinet_major_shareholders")]
    Table,
    DocId,
    Code,
    EdinetCode,
    SubDate,
    Document,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden, Clone, Copy)]
enum LegacyCrossShareholdingDocuments {
    #[sea_orm(iden = "edinet_cross_shareholdings")]
    Table,
    DocId,
    Code,
    EdinetCode,
    SubDate,
    Document,
    CreatedAt,
    UpdatedAt,
}

struct TableColumns<T> {
    table: T,
    document_id: T,
    stock_code: T,
    filer_code: T,
    submitted_on: T,
    payload: T,
    created_at: T,
    updated_at: T,
}

async fn create_table<T: IntoIden + Copy>(
    manager: &SchemaManager<'_>,
    columns: TableColumns<T>,
) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(columns.table)
                .col(
                    ColumnDef::new(columns.document_id)
                        .text()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(columns.stock_code).text())
                .col(ColumnDef::new(columns.filer_code).text().not_null())
                .col(ColumnDef::new(columns.submitted_on).date().not_null())
                .col(ColumnDef::new(columns.payload).json_binary().not_null())
                .col(
                    ColumnDef::new(columns.created_at)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(
                    ColumnDef::new(columns.updated_at)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .to_owned(),
        )
        .await
}

async fn create_index<T: IntoIden + Copy>(
    manager: &SchemaManager<'_>,
    name: &str,
    table: T,
    column: T,
) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name(name)
                .table(table)
                .col(column)
                .to_owned(),
        )
        .await
}

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260925_122813_edinet_holdings_port"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_table(
            manager,
            TableColumns {
                table: LargeVolumeDocuments::Table,
                document_id: LargeVolumeDocuments::DocumentId,
                stock_code: LargeVolumeDocuments::StockCode,
                filer_code: LargeVolumeDocuments::FilerCode,
                submitted_on: LargeVolumeDocuments::SubmittedOn,
                payload: LargeVolumeDocuments::Details,
                created_at: LargeVolumeDocuments::CreatedAt,
                updated_at: LargeVolumeDocuments::UpdatedAt,
            },
        )
        .await?;
        create_table(
            manager,
            TableColumns {
                table: MajorShareholderDocuments::Table,
                document_id: MajorShareholderDocuments::DocumentId,
                stock_code: MajorShareholderDocuments::StockCode,
                filer_code: MajorShareholderDocuments::FilerCode,
                submitted_on: MajorShareholderDocuments::SubmittedOn,
                payload: MajorShareholderDocuments::Details,
                created_at: MajorShareholderDocuments::CreatedAt,
                updated_at: MajorShareholderDocuments::UpdatedAt,
            },
        )
        .await?;
        create_table(
            manager,
            TableColumns {
                table: CrossShareholdingDocuments::Table,
                document_id: CrossShareholdingDocuments::DocumentId,
                stock_code: CrossShareholdingDocuments::StockCode,
                filer_code: CrossShareholdingDocuments::FilerCode,
                submitted_on: CrossShareholdingDocuments::SubmittedOn,
                payload: CrossShareholdingDocuments::Details,
                created_at: CrossShareholdingDocuments::CreatedAt,
                updated_at: CrossShareholdingDocuments::UpdatedAt,
            },
        )
        .await?;

        create_index(
            manager,
            "idx_large_volume_shareholding_documents_stock_code",
            LargeVolumeDocuments::Table,
            LargeVolumeDocuments::StockCode,
        )
        .await?;
        create_index(
            manager,
            "idx_major_shareholder_documents_stock_code",
            MajorShareholderDocuments::Table,
            MajorShareholderDocuments::StockCode,
        )
        .await?;
        create_index(
            manager,
            "idx_cross_shareholding_documents_stock_code",
            CrossShareholdingDocuments::Table,
            CrossShareholdingDocuments::StockCode,
        )
        .await?;
        create_index(
            manager,
            "idx_large_volume_shareholding_documents_submitted_on",
            LargeVolumeDocuments::Table,
            LargeVolumeDocuments::SubmittedOn,
        )
        .await?;
        create_index(
            manager,
            "idx_major_shareholder_documents_submitted_on",
            MajorShareholderDocuments::Table,
            MajorShareholderDocuments::SubmittedOn,
        )
        .await?;
        create_index(
            manager,
            "idx_cross_shareholding_documents_submitted_on",
            CrossShareholdingDocuments::Table,
            CrossShareholdingDocuments::SubmittedOn,
        )
        .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO large_volume_shareholding_documents \
                    (document_id, stock_code, filer_code, submitted_on, details, created_at, updated_at) \
                 SELECT doc_id, code, edinet_code, sub_date, jsonb_build_object( \
                    'report_type', CASE document->>'LargeHldgTypeCode' \
                        WHEN '1' THEN 'report' WHEN '2' THEN 'amendment' \
                        WHEN '3' THEN 'amendment_rapid_transfer' \
                        WHEN '4' THEN 'report_special' WHEN '5' THEN 'amendment_special' \
                        ELSE 'unknown' END, \
                    'change_reason', document->'ChgRsn', \
                    'total_shares_ratio', document->'TotalShsRatio', \
                    'previous_total_shares_ratio', document->'TotalShsRatioLast', \
                    'holders', COALESCE(( \
                        SELECT jsonb_agg(jsonb_build_object( \
                            'name', holder.value->'HldrName', \
                            'holding_purpose', holder.value->'HldgPurp', \
                            'shares_held', holder.value->'ShsHeld', \
                            'shares_ratio', holder.value->'ShsRatio', \
                            'previous_shares_ratio', holder.value->'ShsRatioLast' \
                        ) ORDER BY holder.ordinality) \
                        FROM jsonb_array_elements(CASE \
                            WHEN jsonb_typeof(document->'Hldrs') = 'array' THEN document->'Hldrs' \
                            ELSE '[]'::jsonb END) WITH ORDINALITY AS holder(value, ordinality) \
                    ), '[]'::jsonb) \
                 ), created_at, updated_at FROM edinet_large_volume_shareholdings",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO major_shareholder_documents \
                    (document_id, stock_code, filer_code, submitted_on, details, created_at, updated_at) \
                 SELECT doc_id, code, edinet_code, sub_date, jsonb_build_object( \
                    'period_end', document->'PerEn', \
                    'report_type', CASE document->>'DocTypeCode' \
                        WHEN '120' THEN 'annual' WHEN '140' THEN 'quarterly' \
                        WHEN '160' THEN 'semi_annual' ELSE 'unknown' END, \
                    'holders', COALESCE(( \
                        SELECT jsonb_agg(jsonb_build_object( \
                            'rank', holder.value->'Rank', \
                            'name', holder.value->'HldrName', \
                            'shares_held', holder.value->'ShsHeld', \
                            'shares_ratio', holder.value->'ShsRatio' \
                        ) ORDER BY holder.ordinality) \
                        FROM jsonb_array_elements(CASE \
                            WHEN jsonb_typeof(document->'Hldrs') = 'array' THEN document->'Hldrs' \
                            ELSE '[]'::jsonb END) WITH ORDINALITY AS holder(value, ordinality) \
                    ), '[]'::jsonb) \
                 ), created_at, updated_at FROM edinet_major_shareholders",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO cross_shareholding_documents \
                    (document_id, stock_code, filer_code, submitted_on, details, created_at, updated_at) \
                 SELECT doc_id, code, edinet_code, sub_date, jsonb_build_object( \
                    'period_end', document->'PerEn', \
                    'holdings', COALESCE(( \
                        SELECT jsonb_agg(jsonb_build_object( \
                            'issuer_name', holding.value->'IsrName', \
                            'issuer_stock_code', holding.value->'IsrCode', \
                            'category', holding.category, \
                            'current_shares', holding.value->'CurShs', \
                            'previous_shares', holding.value->'PriShs', \
                            'current_book_value', holding.value->'CurBookVal', \
                            'previous_book_value', holding.value->'PriBookVal', \
                            'mutual_holding', CASE holding.value->>'IsrHoldsCode' \
                                WHEN '1' THEN 'held' WHEN '0' THEN 'not_held' ELSE 'unknown' END \
                        ) ORDER BY holding.category_order, holding.ordinality) \
                        FROM ( \
                            SELECT value, ordinality, 'specified' AS category, 0 AS category_order \
                            FROM jsonb_array_elements(CASE \
                                WHEN jsonb_typeof(document->'Report'->'Spec') = 'array' \
                                    THEN document->'Report'->'Spec' ELSE '[]'::jsonb END) \
                                WITH ORDINALITY AS entry(value, ordinality) \
                            UNION ALL \
                            SELECT value, ordinality, 'deemed' AS category, 1 AS category_order \
                            FROM jsonb_array_elements(CASE \
                                WHEN jsonb_typeof(document->'Report'->'Deem') = 'array' \
                                    THEN document->'Report'->'Deem' ELSE '[]'::jsonb END) \
                                WITH ORDINALITY AS entry(value, ordinality) \
                        ) AS holding \
                    ), '[]'::jsonb) \
                 ), created_at, updated_at FROM edinet_cross_shareholdings",
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(LegacyLargeVolumeDocuments::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(LegacyMajorShareholderDocuments::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(LegacyCrossShareholdingDocuments::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_table(
            manager,
            TableColumns {
                table: LegacyLargeVolumeDocuments::Table,
                document_id: LegacyLargeVolumeDocuments::DocId,
                stock_code: LegacyLargeVolumeDocuments::Code,
                filer_code: LegacyLargeVolumeDocuments::EdinetCode,
                submitted_on: LegacyLargeVolumeDocuments::SubDate,
                payload: LegacyLargeVolumeDocuments::Document,
                created_at: LegacyLargeVolumeDocuments::CreatedAt,
                updated_at: LegacyLargeVolumeDocuments::UpdatedAt,
            },
        )
        .await?;
        create_table(
            manager,
            TableColumns {
                table: LegacyMajorShareholderDocuments::Table,
                document_id: LegacyMajorShareholderDocuments::DocId,
                stock_code: LegacyMajorShareholderDocuments::Code,
                filer_code: LegacyMajorShareholderDocuments::EdinetCode,
                submitted_on: LegacyMajorShareholderDocuments::SubDate,
                payload: LegacyMajorShareholderDocuments::Document,
                created_at: LegacyMajorShareholderDocuments::CreatedAt,
                updated_at: LegacyMajorShareholderDocuments::UpdatedAt,
            },
        )
        .await?;
        create_table(
            manager,
            TableColumns {
                table: LegacyCrossShareholdingDocuments::Table,
                document_id: LegacyCrossShareholdingDocuments::DocId,
                stock_code: LegacyCrossShareholdingDocuments::Code,
                filer_code: LegacyCrossShareholdingDocuments::EdinetCode,
                submitted_on: LegacyCrossShareholdingDocuments::SubDate,
                payload: LegacyCrossShareholdingDocuments::Document,
                created_at: LegacyCrossShareholdingDocuments::CreatedAt,
                updated_at: LegacyCrossShareholdingDocuments::UpdatedAt,
            },
        )
        .await?;

        create_index(
            manager,
            "idx_edinet_large_volume_shareholdings_edinet_code",
            LegacyLargeVolumeDocuments::Table,
            LegacyLargeVolumeDocuments::EdinetCode,
        )
        .await?;
        create_index(
            manager,
            "idx_edinet_large_volume_shareholdings_code",
            LegacyLargeVolumeDocuments::Table,
            LegacyLargeVolumeDocuments::Code,
        )
        .await?;
        create_index(
            manager,
            "idx_edinet_large_volume_shareholdings_sub_date",
            LegacyLargeVolumeDocuments::Table,
            LegacyLargeVolumeDocuments::SubDate,
        )
        .await?;
        create_index(
            manager,
            "idx_edinet_major_shareholders_edinet_code",
            LegacyMajorShareholderDocuments::Table,
            LegacyMajorShareholderDocuments::EdinetCode,
        )
        .await?;
        create_index(
            manager,
            "idx_edinet_major_shareholders_code",
            LegacyMajorShareholderDocuments::Table,
            LegacyMajorShareholderDocuments::Code,
        )
        .await?;
        create_index(
            manager,
            "idx_edinet_major_shareholders_sub_date",
            LegacyMajorShareholderDocuments::Table,
            LegacyMajorShareholderDocuments::SubDate,
        )
        .await?;
        create_index(
            manager,
            "idx_edinet_cross_shareholdings_edinet_code",
            LegacyCrossShareholdingDocuments::Table,
            LegacyCrossShareholdingDocuments::EdinetCode,
        )
        .await?;
        create_index(
            manager,
            "idx_edinet_cross_shareholdings_code",
            LegacyCrossShareholdingDocuments::Table,
            LegacyCrossShareholdingDocuments::Code,
        )
        .await?;
        create_index(
            manager,
            "idx_edinet_cross_shareholdings_sub_date",
            LegacyCrossShareholdingDocuments::Table,
            LegacyCrossShareholdingDocuments::SubDate,
        )
        .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO edinet_large_volume_shareholdings \
                    (doc_id, code, edinet_code, sub_date, document, created_at, updated_at) \
                 SELECT document_id, stock_code, filer_code, submitted_on, jsonb_build_object( \
                    'DocId', document_id, 'Code', stock_code, 'EdinetCode', filer_code, \
                    'SubDate', to_char(submitted_on, 'YYYY-MM-DD'), \
                    'LargeHldgTypeCode', CASE details->>'report_type' \
                        WHEN 'report' THEN '1' WHEN 'amendment' THEN '2' \
                        WHEN 'amendment_rapid_transfer' THEN '3' \
                        WHEN 'report_special' THEN '4' WHEN 'amendment_special' THEN '5' ELSE NULL END, \
                    'ChgRsn', details->'change_reason', \
                    'TotalShsRatio', details->'total_shares_ratio', \
                    'TotalShsRatioLast', details->'previous_total_shares_ratio', \
                    'Hldrs', COALESCE(( \
                        SELECT jsonb_agg(jsonb_build_object( \
                            'HldrName', holder.value->'name', \
                            'HldgPurp', holder.value->'holding_purpose', \
                            'ShsHeld', holder.value->'shares_held', \
                            'ShsRatio', holder.value->'shares_ratio', \
                            'ShsRatioLast', holder.value->'previous_shares_ratio' \
                        ) ORDER BY holder.ordinality) \
                        FROM jsonb_array_elements(details->'holders') \
                            WITH ORDINALITY AS holder(value, ordinality) \
                    ), '[]'::jsonb) \
                 ), created_at, updated_at FROM large_volume_shareholding_documents",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO edinet_major_shareholders \
                    (doc_id, code, edinet_code, sub_date, document, created_at, updated_at) \
                 SELECT document_id, stock_code, filer_code, submitted_on, jsonb_build_object( \
                    'DocId', document_id, 'Code', stock_code, 'EdinetCode', filer_code, \
                    'SubDate', to_char(submitted_on, 'YYYY-MM-DD'), \
                    'PerEn', details->'period_end', \
                    'DocTypeCode', CASE details->>'report_type' \
                        WHEN 'annual' THEN '120' WHEN 'quarterly' THEN '140' \
                        WHEN 'semi_annual' THEN '160' ELSE NULL END, \
                    'Hldrs', COALESCE(( \
                        SELECT jsonb_agg(jsonb_build_object( \
                            'Rank', holder.value->'rank', 'HldrName', holder.value->'name', \
                            'ShsHeld', holder.value->'shares_held', \
                            'ShsRatio', holder.value->'shares_ratio' \
                        ) ORDER BY holder.ordinality) \
                        FROM jsonb_array_elements(details->'holders') \
                            WITH ORDINALITY AS holder(value, ordinality) \
                    ), '[]'::jsonb) \
                 ), created_at, updated_at FROM major_shareholder_documents",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO edinet_cross_shareholdings \
                    (doc_id, code, edinet_code, sub_date, document, created_at, updated_at) \
                 SELECT document_id, stock_code, filer_code, submitted_on, jsonb_build_object( \
                    'DocId', document_id, 'Code', stock_code, 'EdinetCode', filer_code, \
                    'SubDate', to_char(submitted_on, 'YYYY-MM-DD'), \
                    'PerEn', details->'period_end', \
                    'Report', jsonb_build_object( \
                        'Spec', COALESCE(( \
                            SELECT jsonb_agg(jsonb_build_object( \
                                'IsrName', holding.value->'issuer_name', \
                                'IsrCode', holding.value->'issuer_stock_code', \
                                'CurShs', holding.value->'current_shares', \
                                'PriShs', holding.value->'previous_shares', \
                                'CurBookVal', holding.value->'current_book_value', \
                                'PriBookVal', holding.value->'previous_book_value', \
                                'IsrHoldsCode', CASE holding.value->>'mutual_holding' \
                                    WHEN 'held' THEN '1' WHEN 'not_held' THEN '0' ELSE NULL END \
                            ) ORDER BY holding.ordinality) \
                            FROM jsonb_array_elements(details->'holdings') \
                                WITH ORDINALITY AS holding(value, ordinality) \
                            WHERE holding.value->>'category' = 'specified' \
                        ), '[]'::jsonb), \
                        'Deem', COALESCE(( \
                            SELECT jsonb_agg(jsonb_build_object( \
                                'IsrName', holding.value->'issuer_name', \
                                'IsrCode', holding.value->'issuer_stock_code', \
                                'CurShs', holding.value->'current_shares', \
                                'PriShs', holding.value->'previous_shares', \
                                'CurBookVal', holding.value->'current_book_value', \
                                'PriBookVal', holding.value->'previous_book_value', \
                                'IsrHoldsCode', CASE holding.value->>'mutual_holding' \
                                    WHEN 'held' THEN '1' WHEN 'not_held' THEN '0' ELSE NULL END \
                            ) ORDER BY holding.ordinality) \
                            FROM jsonb_array_elements(details->'holdings') \
                                WITH ORDINALITY AS holding(value, ordinality) \
                            WHERE holding.value->>'category' = 'deemed' \
                        ), '[]'::jsonb) \
                    ) \
                 ), created_at, updated_at FROM cross_shareholding_documents",
            )
            .await?;

        manager
            .drop_table(Table::drop().table(LargeVolumeDocuments::Table).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(MajorShareholderDocuments::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(CrossShareholdingDocuments::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
