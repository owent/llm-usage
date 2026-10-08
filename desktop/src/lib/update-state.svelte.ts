import { api, parseError, type UpdateStatusDto } from './api';

export const updateState = $state({ status: null as UpdateStatusDto | null, error: '', pending: false, cancelling: false });

export async function loadUpdateStatus(): Promise<void> {
  const status = await api.updateStatus();
  if (status) updateState.status = status;
}

export function pollUpdates(): () => void {
  let active = true;
  let reading = false;
  async function poll() {
    if (!active || reading) return;
    reading = true;
    try {
      const status = await api.updateStatus();
      if (active && status) updateState.status = status;
    } catch { /* Status polling resumes when IPC is available again. */ }
    finally { reading = false; }
  }
  void poll();
  const timer = setInterval(() => void poll(), 1000);
  return () => { active = false; clearInterval(timer); };
}

export async function updateAction(action: 'check' | 'download' | 'cancel' | 'install'): Promise<void> {
  if (action === 'cancel') {
    if (updateState.cancelling) return;
    updateState.cancelling = true;
    try { await api.cancelUpdate(); await loadUpdateStatus(); }
    catch (error) { updateState.error = parseError(error); }
    finally { updateState.cancelling = false; }
    return;
  }
  if (updateState.pending) return;
  updateState.pending = true;
  updateState.error = '';
  try {
    await ({ check: api.checkUpdate, download: api.downloadUpdate, cancel: api.cancelUpdate, install: api.installUpdate })[action]();
    await loadUpdateStatus();
  } catch (error) { updateState.error = parseError(error); }
  finally { updateState.pending = false; }
}

export function updateBusy(status: UpdateStatusDto | null): boolean {
  return ['checking', 'downloading', 'verifying', 'installing'].includes(status?.phase ?? '');
}
