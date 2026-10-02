import type { TelemetryTargetDto, TelemetryPreviewDto } from './api';

export type TelemetryResult = {
  target: TelemetryTargetDto;
  status: 'applied' | 'failed' | 'manual';
  errorKey?: string;
  preview?: TelemetryPreviewDto;
};

export type TelemetryBatchProgress = {
  completed: number;
  total: number;
  results: TelemetryResult[];
};

type Operations = {
  check: () => Promise<TelemetryTargetDto[]>;
  preview: (id: string) => Promise<TelemetryPreviewDto>;
  apply: (token: string) => Promise<void>;
};

/** Return only localizable codes; never display raw configuration or OS error text. */
export function telemetryFailureKey(error: unknown): string {
  if (error === 'check_failed') return 'telemetry.checkFailed';
  const known = ['config_changed', 'read_only', 'receiver_bind_failed', 'receiver_restart_required',
    'preview_expired', 'environment_override', 'managed_policy', 'existing_destination',
    'unsafe_path', 'unsupported_version', 'telemetry_disabled', 'sync_conflict'];
  return typeof error === 'string' && known.includes(error)
    ? 'telemetry.reason.' + error : 'telemetry.writeFailed';
}

/** Prepare immediately before each apply: profiles may share the same settings file. */
export async function configureTelemetryBatch(
  operations: Operations,
  onProgress: (progress: TelemetryBatchProgress) => void = () => {},
): Promise<TelemetryResult[]> {
  const targets = await operations.check();
  const ready = targets.filter((row) => row.status === 'missing' && row.configurable);
  const results: TelemetryResult[] = targets
    .filter((row) => row.status === 'blocked' && !ready.includes(row))
    .map((target) => ({ target, status: 'manual' }));
  let completed = 0;
  const progress = () => onProgress({ completed, total: ready.length, results: [...results] });
  progress();
  for (const target of ready) {
    try {
      const preview = await operations.preview(target.id);
      await operations.apply(preview.token);
      results.push({ target, status: 'applied', preview });
    } catch (error) {
      results.push({ target, status: 'failed', errorKey: telemetryFailureKey(error) });
    }
    completed++;
    progress();
  }
  return results;
}
