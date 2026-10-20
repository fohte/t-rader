import { describe, expect, it } from 'vitest'

import { getTreemapLabelMode } from '#components/widgets/widget-treemap-label'

describe('treemap label mode', () => {
  it('セル幅と高さに合わせて銘柄名・値・非表示を切り替える', () => {
    function readLabelModes() {
      return [
        getTreemapLabelMode(
          { x: 0, y: 0, width: 180, height: 44 },
          '架空銘柄 001',
          '+0.10%',
        ),
        getTreemapLabelMode(
          { x: 0, y: 0, width: 55, height: 44 },
          '架空銘柄',
          '+1%',
        ),
        getTreemapLabelMode(
          { x: 0, y: 0, width: 28, height: 20 },
          '架空銘柄',
          '+1%',
        ),
      ]
    }

    expect(readLabelModes()).toEqual(['both', 'value', null])
  })
})
