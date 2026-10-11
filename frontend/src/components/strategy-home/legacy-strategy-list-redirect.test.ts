import { createMemoryHistory, createRouter } from '@tanstack/react-router'
import { describe, expect, it } from 'vitest'

import { routeTree } from '#routeTree.gen'

async function getResolvedLocation(initialPath: string) {
  const router = createRouter({
    routeTree,
    history: createMemoryHistory({ initialEntries: [initialPath] }),
  })
  await router.load()
  return router.state.location.href
}

describe('legacy strategy list routes', () => {
  it.each([
    {
      legacyPath: '/runs',
      expected: '/strategies?tab=runs',
    },
    {
      legacyPath: '/indicators',
      expected: '/strategies?tab=indicators',
    },
  ] as const)(
    '$legacyPath を対応する戦略タブへ redirect する',
    async ({ legacyPath, expected }) => {
      expect(await getResolvedLocation(legacyPath)).toEqual(expected)
    },
  )
})
