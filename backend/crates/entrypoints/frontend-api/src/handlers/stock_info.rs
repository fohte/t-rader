use axum::Json;
use axum::extract::{FromRef, State};
use chrono::NaiveDate;
use core_application::financial_summary::FinancialSummaryUseCases;
use core_domain::financial_summary::FinancialSummaryPeriodType;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonPath;
use crate::extractors::JsonQuery;
use crate::models::{
    StockFinancialSummaryResponse, StockGroupMembershipResponse, StockNextEarningsResponse,
    StockValuationResponse,
};

/// 銘柄の所属グループ
#[utoipa::path(
    get,
    path = "/api/stocks/{stock_id}/groups",
    tag = "stocks",
    params(("stock_id" = String, Path, description = "銘柄 ID")),
    responses(
        (status = 200, body = Vec<StockGroupMembershipResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_stock_groups(
    State(state): State<FrontendApiState>,
    JsonPath(stock_id): JsonPath<String>,
) -> Result<Json<Vec<StockGroupMembershipResponse>>, AppError> {
    let memberships = state
        .stock_group_use_cases
        .list_memberships(&[stock_id], None)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(Json(memberships.into_iter().map(Into::into).collect()))
}

/// 銘柄の次の決算予定
#[utoipa::path(
    get,
    path = "/api/stocks/{stock_id}/next-earnings",
    tag = "stocks",
    params(("stock_id" = String, Path, description = "銘柄 ID")),
    responses(
        (status = 200, body = Option<StockNextEarningsResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_stock_next_earnings(
    State(state): State<FrontendApiState>,
    JsonPath(stock_id): JsonPath<String>,
) -> Result<Json<Option<StockNextEarningsResponse>>, AppError> {
    let event = state
        .calendar_event_read_use_cases
        .find_next_earnings_for_stock(&stock_id)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(Json(event.map(Into::into)))
}

/// 銘柄の最新バリュエーション
#[utoipa::path(
    get,
    path = "/api/stocks/{stock_id}/valuation",
    tag = "stocks",
    params(("stock_id" = String, Path, description = "銘柄 ID")),
    responses(
        (status = 200, body = Option<StockValuationResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_stock_valuation(
    State(state): State<FrontendApiState>,
    JsonPath(stock_id): JsonPath<String>,
) -> Result<Json<Option<StockValuationResponse>>, AppError> {
    let valuation = state
        .valuation_use_cases
        .find_latest_for_symbol(&stock_id)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(Json(valuation.map(Into::into)))
}

/// 銘柄の決算短信検索条件
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct StockFinancialSummaryQuery {
    /// 対象期末。期区分と同時に指定する
    pub current_period_end: Option<NaiveDate>,
    /// 対象期区分。期末と同時に指定する
    #[param(inline)]
    pub current_period_type: Option<StockFinancialSummaryPeriodType>,
}

/// 銘柄の決算短信検索に指定できる対象期区分
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
pub enum StockFinancialSummaryPeriodType {
    #[serde(rename = "FY")]
    #[schema(rename = "FY")]
    Fy,
    #[serde(rename = "1Q")]
    #[schema(rename = "1Q")]
    FirstQuarter,
    #[serde(rename = "2Q")]
    #[schema(rename = "2Q")]
    SecondQuarter,
    #[serde(rename = "3Q")]
    #[schema(rename = "3Q")]
    ThirdQuarter,
    #[serde(rename = "4Q")]
    #[schema(rename = "4Q")]
    FourthQuarter,
}

impl From<StockFinancialSummaryPeriodType> for FinancialSummaryPeriodType {
    fn from(value: StockFinancialSummaryPeriodType) -> Self {
        match value {
            StockFinancialSummaryPeriodType::Fy => Self::Fy,
            StockFinancialSummaryPeriodType::FirstQuarter => Self::FirstQuarter,
            StockFinancialSummaryPeriodType::SecondQuarter => Self::SecondQuarter,
            StockFinancialSummaryPeriodType::ThirdQuarter => Self::ThirdQuarter,
            StockFinancialSummaryPeriodType::FourthQuarter => Self::FourthQuarter,
        }
    }
}

/// 銘柄の決算短信要約一覧
#[utoipa::path(
    get,
    path = "/api/stocks/{stock_id}/financial-summaries",
    tag = "stocks",
    params(
        ("stock_id" = String, Path, description = "銘柄 ID"),
        StockFinancialSummaryQuery,
    ),
    responses(
        (status = 200, body = Vec<StockFinancialSummaryResponse>),
        (status = 400, description = "検索条件が不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_stock_financial_summaries(
    State(use_cases): State<FinancialSummaryUseCases>,
    JsonPath(stock_id): JsonPath<String>,
    JsonQuery(params): JsonQuery<StockFinancialSummaryQuery>,
) -> Result<Json<Vec<StockFinancialSummaryResponse>>, AppError> {
    let period_filter = stock_financial_summary_period_filter(params)?;
    let summaries = match period_filter {
        None => use_cases.list_for_symbol(&stock_id).await,
        Some((period_end, period_type)) => {
            use_cases
                .list_for_period(&stock_id, period_end, period_type)
                .await
        }
    }
    .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(Json(summaries.into_iter().map(Into::into).collect()))
}

impl FromRef<FrontendApiState> for FinancialSummaryUseCases {
    fn from_ref(state: &FrontendApiState) -> Self {
        state.financial_summary_use_cases.clone()
    }
}

fn stock_financial_summary_period_filter(
    params: StockFinancialSummaryQuery,
) -> Result<Option<(NaiveDate, FinancialSummaryPeriodType)>, AppError> {
    match (params.current_period_end, params.current_period_type) {
        (None, None) => Ok(None),
        (Some(period_end), Some(period_type)) => Ok(Some((period_end, period_type.into()))),
        _ => Err(AppError::Validation(
            "current_period_end and current_period_type must be provided together".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;

    use axum::body::to_bytes;
    use axum::extract::FromRequestParts;
    use axum::http::{Request, StatusCode};
    use axum::response::IntoResponse;
    use chrono::NaiveDate;
    use core_application::financial_summary::{
        FinancialSummaryRepository, FinancialSummaryRepositoryError, FinancialSummaryUseCases,
        SharedFinancialSummaryRepository,
    };
    use core_application::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork, UnitOfWorkTransaction};
    use core_domain::financial_summary::{FinancialSummary, FinancialSummaryPeriodType};
    use rstest::rstest;
    use serde_json::{Value, json};

    use super::{
        StockFinancialSummaryPeriodType, StockFinancialSummaryQuery, list_stock_financial_summaries,
    };
    use crate::extractors::JsonPath;
    use crate::extractors::JsonQuery;

    async fn response_parts(response: axum::response::Response) -> (StatusCode, Value) {
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body is readable");
        (
            status,
            serde_json::from_slice(&body).expect("response body is valid JSON"),
        )
    }

    fn period_end() -> NaiveDate {
        NaiveDate::from_ymd_opt(2030, 6, 30).expect("valid date")
    }

    type RepositoryFuture<'a, T> =
        Pin<Box<dyn Future<Output = Result<T, FinancialSummaryRepositoryError>> + Send + 'a>>;

    struct EmptyFinancialSummaryRepository;

    impl FinancialSummaryRepository for EmptyFinancialSummaryRepository {
        fn latest_disclosure_date<'life0, 'async_trait>(
            &'life0 self,
        ) -> RepositoryFuture<'async_trait, Option<NaiveDate>>
        where
            'life0: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(std::future::ready(Ok(None)))
        }

        fn upsert<'life0, 'life1, 'async_trait>(
            &'life0 self,
            _transaction: &'life1 UnitOfWorkTransaction,
            summaries: Vec<FinancialSummary>,
        ) -> RepositoryFuture<'async_trait, usize>
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(std::future::ready(Ok(summaries.len())))
        }

        fn find_for_symbol<'life0, 'life1, 'async_trait>(
            &'life0 self,
            _symbol: &'life1 str,
            _limit: u64,
        ) -> RepositoryFuture<'async_trait, Vec<FinancialSummary>>
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(std::future::ready(Ok(Vec::new())))
        }

        fn find_for_period<'life0, 'life1, 'async_trait>(
            &'life0 self,
            _symbol: &'life1 str,
            _period_end: NaiveDate,
            _period_type: FinancialSummaryPeriodType,
        ) -> RepositoryFuture<'async_trait, Vec<FinancialSummary>>
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(std::future::ready(Ok(Vec::new())))
        }
    }

    fn financial_summary_use_cases() -> FinancialSummaryUseCases {
        let unit_of_work: SharedUnitOfWork = Arc::new(FakeUnitOfWork::new());
        let repository: SharedFinancialSummaryRepository =
            Arc::new(EmptyFinancialSummaryRepository);
        FinancialSummaryUseCases::new(unit_of_work, repository)
    }

    async fn handler_response(query: StockFinancialSummaryQuery) -> (StatusCode, Value) {
        let result = list_stock_financial_summaries(
            axum::extract::State(financial_summary_use_cases()),
            JsonPath("demo-code".to_owned()),
            JsonQuery(query),
        )
        .await;
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("invalid period query must fail the handler"),
        };
        response_parts(error.into_response()).await
    }

    #[rstest]
    #[case::end_only(Some(period_end()), None)]
    #[case::type_only(None, Some(StockFinancialSummaryPeriodType::SecondQuarter))]
    #[tokio::test]
    async fn handler_returns_bad_request_when_only_one_period_parameter_is_set(
        #[case] current_period_end: Option<NaiveDate>,
        #[case] current_period_type: Option<StockFinancialSummaryPeriodType>,
    ) {
        assert_eq!(
            handler_response(StockFinancialSummaryQuery {
                current_period_end,
                current_period_type,
            })
            .await,
            (
                StatusCode::BAD_REQUEST,
                json!({
                    "error": "current_period_end and current_period_type must be provided together"
                }),
            ),
        );
    }

    #[tokio::test]
    async fn query_extractor_rejects_an_unsupported_period_type() {
        let request = Request::builder()
            .uri("/api/stocks/demo-code/financial-summaries?current_period_end=2030-06-30&current_period_type=8Q")
            .body(())
            .expect("valid test request");
        let (mut parts, _) = request.into_parts();
        let result =
            JsonQuery::<StockFinancialSummaryQuery>::from_request_parts(&mut parts, &()).await;
        let rejection = match result {
            Err(rejection) => rejection,
            Ok(_) => panic!("unsupported period type must fail query deserialization"),
        };

        assert_eq!(
            response_parts(rejection.into_response()).await,
            (
                StatusCode::BAD_REQUEST,
                json!({
                    "error": "Failed to deserialize query string: current_period_type: unknown variant `8Q`, expected one of `FY`, `1Q`, `2Q`, `3Q`, `4Q`"
                }),
            ),
        );
    }

    #[test]
    fn openapi_documents_period_type_values_and_validation_response() {
        let document = serde_json::to_value(crate::router().into_openapi())
            .expect("OpenAPI document serializes");
        let operation = &document["paths"]["/api/stocks/{stock_id}/financial-summaries"]["get"];
        let parameters = operation["parameters"]
            .as_array()
            .expect("query parameters are listed");
        let period_type_parameter = parameters
            .iter()
            .find(|parameter| parameter["name"] == "current_period_type")
            .expect("period type parameter is documented");

        assert_eq!(
            (period_type_parameter, &operation["responses"]["400"],),
            (
                &json!({
                    "name": "current_period_type",
                    "in": "query",
                    "description": "対象期区分。期末と同時に指定する",
                    "required": false,
                    "schema": {
                        "description": "銘柄の決算短信検索に指定できる対象期区分",
                        "type": "string",
                        "enum": ["FY", "1Q", "2Q", "3Q", "4Q"]
                    }
                }),
                &json!({
                    "description": "検索条件が不正",
                    "content": {
                        "application/json": {
                            "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                        }
                    }
                }),
            ),
        );
    }
}
