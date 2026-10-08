# Writing guidance: remove formulaic AI prose

<a id="写作指导去除-ai-腔"></a>

Use when drafting or editing repository prose: replies, comments, documentation,
examples, agent instructions, commit messages, and PR or release notes.

Lead with the fact, result or action the reader needs, then explain necessary reasons.
Removing empty prose does not identify an author's identity or justify slang, marketing
metaphors or omitted technical conditions as a way to sound natural.

<a id="内容与结构"></a>

## Content and structure

- Describe currently verifiable behavior. Distinguish unknowns, plans and completed work;
  never turn unverified statements into certainty. Keep current rules/status in documentation
  by integrating changes rather than appending revision dates or update logs. Git/backups
  preserve change history. Continue explaining compatible old behavior, versions, keys and APIs.
- Give each paragraph one subject with concrete objects/verbs. Use one name per concept;
  do not vary terms merely for style.
- Remove empty openings, section previews, paragraph summaries and repeated conclusions.
  Keep numbered steps when sequence matters. Fields, options and parallel checks suit lists/
  tables; otherwise prefer connected prose.
- Avoid praising designs with adjectives. State files, conditions, quantities, times and
  measured results. Remove unmeasured performance claims; do not invent numbers to replace
  words such as efficient.
- Technical explanations lead with changes/impact, then evidence. PR descriptions serve
  reviewers who have not read the conversation: include the actual problem, final behavior
  and necessary validation, without discussion history or unrelated alternatives.

<a id="标题宣传与导航"></a>

## Titles, promotion and navigation

Titles use short, formal subject terms naming the section's object/function. Put conditions,
steps and explanations in the body. Remove empty adjectives, promotional claims such as
`一文读懂`/`全网最全`, two-clause slogans and final periods. Do not force line breaks to support
long titles. There is no universal length cap; check both languages/narrow screens for readability.

Homepage copy, feature cards and other promotion must avoid `从……到……` coverage slogans.
Titles avoid `先……再……` and their English From … to …/First … then … slogans; do not replace
them with equivalent `由……走向……` patterns. Invitations/abstract actions such as `看一眼`,
`跑通`, `看清` or `掌握` are unsuitable formal titles. Preserve actual operation order,
input/output relationships and numeric ranges in the body with their objects/conditions;
similar sentence shapes do not justify deleting necessary meaning.

Capabilities must name functions: identify what direct expression expresses, what actions
a clear workflow includes, and what supports reliability. Replace vague praise with
structures, inputs, operations and outcomes rather than different adjectives. Summaries
cannot imply unverified performance/comprehensive support. Links/buttons use destination names.

When reviewing user-identified wording issues, check titles, summaries, cards, buttons,
captions and repeated patterns in neighboring docs/navigation/footers. Write natural titles/
explanations independently in each language. Do not preserve English slogans for literal
equivalence or bulk-rewrite ordinary technical sequences.

<a id="中文表达"></a>

## Chinese wording

In engineering prose, replace the vague or metaphorical uses below with the actual object,
operation or result. This applies to titles, navigation, captions, comments and agent
instructions as well as paragraphs. A search match starts a contextual review; it does not
authorize changing identifiers, original output or the precise technical uses listed under
Exceptions and checks.

| Expression to review | Rewrite according to actual meaning |
| --- | --- |
| `赋能、加持、助力、打造` | Name who provides which function or performs which action |
| `抓手、底座、打通` | Name tools, shared libraries, services/interfaces and their connections |
| `沉淀、落地、闭环` | Record conclusions, save data, implement, deploy, fix and verify |
| `对齐、拉齐、收敛` | Specify what remains consistent, is merged/unified, or has its scope narrowed |
| `口径、维度、颗粒度` | Specify calculation rules, time sources, conditions, metrics or processing units |
| `链路、投影` | State call order, cache/copy/derived view |
| `水位` | Use processing position (maximum processed timestamp/sequence), old/consumed/retained position as appropriate; avoid `进度` because it can mean displayed collection progress |
| `终态、终值` | For jobs/state machines, state completion states/legal transitions; for snapshots, state final values/snapshots |
| `夹具` | Use test data for static files consumed by tests; retain fixture for general xUnit setup |
| `秘密、机密` | Specify keys, credentials, passwords or sensitive data; do not translate secret mechanically as `秘密` |
| `预算` | For execution limits, state retry counts, remaining attempts, timeouts or quantity caps |
| `合同` in development docs/plans | Use design specifications, interface conventions, field rules, platform protocols or acceptance criteria as appropriate; preserve legal contracts, quotations and identifiers |
| `缺证、待证、未证` | State missing samples/checks, unverified local formats or pending compatibility; unverified does not mean unsupported |
| `证据` in engineering explanations | Name source/documentation basis, samples, logs, test/measurement results or records, and the conclusion they support |
| `门槛` | State prerequisites, adoption conditions, versions, acceptance criteria, thresholds or limits; preserve exact comparisons and required/advisory strength |
| `兜底、护栏、门禁、钳制` | State trigger/action: default return, rejection, retry, numeric restriction or merge prevention |
| `全面、深度、系统性、全方位` | Name actual files, scenarios and methods without implying unperformed work |
| `神器、利器、干货、保姆级` | State functions, steps and applicability |
| `强大、无缝、优雅、高效、稳健、轻松` | State prerequisites/verified behavior; remove unsupported praise |
| `载体、面、落点、权威` used without a concrete object | Name the file/database/export, client interface, target file/section or primary maintained document |
| `仲裁、认证、封存` used without explaining data operations | Describe conflict selection, the version/format checked, or the retained archive partition; preserve actual database meanings |
| `热、冷` used without storage context | Identify active transcripts or archived files, and say which the parser reads |

State facts directly and reduce empty signposting such as `值得注意的是`, `需要指出的是`,
`显然` or `毋庸置疑`. Remove fact-free `本质上`, `从某种意义上说` and inflated
`对……进行……处理`. Avoid unnecessary denial-then-affirmation patterns such as `不是 A，而是 B`,
`并非 A，而是 B` and `不在于 A，而在于 B`. State conclusions directly when negation only
adds emphasis; retain genuine corrections, distinctions and exclusions. Do not invent opposing
views or present compatible claims as mutually exclusive. State evidence/priority when emphasizing.

In common Chinese compound-sentence classification, `不是……而是……` is parallel contrast,
denying the first claim and affirming the second. `不仅……而且……` and `不只是……更……`
are progressive: the first claim remains true, so retain both when simplifying. Use `但`,
`然而` and `不过` for actual contrast/concession; do not remove normal contrasts.
Assess added information without frequency/ratio/count quotas. Keep necessary negation,
conditions and technical limits. Body-text `从 X 到 Y` ranges must involve like objects
and actual coverage, without forced triads; promotional/title slogans follow the earlier section.

Remove era-setting openings such as `随着……的发展` or `在……时代` and redundant
`换句话说`/`也就是说` restatements. Name the subject immediately. Vague attribution
such as `众所周知`/`不言而喻` needs a source or removal. Rhetorical `你是否遇到过……？`
does not replace a problem statement. Without meaningful sequence, avoid
`首先/其次/最后` chains. Give the conclusion directly instead of `答案是肯定的`.

Validation descriptions identify objects, methods, results and scope. Unverified means checks
are unfinished; missing real samples means particular inputs are absent; failed validation
means completed checks did not meet criteria. These are not interchangeable. Source/docs
support implementation, synthetic tests check parsing, and real samples check local formats.
Neither substitutes the others or certifies unchecked versions/environments. Preserve condition
strength, versions, quantities and comparison direction rather than replacing them with vague requirements.

<a id="英文表达"></a>

## English wording

Prefer concrete verbs and familiar nouns. Review words such as delve, leverage,
seamless, robust, streamline, unlock, elevate, empower, cutting-edge, game-changer,
foster, facilitate, and utilize when they replace a specific action or evidence.
Keep the established technical meaning when it is the accurate term.

Review engineering uses of contract, evidence, fixture, projection, gate, carrier, surface,
certify and budget as carefully as their Chinese translations. Name specifications or field
rules, source references or test results, test data, cached/derived results, required checks,
files or databases, client interfaces, verified formats, and time/quantity limits where these
are the actual meanings. Do not turn an already vague Chinese expression into English jargon.
Legal contracts, test-framework fixtures, mathematical projections and actual spending budgets
retain their established meanings. For usage/cost notifications, prefer usage and cost alerts;
keep existing setting keys and quoted UI labels unchanged.

Remove staged or rhetorical openings such as “Let's dive in”, “In today's world”, and
“Imagine a world where …”, unnecessary Additionally/Moreover/Furthermore, vague attribution,
stacked hedges, and unsupported praise.
Prefer is/are/has over inflated uses of serves as, boasts, and features.
Cut trailing highlighting/underscoring/ensuring clauses if they add no fact.
Keep one name per concept; avoid forced triads, false ranges, and decorative bold labels.
Use sentence-case headings. Dashes and real contrasts are useful only when the meaning needs them.
Avoid unnecessary “not X but Y” reframing. State Y directly when rejecting X adds no information.
Keep genuine corrections, distinctions, and exclusions; do not frame compatible claims as mutually exclusive.
Preserve both claims when simplifying “not only X but also Y”. Use but, however, and yet according to meaning.
Review the information a sentence adds, without imposing a word ban or frequency quota.

<a id="保留语义的例子"></a>

## Examples that preserve meaning

| Original example | Revision |
| --- | --- |
| `全面打通 Skill 加载链路，形成验证闭环。` | Check Skill discovery from the root/subdirectories and record failures |
| `耗尽重试预算后触发兜底。` | Return an error after the retry limit, only if code actually does so |
| `缺证 IDE 后移 F1。` | Put IDEs with unverified local usage formats in the F1 follow-up plan |
| `取得证据后更新验收状态。` | Update acceptance status after completing/checking the corresponding result |
| `源码证据不能代替桌面证据。` | Source verification does not replace native desktop acceptance |
| `MCP 采用门槛。` | MCP adoption conditions |
| `三次试验不是产品硬门槛。` | Three trials are advisory; the product does not require three |
| `随着 AI 编程工具的普及，用量统计变得越来越重要。` | Remove the background and state collection objects/rules directly |
| `从日志到看板，一站式掌握用量。` | Name the subject, such as Usage dashboard; describe scope/steps in the body |
| `本节不是罗列功能，而是介绍配置步骤。` | State This section explains configuration steps unless exclusion needs clarification |
| `本工具不仅能扫描日志，更能生成报表。` | State This tool scans logs and generates reports, preserving both capabilities |
| `通过本节，你可以轻松了解项目的强大规则体系。` | Remove the introduction and state rules/applicability |
| This robust workflow seamlessly ensures documentation quality. | Run npm run lint:md to check Markdown formatting |

Rewrites must preserve whether the first attempt counts, reset timing and inclusive endpoints.
Retry, default return, input rejection and numeric truncation are different operations;
do not replace them all with handling errors.

<a id="例外与检查"></a>

## Exceptions and checks

Preserve identifiers, configuration keys, API/protocol text, error messages, quotations and
proper names. Keep accurate technical terms for memory alignment, tensor dimensions, network
links/tracing, algorithm convergence, closed-loop control, statistical significance, financial
budgets, SRE error budgets and automata final states. Evidence remains appropriate in legal/
forensic/proof discussions; thresholds remain appropriate for actual entry difficulty.
Words may be quoted when explaining them.

Historical records may change explanatory wording while retaining dates, versions, commands,
results and unexecuted checks. Preserve original inputs, quotations and actual program output.
Source verification times, sample coverage dates, price effective dates and model alias dates
have technical purposes; do not delete them as revision history. Apply the same guidance to
comments while preserving identifiers/executable content. Genuine corrections may remain:
`这里统计的不是请求次数，而是消息条数` distinguishes count units. Normal contrast may remain:
`调用成功，但日志未完整写入。` distinguishes call results/log state.

Review repetition, long modifiers, passive voice, unfamiliar abbreviations, unsupported judgments
and mechanical openings/endings. For denial/affirmation, ask whether negation adds information,
claims truly conflict or emphasis invents an incorrect view. After direct rewrites, verify that
necessary exclusions/first-claim facts remain clear; reorganize repeated sentence patterns.
Search matches only locate issues; words alone do not fail lint or identify AI authorship.
Confirm terms, facts, permission boundaries, conditions and unverified items survive shortening,
then check formatting/links.

<a id="编辑流程"></a>

## Editing workflow

1. Mark essential facts, terms, positions, quotations and formatting. Separate verified,
   inferred and unknown claims. With insufficient material, write supported content/questions
   instead of inventing numbers, sources, experiences or check results.
2. Fix facts/arguments, then structure, then grammar/wording. Decisions lead with judgments;
   tutorials explain prerequisites/steps/results; derivations/reviews retain useful process.
   Do not force three points or equal-length sections.
3. Name objects/actions/conditions/results. Replace empty nouns with accurate verbs and remove
   empty introductions/transitions, exaggerated praise and repeated conclusions.
4. Correct the prohibited vague uses in context, without blind global replacements. Links/buttons name
   destinations. Preserve technical order, ranges, corrections/exclusions, both progressive
   claims and consistent terminology.
5. Compare negation, conditions, quantities, units, timing, causation, commitment strength and
   verification status. Both languages fully express the same meaning in natural order.
   Check tool-accepted keywords when translating example labels and synchronize inputs/resources.
   Handle functional defects separately rather than changing behavior during prose editing.
6. Read relevant paragraphs/full-text patterns and change only defects. Stop/revert if a pass
   offers no explainable improvement or loses meaning/author traits. Typos, random sentence
   lengths and detector scores do not create natural prose.

Translate user, architecture and development documents directly with the current language model.
AI rules, Skills and execution plans keep a single original and need no translation.
Compare each complete paired document, including tables, links, examples
and failure conditions. Review the Chinese wording as well as the English wording, and translate
required sections fully rather than leaving English paragraphs in the Chinese edition.
Record reviewed pairs only after this comparison; a translation service, automated substitution
or matching file hashes does not establish completeness or natural wording.

<a id="参考与取舍"></a>

## References and choices

Adapted for this repository from sibling AICodeReviewer/atsf4g-co
`.agents/skills/ai-agent-maintenance/references/writing-guidance.md`;
see the [source index](source-index.md#本地写作参考).
Adopt concrete wording, paragraph structure, semantic exceptions and manual review.
Do not import other projects' banned-word scripts, business paths or claims identifying
AI authors from Chinese vocabulary. This file contains the needed guidance;
clients need not access sibling repositories.
