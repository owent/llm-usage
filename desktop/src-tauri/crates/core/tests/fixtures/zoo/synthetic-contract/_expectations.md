# synthetic-contract expectations (all synthetic)

<a id="synthetic-contract-期望全合成"></a>

Scenario: one task file, two api_req_started/finished pairs merged LIFO, one
condense_context contributing cost, and one unmatched started with no usage numbers.

Manually calculated expectations under consolidateApiRequests/consolidateTokenUsage:

- Three usage_events; daily call_count=3 on 2026-06-13:
  - syn-zoo-1:api_req_started:1781337600500: primary, time_basis=source_start.
    Merged text has {request, apiProtocol, tokensIn:1000, tokensOut:200,
    cacheWrites:100, cacheReads:8000, cost:0.012}. Fixed-source comments say tokensIn
    **includes cache**. Thus input_total=1000 (reported), input_cache_read=8000,
    input_cache_write=100, input_uncached=NULL, output_total=200, total_tokens=1200
    (in+out, upstream contextTokens arithmetic), cost=12000 micro-USD (estimated).
  - syn-zoo-1:condense_context:1781338000000: auxiliary, time_basis=uncertain.
    Tokens unknown: contextCondense has no token fields and newContextTokens measures
    context size rather than billed usage. cost=3000 micro-USD.
  - syn-zoo-1:api_req_started:1781339000000: primary. Merged tokensIn=500,
    cacheReads=9000, cacheWrites=0, tokensOut=80, cost=0.006 give input_total=500,
    total_tokens=580, cost=6000 micro-USD.
- Unmatched started at ts=1781339500000 has no token numbers: no event and one
  usage_carrier_without_numbers diagnostic.
- api_req_finished creates no separate event; its usage is already merged into started.
- Daily summary (2026-06-13 UTC): call_count=3 (two primary, one auxiliary),
  input_total=1500, cache_read=17000, cache_write=100, output=280, total_tokens=1780.
- No latest_fallback: the documentation-based format identifier always uses KnownVersion.
  Exactly one usage_carrier_without_numbers diagnostic.
- Repeat scanning leaves call_count=3.
