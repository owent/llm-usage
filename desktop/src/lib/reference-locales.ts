const keys=['dashboard.priceReference','cost.unitPrices','cost.unitPriceHint','cost.noUnitPrice','cost.cacheWrite5m','cost.cacheWrite1h','cost.contextTier','chart.totalMissing','cost.detailLimited','quota.details'];
const phrases:Record<string,string[]>={
  'zh-CN':['采用当前可用价目，分币种展示；仅为 API 参考。','查看模型单价','每百万 token；展示实际匹配价目及上下文档位，— 表示该项单价缺失。','暂无适用单价','缓存写入 · 5分钟','缓存写入 · 1小时','输入上下文 ≥ {count} token','完整总 token 未提供','部分历史明细已过期，参考金额仅覆盖仍保留的明细。','额度详情'],
  'zh-TW':['採用目前可用價目，分幣種顯示；僅供 API 參考。','查看模型單價','每百萬 token；顯示實際匹配價目與上下文級距，— 表示該項單價缺失。','暫無適用單價','快取寫入 · 5分鐘','快取寫入 · 1小時','輸入上下文 ≥ {count} token','未提供完整總 token','部分歷史明細已過期，參考金額僅涵蓋保留的明細。','額度詳情'],
  en:['Current available rates, per currency; API reference only.','View model unit prices','Per million tokens; matched rates and context tiers. — means no rate for this component.','No applicable unit price','Cache write · 5 min','Cache write · 1 hour','Input context ≥ {count} tokens','Complete total tokens not provided','Some historical details have expired; reference amounts cover retained details only.','Quota details'],
  ja:['現在利用できる料金を通貨別に表示。API の参考額です。','モデル単価を見る','100万 token あたり。実際に適用した料金とコンテキスト区分を表示。— は単価なし。','適用できる単価なし','キャッシュ書込 · 5分','キャッシュ書込 · 1時間','入力コンテキスト ≥ {count} token','完全な合計 token は未提供','過去の一部明細は期限切れです。参考額は保持された明細のみ対象です。','枠の詳細'],
  ko:['현재 사용 가능한 요금을 통화별로 표시합니다. API 참고 금액입니다.','모델 단가 보기','백만 token당 실제 적용 요금과 컨텍스트 구간입니다. —는 해당 단가가 없음을 뜻합니다.','적용 가능한 단가 없음','캐시 쓰기 · 5분','캐시 쓰기 · 1시간','입력 컨텍스트 ≥ {count} token','전체 token 합계가 제공되지 않음','일부 과거 상세 기록이 만료되었습니다. 참고 금액은 보존된 기록만 포함합니다.','할당량 상세'],
  es:['Tarifas actuales disponibles por moneda; solo referencia de API.','Ver precios unitarios','Por millón de tokens; tarifas aplicadas y tramos de contexto. — indica una tarifa ausente.','Sin precio unitario aplicable','Escritura de caché · 5 min','Escritura de caché · 1 hora','Contexto de entrada ≥ {count} tokens','No se proporcionó el total completo de tokens','Algunos detalles históricos han caducado; los importes solo cubren los conservados.','Detalles de cuota'],
  fr:['Tarifs disponibles actuels par devise ; référence API uniquement.','Voir les prix unitaires','Par million de tokens ; tarifs appliqués et seuils de contexte. — indique un tarif absent.','Aucun prix unitaire applicable','Écriture cache · 5 min','Écriture cache · 1 heure','Contexte d’entrée ≥ {count} tokens','Total complet de tokens non fourni','Certains détails historiques ont expiré ; seuls les détails conservés sont couverts.','Détails du quota'],
  de:['Aktuell verfügbare Tarife je Währung; nur API-Referenz.','Modellpreise ansehen','Je Million Tokens; angewandte Preise und Kontextstufen. — bedeutet kein Preis für diesen Anteil.','Kein anwendbarer Einzelpreis','Cache-Schreiben · 5 Min.','Cache-Schreiben · 1 Std.','Eingabekontext ≥ {count} Tokens','Vollständige Token-Summe nicht angegeben','Einige historische Details sind abgelaufen; Beträge umfassen nur aufbewahrte Details.','Kontingentdetails'],
  'pt-BR':['Tarifas atuais disponíveis por moeda; apenas referência de API.','Ver preços unitários','Por milhão de tokens; tarifas aplicadas e faixas de contexto. — indica uma tarifa ausente.','Sem preço unitário aplicável','Gravação de cache · 5 min','Gravação de cache · 1 hora','Contexto de entrada ≥ {count} tokens','Total completo de tokens não informado','Alguns detalhes históricos expiraram; os valores cobrem apenas os registros preservados.','Detalhes da cota'],
  ru:['Текущие доступные тарифы по валютам; только справочная оценка API.','Посмотреть цены моделей','За миллион токенов; применённые тарифы и пороги контекста. — означает отсутствие тарифа.','Подходящего тарифа нет','Запись кэша · 5 мин','Запись кэша · 1 час','Входной контекст ≥ {count} токенов','Полное число токенов не предоставлено','Некоторые исторические записи удалены по сроку хранения; оценка охватывает только сохранённые записи.','Подробности квоты'],
};
const titles:Record<string,string>={'zh-CN':'API按量付费价格参考','zh-TW':'API 按量付費價格參考',en:'API pay-as-you-go price reference',ja:'API 従量課金の参考額',ko:'API 종량제 가격 참고',es:'Referencia de precios API por uso',fr:'Référence des tarifs API à l’usage',de:'API-Preisreferenz nach Verbrauch','pt-BR':'Referência de preços API por uso',ru:'Справочная стоимость API по использованию'};
// 卡片用短标题（与指标卡等高，单行；完整标题保留在悬浮提示）。
const shortTitles:Record<string,string>={'zh-CN':'API 参考费用','zh-TW':'API 參考費用',en:'API ref. cost',ja:'API 参考額',ko:'API 참고 요금',es:'Costo API ref.',fr:'Coût API réf.',de:'API-Referenz','pt-BR':'Custo API ref.',ru:'Справка API'};
const statusKeys=['cost.reason.no_known_usage','cost.reason.no_provider','cost.reason.no_model','cost.reason.channel_unknown','cost.reason.no_price_row','cost.reason.tier_ambiguous','cost.reason.token_anomaly','cost.reason.internal_overflow','cost.reason.other','cost.partialHint','cost.fxHint'];
const statuses:Record<string,string[]>={
  'zh-CN':['来源未提供可计价 token','供应商未知','模型未知','渠道或币种不明确','没有适用的模型价目','上下文档位未知','token 数据异常','金额超出计算范围','未知原因','部分估算：输入拆分、输出或价格分项缺失，仅计已知部分。','人民币保留原值；美元折算采用 ECB {date} 参考汇率，非交易汇率。'],
  'zh-TW':['來源未提供可計價 token','供應商未知','模型未知','渠道或幣種不明','沒有適用的模型價目','上下文級距未知','token 資料異常','金額超出計算範圍','未知原因','部分估算：輸入拆分、輸出或價格分項缺失，僅計已知部分。','人民幣保留原值；美元換算採用 ECB {date} 參考匯率，非交易匯率。'],
  en:['No priceable tokens reported','Unknown provider','Unknown model','Ambiguous channel or currency','No applicable model rate','Unknown context tier','Invalid token data','Amount exceeds calculation range','Unknown reason','Partial estimate: input split, output or component rates are missing; only known components are priced.','Original CNY retained; USD equivalent uses ECB reference rates dated {date}, not transaction rates.'],
  ja:['課金可能な token の報告なし','提供元不明','モデル不明','経路または通貨が不明','適用できるモデル料金なし','コンテキスト区分不明','token データ異常','計算範囲を超えた金額','理由不明','部分推計：入力内訳、出力または単価が不足。既知の項目のみ計算。','元の人民元額を保持。ドル換算は ECB {date} の参考為替で、取引用ではありません。'],
  ko:['계산 가능한 token 보고 없음','제공자 알 수 없음','모델 알 수 없음','경로 또는 통화 불명확','적용 가능한 모델 요금 없음','컨텍스트 구간 알 수 없음','잘못된 token 데이터','계산 범위를 초과한 금액','이유 알 수 없음','부분 추정: 입력 구분, 출력 또는 항목 요금이 누락되어 알려진 항목만 계산합니다.','원래 CNY 금액을 유지합니다. USD 환산은 ECB {date} 참고 환율이며 거래 환율이 아닙니다.'],
  es:['Sin tokens calculables','Proveedor desconocido','Modelo desconocido','Canal o moneda ambiguos','Sin tarifa de modelo aplicable','Tramo de contexto desconocido','Tokens inválidos','Importe fuera de rango','Motivo desconocido','Estimación parcial: faltan desglose de entrada, salida o tarifas; solo se calculan componentes conocidos.','Se conserva CNY; la equivalencia USD usa tipos de referencia del BCE del {date}, no tipos de transacción.'],
  fr:['Aucun token calculable déclaré','Fournisseur inconnu','Modèle inconnu','Canal ou devise ambigu','Aucun tarif de modèle applicable','Seuil de contexte inconnu','Tokens invalides','Montant hors plage de calcul','Motif inconnu','Estimation partielle : entrée détaillée, sortie ou tarifs manquants ; seuls les éléments connus sont calculés.','CNY conservé ; conversion USD selon les taux de référence BCE du {date}, pas les taux de transaction.'],
  de:['Keine berechenbaren Tokens gemeldet','Anbieter unbekannt','Modell unbekannt','Kanal oder Währung unklar','Kein passender Modellpreis','Kontextstufe unbekannt','Ungültige Token-Daten','Betrag außerhalb des Rechenbereichs','Grund unbekannt','Teilbetrag: Eingabeaufteilung, Ausgabe oder Teilpreise fehlen; nur bekannte Anteile werden berechnet.','Original in CNY bleibt erhalten; USD-Umrechnung nach EZB-Referenzkurs vom {date}, kein Transaktionskurs.'],
  'pt-BR':['Sem tokens calculáveis informados','Provedor desconhecido','Modelo desconhecido','Canal ou moeda ambíguos','Sem tarifa de modelo aplicável','Faixa de contexto desconhecida','Tokens inválidos','Valor fora do intervalo de cálculo','Motivo desconhecido','Estimativa parcial: faltam divisão da entrada, saída ou tarifas; apenas componentes conhecidos são calculados.','CNY original preservado; equivalente USD usa taxas de referência do BCE de {date}, não taxas de transação.'],
  ru:['Нет токенов для расчёта','Поставщик неизвестен','Модель неизвестна','Канал или валюта неоднозначны','Нет подходящего тарифа модели','Уровень контекста неизвестен','Некорректные данные токенов','Сумма вне диапазона расчёта','Причина неизвестна','Частичная оценка: нет разбивки входа, выхода или тарифов; учитываются только известные части.','Исходная сумма CNY сохранена; пересчёт USD по справочным курсам ЕЦБ от {date}, не для транзакций.'],
};
export const referenceCatalogs:Record<string,Record<string,string>>=Object.fromEntries(Object.entries(phrases).map(([locale,values])=>{
  if(values.length!==keys.length) throw new Error('Incomplete reference translations: '+locale);
  if(statuses[locale].length!==statusKeys.length) throw new Error('Incomplete cost status translations: '+locale);
  return [locale,{...Object.fromEntries(keys.map((key,index)=>[key,values[index]])),
    ...Object.fromEntries(statusKeys.map((key,index)=>[key,statuses[locale][index]])),
    'cost.currentSim':titles[locale],'cost.title':titles[locale],'dashboard.modelCost':titles[locale],
    'cost.referenceShort':shortTitles[locale]??titles[locale],
    'dashboard.costCurveHint':values[0],'chart.unknownTotals':`${values[7]}: {names}`}];
}));

const compatibilityHints:Record<string,string>={
  'zh-CN':'已自动检查可识别的用量。来源版本尚未验证；支持更新后会自动复核。',
  'zh-TW':'已自動檢查可辨識的用量。來源版本尚未驗證；支援更新後會自動複核。',
  en:'Recognized usage was checked automatically. This source version is unverified; support updates will recheck it.',
  ja:'認識できる使用量は自動確認済みです。このバージョンは未検証で、対応更新後に再確認します。',
  ko:'인식 가능한 사용량은 자동으로 확인했습니다. 이 버전은 미검증 상태이며 지원 업데이트 후 다시 확인합니다.',
  es:'El uso reconocido se comprobó automáticamente. La versión no está verificada; se revisará al actualizar la compatibilidad.',
  fr:'Les usages reconnus ont été vérifiés automatiquement. Cette version reste non vérifiée et sera réexaminée après une mise à jour.',
  de:'Erkannte Nutzung wurde automatisch geprüft. Diese Version ist nicht verifiziert; ein Support-Update prüft sie erneut.',
  'pt-BR':'O uso reconhecido foi verificado automaticamente. A versão não foi validada; atualizações de suporte farão nova verificação.',
  ru:'Распознанные данные проверены автоматически. Версия не подтверждена; после обновления поддержки проверка повторится.',
};
for(const [locale,catalog] of Object.entries(referenceCatalogs)) {
  catalog['sources.compatHint']=compatibilityHints[locale];
}

const repairKeys=['sources.notFound','sources.otelHint','tokens.observedTotalHint'];
const repairPhrases:Record<string,string[]>={
  'zh-CN':['本机来源已不在，保留历史','本机遥测文件。已核验的 Copilot 导出按会话和日期替代原生贡献，不重复相加。','已观测输入＋输出的下界；原生 Copilot 输入可能仅覆盖末次调用，不代表完整总量。'],
  'zh-TW':['本機來源已不存在，保留歷史','本機遙測檔案。已核驗的 Copilot 匯出按工作階段與日期取代原生貢獻，不重複相加。','已觀測輸入＋輸出的下界；原生 Copilot 輸入可能僅涵蓋末次呼叫，不代表完整總量。'],
  en:['Local source missing; history retained','Local telemetry files. Verified Copilot exports replace native contributions per session and day; they are not added twice.','Lower bound from observed input + output. Native Copilot input may cover only the last call, not the complete turn.'],
  ja:['ローカルソースなし・履歴保持','ローカル遥測ファイル。検証済み Copilot 出力はセッション・日ごとに元の集計を置換し、二重加算しません。','観測済み入力＋出力の下限。Copilot の入力は最後の呼び出しのみの場合があり、完全な合計ではありません。'],
  ko:['로컬 소스 없음 · 기록 유지','로컬 원격 측정 파일입니다. 검증된 Copilot 내보내기는 세션과 날짜별로 원본 집계를 대체하며 중복 합산하지 않습니다.','관측된 입력 + 출력의 하한입니다. Copilot 입력은 마지막 호출만 포함할 수 있으며 전체 합계가 아닙니다.'],
  es:['Fuente local ausente; historial conservado','Archivos de telemetría local. La exportación Copilot verificada sustituye los datos nativos por sesión y día, sin duplicarlos.','Límite inferior de entrada + salida observadas. Copilot puede registrar solo la entrada de la última llamada, no el turno completo.'],
  fr:['Source locale absente ; historique conservé','Fichiers de télémétrie locale. Les exports Copilot vérifiés remplacent les données natives par session et jour, sans double comptage.','Borne inférieure : entrée + sortie observées. L’entrée Copilot peut couvrir uniquement le dernier appel, pas le tour complet.'],
  de:['Lokale Quelle fehlt; Verlauf bleibt erhalten','Lokale Telemetriedateien. Verifizierte Copilot-Exporte ersetzen native Beiträge je Sitzung und Tag ohne Doppelzählung.','Untergrenze aus beobachteter Eingabe + Ausgabe. Die Copilot-Eingabe kann nur den letzten Aufruf statt der gesamten Runde umfassen.'],
  'pt-BR':['Fonte local ausente; histórico mantido','Arquivos de telemetria local. Exportações Copilot verificadas substituem os dados nativos por sessão e dia, sem duplicação.','Limite inferior de entrada + saída observadas. A entrada Copilot pode cobrir apenas a última chamada, não o turno completo.'],
  ru:['Локальный источник отсутствует; история сохранена','Локальные файлы телеметрии. Проверенные экспорты Copilot заменяют исходные данные по сеансу и дню без двойного учёта.','Нижняя граница: наблюдаемый ввод + вывод. Ввод Copilot может охватывать только последний вызов, а не весь ход.'],
};
for(const [locale,catalog] of Object.entries(referenceCatalogs)) {
  repairKeys.forEach((key,index)=>catalog[key]=repairPhrases[locale][index]);
}
const ambiguousModels:Record<string,string>={
  'zh-CN':'模型拼写匹配到多个价目 ID，需核对','zh-TW':'模型拼寫符合多個價目 ID，需核對',en:'Model spelling matches multiple catalog IDs',
  ja:'モデル表記が複数の料金 ID に一致',ko:'모델 표기가 여러 요금 ID와 일치',es:'El nombre coincide con varios ID de tarifas',
  fr:'Le nom correspond à plusieurs identifiants de tarifs',de:'Modellschreibweise entspricht mehreren Preis-IDs',
  'pt-BR':'O nome corresponde a vários IDs de tarifas',ru:'Название соответствует нескольким идентификаторам тарифов',
};
for(const [locale,catalog] of Object.entries(referenceCatalogs)) catalog['cost.reason.model_ambiguous']=ambiguousModels[locale];

const compactChartPhrases:Record<string,[string,string]>={
  'zh-CN':['≥ 已观测下界','覆盖与计价说明'],'zh-TW':['≥ 已觀測下界','涵蓋與計價說明'],
  en:['≥ Observed lower bound','Coverage and pricing details'],ja:['≥ 観測済み下限','集計範囲と料金の詳細'],
  ko:['≥ 관측된 하한','집계 범위 및 요금 설명'],es:['≥ Límite inferior observado','Cobertura y precios'],
  fr:['≥ Borne inférieure observée','Couverture et tarifs'],de:['≥ Beobachtete Untergrenze','Abdeckung und Preisdetails'],
  'pt-BR':['≥ Limite inferior observado','Cobertura e preços'],ru:['≥ Наблюдаемая нижняя граница','Охват и расчёт стоимости'],
};
for(const [locale,catalog] of Object.entries(referenceCatalogs)) {
  [catalog['tokens.lowerBound'],catalog['cost.coverageDetails']]=compactChartPhrases[locale];
}
