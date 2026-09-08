import { afterEach, describe, expect, it, vi } from 'vitest'

afterEach(() => {
  vi.resetModules()
  vi.unstubAllGlobals()
})

describe('model service worker', () => {
  it('sends only unique content-hashed GLB URLs from the asset manifest', async () => {
    vi.stubGlobal('window', {
      __ASSET_MANIFEST__: {
        '/models/characters/knight.glb':
          '/models/characters/knight.92dadd6c.glb',
        '/models/duplicate.glb': '/models/characters/knight.92dadd6c.glb',
        '/models/objects/catalog.json': '/models/objects/catalog.12345678.json',
        '/textures/stone.png': '/textures/stone.12345678.png',
        '/models/unhashed.glb': '/models/unhashed.glb',
      },
    })

    const postMessage = vi.fn()
    const registration = {
      active: { postMessage },
      waiting: null,
      installing: null,
    }
    const serviceWorker = {
      register: vi.fn(async () => registration),
      addEventListener: vi.fn(),
      ready: Promise.resolve(registration),
      controller: registration.active,
    }
    vi.stubGlobal('navigator', { serviceWorker })

    const { registerModelServiceWorker } = await import('./modelServiceWorker')
    await registerModelServiceWorker()
    await serviceWorker.ready

    expect(serviceWorker.register).toHaveBeenCalledWith(
      '/model-service-worker.js',
      { scope: '/', updateViaCache: 'none' }
    )
    expect(postMessage).toHaveBeenCalledTimes(1)
    expect(postMessage).toHaveBeenCalledWith({
      type: 'openmmo:model-manifest',
      urls: ['/models/characters/knight.92dadd6c.glb'],
    })
  })

  it('does nothing when service workers are unavailable', async () => {
    vi.stubGlobal('window', {})
    vi.stubGlobal('navigator', {})

    const { registerModelServiceWorker } = await import('./modelServiceWorker')
    await expect(registerModelServiceWorker()).resolves.toBeUndefined()
  })
})
