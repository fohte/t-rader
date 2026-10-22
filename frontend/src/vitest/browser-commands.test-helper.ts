import type { BrowserCommand } from 'vitest/node'

export const tapLinkOnTouch: BrowserCommand<[selector: string]> = async (
  context,
  selector,
) => {
  if (context.provider.name !== 'playwright') {
    return Promise.reject(new Error('Touch link tests require Playwright'))
  }

  const link = (await context.frame()).locator(selector)
  const position = await link.evaluate((element) => {
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

  await link.tap({ position })
}
