# Tavily account credit budget

Tavily `search` and `extract` calls use a shared account ledger with fixed limits of 25 credits per UTC day and 900 credits per provider month. The account alias comes from `TAVILY_CREDIT_ACCOUNT_ID` and stays unchanged when the API key rotates.

Set `TAVILY_PAYGO_DISABLED=true` only after confirming in the Tavily account that PAYGO is disabled. The usage API reports current plan and usage but does not report whether PAYGO is enabled, so this setting is an operator confirmation. The service stops when the plan is not `Researcher` or when any PAYGO usage is reported.

Before first use, stop the previous sender and identify every consumer that shares the account. Other consumers must be stopped or use the same database ledger; concurrent uncoordinated consumers can spend credits between usage checks. Confirm that the configured API key and every rotated key belong to the same account alias. Then run `cd backend && cargo run -p backend -- --run-job tavily_credit_budget_bootstrap` once with the database and Tavily account configuration. Bootstrap records current monthly usage. Because prior daily usage is unavailable, search and extraction remain blocked until the next UTC midnight.

The bootstrap command does not change provider billing settings. Account ownership, PAYGO status, and other consumers must be verified outside the service. If that account-wide coordination cannot be confirmed, keep Tavily configuration absent so calls fail closed.
