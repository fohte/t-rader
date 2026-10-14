import { cleanup, render } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { StockPageCompanyInfo } from '#components/stock-page/stock-page-company-info'
import type { components } from '#lib/api/schema.gen'

type Valuation = components['schemas']['StockValuationResponse']

afterEach(cleanup)

describe('StockPageCompanyInfo', () => {
  it('予想 PER がない場合は現在の値を表示し ROE の予想値を区別する', () => {
    const valuation: Valuation = {
      code: 'demo-code',
      date: '2030-04-09',
      per: 16.2,
      fwd_per: null,
      pbr: 1.3,
      roe: 9.1,
      fwd_roe: 11,
      eps: 200,
      fwd_eps: 230,
      bps: 2500,
      mkt_cap: 123_000_000_000,
    }
    const { container } = render(
      <StockPageCompanyInfo
        position={null}
        holdingsState="ready"
        earnings={null}
        earningsState="ready"
        valuation={valuation}
        valuationState="ready"
      />,
    )

    const metrics = Array.from(container.querySelectorAll('dt')).map(
      (label) => [
        label.textContent,
        Array.from(label.nextElementSibling?.childNodes ?? []).map(
          (node) => node.textContent ?? '',
        ),
      ],
    )

    expect(metrics).toEqual([
      ['PER', ['16.2']],
      ['PBR', ['1.3']],
      ['ROE (予)', ['11%']],
      ['時価総額', ['1,230 億円']],
      ['保有', ['保有なし', '現在']],
      ['次の決算', ['予定なし']],
      ['PER', ['16.2', '2030-04-09 時点']],
      ['PBR', ['1.3', '2030-04-09 時点']],
    ])
  })
})
