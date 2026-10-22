import { Link } from '@tanstack/react-router'

import type { ExecutionOrigin } from '#lib/execution-origin'

export function ExecutionOriginLinks({ origin }: { origin: ExecutionOrigin }) {
  return (
    <span>
      <Link
        to="/strategies/$id"
        params={{ id: origin.strategyId }}
        className="hover:underline"
      >
        {origin.strategyName}
      </Link>
      <span> · </span>
      <Link
        to="/strategies/$id/runs/$taskId"
        params={{ id: origin.strategyId, taskId: origin.taskId }}
        className="text-muted-foreground-strong hover:text-foreground hover:underline"
      >
        実行詳細 →
      </Link>
    </span>
  )
}
