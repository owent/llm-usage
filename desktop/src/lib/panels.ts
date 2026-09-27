/**
 * 面板布局持久化（显示设置 E10/E11 + 尺寸调整）：
 * - localStorage key `llm-usage-panel-layout-{page}`，值为 { [group]: { order, hidden, sizes } }；
 * - order 为面板 id 顺序（拖拽交换后保存），hidden 为隐藏面板 id 列表；
 * - sizes 为面板 id → { span, height }（编辑模式右下角把手拖出的跨列/高度档位）；
 * - 加载时做清洗：去掉已不存在的历史 id、补上新增 id（追加到末尾）、裁剪非法尺寸值，
 *   localStorage 不可用（隐私模式等）时静默退化为内存布局。
 */
export interface PanelSize {
  /** 6 列网格中的跨列数（1–6）。 */
  span?: number;
  /** 面板高度档位（px；未设置 = 自适应内容高度）。 */
  height?: number;
}

export interface PanelGroupLayout {
  order: string[];
  hidden: string[];
  sizes: Record<string, PanelSize>;
}

const KEY_PREFIX = 'llm-usage-panel-layout-';

function readPage(page: string): Record<string, PanelGroupLayout> {
  try {
    const raw = localStorage.getItem(KEY_PREFIX + page);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Record<string, PanelGroupLayout>;
    if (parsed === null || typeof parsed !== 'object') return {};
    return parsed;
  } catch {
    return {};
  }
}

/** 读取某页某分组的布局；ids 为该分组当前全部面板（顺序即默认顺序）。 */
export function loadPanelGroup(page: string, group: string, ids: string[]): PanelGroupLayout {
  const saved = readPage(page)[group];
  const savedOrder = Array.isArray(saved?.order) ? (saved?.order as string[]) : [];
  const order = [...new Set(savedOrder.filter((id) => ids.includes(id)))];
  for (const id of ids) {
    if (!order.includes(id)) order.push(id);
  }
  const savedHidden = Array.isArray(saved?.hidden) ? (saved?.hidden as string[]) : [];
  const hidden = savedHidden.filter((id) => order.includes(id));
  // 尺寸清洗：只保留当前面板的合法 span（1–6 整数）与高度档位（120–800 整数）。
  const rawSizes =
    saved?.sizes !== null && typeof saved?.sizes === 'object'
      ? (saved?.sizes as Record<string, PanelSize>)
      : {};
  const sizes: Record<string, PanelSize> = {};
  for (const id of order) {
    const raw = rawSizes[id];
    if (raw === null || typeof raw !== 'object') continue;
    const span = Number(raw.span);
    const height = Number(raw.height);
    const next: PanelSize = {};
    if (Number.isInteger(span) && span >= 1 && span <= 6) next.span = span;
    if (Number.isInteger(height) && height >= 120 && height <= 800) next.height = height;
    if (next.span !== undefined || next.height !== undefined) sizes[id] = next;
  }
  return { order, hidden, sizes };
}

/** 保存某页某分组布局（读-改-写整页记录，保留其它分组）。 */
export function savePanelGroup(
  page: string,
  group: string,
  layout: PanelGroupLayout
): void {
  const record = readPage(page);
  const sizes: Record<string, PanelSize> = {};
  for (const [id, size] of Object.entries(layout.sizes ?? {})) {
    sizes[id] = { ...size };
  }
  record[group] = { order: [...layout.order], hidden: [...layout.hidden], sizes };
  try {
    localStorage.setItem(KEY_PREFIX + page, JSON.stringify(record));
  } catch {
    /* 存储不可用时仅保留内存布局 */
  }
}

/** 清除某页全部布局记录（编辑模式“重置布局”用；存储不可用时静默忽略）。 */
export function clearPanelPage(page: string): void {
  try {
    localStorage.removeItem(KEY_PREFIX + page);
  } catch {
    /* 存储不可用时忽略 */
  }
}
