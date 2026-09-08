<script lang="ts">
  import { T } from '@threlte/core'
  import { onMount } from 'svelte'
  import * as THREE from 'three'
  import { stallManager } from '../../managers/stallManager'
  import { currentDungeonDepth } from '../../stores/dungeonStore'
  import { loadGLB } from '../../utils/gltfCache'
  import { hoverMetrics, type HoverMetrics } from '../../utils/hoverMetrics'
  import { makeTextBadge, STALL_SIGN_BADGE_STYLE } from '../../utils/textBadge'

  /** Board height above the table top, in metres. */
  const SIGN_LIFT = 0.4
  /** Widest a board may hang. Sign text is the player's, up to 32 characters,
   *  which would otherwise sprawl 6 m across the neighbouring stalls. Set so
   *  signs up to ~20 characters keep the full 1.5x and only the long ones
   *  shrink to fit. The table is 1.6 m. */
  const SIGN_MAX_WIDTH_M = 4
  const NO_RAYCAST = () => {}

  let stallModel = $state<THREE.Group | null>(null)
  let group = $state<THREE.Group | undefined>(undefined)
  let stallHover = $state<HoverMetrics | null>(null)

  // Stalls exist on the surface only; hide them while underground.
  const stallEntries = $derived(
    $currentDungeonDepth >= 1 ? [] : [...stallManager.stalls]
  )

  onMount(() => {
    let cancelled = false
    loadGLB('/models/objects/black_market_table.glb')
      .then((gltf) => {
        if (cancelled) return
        gltf.scene.traverse((child) => {
          if (child instanceof THREE.Mesh) {
            child.castShadow = true
            child.receiveShadow = true
          }
        })
        stallModel = gltf.scene
        stallHover = hoverMetrics(gltf.scene)
      })
      .catch((error) => console.error('Failed to load stall model:', error))
    return () => {
      cancelled = true
    }
  })

  export function getGroup(): THREE.Group | undefined {
    return group
  }
</script>

<T.Group bind:ref={group}>
  {#if stallModel && stallHover}
    {#each stallEntries as [id, stall] (id)}
      <T.Group
        position={[stall.position.x, stall.position.y, stall.position.z]}
        rotation={[0, stall.rotation, 0]}
        userData={{
          stallId: id,
          hoverName: 'Stall',
          hoverOwnerId: stall.owner,
          hoverLabelY: stallHover.topY,
          hoverRingRadius: stallHover.ringRadius,
          hoverCenter: {
            x: stallHover.center.x,
            y: 0,
            z: stallHover.center.z,
          },
          hoverFloorLevel: stall.floor_level,
        }}
      >
        <T is={stallModel.clone(true)} />
        {#if stall.sign}
          {@const board = makeTextBadge(stall.sign, STALL_SIGN_BADGE_STYLE)}
          {@const fit = Math.min(1, SIGN_MAX_WIDTH_M / board.width)}
          <T.Sprite
            position.y={stallHover.topY + SIGN_LIFT}
            scale={[board.width * fit, board.height * fit, 1]}
            renderOrder={4}
            raycast={NO_RAYCAST}
          >
            <T.SpriteMaterial
              map={board.texture}
              transparent={true}
              depthWrite={false}
            />
          </T.Sprite>
        {/if}
      </T.Group>
    {/each}
  {/if}
</T.Group>
