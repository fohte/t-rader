#!/usr/bin/env bash
set -euo pipefail

mkdir -p "$CAPTURE_OUTPUT_DIR"
jq -n \
  --arg tests "$TEST_OUTCOME" \
  --arg log_capture "$LOG_CAPTURE_OUTCOME" \
  --arg log_upload "$LOG_UPLOAD_OUTCOME" \
  --arg comparison "$INPUT_COMPARISON_OUTCOME" \
  --arg comparison_matches "$INPUT_COMPARISON_MATCHES" \
  --arg ghcr_login "$GHCR_LOGIN_OUTCOME" \
  --arg plan_capture "$PLAN_CAPTURE_OUTCOME" \
  '{backend_tests: $tests, log_capture: $log_capture, log_upload: $log_upload, input_comparison: $comparison, input_comparison_matches: $comparison_matches, ghcr_login: $ghcr_login, plan_capture: $plan_capture}' \
  > "$CAPTURE_OUTPUT_DIR/prerequisite-status.json"
if [[ "$PLAN_CAPTURE_OUTCOME" == "skipped" ]]; then
  printf 'Replay and EXPLAIN were not run because a required prerequisite did not pass.\n' \
    > "$CAPTURE_OUTPUT_DIR/replay-skipped.txt"
fi
