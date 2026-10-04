const keys = ['telemetry.compactTitle','telemetry.stateCounts','telemetry.data.waiting','telemetry.data.verified','telemetry.data.unrecognized','telemetry.data.unavailable','telemetry.data.external','telemetry.status.blocked','dashboard.selectedRange','dashboard.queryRange','dashboard.resetRange','dashboard.modelCost','dashboard.costCurve','dashboard.costCurveHint','dashboard.subtotal','dashboard.modelCosts'];
const phrases: Record<string,string[]> = {
  'zh-CN': ['可观测性','{pending} 待开启 · {waiting} 等待数据 · {verified} 已核验 · {blocked} 配置受限','尚未收到导出数据，正常使用 Agent 后自动核验。','已自动核验至少 {count} 条有效记录。','输出已有内容，当前采样未发现本客户端有效遥测，后续自动复查。','输出暂时无法读取，后续自动复查。','保留现有输出目标，尚未关联到本应用可读数据。','配置受限','选定范围：{range}','查询范围：{range}','恢复整个范围','API 参考估算','费用曲线','按发生时参考价、分币种展示；未知金额保留缺口。','汇总','按模型费用'],
  'zh-TW': ['可觀測性','{pending} 待啟用 · {waiting} 等待資料 · {verified} 已驗證 · {blocked} 配置受限','尚未收到匯出資料，正常使用 Agent 後自動驗證。','已自動驗證至少 {count} 筆有效紀錄。','輸出已有內容，目前取樣未發現有效遙測，稍後自動複查。','暫時無法讀取輸出，稍後自動複查。','保留現有輸出目標，尚未關聯本應用可讀資料。','配置受限','選定範圍：{range}','查詢範圍：{range}','恢復整個範圍','API 參考估算','費用曲線','依發生時參考價、分幣種顯示；未知金額保留缺口。','彙總','依模型費用'],
  en: ['Observability','{pending} to enable · {waiting} waiting · {verified} verified · {blocked} restricted','No export data received yet. Normal Agent use is checked automatically.','At least {count} valid records verified automatically.','Output exists; this sample has no valid client telemetry. Checks continue automatically.','Output is temporarily unreadable. Checks continue automatically.','Existing output is preserved; data is not yet linked to this app.','Configuration restricted','Selected range: {range}','Query range: {range}','Restore full range','API reference estimate','Cost trend','Historical API reference, per currency; unknown amounts remain gaps.','Total','Cost by model'],
  ja: ['観測設定','{pending} 未設定 · {waiting} 待機 · {verified} 確認済 · {blocked} 制限','データ未受信です。Agent の通常利用後に自動確認します。','有効な記録を少なくとも {count} 件自動確認しました。','出力はありますが、今回の標本に有効な記録がありません。自動確認を続けます。','出力を一時的に読めません。自動確認を続けます。','既存の出力を保持しています。このアプリのデータとは未連携です。','設定制限','選択範囲：{range}','照会範囲：{range}','全範囲に戻す','API 参考見積','費用の推移','発生時の参考価格を通貨別に表示。不明な金額は欠損のままです。','合計','モデル別費用'],
  ko: ['관측 설정','{pending} 설정 필요 · {waiting} 대기 · {verified} 검증됨 · {blocked} 제한','내보내기 데이터가 없습니다. Agent 사용 후 자동으로 확인합니다.','유효한 기록을 최소 {count}개 자동 검증했습니다.','출력은 있지만 현재 표본에 유효한 기록이 없습니다. 자동 확인을 계속합니다.','출력을 일시적으로 읽을 수 없습니다. 자동 확인을 계속합니다.','기존 출력은 보존되며 앱 데이터와 아직 연결되지 않았습니다.','설정 제한','선택 범위: {range}','조회 범위: {range}','전체 범위 복원','API 참고 추정','비용 추이','발생 당시 참고 가격을 통화별로 표시합니다. 알 수 없는 금액은 공백입니다.','합계','모델별 비용'],
  es: ['Observabilidad','{pending} por activar · {waiting} esperando · {verified} verificados · {blocked} restringidos','Aún no hay datos. Se comprueban automáticamente tras usar el Agent.','Al menos {count} registros válidos verificados automáticamente.','Hay contenido sin telemetría válida en esta muestra. La comprobación continúa.','Salida temporalmente ilegible. La comprobación automática continúa.','Se conserva la salida existente; los datos aún no están vinculados.','Configuración restringida','Intervalo seleccionado: {range}','Intervalo consultado: {range}','Restaurar intervalo completo','Estimación API de referencia','Tendencia de costes','Referencia histórica por moneda; los importes desconocidos quedan vacíos.','Total','Coste por modelo'],
  fr: ['Observabilité','{pending} à activer · {waiting} en attente · {verified} vérifiés · {blocked} restreints','Aucune donnée reçue. Vérification automatique après usage de l’Agent.','Au moins {count} enregistrements valides vérifiés automatiquement.','La sortie existe sans télémétrie valide dans cet échantillon. Vérification automatique maintenue.','Sortie temporairement illisible. Vérification automatique maintenue.','La sortie existante est conservée ; les données ne sont pas encore liées.','Configuration restreinte','Plage sélectionnée : {range}','Plage consultée : {range}','Restaurer toute la plage','Estimation API de référence','Évolution des coûts','Référence historique par devise ; les montants inconnus restent vides.','Total','Coût par modèle'],
  de: ['Beobachtbarkeit','{pending} zu aktivieren · {waiting} wartend · {verified} geprüft · {blocked} eingeschränkt','Noch keine Daten. Nach Agent-Nutzung erfolgt eine automatische Prüfung.','Mindestens {count} gültige Datensätze automatisch geprüft.','Ausgabe vorhanden, aber keine gültigen Daten in dieser Stichprobe. Automatische Prüfung läuft weiter.','Ausgabe vorübergehend unlesbar. Automatische Prüfung läuft weiter.','Vorhandene Ausgabe bleibt erhalten; Daten sind noch nicht zugeordnet.','Konfiguration eingeschränkt','Ausgewählter Bereich: {range}','Abfragebereich: {range}','Gesamten Bereich wiederherstellen','API-Referenzschätzung','Kostenverlauf','Historische Referenz je Währung; unbekannte Beträge bleiben Lücken.','Summe','Kosten je Modell'],
  'pt-BR': ['Observabilidade','{pending} a ativar · {waiting} aguardando · {verified} verificados · {blocked} restritos','Ainda sem dados. Verificação automática após usar o Agent.','Pelo menos {count} registros válidos verificados automaticamente.','Há saída sem telemetria válida nesta amostra. A verificação automática continua.','Saída temporariamente ilegível. A verificação automática continua.','A saída existente é preservada; os dados ainda não estão vinculados.','Configuração restrita','Intervalo selecionado: {range}','Intervalo consultado: {range}','Restaurar intervalo completo','Estimativa API de referência','Evolução dos custos','Referência histórica por moeda; valores desconhecidos ficam ausentes.','Total','Custo por modelo'],
  ru: ['Наблюдаемость','{pending} включить · {waiting} ожидают · {verified} проверены · {blocked} ограничены','Данные ещё не получены. После работы Agent проверка выполняется автоматически.','Автоматически проверено не менее {count} корректных записей.','Файл содержит данные, но в выборке нет корректной телеметрии. Проверка продолжится.','Данные временно недоступны. Автоматическая проверка продолжится.','Существующий вывод сохранён; данные ещё не связаны с приложением.','Настройка ограничена','Выбранный диапазон: {range}','Диапазон запроса: {range}','Восстановить весь диапазон','Оценка по тарифам API','Динамика затрат','Историческая оценка по валютам; неизвестные суммы остаются пропусками.','Итого','Затраты по моделям'],
};
const compactStateCounts: Record<string, string> = {
  'zh-CN': '{pending} 待开启 · {noData} 暂无数据 · {verified} 已核验',
  'zh-TW': '{pending} 待啟用 · {noData} 暫無資料 · {verified} 已驗證',
  en: '{pending} to enable · {noData} no data yet · {verified} verified',
  ja: '{pending} 未設定 · {noData} データなし · {verified} 確認済',
  ko: '{pending} 설정 필요 · {noData} 데이터 없음 · {verified} 검증됨',
  es: '{pending} por activar · {noData} sin datos · {verified} verificados',
  fr: '{pending} à activer · {noData} sans données · {verified} vérifiés',
  de: '{pending} zu aktivieren · {noData} noch keine Daten · {verified} geprüft',
  'pt-BR': '{pending} a ativar · {noData} sem dados · {verified} verificados',
  ru: '{pending} включить · {noData} пока без данных · {verified} проверены',
};
const rangePhrases: Record<string,string[]> = {
  'zh-CN': ['近 2 个自然日','点击选取时段，或按住鼠标横向拖选连续范围；再次点击同一时段取消','展示完整日期范围，不受图表选区限制'],
  'zh-TW': ['近 2 個日曆日','點擊時段或按住滑鼠橫向拖選連續範圍；再次點擊同一時段取消','顯示完整日期範圍，不受圖表選區限制'],
  en: ['Last 2 calendar days','Click a period or drag horizontally to select a range; click the same period again to clear','Shows the full date range, independent of the chart selection'],
  ja: ['直近 2 暦日','期間をクリックするか、横にドラッグして範囲を選択。同じ期間を再度クリックすると解除','グラフの選択に関係なく、日付範囲全体を表示'],
  ko: ['최근 2일(달력 기준)','기간을 클릭하거나 가로로 드래그하여 범위를 선택하세요. 같은 기간을 다시 클릭하면 해제됩니다','차트 선택과 관계없이 전체 날짜 범위를 표시합니다'],
  es: ['Últimos 2 días naturales','Haz clic en un período o arrastra horizontalmente para seleccionar un intervalo; vuelve a hacer clic para borrar','Muestra todo el intervalo de fechas, sin limitarse a la selección del gráfico'],
  fr: ['2 derniers jours calendaires','Cliquez sur une période ou faites glisser horizontalement pour sélectionner une plage ; recliquez pour effacer','Affiche toute la plage de dates, indépendamment de la sélection du graphique'],
  de: ['Letzte 2 Kalendertage','Zeitraum anklicken oder horizontal ziehen, um einen Bereich auszuwählen; erneut anklicken zum Aufheben','Zeigt den gesamten Datumsbereich unabhängig von der Diagrammauswahl'],
  'pt-BR': ['Últimos 2 dias de calendário','Clique em um período ou arraste horizontalmente para selecionar um intervalo; clique novamente para limpar','Exibe todo o intervalo de datas, independentemente da seleção do gráfico'],
  ru: ['Последние 2 календарных дня','Нажмите на период или перетащите по горизонтали для выбора диапазона; повторное нажатие отменяет выбор','Показывает весь диапазон дат независимо от выбора на графике'],
};
const dragHints: Record<string,string> = {
  'zh-CN':'点击时段，或按住鼠标横向拖选连续范围',
  'zh-TW':'點擊時段，或按住滑鼠橫向拖選連續範圍',
  en:'Click a period or drag horizontally to select a range',
  ja:'期間をクリックするか、横にドラッグして範囲を選択',
  ko:'기간을 클릭하거나 가로로 드래그하여 범위를 선택하세요',
  es:'Haz clic en un período o arrastra horizontalmente para seleccionar un intervalo',
  fr:'Cliquez sur une période ou faites glisser horizontalement pour sélectionner une plage',
  de:'Zeitraum anklicken oder horizontal ziehen, um einen Bereich auszuwählen',
  'pt-BR':'Clique em um período ou arraste horizontalmente para selecionar um intervalo',
  ru:'Нажмите на период или перетащите по горизонтали для выбора диапазона',
};
const cleanupPhrases: Record<string,string[]> = {
  'zh-CN':['取消清理','清理已取消，数据保持不变。','清理已结束或进入提交阶段，无法取消。'],
  'zh-TW':['取消清理','清理已取消，資料保持不變。','清理已結束或進入提交階段，無法取消。'],
  en:['Cancel cleanup','Cleanup cancelled. Data is unchanged.','Cleanup has finished or entered commit and can no longer be cancelled.'],
  ja:['クリーンアップを中止','クリーンアップを中止しました。データは変更されていません。','クリーンアップが終了したか、コミットが開始されたため中止できません。'],
  ko:['정리 취소','정리가 취소되었습니다. 데이터는 변경되지 않았습니다.','정리가 끝났거나 커밋이 시작되어 취소할 수 없습니다.'],
  es:['Cancelar limpieza','Limpieza cancelada. Los datos no han cambiado.','La limpieza terminó o comenzó a confirmar cambios y ya no puede cancelarse.'],
  fr:['Annuler le nettoyage','Nettoyage annulé. Les données sont inchangées.','Le nettoyage est terminé ou la validation a commencé ; il ne peut plus être annulé.'],
  de:['Bereinigung abbrechen','Bereinigung abgebrochen. Die Daten sind unverändert.','Die Bereinigung ist beendet oder bestätigt bereits Änderungen. Ein Abbruch ist nicht mehr möglich.'],
  'pt-BR':['Cancelar limpeza','Limpeza cancelada. Os dados não foram alterados.','A limpeza terminou ou começou a confirmar alterações e não pode mais ser cancelada.'],
  ru:['Отменить очистку','Очистка отменена. Данные не изменены.','Очистка завершена или началась фиксация изменений. Отмена больше невозможна.'],
};
const keyboardHints: Record<string,string> = {
  'zh-CN':'方向键移动，Shift 扩展范围，Enter 确认；Home/End 到首末时段。',
  'zh-TW':'方向鍵移動，Shift 擴展範圍，Enter 確認；Home/End 到首末時段。',
  en:'Arrow keys move, Shift extends the range, Enter applies; Home/End jump to the first/last period.',
  ja:'矢印キーで移動、Shift で範囲を拡張、Enter で確定。Home/End で最初/最後へ。',
  ko:'방향키로 이동, Shift로 범위 확장, Enter로 적용합니다. Home/End로 처음/끝으로 이동합니다.',
  es:'Flechas para mover, Shift para ampliar, Enter para aplicar; Home/End al primer/último período.',
  fr:'Flèches pour déplacer, Maj pour étendre, Entrée pour appliquer ; Début/Fin au premier/dernier intervalle.',
  de:'Pfeiltasten bewegen, Umschalt erweitert, Eingabe übernimmt; Pos1/Ende zum ersten/letzten Zeitraum.',
  'pt-BR':'Setas movem, Shift amplia, Enter aplica; Home/End vão ao primeiro/último período.',
  ru:'Стрелки перемещают, Shift расширяет диапазон, Enter применяет; Home/End к первому/последнему периоду.',
};
const manualOnly: Record<string,string> = {
  'zh-CN':'仅采集以上手工目录（不发现默认来源或账户额度）',
  'zh-TW':'僅採集以上手動目錄（不探索預設來源或帳戶額度）',
  en:'Collect only these manual roots (skip default sources and account quotas)',
  ja:'手動指定したディレクトリのみ収集（既定のソースとアカウント枠は除外）',
  ko:'위 수동 경로에서만 수집 (기본 소스와 계정 할당량 제외)',
  es:'Recopilar solo estas carpetas manuales (omitir fuentes predeterminadas y cuotas de cuenta)',
  fr:'Collecter uniquement ces dossiers manuels (ignorer les sources par défaut et les quotas du compte)',
  de:'Nur diese manuellen Verzeichnisse erfassen (Standardquellen und Kontingente überspringen)',
  'pt-BR':'Coletar apenas estas pastas manuais (ignorar fontes padrão e cotas da conta)',
  ru:'Собирать только из этих каталогов (без стандартных источников и квот аккаунта)',
};
export const polishCatalogs=Object.fromEntries(Object.entries(phrases).map(([locale,values])=>{
  if(values.length!==keys.length) throw new Error('Incomplete polish translations: '+locale);
  return [locale,{
    ...Object.fromEntries(keys.map((key,index)=>[key,values[index]])),
    'telemetry.compactStateCounts': compactStateCounts[locale],
    'filter.quick.2days': rangePhrases[locale][0],
    'overview.periodSummary.hint': rangePhrases[locale][1],
    'dashboard.fullRangePanel': rangePhrases[locale][2],
    'dashboard.dragHint': dragHints[locale],
    'cleanup.cancel': cleanupPhrases[locale][0],
    'cleanup.cancelled': cleanupPhrases[locale][1],
    'cleanup.committed': cleanupPhrases[locale][2],
    'dashboard.keyboardHint': keyboardHints[locale],
    'settings.manualRootsOnly': manualOnly[locale],
  }];
}));
