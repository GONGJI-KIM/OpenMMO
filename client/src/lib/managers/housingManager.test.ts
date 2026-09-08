import { describe, it, expect, vi, beforeEach } from 'vitest'
import { WORLD_MIN_X, WORLD_WIDTH_X } from '../terrain/world-wrap'
import {
  TERRAIN_TILE_SIZE,
  getTerrainChunkFromPosition,
} from '../components/game-scene/terrain-utils'

const removed: string[] = []

vi.mock('../wasm/onlinerpg_shared', () => ({
  passability_add_house: () => {},
  passability_remove_house: (id: string) => removed.push(id),
  passability_update_door: () => {},
  passability_is_movement_blocked: () => false,
  passability_is_circle_blocked: () => false,
}))

const { housingManager } = await import('./housingManager')

/** A house whose origin sits in the middle of chunk (cx, cz). */
function house(id: string, cx: number, cz: number) {
  return {
    id,
    origin: {
      x: cx * TERRAIN_TILE_SIZE + TERRAIN_TILE_SIZE / 2,
      y: 0,
      z: cz * TERRAIN_TILE_SIZE + TERRAIN_TILE_SIZE / 2,
    },
    rooms: [],
    // Non-empty so addToCache skips buildPassability.
    passability: [{ floorLevel: 0, cells: [] }],
  } as never
}

function load(id: string, cx: number, cz: number) {
  housingManager.handleRemoteHousesBatch([house(id, cx, cz)])
}

/** World position at the centre of chunk (cx, cz). */
function at(cx: number, cz: number) {
  return {
    x: cx * TERRAIN_TILE_SIZE + TERRAIN_TILE_SIZE / 2,
    z: cz * TERRAIN_TILE_SIZE + TERRAIN_TILE_SIZE / 2,
  }
}

function evictAt(cx: number, cz: number) {
  const p = at(cx, cz)
  housingManager.evictDistantChunks(p.x, p.z)
}

function ids(): string[] {
  return housingManager
    .getAllHouses()
    .map((h) => h.id)
    .sort()
}

describe('housingManager.evictDistantChunks', () => {
  beforeEach(() => {
    // Collapse whatever a previous test loaded down to nothing. Z does not
    // wrap, so anywhere far enough on that axis evicts everything.
    evictAt(0, 9999)
    removed.length = 0
  })

  it('drops chunks beyond the radius and keeps those inside it', () => {
    load('near', 0, 0)
    load('edge', 2, -2) // exactly at the evict radius
    load('far', 3, 0)
    load('farZ', 0, 5)

    evictAt(0, 0)

    expect(ids()).toEqual(['edge', 'near'])
    expect(removed.sort()).toEqual(['far', 'farZ'])
  })

  it('keeps a chunk adjacent across the wrapped X seam', () => {
    const westmost = getTerrainChunkFromPosition(
      { x: WORLD_MIN_X, y: 0, z: 0 },
      TERRAIN_TILE_SIZE
    ).x
    const eastmost = westmost + WORLD_WIDTH_X / TERRAIN_TILE_SIZE - 1
    load('acrossSeam', westmost, 0)

    // Standing on the east edge: the westmost chunk is one step away going
    // east, though its raw index differs by the full world width.
    evictAt(eastmost, 0)

    expect(removed).toEqual([])
    expect(ids()).toEqual(['acrossSeam'])
  })

  it('lets an evicted chunk be fetched again', async () => {
    load('gone', 9, 9)
    evictAt(0, 0)
    expect(removed).toEqual(['gone'])

    // The chunk key itself must be gone, not just its houses — otherwise
    // ensureChunkLoaded treats the chunk as loaded and never refetches.
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => [house('gone', 9, 9)],
    }))
    vi.stubGlobal('fetch', fetchMock)

    const back = at(9, 9)
    housingManager.loadChunksAround(back.x, back.z)
    await housingManager.waitForPending()

    expect(fetchMock).toHaveBeenCalled()
    expect(ids()).toContain('gone')
    vi.unstubAllGlobals()
  })

  it('does not re-remove a chunk already evicted', () => {
    load('once', 9, 9)
    evictAt(0, 0)
    evictAt(0, 0)

    expect(removed).toEqual(['once'])
  })

  it('moves an updated house between cached chunks', () => {
    load('moved', 0, 0)
    load('moved', 1, 0)

    evictAt(3, 0)

    expect(ids()).toEqual(['moved'])
    expect(housingManager.getHouseById('moved')?.origin).toEqual({
      x: TERRAIN_TILE_SIZE + TERRAIN_TILE_SIZE / 2,
      y: 0,
      z: TERRAIN_TILE_SIZE / 2,
    })
    expect(removed).toEqual([])
  })
})

describe('housingManager.updateStreaming', () => {
  it('loads the chunks around the player and drops the rest', async () => {
    evictAt(0, 9999)
    removed.length = 0
    load('stale', 40, 40)

    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => [house('here', 0, 0)],
    }))
    vi.stubGlobal('fetch', fetchMock)

    const p = at(0, 0)
    housingManager.updateStreaming(p.x, p.z)
    await housingManager.waitForPending()

    expect(removed).toEqual(['stale'])
    expect(ids()).toEqual(['here'])
    vi.unstubAllGlobals()
  })
})
