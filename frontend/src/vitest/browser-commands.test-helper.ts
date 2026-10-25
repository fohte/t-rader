import type { BrowserCommand } from 'vitest/node'

declare module 'vitest/browser' {
  interface BrowserCommands {
    tapElementOnTouch(selector: string): Promise<void>
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

export const tapElementOnTouch = tapOnTouch
