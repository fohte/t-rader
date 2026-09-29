#!/usr/bin/env bash
set -euo pipefail

provider_directory="${1:?provider binary directory is required}"
workspace="${RUNNER_TEMP:?RUNNER_TEMP is required}/terraform-provider-dev-overrides"
cli_config="${RUNNER_TEMP}/terraform-provider-dev-overrides.tfrc"
backend_url="${BACKEND_URL:?BACKEND_URL is required}"

mkdir -p "$workspace"
cat > "$cli_config" << EOF
provider_installation {
  dev_overrides {
    "fohte.net/fohte/trader" = "$provider_directory"
  }
  direct {}
}
EOF

cat > "$workspace/main.tf" << EOF
terraform {
  required_providers {
    trader = {
      source = "fohte.net/fohte/trader"
    }
  }
}

provider "trader" {
  base_url = "$backend_url"
}
EOF

export TF_CLI_CONFIG_FILE="$cli_config"
export TF_IN_AUTOMATION=true

write_strategy_config() {
  local name="$1" description="$2" sort_order="$3"
  cat > "$workspace/strategy.tf" << EOF
resource "trader_strategy" "integration" {
  name        = "$name"
  description = "$description"
  sort_order  = $sort_order
}

output "strategy_id" {
  value = trader_strategy.integration.id
}
EOF
}

write_strategy_config "synthetic strategy" "synthetic description" 3
terraform -chdir="$workspace" apply -auto-approve -input=false -no-color
strategy_id="$(terraform -chdir="$workspace" output -raw strategy_id)"

write_strategy_config "updated synthetic strategy" "updated synthetic description" 5
terraform -chdir="$workspace" apply -auto-approve -input=false -no-color
terraform -chdir="$workspace" state rm trader_strategy.integration
terraform -chdir="$workspace" import -input=false -no-color trader_strategy.integration "$strategy_id"
terraform -chdir="$workspace" plan -input=false -no-color -detailed-exitcode

rm "$workspace/strategy.tf"
terraform -chdir="$workspace" apply -auto-approve -input=false -no-color

remaining_resources="$(terraform -chdir="$workspace" state list)"
if [[ -n "$remaining_resources" ]]; then
  printf 'Expected empty Terraform state after deleting the strategy, got:\n%s\n' "$remaining_resources" >&2
  exit 1
fi
