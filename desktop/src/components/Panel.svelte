<script lang="ts">
  /**
   * 统一面板卡片（任务 D9/E10/E11）：
   * - 卡片式容器（标题 + 内容 + 边框圆角阴影）；
   * - 标题栏为拖拽手柄（Pointer Events 拖拽，父级按 index 交换 grid 位置）；
   * - 右上角显示/隐藏切换（隐藏时折叠内容，仅保留标题栏可再展开）。
   * 隐藏用 CSS 折叠而非销毁 DOM，避免 ECharts 实例随 {#if} 重建失效。
   *
   * 拖拽实现说明（2026-09-26 修复）：Tauri WebView2 默认启用原生 drag-drop
   * 拦截（tauri.conf.json app.dragDropEnabled 未配置 = true），页内 HTML5
   * dragstart/dragover/drop 不会触发，导致“完全无法拖动”。因此改用
   * Pointer Events：pointerdown 在此捕获，移动/抬起由父级在 window 上监听，
   * elementsFromPoint 命中测试决定落点面板。编辑模式（editable）外手柄与
   * 显隐按钮隐藏且不可拖。
   */
  import { t } from '../lib/i18n.svelte';

  let {
    title,
    span = 3,
    hidden = false,
    editable = false,
    dragging = false,
    dropTarget = false,
    /** 布局分组名与面板在分组内的序号（父级拖拽命中测试回读）。 */
    panelGroup = '',
    panelIndex = -1,
    ontoggle,
    onpickstart,
    children,
  }: {
    title: string;
    /** 6 列网格中的跨列数（2/3/4/6）。 */
    span?: number;
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
</script>

<section
  class="pcard s{span}"
  class:hidden
  class:editable
  class:dragging
  class:droptarget={dropTarget}
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
</section>

<style>
  .pcard {
    background: #fff;
    border: 1px solid #e6e8eb;
    border-radius: 10px;
    box-shadow: 0 1px 3px rgba(16, 24, 40, 0.06);
    padding: 0 12px 10px;
    min-width: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
  }
  .pcard.s2 { grid-column: span 2; }
  .pcard.s3 { grid-column: span 3; }
  .pcard.s4 { grid-column: span 4; }
  .pcard.s6 { grid-column: span 6; }
  .phead {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 0 6px;
    border-bottom: 1px solid #f0f1f3;
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
    color: #b6bcc4;
    font-size: 12px;
    letter-spacing: -1px;
    line-height: 1;
  }
  .phead h3 {
    font-size: 13px;
    margin: 0;
    color: #333;
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
    background: #f0f2f5;
  }
  .pbody {
    min-width: 0;
  }
  .pbody.collapsed {
    display: none;
  }
  .pcard.hidden {
    background: #fafbfc;
  }
  .pcard.hidden .phead h3 {
    color: #9aa1a9;
  }
  .pcard.dragging {
    opacity: 0.45;
  }
  .pcard.droptarget {
    outline: 2px dashed #1a56c4;
    outline-offset: 2px;
  }
  @media (max-width: 900px) {
    .pcard.s2,
    .pcard.s3,
    .pcard.s4,
    .pcard.s6 {
      grid-column: 1 / -1;
    }
  }
</style>
