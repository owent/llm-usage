# Zoo 3.86.0 native container usage extraction

<a id="zoo-3860-真实容器用量字段提取"></a>

Official VSIX at fixed commit 6aa9d0174a9ecae155c6c5db9134bead4b67197d. Independent
VS Code 1.140.0 called a local model through the public extension API. Preserve five
messages' type/say/ask, time, partial and verified usage fields; remove bodies, configuration
and original task ID. api-usage.json is the independent service counter;
extension-usage.json is the public usage callback.

One observed main request: input=6,118, output=53, derived total=6,171. Native cache/cost
zeros are initialized defaults and stay unknown. Uncached/reasoning/source total/model/provider
are unknown. Ordinary text/reasoning/resume prompts are not calls. The real task was publicly
cancelled after receiving usage; no task-success claim. Historical migration tests use a
separate synthetic usage-only extract to represent messages consumed by doc1, without
claiming the old reader accepts this complete native array.
