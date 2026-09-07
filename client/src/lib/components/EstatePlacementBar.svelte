<script lang="ts">
  import {
    estateChestError,
    estateChestMode,
    estateChestPending,
    stopEstateChestMode,
  } from '../stores/estateStorageStore'
  import { playerVisualFloorLevel } from '../stores/housingStore'
  import { itemDisplayName } from '../data/itemDefs'
  import PlacementModeBar from './PlacementModeBar.svelte'

  const presentation = $derived(
    $estateChestMode
      ? {
          title: `${itemDisplayName($estateChestMode.item_def_id)} · ${$playerVisualFloorLevel + 1}F`,
          message: $estateChestPending
            ? 'Saving…'
            : ($estateChestError ??
              'Point inside your estate and click to place'),
          hint: 'Left-click to place · Right-click to move · Mouse wheel rotates · Esc to finish',
        }
      : null
  )
</script>

{#if presentation}
  <PlacementModeBar
    title={presentation.title}
    message={presentation.message}
    hint={presentation.hint}
    onaction={stopEstateChestMode}
  />
{/if}
