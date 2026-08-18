import { describe, expect, it } from 'vite-plus/test'

describe('frontend test harness', () => {
  it('runs TypeScript tests through Vite+', () => {
    expect('Muse Bar').toContain('Muse')
  })
})
