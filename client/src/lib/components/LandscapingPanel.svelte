<script lang="ts">
  import {
    landscapingMode,
    landscapingPending,
    landscapingError,
    landscapingHint,
    hasLandscapingToolbox,
    selectLandscapingTool,
  } from '../stores/landscapingStore'
  import {
    fenceMode,
    fencePending,
    fenceError,
    fenceTarget,
    fenceCount,
    showFenceNoSpawnZones,
    stopFenceMode,
  } from '../stores/fenceStore'
  import {
    estateChestError,
    estateChestMode,
    estateChestPending,
    stopEstateChestMode,
  } from '../stores/estateStorageStore'
  import { inventoryStore } from '../stores/inventoryStore'
  import { estateStorageDefs } from '../data/estateFurnitureDefs'
  import { getItemDef, itemDisplayName } from '../data/itemDefs'
  import { playerVisualFloorLevel } from '../stores/housingStore'
  import { networkManager } from '../network/socket'
  import type { LandscapingTool } from '../terrain/landscaping'
  import { isAdminUser } from '../stores/gameStore'
  import { stopHouseInteraction } from '../stores/housePlacementStore'
  import HousePlacementPanel from './HousePlacementPanel.svelte'
  import SplatBrushPanel from './map-editor/SplatBrushPanel.svelte'
  import { draggablePanel } from '../actions/draggablePanel'

  type EditorTab = Exclude<LandscapingTool, 'Fence'> | 'Objects'

  const tabs: EditorTab[] = ['Ground', 'Road', 'Objects', 'House']
  const editorOpen = $derived(
    $landscapingMode !== null || $estateChestMode !== null
  )
  const activeTab = $derived<EditorTab>(
    $estateChestMode || $landscapingMode?.tool === 'Fence'
      ? 'Objects'
      : ($landscapingMode?.tool ?? 'Objects')
  )
  const fenceDefinition = getItemDef('wooden_fence')
  const storageObjects = $derived(
    [...estateStorageDefs.values()].map((definition) => {
      const items = $inventoryStore.bag.filter(
        (item) => item.item_def_id === definition.itemDefId
      )
      return {
        definition: getItemDef(definition.itemDefId),
        itemDefId: definition.itemDefId,
        instanceId: items[0]?.instance_id,
        quantity: items.reduce((total, item) => total + item.quantity, 0),
      }
    })
  )

  function selectTab(tab: EditorTab) {
    if (tab === activeTab) return
    if (tab === 'Objects') {
      selectFence()
      return
    }
    if (tab !== 'House') {
      stopHouseInteraction()
    }
    if ($estateChestMode) networkManager.sendStartLandscapingMode(tab)
    else selectLandscapingTool(tab)
  }

  function selectFence() {
    if ($fenceMode) return
    stopHouseInteraction()
    if ($landscapingMode) {
      stopEstateChestMode()
      selectLandscapingTool('Fence')
    } else {
      networkManager.sendStartLandscapingMode('Fence')
    }
  }

  function selectStorage(instanceId: number | undefined) {
    if (instanceId === undefined || $estateChestPending) return
    stopHouseInteraction()
    networkManager.sendUseItem(instanceId)
  }

  function close() {
    stopHouseInteraction()
    stopFenceMode()
    stopEstateChestMode()
  }
</script>

{#if editorOpen}
  {@const status = $estateChestMode
    ? $estateChestPending
      ? 'Saving…'
      : ($estateChestError ?? 'Point inside your estate and click to place')
    : $landscapingMode?.tool === 'House'
      ? null
      : $landscapingMode?.tool === 'Fence'
        ? $fencePending
          ? 'Saving…'
          : ($fenceError ?? $fenceTarget?.reason)
        : $landscapingPending
          ? 'Saving…'
          : ($landscapingError ?? $landscapingHint)}
  <div class="landscaping-panel" use:draggablePanel={'landscaping'}>
    <div class="panel-header" data-drag-handle>
      <strong>Estate Editor</strong>
      <button
        class="close-btn"
        aria-label="Close estate editor"
        title="Close (Esc)"
        onclick={close}>×</button
      >
    </div>
    <div class="tabs" role="tablist" aria-label="Estate editing tools">
      {#each tabs as tab (tab)}
        <button
          role="tab"
          aria-selected={activeTab === tab}
          class:active={activeTab === tab}
          disabled={tab !== 'Objects' && !$hasLandscapingToolbox}
          title={tab !== 'Objects' && !$hasLandscapingToolbox
            ? "Carry a Landscaper's Toolbox to use this tool"
            : tab}
          onclick={() => selectTab(tab)}>{tab}</button
        >
      {/each}
    </div>
    {#if activeTab === 'House'}
      <HousePlacementPanel />
    {:else if activeTab === 'Objects'}
      <div class="object-content">
        <strong>Placeable Objects</strong>
        <div class="object-list">
          <button
            class="object-row"
            class:active={$fenceMode !== null}
            title={$fenceCount
              ? 'Place or recover fences'
              : 'Select to recover placed fences'}
            onclick={selectFence}
          >
            {#if fenceDefinition}
              <img src="/items/{fenceDefinition.icon}" alt="" />
            {/if}
            <span>{itemDisplayName('wooden_fence')}</span>
            <small>×{$fenceCount}</small>
          </button>
          {#each storageObjects as object (object.itemDefId)}
            <button
              class="object-row"
              class:active={$estateChestMode?.item_def_id === object.itemDefId}
              disabled={object.instanceId === undefined || $estateChestPending}
              title={object.quantity
                ? `Place ${itemDisplayName(object.itemDefId)}`
                : 'None in your bag'}
              onclick={() => selectStorage(object.instanceId)}
            >
              {#if object.definition}
                <img src="/items/{object.definition.icon}" alt="" />
              {/if}
              <span>{itemDisplayName(object.itemDefId)}</span>
              <small>×{object.quantity}</small>
            </button>
          {/each}
        </div>
        {#if $isAdminUser && $fenceMode}
          <label class="zone-toggle">
            <input type="checkbox" bind:checked={$showFenceNoSpawnZones} />
            Show no-spawn zones
          </label>
        {/if}
        {#if $estateChestMode}
          <small
            >{$playerVisualFloorLevel + 1}F · Left-click to place · Right-click
            to move · Mouse wheel rotates · Esc to finish</small
          >
        {:else if $fenceMode}
          <small
            >Left-click to place or recover · Right-click to move · Esc to
            finish</small
          >
        {/if}
      </div>
    {:else}
      <SplatBrushPanel
        sizeLabel={activeTab === 'Road' ? 'Width' : 'Size'}
        title={activeTab === 'Ground' ? 'Ground Brush' : 'Road Tool'}
        hint={activeTab === 'Ground' ? '(drag to paint)' : '(click two points)'}
        availableLayers={$landscapingMode?.palette ?? []}
      />
    {/if}
    {#if status}
      <div class="paint-status" role="status">{status}</div>
    {/if}
  </div>
{/if}

<style>
  .landscaping-panel {
    position: fixed;
    bottom: 100px;
    left: 16px;
    z-index: 40;
    max-width: calc(100vw - 32px);
    max-height: calc(100vh - 120px);
    overflow: auto;
    border-radius: 8px;
    background: #211c16ed;
    color: #f3e8d2;
    pointer-events: auto;
  }
  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 4px 8px;
    font-family: 'Courier New', monospace;
    font-size: 13px;
  }
  button {
    cursor: pointer;
    color: inherit;
    background: #3a3024;
    border: 1px solid #766247;
    border-radius: 4px;
    padding: 5px 14px;
    font-family: 'Courier New', monospace;
    font-size: 12px;
    font-weight: bold;
  }
  .close-btn {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: none;
    background: transparent;
    font-size: 16px;
    line-height: 1;
  }
  .close-btn:hover {
    background: #3a3024;
  }
  button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .tabs {
    display: flex;
    gap: 2px;
    margin: 0 0 4px;
    padding: 2px;
    background: rgba(0, 0, 0, 0.7);
  }
  .tabs button {
    flex: 1;
    padding: 4px 10px;
    border: none;
    background: transparent;
    color: #888;
    letter-spacing: 0.5px;
    transition:
      background 150ms ease,
      color 150ms ease;
  }
  .tabs button:hover:not(:disabled) {
    color: #ccc;
  }
  .tabs button.active {
    background: rgba(226, 185, 59, 0.25);
    color: #e2b93b;
  }
  .landscaping-panel :global(.splat-brush-panel) {
    border: none;
    border-block: 1px solid rgba(226, 185, 59, 0.3);
    border-radius: 0;
    box-shadow: none;
  }
  .object-content,
  .paint-status {
    display: grid;
    gap: 6px;
    padding: 12px 16px;
    font-family: 'Courier New', monospace;
    font-size: 12px;
  }
  .object-list {
    display: grid;
    gap: 5px;
    min-width: 280px;
  }
  .object-row {
    display: grid;
    grid-template-columns: 28px 1fr auto;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    text-align: left;
  }
  .object-row.active {
    border-color: #e2b93b;
    background: rgba(226, 185, 59, 0.2);
  }
  .object-row img {
    width: 28px;
    height: 28px;
    object-fit: contain;
  }
  .object-row small,
  .object-content > small {
    color: #aaa;
  }
  .zone-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
  }
  .zone-toggle input {
    accent-color: #e2b93b;
  }
</style>
