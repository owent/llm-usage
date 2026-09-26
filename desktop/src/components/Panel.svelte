<script lang="ts">
  /**
   * 统一面板卡片（任务 D9/E10/E11 + 尺寸调整）：
   * - 卡片式容器（标题 + 内容 + 边框圆角阴影）；
   * - 标题栏为拖拽手柄（Pointer Events 拖拽，父级按 index 交换 grid 位置）；
   * - 右上角显示/隐藏切换（隐藏时折叠内容，仅保留标题栏可再展开）；
   * - 编辑模式右下角 resize 把手：拖动时面板边缘实时跟踪指针（按网格实际
   *   列宽换算目标 span 1–6，高度连续缩放 120–800px），pointerup 吸附最近
   *   合法档位并经 onsize 回调持久化（2026-09-26 由档位步进改为边缘跟踪）。
   * 隐藏用 CSS 折叠而非销毁 DOM，避免 ECharts 实例随 {#if} 重建失效。
   *
   * 拖拽实现说明（2026-09-26 修复）：Tauri WebView2 默认启用原生 drag-drop
   * 拦截（tauri.conf.json app.dragDropEnabled 未配置 = true），页内 HTML5
   * dragstart/dragover/drop 不会触发，导致“完全无法拖动”。因此改用
   * Pointer Events：pointerdown 在此捕获，移动/抬起由父级在 window 上监听，
   * elementsFromPoint 命中测试决定落点面板。编辑模式（editable）外手柄与
   * 显隐按钮隐藏且不可拖。resize 把手同样用 pointer capture（事件固定派发
   * 到把手元素），移动中本地预览（liveSpan/liveHeight），抬起才提交回调。
   */
  import { t } from '../lib/i18n.svelte';

  /** 高度拖动范围（px；与 lib/panels.ts 持久化清洗范围一致）。 */
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
    /** 布局分组名与面板在分组内的序号（父级拖拽命中测试回读）。 */
    panelGroup = '',
    panelIndex = -1,
    ontoggle,
    onpickstart,
    /** 编辑模式拖动右下角把手后的最终尺寸（height 未定义 = 自适应）。 */
    onsize,
    children,
  }: {
    title: string;
    /** 6 列网格中的跨列数（1–6）。 */
    span?: number;
    /** 面板高度档位（px；未设置 = 自适应内容高度）。 */
    height?: number;
    hidden?: boolean;
    /** 编辑模式：显示手柄/显隐按钮并允许拖拽。 */
    editable?: boolean;
    dragging?: boolean;
    dropTarget?: boolean;
    panelGroup?: string;
    panelIndex?: number;
    ontoggle: () => void;
    /** 编辑模式下按下标题栏（非按钮处）开始拖拽。 */
    onpickstart: (e: PointerEvent) => void;
    onsize?: (span: number, height: number | undefined) => void;
    children: import('svelte').Snippet;
  } = $props();

  function handlePointerDown(e: PointerEvent): void {
    if (!editable) return;
    if (!e.isPrimary || e.button !== 0) return;
    // 显隐按钮的点击不进入拖拽。
    if ((e.target as HTMLElement).closest('.ptoggle')) return;
    // 指针捕获：移动/抬起即使移出元素或窗口边界也仍派发到本元素（冒泡至 window）。
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    onpickstart(e);
  }

  // ---- resize 把手（Pointer Events；边缘连续跟踪，抬起提交） ----
  /**
   * 2026-09-26 重做：原实现按档位步进（span 1→2→3→4 循环、高度 200/300/400/500
   * 档），面板边缘不跟手且永远到不了 5/6 列。现改为边缘跟踪：
   * - pointerdown 时解析所在 .panel-grid 的 computed grid-template-columns 实际
   *   列宽与列间距，得到“单列步长 = 列宽 + gap”与面板起点；
   * - pointermove 按指针 x 相对面板起点的位置实时换算目标 span（1–6 连续预览，
   *   且不超过面板到网格右缘可容纳的列数），高度按起始高度 + dy 连续缩放；
   * - pointerup 提交最近合法 span（1–6 整数）与四舍五入后的高度（120–800px）。
   */
  let resizeDrag = $state<{
    startX: number;
    startY: number;
    panelLeft: number;
    /** 单列步长（列宽 + 列间距，px）；0 = 非 6 列布局（如窄屏单列），宽度不动。 */
    colPitch: number;
    /** 面板起点到网格右缘可容纳的最大列数（≤6）。 */
    maxSpan: number;
    /** 拖动起始高度（auto 时取当前内容高度）。 */
    baseHeight: number;
    movedX: boolean;
    movedY: boolean;
  } | null>(null);
  /** 拖动中的预览尺寸（null = 未在拖动，回退到 props）。 */
  let liveSpan = $state<number | null>(null);
  let liveHeight = $state<number | null>(null);

  const effSpan = $derived(liveSpan ?? span);
  const effHeight = $derived(liveHeight ?? height ?? null);

  /** 拖动结束后的那次 click 要跳过（pointerup 已提交拖动结果）。 */
  let suppressClick = false;

  function resizeDown(e: PointerEvent): void {
    if (!editable || !onsize) return;
    if (!e.isPrimary || e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    // 指针捕获：移动/抬起始终派发到把手元素，移出面板也不丢事件。
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
      // computed track 列表（px，含小数）；窄屏 1 列布局不足 6 轨时不改宽度。
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
      // 指针到面板起点的列数（半列宽容差后四舍五入 = 吸附到最近网格线）。
      const cols = (e.clientX - resizeDrag.panelLeft + resizeDrag.colPitch / 2) / resizeDrag.colPitch;
      liveSpan = Math.max(1, Math.min(resizeDrag.maxSpan, Math.round(cols)));
    }
    if (resizeDrag.movedY) {
      liveHeight = Math.min(HEIGHT_MAX, Math.max(HEIGHT_MIN, Math.round(resizeDrag.baseHeight + dy)));
    }
  }

  function resizeUp(): void {
    if (!resizeDrag) return;
    // 未发生该方向位移时保留原值（宽度未动保 span，高度未动保 auto/原档位）。
    const nextSpan = resizeDrag.movedX && liveSpan !== null ? liveSpan : span;
    const nextHeight = resizeDrag.movedY && liveHeight !== null ? liveHeight : height;
    const moved = resizeDrag.movedX || resizeDrag.movedY;
    resizeDrag = null;
    liveSpan = null;
    liveHeight = null;
    suppressClick = moved;
    onsize?.(nextSpan, nextHeight);
  }

  /** 纯点击（含键盘触发）：跨列数在 1–6 间前进一步，高度保持现状。 */
  function resizeClick(): void {
    if (!editable || !onsize) return;
    const skip = suppressClick;
    suppressClick = false;
    if (skip) return;
    onsize?.((span % 6) + 1, height);
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
        {hidden ? '🚫' : '👁'}
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
      onpointercancel={resizeUp}
      onclick={resizeClick}
    ></button>
  {/if}
</section>

<style>
  .pcard {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: var(--shadow);
    padding: 0 12px 10px;
    min-width: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    /* resize 把手绝对定位于右下角；style:height 以整卡高度计算。 */
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
    padding: 8px 0 6px;
    border-bottom: 1px solid var(--border-light);
    user-select: none;
  }
  /* 仅编辑模式可拖拽：grab 光标 + 禁用触摸滚动（pointer 拖拽不被打断）。 */
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
    font-size: 13px;
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
    /* 高度档位生效时内容区填充剩余高度并可滚动（图表/表格超出时）。 */
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }
  .pbody.collapsed {
    display: none;
  }
  /* 右下角 resize 把手（8×8 三角形视觉，14×14 命中区）；仅编辑模式渲染。 */
  .presize {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 14px;
    height: 14px;
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
