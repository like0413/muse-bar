import { describe, expect, it } from 'vite-plus/test'

import { routes } from '../router/routes'

describe('application routes', () => {
  it('exposes separate bar and settings entry points', () => {
    expect(routes.map(({ path }) => path)).toEqual(['/', '/bar', '/settings'])
  })
})
