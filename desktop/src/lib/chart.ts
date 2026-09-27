/**
 * 图表 Tooltip 离开即隐藏（统一封装；2026-09-27 浏览器实测定论）。
 * **机制**：ECharts 6 的手动 hideTip 动作内部同样走 hideLater(hideDelay)，
 * 因此 tooltip 必须 `hideDelay: 0`（大值会把手动的隐藏也延迟掉，之前
 * hideDelay: 999999 导致一切隐藏路径失效）。正常离开由 ECharts 内部处理
 * 即时隐藏；本封装覆盖无事件路径：
 * - globalout：指针离开画布；
 * - document mousemove 目标不在本图表容器内（含其 tooltip 子元素）：
 *   兜底"画布从指针底下移走"（点击选点后汇总条展开导致布局位移，浏览器
 *   不派发 mouseout）与 WebView2 漏派发 mouseout 的情况；
 * - document mouseout 且 relatedTarget 为空：指针移出应用窗口；
 * - window blur：窗口失焦（切应用/点标题栏）。
 * 返回清理函数（组件卸载时调用）。
 */
import type { ECharts, EChartsCoreOption } from 'echarts/core';

/** Source names are data, including inside ECharts' HTML tooltips. */
export function escapeHtml(value: unknown): string {
  return String(value ?? '').replace(/[&<>"']/g, (c) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  })[c]!);
}

export const CHART_PALETTE = ['#5470e8', '#21a590', '#eda853', '#9270dc', '#dc7595', '#52a7ca', '#7b9561', '#ac8064'];

/** ECharts axes and legends have their own defaults; global textStyle alone
 * does not make them readable on a dark canvas. Keep one theme boundary. */
export function setChartOption(chart: ECharts, dark: boolean, option: EChartsCoreOption, options: { notMerge?: boolean } = {}): void {
  const color = dark ? '#b0bfd4' : '#5b6a82';
  const border = dark ? '#34435d' : '#dfe6f0';
  type Axis = { axisLabel?: object; nameTextStyle?: object; axisLine?: { lineStyle?: object } };
  const paint = (axis: Axis) => ({ ...axis, axisLabel: { ...axis.axisLabel, color },
    nameTextStyle: { ...axis.nameTextStyle, color },
    axisLine: { ...axis.axisLine, lineStyle: { ...axis.axisLine?.lineStyle, color: border } } });
  const axes = (value: Axis | Axis[] | undefined) => value === undefined ? undefined : Array.isArray(value) ? value.map(paint) : paint(value);
  const legend = option.legend as { textStyle?: object } | undefined;
  chart.setOption({ ...option,
    animation: !window.matchMedia('(prefers-reduced-motion: reduce)').matches,
    animationDurationUpdate: 180,
    textStyle: { ...option.textStyle, color },
    xAxis: axes(option.xAxis as Axis | Axis[] | undefined), yAxis: axes(option.yAxis as Axis | Axis[] | undefined),
    legend: legend ? { ...legend, textStyle: { ...legend.textStyle, color }, inactiveColor: dark ? '#687b99' : '#a0acc0' } : undefined,
    tooltip: option.tooltip ? { ...option.tooltip, backgroundColor: dark ? '#1c2940' : '#ffffff', borderColor: border, textStyle: { color }, confine: true } : undefined,
  }, options);
}

export function setupTooltipAutoHide(chart: ECharts): () => void {
  const dom = chart.getDom();
  let visible = false;
  const shown = () => { visible = true; };
  const hidden = () => { visible = false; };
  const hide = (): void => {
    if (!visible || chart.isDisposed()) return;
    visible = false;
    chart.dispatchAction({ type: 'hideTip' });
  };
  chart.on('showTip', shown);
  chart.on('hideTip', hidden);
  const onDocMouseMove = (e: MouseEvent): void => {
    // 事件目标在本图表容器（含 tooltip 子元素）内 = 仍在浏览本图表，不隐藏。
    if (e.target instanceof Node && dom.contains(e.target)) return;
    hide();
  };
  const onDocMouseOut = (e: MouseEvent): void => {
    if (!e.relatedTarget) hide();
  };
  chart.on('globalout', hide);
  document.addEventListener('mousemove', onDocMouseMove);
  document.addEventListener('mouseout', onDocMouseOut);
  window.addEventListener('blur', hide);
  return () => {
    chart.off('globalout', hide);
    chart.off('showTip', shown);
    chart.off('hideTip', hidden);
    document.removeEventListener('mousemove', onDocMouseMove);
    document.removeEventListener('mouseout', onDocMouseOut);
    window.removeEventListener('blur', hide);
  };
}

/**
 * item 触发图表（饼图/热力图）的图内空白隐藏：从图形移到画布空白处时
 * 立即隐藏——zr mousemove 无命中图形（e.target 为空）即派发 hideTip。
 * axis 触发图表勿用（网格区按设计持续显示）。
 */
export function hideTooltipOnBlank(chart: ECharts): () => void {
  const onMove = (e: { target?: unknown }): void => {
    if (!e.target && !chart.isDisposed()) chart.dispatchAction({ type: 'hideTip' });
  };
  chart.getZr().on('mousemove', onMove);
  return () => {
    if (!chart.isDisposed()) chart.getZr().off('mousemove', onMove);
  };
}
