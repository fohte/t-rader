import type { cdp } from 'vitest/browser'
import type { BrowserCommand } from 'vitest/node'

declare module 'vitest/browser' {
  interface CDPSession {
    send(method: string, params?: Record<string, unknown>): Promise<unknown>
  }

  interface BrowserCommands {
    tapElementOnTouch(selector: string): Promise<void>
  }
}

const MOBILE_VIEWPORT = { width: 390, height: 844 }
const EXPECTED_MOBILE_VIEWPORT = {
  width: MOBILE_VIEWPORT.width,
  height: MOBILE_VIEWPORT.height,
  hasNoHover: true,
  hasCoarsePointer: true,
  maxTouchPoints: 1,
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

const tapOnTouch: BrowserCommand<[selector: string]> = async (
  context,
  selector,
) => {
  if (context.provider.name !== 'playwright') {
    return Promise.reject(
      new Error('Touch interaction tests require Playwright'),
    )
  }

  const target = (await context.frame()).locator(selector)
  const position = await target.evaluate((element) => {
    const bounds = element.getBoundingClientRect()
    const firstRect = element.getClientRects().item(0)
    if (firstRect == null) return null

    return {
      x: firstRect.left - bounds.left + firstRect.width / 2,
      y: firstRect.top - bounds.top + firstRect.height / 2,
    }
  })
  if (position == null) {
    return Promise.reject(new Error('The touch target has no visible bounds'))
  }

  await target.tap({ position })
}

async function withMobileViewport<T>(
  session: ReturnType<typeof cdp>,
  operation: () => Promise<T>,
): Promise<T> {
  const [{ expect }, { page }] = await Promise.all([
    import('vitest'),
    import('vitest/browser'),
  ])
  const originalViewport = {
    width: window.innerWidth,
    height: window.innerHeight,
  }

  return session
    .send('Emulation.setDeviceMetricsOverride', {
      ...MOBILE_VIEWPORT,
      deviceScaleFactor: 1,
      mobile: true,
    })
    .then(() =>
      session.send('Emulation.setTouchEmulationEnabled', {
        enabled: true,
        maxTouchPoints: 1,
      }),
    )
    .then(() => page.viewport(MOBILE_VIEWPORT.width, MOBILE_VIEWPORT.height))
    .then(() => {
      expect(readMobileViewportState()).toEqual(EXPECTED_MOBILE_VIEWPORT)
      return operation()
    })
    .finally(() =>
      Promise.all([
        session.send('Emulation.setTouchEmulationEnabled', { enabled: false }),
        session.send('Emulation.clearDeviceMetricsOverride'),
      ]).finally(() =>
        page.viewport(originalViewport.width, originalViewport.height),
      ),
    )
}

async function tapTouchTarget(selector: string) {
  const { commands } = await import('vitest/browser')
  await commands.tapElementOnTouch(selector)
}

async function tapLinkOnTouch(name: string) {
  const { page } = await import('vitest/browser')
  const link = page.getByRole('link', { name })
  await tapTouchTarget(link.selector)
}

export const touchBrowserTestHelpers = {
  tapElementOnTouch: tapOnTouch,
  EXPECTED_MOBILE_VIEWPORT,
  readMobileViewportState,
  withMobileViewport,
  tapTouchTarget,
  tapLinkOnTouch,
}
