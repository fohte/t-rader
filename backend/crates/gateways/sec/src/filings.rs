use std::collections::HashSet;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::sec_filing_source::{
    SecFilingDocument, SecFilingSource, SecFilingSourceError,
};
use reqwest::Url;
use serde::Deserialize;
use serde_json::Value;

use crate::{client::SecClient, error::SecClientError};

#[derive(Deserialize)]
struct RawSubmissions {
    cik: Value,
    tickers: Vec<String>,
    filings: RawFilingHistory,
}

#[derive(Deserialize)]
struct RawFilingHistory {
    recent: RawFilingArrays,
    #[serde(default)]
    files: Vec<RawFilingFile>,
}

#[derive(Deserialize)]
struct RawFilingArrays {
    form: Vec<String>,
    #[serde(rename = "reportDate")]
    report_date: Vec<Option<String>>,
    #[serde(rename = "accessionNumber")]
    accession_number: Vec<Option<String>>,
    #[serde(rename = "primaryDocument")]
    primary_document: Vec<Option<String>>,
}

#[derive(Deserialize)]
struct RawFilingFile {
    name: String,
    #[serde(rename = "filingTo")]
    filing_to: String,
}

struct FilingRecord {
    form: String,
    report_date: NaiveDate,
    accession_number: String,
    primary_document: String,
}

#[async_trait]
impl SecFilingSource for SecClient {
    async fn fetch_periodic_reports(
        &self,
        cik: &str,
        ticker: &str,
        report_dates: &[NaiveDate],
    ) -> Result<Vec<SecFilingDocument>, SecFilingSourceError> {
        self.fetch_periodic_reports_inner(cik, ticker, report_dates)
            .await
            .map_err(|error| SecFilingSourceError::Failed(error.to_string()))
    }
}

impl SecClient {
    async fn fetch_periodic_reports_inner(
        &self,
        cik: &str,
        ticker: &str,
        report_dates: &[NaiveDate],
    ) -> Result<Vec<SecFilingDocument>, SecClientError> {
        let Some(cik) = normalize_cik(cik) else {
            return Ok(Vec::new());
        };
        let Some(ticker) = normalize_ticker(ticker) else {
            return Ok(Vec::new());
        };
        if report_dates.is_empty() {
            return Ok(Vec::new());
        }

        let submissions_url = self
            .data_url()
            .join(&format!("submissions/CIK{:0>10}.json", cik))
            .map_err(|error| SecClientError::Config(error.to_string()))?;
        let body = self.get_text(submissions_url).await?;
        let submissions: RawSubmissions = serde_json::from_str(&body)
            .map_err(|error| SecClientError::Parse(error.to_string()))?;
        if normalize_cik(&value_as_string(&submissions.cik).unwrap_or_default()).as_deref()
            != Some(cik.as_str())
            || !submissions
                .tickers
                .iter()
                .filter_map(|value| normalize_ticker(value))
                .any(|value| value == ticker)
        {
            return Ok(Vec::new());
        }

        let requested_dates = report_dates.iter().copied().collect::<HashSet<_>>();
        let mut records = parse_filing_arrays(submissions.filings.recent)?;
        let recent_dates = records
            .iter()
            .filter(|record| is_supported_form(&record.form))
            .filter(|record| requested_dates.contains(&record.report_date))
            .map(|record| record.report_date)
            .collect::<HashSet<_>>();
        let missing_dates = requested_dates
            .difference(&recent_dates)
            .copied()
            .collect::<HashSet<_>>();
        if let Some(earliest_missing_date) = missing_dates.iter().min().copied() {
            for file in submissions.filings.files.into_iter().filter(|file| {
                parse_report_date(&file.filing_to)
                    .is_some_and(|filing_to| filing_to >= earliest_missing_date)
            }) {
                if !is_safe_archive_filename(&file.name) {
                    return Err(SecClientError::Parse(
                        "SEC submissions history contains an invalid archive filename".to_owned(),
                    ));
                }
                let archive_url = self
                    .data_url()
                    .join(&format!("submissions/{}", file.name))
                    .map_err(|error| SecClientError::Config(error.to_string()))?;
                let body = self.get_text(archive_url).await?;
                let archive: RawFilingArrays = serde_json::from_str(&body)
                    .map_err(|error| SecClientError::Parse(error.to_string()))?;
                records.extend(parse_filing_arrays(archive)?);
            }
        }

        let mut documents = Vec::new();
        let mut seen_accessions = HashSet::new();
        for record in records.into_iter().filter(|record| {
            is_supported_form(&record.form) && requested_dates.contains(&record.report_date)
        }) {
            if !seen_accessions.insert(record.accession_number.clone()) {
                continue;
            }
            let Some(accession_path) = accession_path(&record.accession_number, &cik) else {
                continue;
            };
            if !is_safe_primary_document(&record.primary_document) {
                continue;
            }
            let document_url = filing_document_url(
                self.archive_url(),
                &cik,
                &accession_path,
                &record.primary_document,
            )?;
            if !self.document_is_available(document_url.clone()).await? {
                continue;
            }
            documents.push(SecFilingDocument {
                form: record.form,
                report_date: record.report_date,
                accession_number: record.accession_number,
                url: document_url.to_string(),
            });
        }

        Ok(documents)
    }

    async fn document_is_available(&self, url: Url) -> Result<bool, SecClientError> {
        let mut response = self.get_response(url).await?;
        let status = response.status().as_u16();
        if status == 404 {
            return Ok(false);
        }
        if !(200..300).contains(&status) {
            return Err(SecClientError::Status(status));
        }
        response
            .chunk()
            .await
            .map(|chunk| chunk.is_some_and(|chunk| !chunk.is_empty()))
            .map_err(|error| SecClientError::Request(error.without_url().to_string()))
    }
}

fn parse_filing_arrays(arrays: RawFilingArrays) -> Result<Vec<FilingRecord>, SecClientError> {
    let expected = arrays.form.len();
    if arrays.report_date.len() != expected
        || arrays.accession_number.len() != expected
        || arrays.primary_document.len() != expected
    {
        return Err(SecClientError::Parse(
            "SEC submissions filing arrays have inconsistent lengths".to_owned(),
        ));
    }

    Ok((0..expected)
        .filter_map(|index| {
            let form = arrays.form[index].clone();
            let report_date = arrays.report_date[index]
                .as_deref()
                .and_then(parse_report_date)?;
            let accession_number = arrays.accession_number[index].as_deref()?.trim().to_owned();
            let primary_document = arrays.primary_document[index].as_deref()?.trim().to_owned();
            (!accession_number.is_empty() && !primary_document.is_empty()).then_some(FilingRecord {
                form,
                report_date,
                accession_number,
                primary_document,
            })
        })
        .collect())
}

fn normalize_cik(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 10
        || !value.chars().all(|character| character.is_ascii_digit())
    {
        return None;
    }
    let value = value.trim_start_matches('0');
    (!value.is_empty()).then(|| value.to_owned())
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => number.as_u64().map(|number| number.to_string()),
        Value::String(value) => Some(value.clone()),
        _ => None,
    }
}

fn normalize_ticker(value: &str) -> Option<String> {
    let ticker = value.trim().to_ascii_uppercase().replace('.', "-");
    if ticker.is_empty()
        || !ticker.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '-'
        })
    {
        return None;
    }
    Some(ticker)
}

fn parse_report_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

fn is_supported_form(form: &str) -> bool {
    matches!(form, "10-Q" | "10-K")
}

fn is_safe_archive_filename(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

fn accession_path(value: &str, expected_cik: &str) -> Option<String> {
    let mut parts = value.split('-');
    let cik = parts.next()?;
    let year = parts.next()?;
    let sequence = parts.next()?;
    if parts.next().is_some()
        || cik.len() != 10
        || year.len() != 2
        || sequence.len() != 6
        || ![cik, year, sequence]
            .iter()
            .all(|part| part.chars().all(|character| character.is_ascii_digit()))
    {
        return None;
    }
    if normalize_cik(cik).as_deref() != Some(expected_cik) {
        return None;
    }
    Some(format!("{cik}{year}{sequence}"))
}

fn is_safe_primary_document(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.chars().all(|character| {
            !character.is_control() && !matches!(character, '/' | '\\' | '?' | '#')
        })
}

fn filing_document_url(
    archive_base: &Url,
    cik: &str,
    accession_path: &str,
    primary_document: &str,
) -> Result<Url, SecClientError> {
    let mut url = archive_base.clone();
    let mut path = url.path_segments_mut().map_err(|_| {
        SecClientError::Config("SEC archive URL cannot accept path segments".to_owned())
    })?;
    path.pop_if_empty()
        .push(cik)
        .push(accession_path)
        .push(primary_document);
    drop(path);
    Ok(url)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::sec_filing_source::{
        SecFilingDocument, SecFilingSource, SecFilingSourceError,
    };
    use indoc::indoc;
    use rstest::rstest;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::client::{SecClient, SecRateLimiter};

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap_or(NaiveDate::MIN)
    }

    fn test_client(server: &MockServer) -> SecClient {
        let base = format!("{}/", server.uri());
        SecClient::build(
            "Synthetic Client contact@fictional.invalid",
            &format!("{base}company_tickers_exchange.json"),
            &base,
            &format!("{base}Archives/edgar/data/"),
            SecRateLimiter::TestNoop,
        )
        .expect("client initializes")
    }

    fn submissions(ticker: &str, cik: u64) -> String {
        submissions_with_accession(ticker, cik, "0900000001-99-000001")
    }

    fn submissions_with_accession(ticker: &str, cik: u64, accession: &str) -> String {
        format!(
            r#"{{
                "cik": {cik},
                "tickers": ["{ticker}"],
                "filings": {{
                    "recent": {{
                        "form": ["10-Q", "8-K", "10-K"],
                        "reportDate": ["2099-06-30", "2099-06-30", "2099-06-29"],
                        "accessionNumber": ["{accession}", "0900000001-99-000002", "0900000001-99-000003"],
                        "primaryDocument": ["quarter.htm", "current.htm", "annual.htm"]
                    }},
                    "files": []
                }}
            }}"#
        )
    }

    #[rstest]
    #[tokio::test]
    async fn fetches_only_verified_periodic_reports_for_the_exact_report_date() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/submissions/CIK0900000001.json"))
            .and(header(
                "user-agent",
                "Synthetic Client contact@fictional.invalid",
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(submissions("QZ-A", 900000001)),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(
                "/Archives/edgar/data/900000001/090000000199000001/quarter.htm",
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_string("<html>synthetic filing</html>"),
            )
            .mount(&server)
            .await;
        let client = test_client(&server);

        let actual = client
            .fetch_periodic_reports("900000001", "QZ-A", &[date(2099, 6, 30)])
            .await;

        assert_eq!(
            actual,
            Ok(vec![SecFilingDocument {
                form: "10-Q".to_owned(),
                report_date: date(2099, 6, 30),
                accession_number: "0900000001-99-000001".to_owned(),
                url: format!(
                    "{}/Archives/edgar/data/900000001/090000000199000001/quarter.htm",
                    server.uri()
                ),
            }]),
        );
    }

    #[rstest]
    #[case::cik_mismatch("QZ-A", 900000002, "QZ-A")]
    #[case::ticker_mismatch("QZ-B", 900000001, "QZ-A")]
    #[tokio::test]
    async fn does_not_link_when_submissions_do_not_confirm_the_cik_and_ticker(
        #[case] submissions_ticker: &str,
        #[case] submissions_cik: u64,
        #[case] requested_ticker: &str,
    ) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/submissions/CIK0900000001.json"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(submissions(submissions_ticker, submissions_cik)),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(
                "/Archives/edgar/data/900000001/090000000199000001/quarter.htm",
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_string("<html>synthetic filing</html>"),
            )
            .mount(&server)
            .await;
        let client = test_client(&server);

        let actual = client
            .fetch_periodic_reports("900000001", requested_ticker, &[date(2099, 6, 30)])
            .await;

        assert_eq!(actual, Ok(Vec::new()));
    }

    #[rstest]
    #[tokio::test]
    async fn leaves_a_missing_primary_document_unlinked() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/submissions/CIK0900000001.json"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(submissions("QZ-A", 900000001)),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(
                "/Archives/edgar/data/900000001/090000000199000001/quarter.htm",
            ))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server);

        let actual = client
            .fetch_periodic_reports("900000001", "QZ-A", &[date(2099, 6, 30)])
            .await;

        assert_eq!(actual, Ok(Vec::new()));
    }

    #[rstest]
    #[tokio::test]
    async fn does_not_link_when_no_periodic_report_matches_the_report_date() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/submissions/CIK0900000001.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(indoc! {
                r#"{
                    "cik": 900000001,
                    "tickers": ["QZ-A"],
                    "filings": {
                        "recent": {
                            "form": ["10-K"],
                            "reportDate": ["2099-06-29"],
                            "accessionNumber": ["0900000001-99-000003"],
                            "primaryDocument": ["annual.htm"]
                        },
                        "files": []
                    }
                }"#
            }))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(
                "/Archives/edgar/data/900000001/090000000199000003/annual.htm",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string("synthetic filing"))
            .expect(0)
            .mount(&server)
            .await;
        let client = test_client(&server);

        let actual = client
            .fetch_periodic_reports("900000001", "QZ-A", &[date(2099, 6, 30)])
            .await;
        server.verify().await;

        assert_eq!(actual, Ok(Vec::new()));
    }

    #[rstest]
    #[tokio::test]
    async fn fetches_a_historical_filing_from_the_submissions_archive() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/submissions/CIK0900000001.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(indoc! {
                r#"{
                    "cik": 900000001,
                    "tickers": ["QZ-A"],
                    "filings": {
                        "recent": {
                            "form": [],
                            "reportDate": [],
                            "accessionNumber": [],
                            "primaryDocument": []
                        },
                        "files": [{"name": "old-submissions.json", "filingTo": "2099-07-30"}]
                    }
                }"#
            }))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/submissions/old-submissions.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(indoc! {
                r#"{
                    "form": ["10-K"],
                    "reportDate": ["2099-06-30"],
                    "accessionNumber": ["0900000001-99-000004"],
                    "primaryDocument": ["annual.htm"]
                }"#
            }))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(
                "/Archives/edgar/data/900000001/090000000199000004/annual.htm",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string("synthetic filing"))
            .mount(&server)
            .await;
        let client = test_client(&server);

        let actual = client
            .fetch_periodic_reports("900000001", "QZ-A", &[date(2099, 6, 30)])
            .await;

        assert_eq!(
            actual,
            Ok(vec![SecFilingDocument {
                form: "10-K".to_owned(),
                report_date: date(2099, 6, 30),
                accession_number: "0900000001-99-000004".to_owned(),
                url: format!(
                    "{}/Archives/edgar/data/900000001/090000000199000004/annual.htm",
                    server.uri()
                ),
            }]),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn does_not_build_a_document_url_from_an_accession_for_another_cik() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/submissions/CIK0900000001.json"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(submissions_with_accession(
                    "QZ-A",
                    900000001,
                    "0900000002-99-000001",
                )),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(
                "/Archives/edgar/data/900000001/090000000299000001/quarter.htm",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string("synthetic filing"))
            .mount(&server)
            .await;
        let client = test_client(&server);

        let actual = client
            .fetch_periodic_reports("900000001", "QZ-A", &[date(2099, 6, 30)])
            .await;

        assert_eq!(actual, Ok(Vec::new()));
    }

    #[rstest]
    #[tokio::test]
    async fn leaves_filings_pending_when_the_sec_request_fails() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/submissions/CIK0900000001.json"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let client = test_client(&server);

        let actual = client
            .fetch_periodic_reports("900000001", "QZ-A", &[date(2099, 6, 30)])
            .await;

        assert_eq!(
            actual,
            Err(SecFilingSourceError::Failed(
                "SEC returned HTTP status 503".to_owned(),
            )),
        );
    }
}
