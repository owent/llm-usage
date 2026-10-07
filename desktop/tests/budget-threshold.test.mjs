import assert from 'node:assert/strict';
import test from 'node:test';
import { budgetThreshold, budgetDisplay } from '../src/lib/budget-threshold.ts';

test('budget decimal input preserves micro amounts and rejects overflow or fraction tokens', () => {
  assert.equal(budgetThreshold('0.000001','estimated_cost'),'1');
  assert.equal(budgetThreshold('100.123456','estimated_cost'),'100123456');
  assert.equal(budgetDisplay('100123456','estimated_cost'),'100.123456');
  assert.equal(budgetThreshold('9007199254740993','total_tokens'),'9007199254740993');
  for (const [raw,metric] of [['1.5','total_tokens'],['0','total_tokens'],['0.0000001','estimated_cost'],['9223372036854775808','total_tokens']]) {
    assert.throws(() => budgetThreshold(raw,metric));
  }
});
