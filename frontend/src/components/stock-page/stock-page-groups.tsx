import { Chip } from '@fohte/ui/chip'
import { Link } from '@tanstack/react-router'

import type { components } from '#lib/api/schema.gen'
import type { StockPageListState } from '#lib/stock-page-model'
import { cn } from '#lib/utils'

type GroupMembership = components['schemas']['StockGroupMembershipResponse']

export function StockPageGroups({
  memberships,
  state,
  className,
}: {
  memberships: GroupMembership[]
  state: StockPageListState
  className?: string
}) {
  return (
    <div className={cn('flex flex-wrap items-center gap-1.5', className)}>
      {state === 'loading' ? (
        <p className="font-mono text-2xs text-muted-foreground">
          所属グループを読み込み中
        </p>
      ) : state === 'error' ? (
        <p className="font-mono text-2xs text-destructive">
          所属グループの取得に失敗しました
        </p>
      ) : memberships.length === 0 ? (
        <p className="font-mono text-2xs text-muted-foreground">
          所属グループなし
        </p>
      ) : (
        memberships.map(({ axis, group }) => (
          <Link
            key={`${axis.key}/${group.key}`}
            to="/groups/$axisKey/$groupKey"
            params={{ axisKey: axis.key, groupKey: group.key }}
            className="inline-flex"
          >
            <Chip size="md">
              {axis.name} / {group.name}
            </Chip>
          </Link>
        ))
      )}
    </div>
  )
}
