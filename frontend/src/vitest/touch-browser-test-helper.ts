import { expect } from 'vitest'
import { cdp, commands, page } from 'vitest/browser'

const MOBILE_VIEWPORT = { width: 390, height: 844 }
export const EXPECTED_MOBILE_VIEWPORT = {
  width: MOBILE_VIEWPORT.width,
  height: MOBILE_VIEWPORT.height,
  hasNoHover: true,
  hasCoarsePointer: true,
  maxTouchPoints: 1,
}

declare module 'vitest/browser' {
  interface CDPSession {
    send(method: string, params?: Record<string, unknown>): Promise<unknown>
  }

  interface BrowserCommands {
    tapElementOnTouch(selector: string): Promise<void>
  }
}

export function readMobileViewportState() {
  return {
    width: window.innerWidth,
    height: window.innerHeight,
    hasNoHover: window.matchMedia('(hover: none)').matches,
    hasCoarsePointer: window.matchMedia('(pointer: coarse)').matches,
    maxTouchPoints: navigator.maxTouchPoints,
  }
}

export function withMobileViewport<T>(
  session: ReturnType<typeof cdp>,
  operation: () => Promise<T>,
): Promise<T> {
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

export async function tapElementOnTouch(selector: string) {
  await commands.tapElementOnTouch(selector)
}

export async function tapLinkOnTouch(name: string) {
  const link = page.getByRole('link', { name })
  await tapElementOnTouch(link.selector)
}
