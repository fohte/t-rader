#!/usr/bin/env bash
set -euo pipefail

if [[ "$TEST_OUTCOME" != "success" ||
  "$LOG_CAPTURE_OUTCOME" != "success" ||
  "$LOG_UPLOAD_OUTCOME" != "success" ||
  "$GROUP_COMPARISON_MATCHES" != "true" ||
  "$PLAN_CAPTURE_OUTCOME" != "success" ]]; then
  printf 'tests=%s log_capture=%s log_upload=%s input_comparison=%s input_matches=%s group_matches=%s plan_capture=%s\n' \
    "$TEST_OUTCOME" "$LOG_CAPTURE_OUTCOME" "$LOG_UPLOAD_OUTCOME" \
    "$INPUT_COMPARISON_OUTCOME" "$INPUT_COMPARISON_MATCHES" "$GROUP_COMPARISON_MATCHES" \
    "$PLAN_CAPTURE_OUTCOME" \
    >> "$GITHUB_STEP_SUMMARY"
  exit 1
fi

if [[ "$INPUT_COMPARISON_OUTCOME" != "success" || "$INPUT_COMPARISON_MATCHES" != "true" ]]; then
  printf 'The group query plan capture completed independently, but full input equivalence remains failed. No full replay was performed.\n' \
    >> "$GITHUB_STEP_SUMMARY"
  exit 1
fi
