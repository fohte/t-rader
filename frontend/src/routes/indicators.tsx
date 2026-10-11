import { createFileRoute, redirect } from '@tanstack/react-router'

import { getLegacyStrategyListTabRedirectOptions } from '#components/strategy-home/strategy-list-tabs'

export const Route = createFileRoute('/indicators')({
  beforeLoad: () => {
    // eslint-disable-next-line @typescript-eslint/only-throw-error, no-restricted-syntax -- TanStack Router の redirect API は throw を要求する
    throw redirect(getLegacyStrategyListTabRedirectOptions('indicators'))
  },
})
