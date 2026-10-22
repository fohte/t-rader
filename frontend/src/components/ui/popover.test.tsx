import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { Popover, PopoverContent, PopoverTrigger } from '#components/ui/popover'

afterEach(cleanup)

describe('PopoverContent', () => {
  it('centers the popover by default', () => {
    render(
      <Popover defaultOpen>
        <PopoverTrigger>開く</PopoverTrigger>
        <PopoverContent>サンプルの内容</PopoverContent>
      </Popover>,
    )

    const content = screen
      .getByText('サンプルの内容')
      .closest('[data-slot="popover-content"]')

    expect(
      `${content?.getAttribute('data-align') ?? ''}|${content?.getAttribute('data-side') ?? ''}`,
    ).toEqual('center|bottom')
  })

  it('forwards explicit position props', () => {
    render(
      <Popover defaultOpen>
        <PopoverTrigger>開く</PopoverTrigger>
        <PopoverContent side="top" sideOffset={8} align="end" alignOffset={4}>
          サンプルの内容
        </PopoverContent>
      </Popover>,
    )

    const content = screen
      .getByText('サンプルの内容')
      .closest('[data-slot="popover-content"]')

    expect(
      `${content?.getAttribute('data-align') ?? ''}|${content?.getAttribute('data-side') ?? ''}`,
    ).toEqual('end|top')
  })
})
