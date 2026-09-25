# session-7.4.9-family._expectations.md 期望值（独立核算）

核算方法：直接对脱敏 JSON 的 assistant 消息逐条求和；
total 口径 = input+output+reasoning+cache.read+cache.write（全互斥）。

- session anon-1 ver=7.4.9 main(primary): msgs=37 input=89396 output=7318 reasoning=24825 cache_read=1863424 cache_write=0 derived_total=1984963
  reported_total_sum=1984963 | snapshot(in+out+reason+cr+cw)=1984963.0 reconcile=matched models={'glm-5.2': 37}

- session anon-3 ver=7.4.9 child(sub_agent): msgs=2 input=10044 output=234 reasoning=2711 cache_read=27008 cache_write=0 derived_total=39997
  reported_total_sum=39997 | snapshot(in+out+reason+cr+cw)=39997.0 reconcile=matched models={'glm-5.2': 2}

- session anon-4 ver=7.4.9 child(sub_agent): msgs=6 input=15593 output=772 reasoning=11904 cache_read=138816 cache_write=0 derived_total=167085
  reported_total_sum=167085 | snapshot(in+out+reason+cr+cw)=167085.0 reconcile=matched models={'glm-5.2': 6}

- session anon-5 ver=7.4.9 child(sub_agent): msgs=3 input=3359 output=444 reasoning=3642 cache_read=54144 cache_write=0 derived_total=61589
  reported_total_sum=61589 | snapshot(in+out+reason+cr+cw)=61589.0 reconcile=matched models={'glm-5.2': 3}

- session anon-6 ver=7.4.9 child(sub_agent): msgs=3 input=3382 output=108 reasoning=2220 cache_read=54080 cache_write=0 derived_total=59790
  reported_total_sum=59790 | snapshot(in+out+reason+cr+cw)=59790.0 reconcile=matched models={'glm-5.2': 3}

- 合计: assistant msgs=51 input=121774 output=8876 reasoning=45302 cache_read=2137472 cache_write=0 derived_total=2313424 reported_total_sum=2313424
  missing_total_msgs=1
