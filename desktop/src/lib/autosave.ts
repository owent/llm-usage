export type AutosaveStatus = 'idle' | 'pending' | 'saving' | 'saved' | 'error';

export function createAutosave<T>(options: {
  initialKey: string;
  read: () => T;
  save: (value: T) => Promise<void>;
  onSaved: (value: T) => void;
  onStatus: (status: AutosaveStatus, error?: unknown) => void;
  delay?: number;
}) {
  let key = options.initialKey;
  let savedKey = key;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let running: Promise<void> | undefined;
  let writingKey = key;
  let requested = false;
  let composing = false;
  let disposed = false;
  let saved = false;
  let error: unknown;

  function cancelTimer() {
    clearTimeout(timer);
    timer = undefined;
  }

  function publish() {
    if (disposed) return;
    options.onStatus(error !== undefined ? 'error' : running ? 'saving'
      : key !== savedKey ? 'pending' : saved ? 'saved' : 'idle', error);
  }

  function schedule() {
    cancelTimer();
    if (!composing && (key !== savedKey || running)) {
      timer = setTimeout(() => void flush(), options.delay ?? 600);
    }
  }

  function edit(nextKey: string) {
    if (disposed || nextKey === key) return;
    key = nextKey;
    error = undefined;
    requested = false;
    schedule();
    publish();
  }

  async function flush(retry = false): Promise<boolean> {
    cancelTimer();
    if (disposed || composing) return false;
    if (error !== undefined && !retry) return false;
    requested ||= !running || key !== writingKey;
    if (!running) {
      writingKey = key;
      running = Promise.resolve().then(async () => {
        while (requested && !disposed && !composing) {
          requested = false;
          if (key === savedKey) break;
          writingKey = key;
          error = undefined;
          publish();
          try {
            const value = options.read();
            await options.save(value);
            savedKey = writingKey;
            saved = true;
            options.onSaved(value);
          } catch (failure) {
            if (key === writingKey) error = failure;
          }
        }
      }).finally(() => {
        running = undefined;
        publish();
      });
      publish();
    }
    await running;
    return key === savedKey && error === undefined;
  }

  function composition(active: boolean) {
    composing = active;
    if (active) {
      cancelTimer();
      requested = false;
    } else schedule();
  }

  function dispose() {
    disposed = true;
    cancelTimer();
  }

  return { edit, flush, composition, dispose };
}
