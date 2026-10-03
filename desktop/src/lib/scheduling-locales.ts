const keys = ['sources.schedule.preview', 'system.refreshTask.desired', 'system.refreshTask.repair', 'system.refreshTask.interval'];
const phrases: Record<string, string[]> = {
  'zh-CN': ['接下来三次', '启用意图', '重试应用', '每分钟检查到期规则（无窗口；不足一分钟的间隔受限）'],
  'zh-TW': ['接下來三次', '啟用意圖', '重試套用', '每分鐘檢查到期規則（無視窗；不足一分鐘的間隔受限）'],
  en: ['Next three runs', 'Requested state', 'Retry applying', 'Checks due rules every minute (headless; sub-minute intervals are limited)'],
  ja: ['次の3回', '設定上の状態', '適用を再試行', '毎分、期限を確認（画面なし。1分未満の間隔は制限されます）'],
  ko: ['다음 3회 실행', '요청한 상태', '적용 재시도', '매분 실행 기한 확인 (창 없음, 1분 미만 간격 제한)'],
  es: ['Próximas tres ejecuciones', 'Estado solicitado', 'Reintentar aplicar', 'Comprueba los plazos cada minuto (sin interfaz; intervalos menores limitados)'],
  fr: ['Trois prochaines exécutions', 'État demandé', 'Réessayer', 'Vérification chaque minute (sans fenêtre ; intervalles plus courts limités)'],
  de: ['Nächste drei Ausführungen', 'Gewünschter Zustand', 'Erneut anwenden', 'Prüft jede Minute fällige Regeln (ohne Fenster; kürzere Intervalle begrenzt)'],
  'pt-BR': ['Próximas três execuções', 'Estado solicitado', 'Tentar aplicar novamente', 'Verifica prazos a cada minuto (sem janela; intervalos menores limitados)'],
  ru: ['Следующие три запуска', 'Запрошенное состояние', 'Повторить применение', 'Проверка сроков каждую минуту (без окна; интервалы меньше минуты ограничены)'],
};
export const schedulingCatalogs: Record<string, Record<string, string>> = Object.fromEntries(
  Object.entries(phrases).map(([locale, values]) => [locale, Object.fromEntries(keys.map((key, i) => [key, values[i]]))]),
);
