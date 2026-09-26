/**
 * 面板布局持久化（显示设置 E10/E11）：
 * - localStorage key `llm-usage-panel-layout-{page}`，值为 { [group]: { order, hidden } }；
 * - order 为面板 id 顺序（拖拽交换后保存），hidden 为隐藏面板 id 列表；
 * - 加载时做清洗：去掉已不存在的历史 id、补上新增 id（追加到末尾），
 *   localStorage 不可用（隐私模式等）时静默退化为内存布局。
 */
export interface PanelGroupLayout {
  order: string[];
  hidden: string[];
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
  const order = ids.filter((id) => savedOrder.includes(id));
  for (const id of ids) {
    if (!order.includes(id)) order.push(id);
  }
  const savedHidden = Array.isArray(saved?.hidden) ? (saved?.hidden as string[]) : [];
  const hidden = savedHidden.filter((id) => order.includes(id));
  return { order, hidden };
}

/** 保存某页某分组布局（读-改-写整页记录，保留其它分组）。 */
export function savePanelGroup(
  page: string,
  group: string,
  layout: PanelGroupLayout
): void {
  const record = readPage(page);
  record[group] = { order: [...layout.order], hidden: [...layout.hidden] };
  try {
    localStorage.setItem(KEY_PREFIX + page, JSON.stringify(record));
  } catch {
    /* 存储不可用时仅保留内存布局 */
  }
}
