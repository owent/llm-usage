import { api, type TelemetryTargetDto, type TelemetryPreviewDto } from './api';
import { configureTelemetryBatch, telemetryFailureKey, type TelemetryResult } from './telemetry-batch';

/** Share async results and coalesce concurrent checks; opening details rechecks installation. */
export const telemetry = $state({ rows: [] as TelemetryTargetDto[], checking: false, checked: false, error: '' });
let pending: Promise<void> | undefined;
let checkedAt = 0;

/** Configuration and undo state survives navigating between overview and settings. */
export const telemetrySetup = $state({
  busy: false, batch: false, error: '', preview: null as TelemetryPreviewDto | null,
  completed: 0, total: 0, results: [] as TelemetryResult[],
  applied: {} as Record<string, TelemetryPreviewDto>,
});
export function checkTelemetry(force = false): Promise<void> {
  if (pending) return pending;
  if (telemetry.checked && !force && Date.now() - checkedAt < 30_000) return Promise.resolve();
  telemetry.checking = true;
  telemetry.error = '';
  pending = api.telemetryCheck().then((rows) => {
    telemetry.rows = rows;
    telemetry.checked = true;
    checkedAt = Date.now();
  }).catch(() => { telemetry.error = 'telemetry.checkFailed'; })
    .finally(() => { telemetry.checking = false; pending = undefined; });
  return pending;
}

function rememberApplied(results: TelemetryResult[]) {
  for (const result of results) {
    if (result.status === 'applied' && result.preview) {
      telemetrySetup.applied[result.target.id] = result.preview;
    }
  }
}

export async function configureAllTelemetry(): Promise<void> {
  if (telemetrySetup.busy) return;
  telemetrySetup.busy = true;
  telemetrySetup.batch = true;
  telemetrySetup.error = '';
  telemetrySetup.preview = null;
  telemetrySetup.completed = telemetrySetup.total = 0;
  telemetrySetup.results = [];
  try {
    await configureTelemetryBatch({
      check: async () => {
        await checkTelemetry(true);
        if (telemetry.error) throw 'check_failed';
        return telemetry.rows;
      },
      preview: api.telemetryPreview,
      apply: api.telemetryApply,
    }, (progress) => {
      telemetrySetup.completed = progress.completed;
      telemetrySetup.total = progress.total;
      telemetrySetup.results = progress.results;
      rememberApplied(progress.results);
    });
  } catch (error) {
    telemetrySetup.error = telemetryFailureKey(error);
  } finally {
    await checkTelemetry(true);
    telemetrySetup.busy = false;
    telemetrySetup.batch = false;
  }
}

export async function prepareTelemetry(id: string): Promise<void> {
  if (telemetrySetup.busy) return;
  telemetrySetup.busy = true; telemetrySetup.error = ''; telemetrySetup.preview = null;
  try { telemetrySetup.preview = await api.telemetryPreview(id); }
  catch (error) { telemetrySetup.error = telemetryFailureKey(error); }
  finally { telemetrySetup.busy = false; }
}

export async function applyPreparedTelemetry(): Promise<void> {
  const preview = telemetrySetup.preview;
  if (!preview || telemetrySetup.busy) return;
  telemetrySetup.busy = true; telemetrySetup.error = '';
  try {
    await api.telemetryApply(preview.token);
    const result: TelemetryResult = { target: preview.target, status: 'applied', preview };
    telemetrySetup.results = [result];
    rememberApplied([result]);
    telemetrySetup.preview = null;
    await checkTelemetry(true);
  } catch (error) { telemetrySetup.error = telemetryFailureKey(error); }
  finally { telemetrySetup.busy = false; }
}

export async function undoTelemetry(id: string): Promise<void> {
  const applied = telemetrySetup.applied[id];
  if (!applied || telemetrySetup.busy) return;
  telemetrySetup.busy = true; telemetrySetup.error = '';
  try {
    const conflicts = await api.telemetryUndo(applied.token);
    delete telemetrySetup.applied[id];
    telemetrySetup.results = telemetrySetup.results.filter((r) => r.target.id !== id);
    if (conflicts.length) telemetrySetup.error = 'telemetry.reason.config_changed';
    await checkTelemetry(true);
  } catch (error) { telemetrySetup.error = telemetryFailureKey(error); }
  finally { telemetrySetup.busy = false; }
}
