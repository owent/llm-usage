<script lang="ts">
  import { t } from '../lib/i18n.svelte';

  /** Height drag range in pixels, matching lib/panels.ts persisted-value validation. */
  const HEIGHT_MIN = 120;
  const HEIGHT_MAX = 800;

  let {
    title,
    span = 3,
    height,
    hidden = false,
    editable = false,
    dragging = false,
    dropTarget = false,
    /** Layout group and index used by the parent's drag hit testing. */
    panelGroup = '',
    panelIndex = -1,
    ontoggle,
    onpickstart,
    /** Final edit-mode resize dimensions; undefined height fits content. */
    onsize,
    children,
  }: {
    title: string;
    /** Columns occupied in a six-column grid, from 1 to 6. */
    span?: number;
    /** Height in pixels; absent height fits content. */
    height?: number;
    hidden?: boolean;
    /** Edit mode displays drag/toggle controls and permits dragging. */
    editable?: boolean;
    dragging?: boolean;
    dropTarget?: boolean;
    panelGroup?: string;
    panelIndex?: number;
    ontoggle: () => void;
    /** Start edit-mode dragging on header areas outside buttons. */
    onpickstart: (e: PointerEvent) => void;
    onsize?: (span: number, height: number | undefined) => void;
    children: import('svelte').Snippet;
  } = $props();

  function handlePointerDown(e: PointerEvent): void {
    if (!editable) return;
    if (!e.isPrimary || e.button !== 0) return;
    // The visibility button does not start a drag.
    if ((e.target as HTMLElement).closest('.ptoggle')) return;
    // Request pointer capture so moves/releases outside this element still reach it and bubble to window.
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    onpickstart(e);
  }

  // ---- Pointer resize: preview edge movement, commit on release. ----
  /**
   * Resize updated 2026-09-26 follows pointer movement. The earlier stepped widths 1/2/3/4 and
   * heights 200/300/400/500 did not follow the edge or reach columns 5/6.
   * - pointerdown reads .panel-grid computed tracks/gap, column pitch and panel origin.
   * - pointermove derives span 1-6 from pointer x, limited by remaining grid columns,
   *   and height from starting height plus dy.
   * - pointerup commits integer span and rounded height in the 120-800 pixel range.
   */
  let resizeDrag = $state<{
    startX: number;
    startY: number;
    panelLeft: number;
    /** Column width plus gap in pixels; zero leaves width unchanged outside a six-track layout. */
    colPitch: number;
    /** Maximum columns available between panel origin and the grid's right edge, at most six. */
    maxSpan: number;
    /** Starting height; auto uses the current content height. */
    baseHeight: number;
    movedX: boolean;
    movedY: boolean;
  } | null>(null);
  /** Live resize preview; null uses the supplied props. */
  let liveSpan = $state<number | null>(null);
  let liveHeight = $state<number | null>(null);

  const effSpan = $derived(liveSpan ?? span);
  const effHeight = $derived(liveHeight ?? height ?? null);

  function resizeDown(e: PointerEvent): void {
    if (!editable || !onsize) return;
    if (!e.isPrimary || e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    // Request pointer capture to retain handle events when the pointer leaves the panel.
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    const card = (e.currentTarget as HTMLElement).closest('.pcard') as HTMLElement | null;
    const grid = (card?.parentElement as HTMLElement | null) ?? null;
    let panelLeft = 0;
    let colPitch = 0;
    let maxSpan = span;
    if (card && grid) {
      const cardRect = card.getBoundingClientRect();
      const gridRect = grid.getBoundingClientRect();
      panelLeft = cardRect.left;
      // Computed tracks in pixels, including fractions; fewer than six tracks leaves width unchanged.
      const tracks = getComputedStyle(grid)
        .gridTemplateColumns.split(' ')
        .map((s) => Number.parseFloat(s))
        .filter((w) => Number.isFinite(w) && w > 0);
      if (tracks.length >= 6) {
        const gap = Number.parseFloat(getComputedStyle(grid).columnGap) || 0;
        const colWidth = tracks.slice(0, 6).reduce((s, w) => s + w, 0) / 6;
        colPitch = colWidth + gap;
        maxSpan = Math.max(
          1,
          Math.min(6, Math.round((gridRect.right - cardRect.left + gap / 2) / colPitch))
        );
      }
    }
    resizeDrag = {
      startX: e.clientX,
      startY: e.clientY,
      panelLeft,
      colPitch,
      maxSpan,
      baseHeight: height ?? card?.offsetHeight ?? HEIGHT_MIN,
      movedX: false,
      movedY: false,
    };
    liveSpan = span;
    liveHeight = height ?? null;
  }

  function resizeMove(e: PointerEvent): void {
    if (!resizeDrag) return;
    const dx = e.clientX - resizeDrag.startX;
    const dy = e.clientY - resizeDrag.startY;
    if (Math.abs(dx) > 3) resizeDrag.movedX = true;
    if (Math.abs(dy) > 3) resizeDrag.movedY = true;
    if (resizeDrag.movedX && resizeDrag.colPitch > 0) {
      // Add half a column pitch, then round the pointer's column position to the target span.
      const cols = (e.clientX - resizeDrag.panelLeft + resizeDrag.colPitch / 2) / resizeDrag.colPitch;
      liveSpan = Math.max(1, Math.min(resizeDrag.maxSpan, Math.round(cols)));
    }
    if (resizeDrag.movedY) {
      liveHeight = Math.min(HEIGHT_MAX, Math.max(HEIGHT_MIN, Math.round(resizeDrag.baseHeight + dy)));
    }
  }

  function resizeUp(): void {
    if (!resizeDrag) return;
    // Preserve each unchanged axis: span for width, and auto/original height for height.
    const nextSpan = resizeDrag.movedX && liveSpan !== null ? liveSpan : span;
    const nextHeight = resizeDrag.movedY && liveHeight !== null ? liveHeight : height;
    const moved = resizeDrag.movedX || resizeDrag.movedY;
    resizeDrag = null;
    liveSpan = null;
    liveHeight = null;
    if (moved) onsize?.(nextSpan, nextHeight);
  }

  function resizeKey(e: KeyboardEvent): void {
    if (!editable || !onsize) return;
    const horizontal = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
    const vertical = e.key === 'ArrowDown' ? 20 : e.key === 'ArrowUp' ? -20 : 0;
    if (!horizontal && !vertical && e.key !== 'Home') return;
    e.preventDefault();
    onsize(Math.max(1, Math.min(6, span + horizontal)),
      e.key === 'Home' ? undefined : vertical ? Math.max(HEIGHT_MIN, Math.min(HEIGHT_MAX, (height ?? 320) + vertical)) : height);
  }
</script>

<section
  class="pcard s{effSpan}"
  class:hidden
  class:editable
  class:dragging
  class:droptarget={dropTarget}
  style:height={effHeight === null ? null : `${effHeight}px`}
  aria-label={title}
  data-panel-group={panelGroup}
  data-panel-index={panelIndex}
>
  <header class="phead" role="group" onpointerdown={handlePointerDown}>
    {#if editable}
      <span class="handle" title={t('panel.dragHint')} aria-hidden="true">⋮⋮</span>
    {/if}
    <h3>{title}</h3>
    {#if editable}
      <button
        type="button"
        class="ptoggle"
        title={hidden ? t('panel.show') : t('panel.hide')}
        aria-label={hidden ? t('panel.show') : t('panel.hide')}
        onclick={ontoggle}
      >
        {hidden ? t('panel.show') : t('panel.hide')}
      </button>
    {/if}
  </header>
  <div class="pbody" class:collapsed={hidden}>
    {@render children()}
  </div>
  {#if editable && onsize}
    <button
      type="button"
      class="presize"
      title={t('panel.resizeHint')}
      aria-label={t('panel.resizeHint')}
      onpointerdown={resizeDown}
      onpointermove={resizeMove}
      onpointerup={resizeUp}
      onpointercancel={() => { resizeDrag = null; liveSpan = null; liveHeight = null; }}
      onkeydown={resizeKey}
    ></button>
  {/if}
</section>

<style>
  .pcard {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 16px;
    box-shadow: var(--shadow);
    padding: 0 20px 16px;
    min-width: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    /* Position the resize handle at bottom right; style:height includes the entire card. */
    position: relative;
    box-sizing: border-box;
  }
  .pcard.s1 { grid-column: span 1; }
  .pcard.s2 { grid-column: span 2; }
  .pcard.s3 { grid-column: span 3; }
  .pcard.s4 { grid-column: span 4; }
  .pcard.s5 { grid-column: span 5; }
  .pcard.s6 { grid-column: span 6; }
  .phead {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 18px 0 14px;
    border-bottom: 1px solid var(--border-light);
    user-select: none;
  }
  /* Editable headers use grab and disable touch scrolling during pointer dragging. */
  .pcard.editable .phead {
    cursor: grab;
    touch-action: none;
  }
  .pcard.editable .phead:active {
    cursor: grabbing;
  }
  .handle {
    color: var(--scrollbar);
    font-size: 12px;
    letter-spacing: -1px;
    line-height: 1;
  }
  .phead h3 {
    font-size: 14px;
    font-weight: 650;
    margin: 0;
    color: var(--text-heading);
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ptoggle {
    border: none;
    background: transparent;
    font-size: 13px;
    line-height: 1;
    padding: 2px 4px;
    border-radius: 5px;
    cursor: pointer;
    opacity: 0.55;
  }
  .ptoggle:hover {
    opacity: 1;
    background: var(--bg-hover);
  }
  .pbody {
    min-width: 0;
    /* Sized content fills remaining height and scrolls overflowing charts/tables. */
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }
  .pbody.collapsed {
    display: none;
  }
  /* Edit-only resize handle: 8x8 triangle inside a 24x24 hit area. */
  .presize {
    position: absolute;
    right: 3px;
    bottom: 3px;
    width: 24px;
    height: 24px;
    padding: 0;
    border: none;
    background: transparent;
    cursor: nwse-resize;
    touch-action: none;
    z-index: 2;
  }
  .presize::after {
    content: '';
    position: absolute;
    right: 2px;
    bottom: 2px;
    width: 0;
    height: 0;
    border-style: solid;
    border-width: 0 0 8px 8px;
    border-color: transparent transparent var(--text-muted) transparent;
  }
  .presize:hover::after {
    border-bottom-color: var(--accent);
  }
  .pcard.hidden:not(.editable) { display: none; }
  .pcard.hidden {
    background: var(--bg-card-hover);
  }
  .pcard.hidden .phead h3 {
    color: var(--text-muted);
  }
  .pcard.dragging {
    opacity: 0.45;
  }
  .pcard.droptarget {
    outline: 2px dashed var(--accent);
    outline-offset: 2px;
  }
  @media (max-width: 900px) {
    .pcard.s1,
    .pcard.s2,
    .pcard.s3,
    .pcard.s4,
    .pcard.s5,
    .pcard.s6 {
      grid-column: 1 / -1;
    }
  }
</style>
