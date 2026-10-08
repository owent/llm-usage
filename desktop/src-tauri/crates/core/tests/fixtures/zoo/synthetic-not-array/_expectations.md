# synthetic-not-array expectations (synthetic)

<a id="synthetic-not-array-期望全合成"></a>

The top level is not a JSON array, required by taskMessages.ts.

Expected: detect returns UnknownFormat because the file does not begin with `[`.
Scanning does not read it; no events.
