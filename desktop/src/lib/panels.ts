/**
 * Saved panel layout (display E10/E11 and resizing):
 * - localStorage key llm-usage-panel-layout-{page}: { [group]: { order, hidden, sizes } };
 * - order stores panel IDs after drag swaps; hidden lists hidden IDs;
 * - sizes maps IDs to { span, height } set by the edit-mode bottom-right handle;
 * - load removes obsolete IDs, appends new ones and rejects invalid sizes;
 *   unavailable storage, such as privacy mode, falls back to in-memory layout.
 */
export interface PanelSize {
  /** Column span within a six-column grid, 1–6. */
  span?: number;
  /** Height in pixels; absent means content-based height. */
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

/** Load page/group layout; ids contains all current panels in default order. */
export function loadPanelGroup(page: string, group: string, ids: string[]): PanelGroupLayout {
  const saved = readPage(page)[group];
  const savedOrder = Array.isArray(saved?.order) ? (saved?.order as string[]) : [];
  const order = [...new Set(savedOrder.filter((id) => ids.includes(id)))];
  for (const id of ids) {
    if (!order.includes(id)) order.push(id);
  }
  const savedHidden = Array.isArray(saved?.hidden) ? (saved?.hidden as string[]) : [];
  const hidden = savedHidden.filter((id) => order.includes(id));
  // Keep current panels only, with integer span 1–6 and height 120–800.
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

/** Read-modify-write the page/group layout, preserving other groups. */
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
    /* Unavailable storage keeps only in-memory layout. */
  }
}

/** Reset all page layouts in edit mode; ignore unavailable storage. */
export function clearPanelPage(page: string): void {
  try {
    localStorage.removeItem(KEY_PREFIX + page);
  } catch {
    /* Ignore unavailable storage. */
  }
}
