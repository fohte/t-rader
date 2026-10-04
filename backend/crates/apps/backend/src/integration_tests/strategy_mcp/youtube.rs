#[cfg(test)]
mod tests {
    use rstest::rstest;
    use uuid::Uuid;

    use super::super::dto::QueryYoutubeParams;
    use super::super::tests_common::mock_db_with_strategy;

    fn params(youtube_url: &str, questions: &[&str]) -> QueryYoutubeParams {
        QueryYoutubeParams {
            youtube_url: youtube_url.into(),
            questions: questions
                .iter()
                .map(|question| (*question).into())
                .collect(),
        }
    }

    #[tokio::test]
    async fn query_youtube_requires_litellm_client() {
        let strategy_id = Uuid::new_v4();
        let server = super::super::tests_common::build_server(mock_db_with_strategy(strategy_id));
        let err = server
            .query_youtube(
                strategy_id,
                "example-model-youtube".to_string(),
                params(
                    "https://www.youtube.com/watch?v=sample-video-id",
                    &["summarize the video"],
                ),
            )
            .await
            .expect_err("expected internal error");
        assert_eq!(
            err,
            rmcp::ErrorData::internal_error("litellm client is not configured", None),
        );
    }

    #[rstest]
    #[case::empty_youtube_url("", &["question"], "youtube_url must not be empty")]
    #[case::non_youtube_host(
        "https://example.invalid/watch?v=sample-video-id",
        &["question"],
        "youtube_url must use an allowed YouTube host"
    )]
    #[case::lookalike_youtube_host(
        "https://youtube.com.example.invalid/watch?v=sample-video-id",
        &["question"],
        "youtube_url must use an allowed YouTube host"
    )]
    #[case::empty_questions(
        "https://www.youtube.com/watch?v=sample-video-id",
        &[],
        "questions must not be empty"
    )]
    #[case::blank_question(
        "https://www.youtube.com/watch?v=sample-video-id",
        &["  \t  "],
        "questions must not contain empty values"
    )]
    #[tokio::test]
    async fn query_youtube_rejects_invalid_params(
        #[case] youtube_url: &str,
        #[case] questions: &[&str],
        #[case] expected_message: &str,
    ) {
        let strategy_id = Uuid::new_v4();
        let server = super::super::tests_common::build_server(mock_db_with_strategy(strategy_id));
        let err = server
            .query_youtube(
                strategy_id,
                "example-model-youtube".to_string(),
                params(youtube_url, questions),
            )
            .await
            .expect_err("expected invalid params");
        assert_eq!(
            err,
            rmcp::ErrorData::invalid_params(expected_message.to_string(), None),
        );
    }
}
