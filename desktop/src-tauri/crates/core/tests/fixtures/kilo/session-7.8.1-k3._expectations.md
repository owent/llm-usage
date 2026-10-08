# session-7.8.1-k3._expectations.md expected values (independent calculation)

<a id="session-781-k3_expectationsmd-期望值独立核算"></a>

Source: one locally read kilo.db session with session.version=7.8.1. IDs, paths and bodies
are removed; tokens, timestamps, finish, modelID and providerID are retained. Extraction
script: `build/dashboard-fixes/extract_kilo_781.py`.

Calculation: sum each assistant message in the anonymized JSON. Total is
input+output+reasoning+cache.read+cache.write, all mutually exclusive. The verified 7.8.1
format uses top-level modelID/providerID and shares an implementation with 7.4.8/7.4.9;
this does not verify other 7.8.x releases. On 2026-10-03, a read-only SQLite Online Backup
rechecked this version's native assistant field shapes and sums below, avoiding WAL
omissions caused by immutable reads.

- session anon-1, ver=7.8.1, main(primary): assistant_msgs=34, plus one user; records_seen=35.
  input=125298, output=6637, reasoning=37853, cache_read=3019520, cache_write=0,
  derived_total=3189308, reported_total_sum=3189308, missing_total_msgs=0,
  finish=tool-calls, no error. snapshot(in+out+reason+cr+cw)=3189308,
  reconcile=matched, models={'k3-256k': 34}.

Derived values under map_kilo:

- input_total = input + cache_read + cache_write = 125298 + 3019520 + 0 = 3144818
- output_total = output + reasoning = 6637 + 37853 = 44490
- total_tokens = input_total + output_total = 3144818 + 44490 = 3189308
- cache_read_known = 3019520
