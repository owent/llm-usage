# synthetic-not-array 期望（全合成）

<a id="synthetic-not-array-expectations-synthetic"></a>

场景：顶层不是 JSON 数组（taskMessages.ts 要求顶层数组）。

期望：

- detect 直接 UnknownFormat（文件头不以 `[` 开头）；
  扫描层不接触该文件；0 事件。
