#!/usr/bin/env bash
set -euo pipefail

readonly SOURCE_CHECKOUT_SHA='d24a3e71dac02684f1d752c4d37aeb6462ca5c91'
readonly EXPECTED_BRANCH='fohte/query-perf-plan-capture-39714934810'
readonly REPORT_FILE="${1:-query-perf-plan-capture/environment/source-change-check.txt}"

mkdir -p "$(dirname "$REPORT_FILE")"
checkout_sha=$(git rev-parse HEAD)
branch=$(git branch --show-current)
if [[ "$branch" != "$EXPECTED_BRANCH" ]]; then
  printf 'branch_mismatch expected=%s actual=%s\n' "$EXPECTED_BRANCH" "$branch" > "$REPORT_FILE"
  exit 1
fi
if ! git merge-base --is-ancestor "$SOURCE_CHECKOUT_SHA" "$checkout_sha"; then
  printf 'source_is_not_an_ancestor source=%s checkout=%s\n' "$SOURCE_CHECKOUT_SHA" "$checkout_sha" > "$REPORT_FILE"
  exit 1
fi

changed_files=$(git diff --name-only "$SOURCE_CHECKOUT_SHA" "$checkout_sha")
if [[ -z "$changed_files" ]]; then
  printf 'no_diagnostic_changes_found\n' > "$REPORT_FILE"
  exit 1
fi

while IFS= read -r path; do
  case "$path" in
    .github/workflows/query-perf-plan-capture-39714934810.yml | \
      backend/scripts/capture-group-query-plan-observation.sh | \
      backend/scripts/capture-group-query-plans.sh | \
      backend/scripts/capture-query-perf-plan.sh | \
      backend/scripts/query-perf-plan-capture-tune-postgres.sh | \
      backend/scripts/query-perf-plan-capture-copy-query-log.sh | \
      backend/scripts/query-perf-plan-capture-compare-input.sh | \
      backend/scripts/query-perf-plan-capture-record-prerequisites.sh | \
      backend/scripts/query-perf-plan-capture-check-results.sh | \
      backend/scripts/prepare-group-query-plan-input.sh | \
      backend/scripts/verify-query-perf-plan-capture-source.sh | \
      backend/scripts/query-perf-input-manifest/Cargo.toml | \
      backend/scripts/query-perf-input-manifest/Cargo.lock | \
      backend/scripts/query-perf-input-manifest/src/cli.rs | \
      backend/scripts/query-perf-input-manifest/src/comparison.rs | \
      backend/scripts/query-perf-input-manifest/src/comparison/tests.rs | \
      backend/scripts/query-perf-input-manifest/src/constants.rs | \
      backend/scripts/query-perf-input-manifest/src/manifest.rs | \
      backend/scripts/query-perf-input-manifest/src/model.rs | \
      backend/scripts/query-perf-input-manifest/src/main.rs | \
      backend/scripts/query-perf-input-manifest/src/replay_exclusions.rs | \
      backend/scripts/query-perf-input-manifest/src/runner.rs | \
      backend/scripts/query-perf-input-manifest/src/util.rs | \
      backend/scripts/query-perf-input-manifest/reference-manifest.json)
      ;;
    *)
      printf 'unexpected_changed_path=%s\n' "$path" > "$REPORT_FILE"
      exit 1
      ;;
  esac
done <<< "$changed_files"

if ! git diff --quiet "$SOURCE_CHECKOUT_SHA" "$checkout_sha" -- \
  .github/workflows/test.yml \
  backend/crates \
  backend/migration \
  backend/perf-db \
  backend/scripts/build-perf-db-image.sh; then
  printf 'protected_source_difference_detected\n' > "$REPORT_FILE"
  exit 1
fi

{
  printf 'source_checkout_sha=%s\n' "$SOURCE_CHECKOUT_SHA"
  printf 'checkout_sha=%s\n' "$checkout_sha"
  printf 'branch=%s\n' "$branch"
  printf 'source_is_ancestor=true\n'
  printf 'product_and_test_sources_unchanged=true\n'
  printf 'changed_diagnostic_files:\n%s\n' "$changed_files"
} > "$REPORT_FILE"
