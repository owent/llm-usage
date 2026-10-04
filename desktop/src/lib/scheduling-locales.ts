const keys = ['sources.schedule.preview', 'system.refreshTask.desired', 'system.refreshTask.repair', 'system.refreshTask.interval', 'settings.pauseOnSaver', 'settings.closeToTray'];
const phrases: Record<string, string[]> = {
  'zh-CN': ['接下来三次', '启用意图', '重试应用', '每分钟检查到期规则（无窗口；不足一分钟的间隔受限）', 'Windows 节能模式暂停自动采集', '关闭窗口后留在 Windows 托盘（保存后生效）'],
  'zh-TW': ['接下來三次', '啟用意圖', '重試套用', '每分鐘檢查到期規則（無視窗；不足一分鐘的間隔受限）', 'Windows 節能模式暫停自動採集', '關閉視窗後留在 Windows 系統匣（儲存後生效）'],
  en: ['Next three runs', 'Requested state', 'Retry applying', 'Checks due rules every minute (headless; sub-minute intervals are limited)', 'Pause automatic collection in Windows battery saver', 'Keep running in the Windows tray when closing the window (save to apply)'],
  ja: ['次の3回', '設定上の状態', '適用を再試行', '毎分、期限を確認（画面なし。1分未満の間隔は制限されます）', 'Windows のバッテリー節約時は自動収集を一時停止', '閉じると Windows トレイで実行を継続（保存して適用）'],
  ko: ['다음 3회 실행', '요청한 상태', '적용 재시도', '매분 실행 기한 확인 (창 없음, 1분 미만 간격 제한)', 'Windows 배터리 절약 모드에서 자동 수집 일시 중지', '창을 닫으면 Windows 트레이에서 계속 실행 (저장 후 적용)'],
  es: ['Próximas tres ejecuciones', 'Estado solicitado', 'Reintentar aplicar', 'Comprueba los plazos cada minuto (sin interfaz; intervalos menores limitados)', 'Pausar la recopilación automática en el ahorro de batería de Windows', 'Seguir en la bandeja de Windows al cerrar la ventana (guardar para aplicar)'],
  fr: ['Trois prochaines exécutions', 'État demandé', 'Réessayer', 'Vérification chaque minute (sans fenêtre ; intervalles plus courts limités)', 'Suspendre la collecte automatique en mode économie de batterie Windows', 'Continuer dans la zone de notification Windows après fermeture (enregistrer pour appliquer)'],
  de: ['Nächste drei Ausführungen', 'Gewünschter Zustand', 'Erneut anwenden', 'Prüft jede Minute fällige Regeln (ohne Fenster; kürzere Intervalle begrenzt)', 'Automatische Erfassung im Windows-Energiesparmodus pausieren', 'Beim Schließen im Windows-Infobereich weiterlaufen (zum Anwenden speichern)'],
  'pt-BR': ['Próximas três execuções', 'Estado solicitado', 'Tentar aplicar novamente', 'Verifica prazos a cada minuto (sem janela; intervalos menores limitados)', 'Pausar a coleta automática no modo de economia de bateria do Windows', 'Continuar na bandeja do Windows ao fechar a janela (salvar para aplicar)'],
  ru: ['Следующие три запуска', 'Запрошенное состояние', 'Повторить применение', 'Проверка сроков каждую минуту (без окна; интервалы меньше минуты ограничены)', 'Приостанавливать автоматический сбор в режиме экономии батареи Windows', 'Оставаться в трее Windows после закрытия окна (сохраните для применения)'],
};
export const schedulingCatalogs: Record<string, Record<string, string>> = Object.fromEntries(
  Object.entries(phrases).map(([locale, values]) => [locale, Object.fromEntries(keys.map((key, i) => [key, values[i]]))]),
);
const fileWatch: Record<string, string> = {
  'zh-CN': '监听本机来源文件变更（Windows；每日/每周计划除外）',
  'zh-TW': '監聽本機來源檔案變更（Windows；每日/每週排程除外）',
  en: 'Watch local source file changes (Windows; excludes daily/weekly schedules)',
  ja: 'ローカルソースのファイル変更を監視（Windows、毎日・毎週の予定を除く）',
  ko: '로컬 소스 파일 변경 감시 (Windows, 일별/주별 일정 제외)',
  es: 'Observar cambios en archivos locales (Windows; excluye planes diarios/semanales)',
  fr: 'Surveiller les fichiers locaux (Windows ; hors horaires quotidiens/hebdomadaires)',
  de: 'Lokale Quelldateien überwachen (Windows; ohne tägliche/wöchentliche Zeitpläne)',
  'pt-BR': 'Monitorar arquivos locais (Windows; exceto agendas diárias/semanais)',
  ru: 'Следить за локальными файлами (Windows; кроме ежедневных/еженедельных расписаний)',
};
for (const [locale, value] of Object.entries(fileWatch)) {
  schedulingCatalogs[locale]['settings.fileWatch'] = value;
}
