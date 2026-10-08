# Cline VS Code 4.1.22 native SDK sample

<a id="cline-vs-code-4122-真实-sdk-样本"></a>

The official unmodified VSIX called a local model through public startNewTask in an
isolated VS Code GUI. Native JSON retains only envelope/id/role/ts/modelInfo/metrics/known
metadata; session/message IDs are anonymized and bodies/configuration omitted. Three native
metrics match independent API results individually: input=8,922, output=48, total=8,970;
positive cache-read sum=5,881. Default-zero cache write/first cache read stay unknown;
derive no uncached input. Metrics may combine runs/retries; import three observations,
with calls unknown. Mutable session version metadata does not verify historical messages,
so use latest_fallback. This sample verifies neither task completion nor real legacy-UI data.

provenance.json records original/distribution/model digests, environment and scope.
api-usage.json contains independent responses. Execution details are in the
[Cline record](../../../../../../../../docs/validation/desktop-usage/cline-container-sample.md).
