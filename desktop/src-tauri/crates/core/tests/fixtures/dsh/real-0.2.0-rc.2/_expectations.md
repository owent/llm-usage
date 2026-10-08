# DSH 0.2.0-rc.2 v4 native sample with selected fields

<a id="dsh-020-rc2-v4-真实样本的字段提取"></a>

<a id="dsh-020-rc2-v4-真实按允许字段提取"></a>

The official npm CLI called a real local model in an isolated container with networking
disabled. Both CLI runs exited 0. The native compressed file was 17,575 bytes and the
decompressed file 51,114 bytes; provenance.json records their digests. This test sample
retains only the header, event envelope, turn/step, timestamps, anonymized identities,
model and usage. It excludes content, reasoning text, request configuration, tool arguments
and original directories. Ordinary non-usage events retain only permitted data fields;
this extraction does not verify the complete DSH recovery format.

| Sample | Uncached input | Cache read | Total input | Output | Total |
| --- | --- | --- | --- | --- | --- |
| Main loop 1 | 5,572 | Unknown | 5,572 | 64 | 5,636 |
| Main loop 2 | 34 | 5,568 | 5,602 | 41 | 5,643 |
| Known sum | 5,606 | 5,568 | 11,174 | 105 | 11,279 |

There are two calls. Do not add stream usage from the same settlement. pi-ai calculates
total itself, so source_total is unknown; cache write, reasoning and cost are unknown.
The title API used another 205 tokens, but native data has only the request marker and
no result usage: do not invent a third event. Separate synthetic mutation tests cover
inheritance, retries, invalid types, limits, incomplete writes and rollback. Those paths
have no claimed native samples. Reading the actual native zstd output is recorded separately.
