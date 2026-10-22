import '#index.css'

import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { cdp, page } from 'vitest/browser'

import { MarkdownBody } from '#components/note-detail/markdown-body'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

const NOTE_ID = '00000000-0000-4000-8000-000000000101'
const VERSION_ID = '00000000-0000-4000-8000-000000000201'
const CURRENT_NOTE_ID = '00000000-0000-4000-8000-000000000102'
const NOTE_LINKS: components['schemas']['NoteLinkItem'][] = [
  {
    note_id: NOTE_ID,
    version_id: VERSION_ID,
    version_no: 2,
    title: '関連する記録',
  },
]
const CURRENT_NOTE_LINKS: components['schemas']['NoteLinkItem'][] = [
  {
    note_id: CURRENT_NOTE_ID,
    version_id: null,
    version_no: 3,
    title: '現行バージョンの記録',
  },
]

interface CdpTargetInfo {
  targetInfo: { targetId: string; type: string; url: string }
}

declare module 'vitest/browser' {
  interface CDPSession {
    send(
      method: string,
      params?: Record<string, boolean | string>,
    ): Promise<unknown>
    on(event: string, listener: (target: CdpTargetInfo) => void): void
    off(event: string, listener: (target: CdpTargetInfo) => void): void
  }
}

function renderNote(source: string, noteLinks = NOTE_LINKS) {
  const router = createStoryRouter(
    () => <MarkdownBody source={source} noteLinks={noteLinks} />,
    { paths: ['/notes/$noteId'], initialPath: '/notes/source' },
  )
  render(<RouterProvider router={router} />)
  return router
}

function readLocation(router: ReturnType<typeof createStoryRouter>) {
  return {
    pathname: router.state.location.pathname,
    search: router.state.location.search,
  }
}

afterEach(cleanup)

describe('MarkdownBody links', () => {
  it('keeps bare URLs and both link types visible without hover', async () => {
    const source = [
      'https://example.invalid/source',
      '',
      '[外部リンク](https://example.invalid/article)',
      '',
      `[[note:${NOTE_ID}]]`,
    ].join('\n')
    const router = createStoryRouter(
      () => (
        <div>
          <span data-testid="primary-color" className="text-primary">
            primary
          </span>
          <span data-testid="body-color" className="text-foreground">
            body
          </span>
          <MarkdownBody source={source} noteLinks={NOTE_LINKS} />
        </div>
      ),
      { paths: ['/notes/$noteId'], initialPath: '/notes/source' },
    )
    render(<RouterProvider router={router} />)

    const primaryColor = getComputedStyle(
      await screen.findByTestId('primary-color'),
    ).color
    const bodyColor = getComputedStyle(
      await screen.findByTestId('body-color'),
    ).color
    const links = screen.getAllByRole('link')

    expect(
      links.map((link) => {
        const style = getComputedStyle(link)
        return {
          label: link.textContent,
          usesPrimaryColor: style.color === primaryColor,
          differsFromBody: style.color !== bodyColor,
          isUnderlined: style.textDecorationLine === 'underline',
          underlineOffset: style.textUnderlineOffset,
        }
      }),
    ).toEqual([
      {
        label: 'https://example.invalid/source',
        usesPrimaryColor: true,
        differsFromBody: true,
        isUnderlined: true,
        underlineOffset: '4px',
      },
      {
        label: '外部リンク',
        usesPrimaryColor: true,
        differsFromBody: true,
        isUnderlined: true,
        underlineOffset: '4px',
      },
      {
        label: '関連する記録 (v2)',
        usesPrimaryColor: true,
        differsFromBody: true,
        isUnderlined: true,
        underlineOffset: '4px',
      },
    ])
  })

  const externalLinks = [
    {
      name: 'a bare GFM URL',
      source: (url: string) => url,
      linkName: (url: string) => url,
      fixturePath: '/__link-fixture__/bare-url?source=markdown',
    },
    {
      name: 'an external Markdown link',
      source: (url: string) => `[ローカルの出典](${url})`,
      linkName: () => 'ローカルの出典',
      fixturePath: '/__link-fixture__/markdown-link?source=markdown',
    },
  ]

  it.each(externalLinks)(
    '$name opens its local target in a new tab',
    async ({ source, linkName, fixturePath }) => {
      const targetUrl = new URL(fixturePath, window.location.origin).href
      renderNote(source(targetUrl))

      const session = cdp()
      await session.send('Target.setDiscoverTargets', { discover: true })

      const openedTarget = new Promise<CdpTargetInfo['targetInfo']>(
        (resolve) => {
          const onTarget = ({ targetInfo }: CdpTargetInfo) => {
            if (targetInfo.type !== 'page' || targetInfo.url !== targetUrl)
              return
            session.off('Target.targetCreated', onTarget)
            session.off('Target.targetInfoChanged', onTarget)
            resolve(targetInfo)
          }
          session.on('Target.targetCreated', onTarget)
          session.on('Target.targetInfoChanged', onTarget)
        },
      )

      await page.getByRole('link', { name: linkName(targetUrl) }).click()
      const target = await openedTarget

      expect(target.url).toEqual(targetUrl)
      await session.send('Target.closeTarget', { targetId: target.targetId })
      await session.send('Target.setDiscoverTargets', { discover: false })
    },
  )

  const internalLinks = [
    {
      name: 'a selected note version',
      source: `[[note:${NOTE_ID}]]`,
      noteLinks: NOTE_LINKS,
      linkName: '関連する記録 (v2)',
      expectedLocation: {
        pathname: `/notes/${NOTE_ID}`,
        search: { version_id: VERSION_ID },
      },
    },
    {
      name: 'the current note version',
      source: `[[note:${CURRENT_NOTE_ID}@current]]`,
      noteLinks: CURRENT_NOTE_LINKS,
      linkName: '現行バージョンの記録 (現行バージョンに追従)',
      expectedLocation: {
        pathname: `/notes/${CURRENT_NOTE_ID}`,
        search: {},
      },
    },
  ]

  it.each(internalLinks)(
    'navigates to $name when clicked',
    async ({ source, noteLinks, linkName, expectedLocation }) => {
      const router = renderNote(source, noteLinks)

      await page.getByRole('link', { name: linkName }).click()
      await waitFor(() => {
        expect(readLocation(router)).toEqual(expectedLocation)
      })
    },
  )
})
