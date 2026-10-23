import test from 'node:test';
import assert from 'node:assert/strict';
import { createAutosave } from '../src/lib/autosave.ts';

const settle = async () => { for (let i = 0; i < 12; ++i) await Promise.resolve(); };
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function fixture(t, save = async () => {}) {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  let value = 'initial';
  const writes = [], acknowledgements = [], statuses = [];
  let invalid = false;
  const autosave = createAutosave({
    initialKey: value,
    read: () => { if (invalid) throw new Error('invalid draft'); return { value }; },
    save: async (snapshot) => { writes.push(snapshot); await save(snapshot); },
    onSaved: (snapshot) => acknowledgements.push(snapshot),
    onStatus: (status, error) => statuses.push({ status, error }),
  });
  t.after(() => autosave.dispose());
  return { autosave, writes, acknowledgements, statuses,
    edit(next) { value = next; autosave.edit(next); },
    invalidate(active) { invalid = active; },
  };
}

test('opening and unchanged flushes do not write; text edits debounce the latest snapshot', async (t) => {
  const f = fixture(t);
  assert.equal(await f.autosave.flush(), true);
  assert.deepEqual(f.writes, []);
  f.edit('a'); t.mock.timers.tick(500); await settle();
  f.edit('ab'); t.mock.timers.tick(599); await settle();
  assert.deepEqual(f.writes, []);
  t.mock.timers.tick(1); await settle();
  assert.deepEqual(f.writes, [{ value: 'ab' }]);
  assert.equal(f.statuses.at(-1).status, 'saved');
  f.edit('ab'); assert.equal(await f.autosave.flush(), true);
  assert.equal(f.writes.length, 1);
});

test('blur, selection and navigation flush without waiting or duplicating a write', async (t) => {
  const pending = deferred();
  const f = fixture(t, () => pending.promise);
  f.edit('choice');
  const first = f.autosave.flush();
  const second = f.autosave.flush();
  await settle();
  assert.deepEqual(f.writes, [{ value: 'choice' }]);
  pending.resolve();
  assert.deepEqual(await Promise.all([first, second]), [true, true]);
  t.mock.timers.tick(600); await settle();
  assert.equal(f.writes.length, 1);
});

test('slow writes serialize and preserve newer edits with immutable snapshots', async (t) => {
  const pending = deferred();
  let active = 0, peak = 0;
  const f = fixture(t, async () => { peak = Math.max(peak, ++active); if (f.writes.length === 1) await pending.promise; --active; });
  f.edit('older'); const first = f.autosave.flush(); await settle();
  f.edit('newer'); const second = f.autosave.flush(); await settle();
  assert.deepEqual(f.writes, [{ value: 'older' }]);
  pending.resolve();
  assert.deepEqual(await Promise.all([first, second]), [true, true]);
  assert.deepEqual(f.writes, [{ value: 'older' }, { value: 'newer' }]);
  assert.deepEqual(f.acknowledgements, f.writes);
  assert.equal(peak, 1);
});

test('reverting during a write persists the original value after the older write', async (t) => {
  const pending = deferred();
  const f = fixture(t, () => f.writes.length === 1 ? pending.promise : Promise.resolve());
  f.edit('temporary'); const first = f.autosave.flush(); await settle();
  f.edit('initial'); const second = f.autosave.flush(); pending.resolve();
  assert.deepEqual(await Promise.all([first, second]), [true, true]);
  assert.deepEqual(f.writes, [{ value: 'temporary' }, { value: 'initial' }]);
});

test('typing during a request keeps its remaining debounce after acknowledgement', async (t) => {
  const pending = deferred();
  const f = fixture(t, () => f.writes.length === 1 ? pending.promise : Promise.resolve());
  f.edit('first'); const first = f.autosave.flush(); await settle();
  f.edit('second'); t.mock.timers.tick(200); pending.resolve();
  assert.equal(await first, false);
  assert.equal(f.statuses.at(-1).status, 'pending');
  t.mock.timers.tick(399); await settle(); assert.equal(f.writes.length, 1);
  t.mock.timers.tick(1); await settle(); assert.equal(f.writes.at(-1).value, 'second');
});

test('invalid and failed writes retain the draft and do not retry on a timer', async (t) => {
  let fail = true;
  const f = fixture(t, async () => { if (fail) throw new Error('storage unavailable'); });
  f.invalidate(true); f.edit('incomplete');
  assert.equal(await f.autosave.flush(), false);
  assert.equal(f.writes.length, 0);
  assert.match(f.statuses.at(-1).error.message, /invalid/);
  t.mock.timers.tick(60_000); await settle(); assert.equal(f.writes.length, 0);
  f.invalidate(false); f.edit('complete');
  const first = f.autosave.flush(), duplicate = f.autosave.flush();
  assert.deepEqual(await Promise.all([first, duplicate]), [false, false]);
  t.mock.timers.tick(60_000); await settle(); assert.equal(f.writes.length, 1);
  assert.equal(await f.autosave.flush(), false);
  assert.equal(f.writes.length, 1);
  fail = false;
  assert.equal(await f.autosave.flush(true), true);
  assert.equal(f.writes.at(-1).value, 'complete');
});

test('a failed stale write does not discard a newer queued snapshot', async (t) => {
  const pending = deferred();
  const f = fixture(t, () => f.writes.length === 1 ? pending.promise : Promise.resolve());
  f.edit('old'); const first = f.autosave.flush(); await settle();
  f.edit('new'); const next = f.autosave.flush();
  pending.reject(new Error('old request failed'));
  assert.deepEqual(await Promise.all([first, next]), [true, true]);
  assert.deepEqual(f.acknowledgements, [{ value: 'new' }]);
});

test('IME defers blur and debounce until composition ends; dispose cancels timers', async (t) => {
  const f = fixture(t);
  f.autosave.composition(true); f.edit('组词');
  t.mock.timers.tick(6000); await settle();
  assert.equal(await f.autosave.flush(), false); assert.deepEqual(f.writes, []);
  f.autosave.composition(false); t.mock.timers.tick(600); await settle();
  assert.deepEqual(f.writes, [{ value: '组词' }]);
  f.edit('unsent'); f.autosave.dispose(); t.mock.timers.tick(600); await settle();
  assert.equal(f.writes.length, 1);
});
