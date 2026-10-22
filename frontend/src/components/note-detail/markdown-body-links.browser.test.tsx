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
const MOBILE_VIEWPORT = { width: 390, height: 844 }
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
      method: 'DOM.getNodeForLocation',
      params: {
        x: number
        y: number
        includeUserAgentShadowDOM: boolean
      },
    ): Promise<{ backendNodeId: number }>
    send(
      method: 'DOM.describeNode',
      params: { backendNodeId: number },
    ): Promise<{ node: { nodeName: string; attributes: string[] } }>
    send(method: string, params?: Record<string, unknown>): Promise<unknown>
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

type CdpSession = ReturnType<typeof cdp>

async function withMobileViewport<T>(
  session: CdpSession,
  operation: () => Promise<T>,
): Promise<T> {
  const originalViewport = {
    width: window.innerWidth,
    height: window.innerHeight,
  }
  await session.send('Emulation.setDeviceMetricsOverride', {
    ...MOBILE_VIEWPORT,
    deviceScaleFactor: 1,
    mobile: true,
  })
  await session.send('Emulation.setTouchEmulationEnabled', {
    enabled: true,
    maxTouchPoints: 1,
  })
  await page.viewport(MOBILE_VIEWPORT.width, MOBILE_VIEWPORT.height)

  try {
    expect(readMobileViewportState()).toEqual({
      width: MOBILE_VIEWPORT.width,
      height: MOBILE_VIEWPORT.height,
      hasNoHover: true,
      hasCoarsePointer: true,
      maxTouchPoints: 1,
    })

    return await operation()
  } finally {
    try {
      await Promise.all([
        session.send('Emulation.setTouchEmulationEnabled', { enabled: false }),
        session.send('Emulation.clearDeviceMetricsOverride'),
      ])
    } finally {
      await page.viewport(originalViewport.width, originalViewport.height)
    }
  }
}

function readFrameMetrics(frame: Element | null) {
  if (
    frame == null ||
    !('offsetWidth' in frame) ||
    !('offsetHeight' in frame) ||
    !('clientLeft' in frame) ||
    !('clientTop' in frame)
  ) {
    return undefined
  }

  const { offsetWidth, offsetHeight, clientLeft, clientTop } = frame
  if (
    typeof offsetWidth !== 'number' ||
    typeof offsetHeight !== 'number' ||
    typeof clientLeft !== 'number' ||
    typeof clientTop !== 'number'
  ) {
    return undefined
  }

  return { offsetWidth, offsetHeight, clientLeft, clientTop }
}

function getMainFramePoint(element: HTMLElement) {
  const frame = window.frameElement
  const frameBounds = frame?.getBoundingClientRect()
  const frameMetrics = readFrameMetrics(frame)
  const scaleX =
    frameBounds && frameMetrics && frameMetrics.offsetWidth > 0
      ? frameBounds.width / frameMetrics.offsetWidth
      : 1
  const scaleY =
    frameBounds && frameMetrics && frameMetrics.offsetHeight > 0
      ? frameBounds.height / frameMetrics.offsetHeight
      : 1
  const bounds = element.getClientRects()[0] ?? element.getBoundingClientRect()
  return {
    x:
      (frameBounds?.left ?? 0) +
      (frameMetrics?.clientLeft ?? 0) * scaleX +
      (bounds.left + bounds.width / 2) * scaleX,
    y:
      (frameBounds?.top ?? 0) +
      (frameMetrics?.clientTop ?? 0) * scaleY +
      (bounds.top + bounds.height / 2) * scaleY,
  }
}

function serializeLinkTarget(
  nodeName: string,
  href: string | null | undefined,
) {
  return JSON.stringify({ nodeName, href })
}

function readMobileViewportState() {
  return {
    width: window.innerWidth,
    height: window.innerHeight,
    hasNoHover: window.matchMedia('(hover: none)').matches,
    hasCoarsePointer: window.matchMedia('(pointer: coarse)').matches,
    maxTouchPoints: navigator.maxTouchPoints,
  }
}

async function tapLink(session: CdpSession, name: string) {
  const link = await screen.findByRole('link', { name })
  const { x, y } = getMainFramePoint(link)
  const { backendNodeId } = await session.send('DOM.getNodeForLocation', {
    x: Math.round(x),
    y: Math.round(y),
    includeUserAgentShadowDOM: true,
  })
  const { node } = await session.send('DOM.describeNode', {
    backendNodeId,
  })
  const hrefIndex = node.attributes.indexOf('href')
  expect(
    serializeLinkTarget(node.nodeName, node.attributes[hrefIndex + 1]),
  ).toEqual(serializeLinkTarget('A', link.getAttribute('href')))
  await session.send('Input.synthesizeTapGesture', {
    x,
    y,
    gestureSourceType: 'touch',
  })
}

async function expectNewTab(
  session: CdpSession,
  targetUrl: string,
  activateLink: () => Promise<void>,
) {
  await session.send('Target.setDiscoverTargets', { discover: true })

  let targetId: string | undefined
  let resolveOpenedTarget:
    ((targetInfo: CdpTargetInfo['targetInfo']) => void) | undefined
  const openedTarget = new Promise<CdpTargetInfo['targetInfo']>((resolve) => {
    resolveOpenedTarget = resolve
  })
  const onTarget = ({ targetInfo }: CdpTargetInfo) => {
    if (targetInfo.type !== 'page' || targetInfo.url !== targetUrl) return
    targetId = targetInfo.targetId
    resolveOpenedTarget?.(targetInfo)
  }
  session.on('Target.targetCreated', onTarget)
  session.on('Target.targetInfoChanged', onTarget)

  try {
    await activateLink()
    const target = await openedTarget
    targetId = target.targetId

    expect(target.url).toEqual(targetUrl)
  } finally {
    session.off('Target.targetCreated', onTarget)
    session.off('Target.targetInfoChanged', onTarget)
    const cleanupCommands = [
      session.send('Target.setDiscoverTargets', { discover: false }),
    ]
    if (targetId != null) {
      cleanupCommands.push(session.send('Target.closeTarget', { targetId }))
    }
    await Promise.all(cleanupCommands)
  }
}

const VISIBILITY_SOURCE = [
  'https://example.invalid/source',
  '',
  '[外部リンク](https://example.invalid/article)',
  '',
  `[[note:${NOTE_ID}]]`,
  `[[note:${CURRENT_NOTE_ID}@current]]`,
].join('\n')
const VISIBILITY_NOTE_LINKS = [...NOTE_LINKS, ...CURRENT_NOTE_LINKS]
const EXPECTED_LINK_APPEARANCE = [
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
  {
    label: '現行バージョンの記録 (現行バージョンに追従)',
    usesPrimaryColor: true,
    differsFromBody: true,
    isUnderlined: true,
    underlineOffset: '4px',
  },
]

function renderLinkVisibilityExample() {
  const router = createStoryRouter(
    () => (
      <div>
        <span data-testid="primary-color" className="text-primary">
          primary
        </span>
        <span data-testid="body-color" className="text-foreground">
          body
        </span>
        <MarkdownBody
          source={VISIBILITY_SOURCE}
          noteLinks={VISIBILITY_NOTE_LINKS}
        />
      </div>
    ),
    { paths: ['/notes/$noteId'], initialPath: '/notes/source' },
  )
  render(<RouterProvider router={router} />)
}

async function readLinkAppearance() {
  const primaryColor = getComputedStyle(
    await screen.findByTestId('primary-color'),
  ).color
  const bodyColor = getComputedStyle(
    await screen.findByTestId('body-color'),
  ).color

  return screen.getAllByRole('link').map((link) => {
    const style = getComputedStyle(link)
    return {
      label: link.textContent,
      usesPrimaryColor: style.color === primaryColor,
      differsFromBody: style.color !== bodyColor,
      isUnderlined: style.textDecorationLine === 'underline',
      underlineOffset: style.textUnderlineOffset,
    }
  })
}

afterEach(cleanup)

describe('MarkdownBody links', () => {
  it('keeps every link visible in a desktop viewport', async () => {
    renderLinkVisibilityExample()

    expect(await readLinkAppearance()).toEqual(EXPECTED_LINK_APPEARANCE)
  })

  it('keeps every link visible without hover in a touch viewport', async () => {
    const session = cdp()

    await withMobileViewport(session, async () => {
      renderLinkVisibilityExample()

      expect(await readLinkAppearance()).toEqual(EXPECTED_LINK_APPEARANCE)
    })
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
      await expectNewTab(session, targetUrl, () =>
        page.getByRole('link', { name: linkName(targetUrl) }).click(),
      )
    },
  )

  it.each(externalLinks)(
    '$name opens its local target in a new tab when tapped on mobile',
    async ({ source, linkName, fixturePath }) => {
      const targetUrl = new URL(fixturePath, window.location.origin).href
      const session = cdp()

      await withMobileViewport(session, async () => {
        renderNote(source(targetUrl))
        await expectNewTab(session, targetUrl, () =>
          tapLink(session, linkName(targetUrl)),
        )
      })
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

  it.each(internalLinks)(
    'navigates to $name when tapped on mobile',
    async ({ source, noteLinks, linkName, expectedLocation }) => {
      const session = cdp()

      await withMobileViewport(session, async () => {
        const router = renderNote(source, noteLinks)

        await tapLink(session, linkName)
        await waitFor(() => {
          expect(readLocation(router)).toEqual(expectedLocation)
        })
      })
    },
  )
})
