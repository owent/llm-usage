/** Verified setup and partial-usage messages, complete in every supported language. */
const keys = [
  'heatmap.previousYear', 'heatmap.nextYear', 'heatmap.future', 'chart.unknownTotals',
  'telemetry.title', 'telemetry.hint', 'telemetry.refresh', 'telemetry.checking', 'telemetry.checkFailed', 'telemetry.empty',
  'telemetry.status.missing', 'telemetry.status.configured', 'telemetry.status.blocked', 'telemetry.guide', 'telemetry.launcher', 'telemetry.configure',
  'telemetry.preview', 'telemetry.output', 'telemetry.launcherHint', 'telemetry.mergeHint', 'telemetry.receiverHint', 'telemetry.apply', 'telemetry.cancel',
  'telemetry.saved', 'telemetry.undo', 'telemetry.writeFailed', 'telemetry.reason.environment_override', 'telemetry.reason.managed_policy',
  'telemetry.reason.unsupported_version', 'telemetry.reason.invalid_config', 'telemetry.reason.unsafe_path', 'telemetry.reason.telemetry_disabled',
  'telemetry.reason.sync_conflict', 'telemetry.reason.config_changed', 'telemetry.reason.read_only', 'telemetry.reason.receiver_bind_failed',
  'telemetry.reason.receiver_restart_required', 'telemetry.reason.preview_expired',
] as const;

const phrases: Record<string, string[]> = {
  'zh-CN': [
    '上一年', '下一年', '尚未到来', '{names} 的总 token 未知；悬浮提示保留已报告的输入和输出。',
    '可观测性采集配置', '检查用户配置，帮助补充后续记录；工作区、启动环境和企业策略可能覆盖这些设置。新输出先保留用于核验，不与原生用量重复计入。', '重新检查', '正在后台检查…', '遥测配置检查失败，可重新检查。', '未发现支持自动配置的本机 Agent。',
    '未配置输出', '用户层已配置', '需手工核对', '官方配置说明', '创建启动脚本', '一键配置',
    '修改预览', '输出位置', '创建专用脚本，以后通过该脚本启动 Copilot；仅影响该进程，不修改 CLI 状态文件或全局环境。', '只合并列出的配置键，保留其他设置与注释，原文件先备份。', '会启用本应用的本机遥测接收器；接收时需要保持本应用运行。', '应用配置', '取消',
    '配置已写入。请重载 IDE 或重启 Agent；正常使用后产生新记录，历史无法补回。', '撤销本次配置', '配置写入失败，原文件保留；请检查路径或重新预览。', '检测到进程环境覆盖，请先核对启动环境。', '检测到托管配置，请按管理员策略手工核对。',
    '未验证此安装的配置键，请按官方说明手工配置。', '配置不可读、过大、格式无效或包含重复键，请先修正。', '配置路径包含链接或不是绝对路径，请手工核对。', '用户已关闭遥测，请先自行调整遥测开关。',
    '用户明确要求同步这些键，请先核对 Settings Sync 排除项。', '配置在预览或应用后已被修改；保留新修改，请重新预览。', '配置文件只读，未写入。', '本机接收器端口绑定失败，未写入 Agent 配置。',
    '接收器端口已变化，请重启本应用后重新检查。', '预览已过期或应用已重启，请重新预览。',
  ],
  en: [
    'Previous year', 'Next year', 'Future date', 'Total tokens for {names} are unknown; tooltips retain reported input and output.',
    'Local telemetry setup', 'Check user settings to capture future records. Workspace settings, launch environments and managed policies may override them. New output is kept for validation and is not added again to native usage.', 'Check again', 'Checking in the background…', 'Telemetry check failed. Try again.', 'No local Agent with supported setup was found.',
    'Output not configured', 'Configured at user level', 'Manual review needed', 'Official setup guide', 'Create launcher', 'Configure',
    'Change preview', 'Output location', 'Create a dedicated script and launch Copilot through it. Only that process is affected; CLI state files and global environment are preserved.', 'Merge only the listed keys, preserve other settings and comments, and back up the original file.', 'Enable this app’s local telemetry receiver. Keep the app running to receive data.', 'Apply configuration', 'Cancel',
    'Configuration saved. Reload the IDE or restart the Agent. Normal use creates future records; past usage cannot be recovered.', 'Undo this change', 'Could not write configuration. The original is preserved; check the path or preview again.', 'Process environment overrides were detected. Check the launch environment.', 'Managed configuration was detected. Check administrator policies manually.',
    'Configuration keys for this installation are unverified. Use the official guide.', 'Configuration is unreadable, too large, invalid or has duplicate keys. Fix it first.', 'The configuration path contains links or is not absolute. Review it manually.', 'User telemetry is disabled. Adjust that setting first.',
    'These keys are explicitly included in Settings Sync. Review sync exclusions first.', 'Configuration changed after preview or apply. Your edits are preserved; preview again.', 'Configuration is read-only. Nothing was written.', 'Could not bind the local receiver port. Agent configuration was not written.',
    'The receiver port changed. Restart this app and check again.', 'The preview expired or the app restarted. Preview again.',
  ],
  'zh-TW': [
    '上一年', '下一年', '尚未到來', '{names} 的總 token 未知；浮動提示保留已回報的輸入與輸出。',
    '補充本機遙測', '檢查使用者設定以補充後續紀錄；工作區、啟動環境與企業原則可能覆寫設定。新輸出先保留供驗證，不重複計入原生用量。', '重新檢查', '正在背景檢查…', '遙測設定檢查失敗，請重試。', '未找到支援自動設定的本機 Agent。',
    '未設定輸出', '使用者層已設定', '需手動核對', '官方設定說明', '建立啟動指令碼', '一鍵設定',
    '變更預覽', '輸出位置', '建立專用指令碼，之後透過它啟動 Copilot；僅影響該程序，不修改 CLI 狀態檔或全域環境。', '只合併列出的鍵，保留其他設定與註解，先備份原檔。', '啟用本應用程式的本機遙測接收器；接收時須保持應用程式執行。', '套用設定', '取消',
    '設定已儲存。請重新載入 IDE 或重新啟動 Agent；正常使用會產生後續紀錄，歷史無法補回。', '復原本次設定', '設定寫入失敗，原檔保留；請檢查路徑或重新預覽。', '偵測到程序環境覆寫，請核對啟動環境。', '偵測到受管理設定，請依管理員原則手動核對。',
    '尚未驗證此安裝的設定鍵，請依官方說明手動設定。', '設定無法讀取、過大、格式無效或包含重複鍵，請先修正。', '設定路徑包含連結或不是絕對路徑，請手動核對。', '使用者已關閉遙測，請先自行調整。',
    '這些鍵已明確納入 Settings Sync，請先核對排除項。', '設定在預覽或套用後已變更；新修改已保留，請重新預覽。', '設定檔唯讀，未寫入。', '本機接收器連接埠繫結失敗，未寫入 Agent 設定。',
    '接收器連接埠已變更，請重新啟動本應用程式後檢查。', '預覽已過期或應用程式已重啟，請重新預覽。',
  ],
  ja: [
    '前年', '翌年', '未来の日付', '{names} の合計トークンは不明です。ツールチップに報告済みの入力と出力を表示します。',
    'ローカルテレメトリ設定', 'ユーザー設定を確認し、今後の記録を補います。ワークスペース、起動環境、管理ポリシーが優先される場合があります。新しい出力は検証用に保存し、既存の使用量に重複加算しません。', '再確認', 'バックグラウンドで確認中…', 'テレメトリ設定の確認に失敗しました。再試行してください。', '自動設定に対応するローカル Agent が見つかりません。',
    '出力未設定', 'ユーザー設定あり', '手動確認が必要', '公式設定ガイド', '起動スクリプトを作成', '設定する',
    '変更プレビュー', '出力先', '専用スクリプトを作成し、それを使って Copilot を起動します。そのプロセスだけに適用し、CLI 状態ファイルやグローバル環境は変更しません。', '表示したキーのみをマージし、他の設定とコメントを保持して元ファイルをバックアップします。', 'このアプリのローカルテレメトリー受信機を有効にします。受信中はアプリを起動しておいてください。', '設定を適用', 'キャンセル',
    '設定を保存しました。IDE を再読み込みするか Agent を再起動してください。通常の利用で今後の記録が生成されます。過去の使用量は復元できません。', '今回の変更を元に戻す', '書き込みに失敗しました。元ファイルは保持しています。パスを確認するか再プレビューしてください。', 'プロセス環境の上書きを検出しました。起動環境を確認してください。', '管理設定を検出しました。管理者ポリシーを手動で確認してください。',
    'このインストールの設定キーは未検証です。公式ガイドを参照してください。', '設定を読み取れない、サイズ超過、形式不正、またはキー重複があります。先に修正してください。', '設定パスにリンクがあるか、絶対パスではありません。手動で確認してください。', 'ユーザーのテレメトリが無効です。先に設定を変更してください。',
    'これらのキーは Settings Sync に明示的に含まれています。除外設定を確認してください。', 'プレビューまたは適用後に設定が変更されました。編集を保持しています。再プレビューしてください。', '設定ファイルは読み取り専用です。書き込んでいません。', 'ローカル受信ポートを確保できません。Agent 設定は書き込んでいません。',
    '受信ポートが変更されました。このアプリを再起動して再確認してください。', 'プレビューが期限切れかアプリが再起動されました。再プレビューしてください。',
  ],
  ko: [
    '이전 연도', '다음 연도', '미래 날짜', '{names}의 총 토큰은 알 수 없습니다. 도움말에는 보고된 입력과 출력을 표시합니다.',
    '로컬 원격 분석 설정', '사용자 설정을 확인하여 이후 기록을 보완합니다. 작업 영역, 실행 환경, 관리 정책이 우선할 수 있습니다. 새 출력은 검증용으로 보관하며 기존 사용량에 중복 합산하지 않습니다.', '다시 확인', '백그라운드에서 확인 중…', '설정 확인에 실패했습니다. 다시 시도하세요.', '자동 설정을 지원하는 로컬 Agent를 찾지 못했습니다.',
    '출력 미설정', '사용자 설정 있음', '수동 확인 필요', '공식 설정 안내', '실행 스크립트 만들기', '설정',
    '변경 미리 보기', '출력 위치', '전용 스크립트를 만들고 이를 통해 Copilot을 실행하세요. 해당 프로세스에만 적용되며 CLI 상태 파일이나 전역 환경은 변경하지 않습니다.', '표시된 키만 병합하고 다른 설정과 주석을 유지하며 원본을 백업합니다.', '앱의 로컬 텔레메트리 수신기를 활성화합니다. 수신 중에는 앱을 실행해 두세요.', '설정 적용', '취소',
    '설정을 저장했습니다. IDE를 다시 로드하거나 Agent를 재시작하세요. 이후 사용 시 새 기록이 생성되며 과거 사용량은 복구할 수 없습니다.', '이번 변경 취소', '설정을 쓰지 못했습니다. 원본은 유지됩니다. 경로를 확인하거나 다시 미리 보세요.', '프로세스 환경의 덮어쓰기를 감지했습니다. 실행 환경을 확인하세요.', '관리 설정을 감지했습니다. 관리자 정책을 수동으로 확인하세요.',
    '이 설치의 설정 키는 검증되지 않았습니다. 공식 안내를 참고하세요.', '설정을 읽을 수 없거나 크기 초과, 잘못된 형식 또는 중복 키가 있습니다. 먼저 수정하세요.', '설정 경로에 링크가 있거나 절대 경로가 아닙니다. 수동으로 확인하세요.', '사용자 원격 분석이 비활성화되어 있습니다. 먼저 설정을 조정하세요.',
    '이 키는 Settings Sync에 명시적으로 포함됩니다. 제외 항목을 확인하세요.', '미리 보기 또는 적용 후 설정이 변경되었습니다. 편집은 유지됩니다. 다시 미리 보세요.', '설정 파일이 읽기 전용입니다. 쓰지 않았습니다.', '로컬 수신 포트를 열지 못했습니다. Agent 설정은 쓰지 않았습니다.',
    '수신 포트가 변경되었습니다. 앱을 재시작하고 다시 확인하세요.', '미리 보기가 만료되었거나 앱이 재시작되었습니다. 다시 미리 보세요.',
  ],
  es: [
    'Año anterior', 'Año siguiente', 'Fecha futura', 'El total de tokens de {names} es desconocido; la información emergente conserva la entrada y salida reportadas.',
    'Telemetría local', 'Comprueba la configuración del usuario para registrar usos futuros. El espacio de trabajo, el entorno y las políticas pueden tener prioridad. La salida nueva se guarda para validar, sin duplicar el uso nativo.', 'Comprobar de nuevo', 'Comprobando en segundo plano…', 'Falló la comprobación. Inténtalo de nuevo.', 'No se encontró un Agent local con configuración compatible.',
    'Salida sin configurar', 'Configurado para el usuario', 'Revisión manual necesaria', 'Guía oficial', 'Crear lanzador', 'Configurar',
    'Vista previa del cambio', 'Ubicación de salida', 'Crea un script dedicado y úsalo para iniciar Copilot. Solo afecta a ese proceso; conserva el estado de la CLI y el entorno global.', 'Combina solo las claves indicadas, conserva otros ajustes y comentarios y guarda una copia del original.', 'Activa el receptor local de telemetría. Mantén esta aplicación abierta para recibirlos.', 'Aplicar configuración', 'Cancelar',
    'Configuración guardada. Recarga el IDE o reinicia el Agent. El uso normal creará registros futuros; no se recupera el historial.', 'Deshacer este cambio', 'No se pudo escribir. Se conserva el original; revisa la ruta o genera otra vista previa.', 'Se detectaron variables del entorno que prevalecen. Revisa el entorno de inicio.', 'Se detectó configuración administrada. Revisa las políticas del administrador.',
    'No se han verificado las claves de esta instalación. Consulta la guía oficial.', 'La configuración no se puede leer, es demasiado grande, no es válida o contiene claves duplicadas.', 'La ruta contiene enlaces o no es absoluta. Revísala manualmente.', 'La telemetría del usuario está desactivada. Ajusta ese valor primero.',
    'Estas claves están incluidas explícitamente en Settings Sync. Revisa las exclusiones.', 'La configuración cambió tras la vista previa o aplicación. Se conservan tus cambios; genera otra vista previa.', 'El archivo es de solo lectura. No se escribió nada.', 'No se pudo abrir el puerto receptor. No se modificó la configuración del Agent.',
    'El puerto receptor cambió. Reinicia esta aplicación y vuelve a comprobar.', 'La vista previa caducó o la aplicación se reinició. Genera otra vista previa.',
  ],
  fr: [
    'Année précédente', 'Année suivante', 'Date future', 'Le total des tokens de {names} est inconnu ; les infobulles conservent les entrées et sorties signalées.',
    'Télémétrie locale', 'Vérifie les paramètres utilisateur pour les futurs enregistrements. L’espace de travail, l’environnement et les stratégies peuvent prévaloir. La nouvelle sortie est conservée pour validation sans doubler l’usage natif.', 'Vérifier à nouveau', 'Vérification en arrière-plan…', 'La vérification a échoué. Réessayez.', 'Aucun Agent local avec configuration compatible trouvé.',
    'Sortie non configurée', 'Configuré pour l’utilisateur', 'Vérification manuelle requise', 'Guide officiel', 'Créer un lanceur', 'Configurer',
    'Aperçu des modifications', 'Emplacement de sortie', 'Crée un script dédié pour lancer Copilot. Seul ce processus est affecté ; les fichiers d’état de la CLI et l’environnement global sont préservés.', 'Fusionne uniquement les clés indiquées, préserve les autres paramètres et commentaires et sauvegarde l’original.', 'Active le récepteur local de télémétrie. Gardez cette application ouverte pendant la réception.', 'Appliquer', 'Annuler',
    'Configuration enregistrée. Rechargez l’IDE ou redémarrez l’Agent. L’usage normal créera les futurs enregistrements ; le passé ne peut pas être récupéré.', 'Annuler cette modification', 'Écriture impossible. L’original est préservé ; vérifiez le chemin ou recréez l’aperçu.', 'Des variables d’environnement prioritaires ont été détectées. Vérifiez l’environnement de lancement.', 'Une configuration administrée a été détectée. Vérifiez les stratégies de l’administrateur.',
    'Les clés de cette installation ne sont pas vérifiées. Consultez le guide officiel.', 'Configuration illisible, trop grande, invalide ou avec des clés en double. Corrigez-la d’abord.', 'Le chemin contient des liens ou n’est pas absolu. Vérifiez-le manuellement.', 'La télémétrie utilisateur est désactivée. Ajustez ce paramètre d’abord.',
    'Ces clés sont explicitement incluses dans Settings Sync. Vérifiez les exclusions.', 'La configuration a changé depuis l’aperçu ou l’application. Vos modifications sont préservées ; recréez l’aperçu.', 'Le fichier est en lecture seule. Aucune écriture.', 'Impossible d’ouvrir le port récepteur. La configuration de l’Agent est préservée.',
    'Le port récepteur a changé. Redémarrez cette application et revérifiez.', 'L’aperçu a expiré ou l’application a redémarré. Recréez l’aperçu.',
  ],
  de: [
    'Vorheriges Jahr', 'Nächstes Jahr', 'Zukünftiges Datum', 'Die Gesamttokens für {names} sind unbekannt; Tooltips zeigen gemeldete Ein- und Ausgaben.',
    'Lokale Telemetrie', 'Prüft Benutzereinstellungen für künftige Aufzeichnungen. Arbeitsbereich, Startumgebung und Richtlinien können Vorrang haben. Neue Ausgaben dienen der Prüfung und werden nicht zum nativen Verbrauch hinzuaddiert.', 'Erneut prüfen', 'Prüfung im Hintergrund…', 'Die Prüfung ist fehlgeschlagen. Bitte erneut versuchen.', 'Kein lokaler Agent mit unterstützter Einrichtung gefunden.',
    'Ausgabe nicht eingerichtet', 'Auf Benutzerebene eingerichtet', 'Manuelle Prüfung nötig', 'Offizielle Anleitung', 'Startskript erstellen', 'Einrichten',
    'Änderungsvorschau', 'Ausgabeort', 'Erstellt ein eigenes Skript zum Starten von Copilot. Nur dieser Prozess ist betroffen; CLI-Zustand und globale Umgebung bleiben erhalten.', 'Führt nur die angegebenen Schlüssel zusammen, erhält andere Einstellungen und Kommentare und sichert die Originaldatei.', 'Aktiviert den lokalen Telemetrieempfänger. Lassen Sie diese Anwendung zum Empfang geöffnet.', 'Konfiguration anwenden', 'Abbrechen',
    'Konfiguration gespeichert. IDE neu laden oder Agent neu starten. Die normale Nutzung erzeugt künftige Aufzeichnungen; frühere Nutzung wird nicht wiederhergestellt.', 'Diese Änderung rückgängig machen', 'Schreiben fehlgeschlagen. Das Original bleibt erhalten; Pfad prüfen oder Vorschau neu erstellen.', 'Überschreibungen durch Prozessvariablen erkannt. Startumgebung prüfen.', 'Verwaltete Konfiguration erkannt. Administratorrichtlinien manuell prüfen.',
    'Die Schlüssel dieser Installation sind nicht verifiziert. Offizielle Anleitung verwenden.', 'Konfiguration unlesbar, zu groß, ungültig oder mit doppelten Schlüsseln. Zuerst korrigieren.', 'Der Pfad enthält Links oder ist nicht absolut. Manuell prüfen.', 'Benutzertelemetrie ist deaktiviert. Diese Einstellung zuerst anpassen.',
    'Diese Schlüssel sind ausdrücklich in Settings Sync enthalten. Ausschlüsse prüfen.', 'Konfiguration nach Vorschau oder Anwendung geändert. Ihre Änderungen bleiben erhalten; Vorschau neu erstellen.', 'Konfiguration ist schreibgeschützt. Nichts wurde geschrieben.', 'Lokaler Empfangsport konnte nicht geöffnet werden. Agent-Konfiguration blieb unverändert.',
    'Empfangsport geändert. Anwendung neu starten und erneut prüfen.', 'Vorschau abgelaufen oder Anwendung neu gestartet. Vorschau neu erstellen.',
  ],
  'pt-BR': [
    'Ano anterior', 'Próximo ano', 'Data futura', 'O total de tokens de {names} é desconhecido; as dicas mostram a entrada e saída informadas.',
    'Telemetria local', 'Verifica configurações do usuário para registros futuros. O espaço de trabalho, ambiente e políticas podem ter prioridade. Novas saídas são guardadas para validação, sem duplicar o uso nativo.', 'Verificar novamente', 'Verificando em segundo plano…', 'A verificação falhou. Tente novamente.', 'Nenhum Agent local com configuração compatível foi encontrado.',
    'Saída não configurada', 'Configurado para o usuário', 'Revisão manual necessária', 'Guia oficial', 'Criar iniciador', 'Configurar',
    'Prévia das alterações', 'Local de saída', 'Cria um script dedicado para iniciar o Copilot. Afeta apenas esse processo; preserva o estado da CLI e o ambiente global.', 'Mescla apenas as chaves indicadas, preserva outros ajustes e comentários e salva uma cópia do original.', 'Ativa o receptor local de telemetria. Mantenha este aplicativo aberto para receber registros.', 'Aplicar configuração', 'Cancelar',
    'Configuração salva. Recarregue o IDE ou reinicie o Agent. O uso normal criará registros futuros; o histórico não pode ser recuperado.', 'Desfazer esta alteração', 'Não foi possível escrever. O original foi preservado; verifique o caminho ou gere outra prévia.', 'Foram detectadas variáveis de processo com prioridade. Verifique o ambiente de inicialização.', 'Foi detectada configuração gerenciada. Verifique as políticas do administrador.',
    'As chaves desta instalação não foram verificadas. Consulte o guia oficial.', 'Configuração ilegível, muito grande, inválida ou com chaves duplicadas. Corrija primeiro.', 'O caminho contém links ou não é absoluto. Verifique manualmente.', 'A telemetria do usuário está desativada. Ajuste isso primeiro.',
    'Estas chaves estão explicitamente incluídas no Settings Sync. Verifique as exclusões.', 'A configuração mudou após a prévia ou aplicação. Suas edições foram preservadas; gere outra prévia.', 'O arquivo é somente leitura. Nada foi escrito.', 'Não foi possível abrir a porta receptora. A configuração do Agent foi preservada.',
    'A porta receptora mudou. Reinicie este aplicativo e verifique novamente.', 'A prévia expirou ou o aplicativo reiniciou. Gere outra prévia.',
  ],
  ru: [
    'Предыдущий год', 'Следующий год', 'Будущая дата', 'Общее число токенов для {names} неизвестно; подсказки сохраняют сообщённые входные и выходные значения.',
    'Локальная телеметрия', 'Проверяет настройки пользователя для будущих записей. Рабочая область, среда запуска и политики могут иметь приоритет. Новые данные сохраняются для проверки без повторного учёта нативного использования.', 'Проверить снова', 'Проверка в фоне…', 'Проверка не удалась. Повторите попытку.', 'Локальный Agent с поддерживаемой настройкой не найден.',
    'Вывод не настроен', 'Настроено для пользователя', 'Нужна ручная проверка', 'Официальная инструкция', 'Создать скрипт запуска', 'Настроить',
    'Предпросмотр изменений', 'Путь вывода', 'Создаёт отдельный скрипт запуска Copilot. Меняет только среду этого процесса; состояние CLI и глобальная среда сохраняются.', 'Объединяет только указанные ключи, сохраняет прочие настройки и комментарии и создаёт резервную копию оригинала.', 'Включает локальный приёмник телеметрии. Оставьте приложение открытым для приёма.', 'Применить настройки', 'Отмена',
    'Настройки сохранены. Перезагрузите IDE или перезапустите Agent. Обычная работа создаст будущие записи; прошлые данные не восстановятся.', 'Отменить это изменение', 'Не удалось записать настройки. Оригинал сохранён; проверьте путь или повторите предпросмотр.', 'Обнаружено переопределение через среду процесса. Проверьте среду запуска.', 'Обнаружены управляемые настройки. Проверьте политики администратора вручную.',
    'Ключи этой установки не проверены. Используйте официальную инструкцию.', 'Настройки недоступны, слишком велики, некорректны или содержат повторяющиеся ключи. Сначала исправьте их.', 'Путь содержит ссылки или не является абсолютным. Проверьте вручную.', 'Телеметрия пользователя отключена. Сначала измените эту настройку.',
    'Эти ключи явно включены в Settings Sync. Проверьте исключения.', 'Настройки изменились после предпросмотра или применения. Ваши правки сохранены; повторите предпросмотр.', 'Файл доступен только для чтения. Ничего не записано.', 'Не удалось открыть локальный порт приёмника. Настройки Agent не изменены.',
    'Порт приёмника изменился. Перезапустите приложение и проверьте снова.', 'Предпросмотр истёк или приложение перезапущено. Повторите предпросмотр.',
  ],
};

export const telemetryCatalogs: Record<string, Record<string, string>> = Object.fromEntries(
  Object.entries(phrases).map(([locale, values]) => {
    if (values.length !== keys.length) throw new Error(`Incomplete telemetry translation: ${locale}`);
    return [locale, Object.fromEntries(keys.map((key, i) => [key, values[i]]))];
  }),
);

const destinationMessages: Record<string, string> = {
  'zh-CN': '已有输出目标但遥测未启用，请先手工核对；保留原目标。',
  'zh-TW': '已有輸出目標但遙測未啟用，請先手動核對；保留原目標。',
  en: 'An output destination exists but telemetry is disabled. Review it manually; the destination is preserved.',
  ja: '出力先はありますがテレメトリが無効です。出力先を保持しているため、手動で確認してください。',
  ko: '출력 대상이 있지만 원격 분석이 꺼져 있습니다. 기존 대상은 유지되므로 수동으로 확인하세요.',
  es: 'Hay un destino de salida pero la telemetría está desactivada. Revísalo manualmente; se conserva el destino.',
  fr: 'Une destination existe mais la télémétrie est désactivée. Vérifiez manuellement ; la destination est préservée.',
  de: 'Ein Ausgabeziel ist vorhanden, Telemetrie aber deaktiviert. Manuell prüfen; das Ziel bleibt erhalten.',
  'pt-BR': 'Há um destino de saída, mas a telemetria está desativada. Revise manualmente; o destino foi preservado.',
  ru: 'Адрес вывода задан, но телеметрия отключена. Проверьте вручную; адрес сохранён.',
};
for (const [locale, catalog] of Object.entries(telemetryCatalogs)) {
  catalog['telemetry.reason.existing_destination'] = destinationMessages[locale];
}

const batchKeys = ['telemetry.configureAll', 'telemetry.details', 'telemetry.brief',
  'telemetry.progress', 'telemetry.result', 'telemetry.bulkHint'];
const batchMessages: Record<string, string[]> = {
  'zh-CN': ['一键开启全部', '查看详情', '检测到 {count} 项本机 Agent 遥测配置待开启。',
    '正在配置：{completed}/{total} 项。', '已保存 {success} 项，失败 {failed} 项，需手工核对 {manual} 项。',
    '批量配置保留已有输出和其他设置，跳过受管理或冲突项；保存后需重载 IDE/Agent，CLI 启动脚本需自行使用。'],
  en: ['Enable all', 'View details', '{count} local Agent telemetry configurations need setup.',
    'Configuring: {completed}/{total}.', 'Saved {success}; failed {failed}; manual review {manual}.',
    'Batch setup preserves existing outputs and other settings, and skips managed or conflicting entries. Reload the IDE/Agent after saving; CLI launchers must be used separately.'],
  'zh-TW': ['一鍵開啟全部', '查看詳情', '偵測到 {count} 項本機 Agent 遙測設定待開啟。',
    '正在設定：{completed}/{total} 項。', '已儲存 {success} 項，失敗 {failed} 項，需手動核對 {manual} 項。',
    '批次設定保留既有輸出和其他設定，略過受管理或衝突項；儲存後須重新載入 IDE/Agent，CLI 啟動指令碼須自行使用。'],
  ja: ['すべて有効にする', '詳細を見る', 'ローカル Agent のテレメトリ設定 {count} 件が未設定です。',
    '設定中：{completed}/{total} 件。', '保存 {success} 件、失敗 {failed} 件、手動確認 {manual} 件。',
    '一括設定は既存の出力や他の設定を保持し、管理対象や競合項目をスキップします。保存後に IDE/Agent を再起動し、CLI は専用スクリプトで起動してください。'],
  ko: ['모두 활성화', '상세 보기', '로컬 Agent 원격 분석 설정 {count}개가 필요합니다.',
    '설정 중: {completed}/{total}개.', '저장 {success}개, 실패 {failed}개, 수동 확인 {manual}개.',
    '일괄 설정은 기존 출력과 다른 설정을 유지하고 관리 또는 충돌 항목을 건너뜁니다. 저장 후 IDE/Agent를 다시 시작하고 CLI는 전용 스크립트로 실행하세요.'],
  es: ['Activar todo', 'Ver detalles', '{count} configuraciones de telemetría de Agent locales requieren configuración.',
    'Configurando: {completed}/{total}.', 'Guardadas {success}; fallidas {failed}; revisión manual {manual}.',
    'La configuración por lotes conserva salidas y otros ajustes y omite entradas administradas o en conflicto. Reinicia el IDE/Agent después de guardar; usa los lanzadores CLI por separado.'],
  fr: ['Tout activer', 'Voir les détails', '{count} configurations de télémétrie des Agents locaux sont à configurer.',
    'Configuration : {completed}/{total}.', 'Enregistrées {success} ; échecs {failed} ; vérification manuelle {manual}.',
    'La configuration groupée préserve les sorties et autres paramètres, et ignore les entrées gérées ou en conflit. Relancez IDE/Agent après enregistrement ; utilisez les scripts CLI séparément.'],
  de: ['Alle aktivieren', 'Details anzeigen', '{count} Telemetriekonfigurationen lokaler Agents müssen eingerichtet werden.',
    'Einrichtung: {completed}/{total}.', 'Gespeichert {success}; fehlgeschlagen {failed}; manuelle Prüfung {manual}.',
    'Die Sammeleinrichtung erhält bestehende Ausgaben und andere Einstellungen und überspringt verwaltete oder widersprüchliche Einträge. IDE/Agent danach neu starten; CLI-Startskripte separat verwenden.'],
  'pt-BR': ['Ativar todos', 'Ver detalhes', '{count} configurações de telemetria de Agents locais precisam ser configuradas.',
    'Configurando: {completed}/{total}.', 'Salvas {success}; falhas {failed}; revisão manual {manual}.',
    'A configuração em lote preserva saídas e outros ajustes e ignora entradas gerenciadas ou em conflito. Reinicie o IDE/Agent após salvar; use os iniciadores CLI separadamente.'],
  ru: ['Включить всё', 'Подробнее', 'Требуется настройка {count} конфигураций телеметрии локальных Agents.',
    'Настройка: {completed}/{total}.', 'Сохранено {success}; ошибок {failed}; ручная проверка {manual}.',
    'Пакетная настройка сохраняет вывод и другие параметры и пропускает управляемые или конфликтующие записи. Затем перезапустите IDE/Agent; для CLI используйте отдельные скрипты запуска.'],
};
for (const [locale, catalog] of Object.entries(telemetryCatalogs)) {
  const messages = batchMessages[locale];
  if (messages.length !== batchKeys.length) throw new Error('Incomplete telemetry batch translation: ' + locale);
  batchKeys.forEach((key, i) => { catalog[key] = messages[i]; });
}

export function telemetryReasonKey(code: string): string {
  const known = ['environment_override', 'managed_policy', 'unsupported_version', 'unsafe_path', 'telemetry_disabled',
    'sync_conflict', 'config_changed', 'read_only', 'receiver_bind_failed', 'receiver_restart_required', 'preview_expired', 'existing_destination', 'authentication_unverified', 'credential_store_unavailable'];
  return 'telemetry.reason.' + (known.includes(code) ? code : 'invalid_config');
}

const authenticationMessages: Record<string, [string, string, string]> = {
  'zh-CN': ['此 Agent 的认证配置尚未核验，请手工核对；本机原生采集仍可使用。', '系统凭据存储不可用，HTTP 配置未写入；可使用已核验的本地文件输出。', '接收器会为此本机 Agent 创建独立认证令牌。预览不显示令牌；撤销时吊销。接收时请保持本应用运行。'],
  'zh-TW': ['此 Agent 的驗證設定尚未核驗，請手動核對；本機原生採集仍可使用。', '系統認證儲存不可用，未寫入 HTTP 設定；可使用已核驗的本機檔案輸出。', '接收器會為此本機 Agent 建立獨立驗證權杖。預覽不顯示權杖，復原時撤銷。接收時請保持應用程式執行。'],
  en: ['Authentication setup for this Agent is unverified. Review it manually; native local collection remains available.', 'System credential storage is unavailable. HTTP configuration was not written; use a verified local file exporter.', 'A separate authentication token will be created for this local Agent. The preview hides it; undo revokes it. Keep this app running to receive data.'],
  ja: ['この Agent の認証設定は未検証です。手動で確認してください。既存のローカル収集は利用できます。', 'システム資格情報ストアが利用できません。HTTP 設定は書き込まれていません。検証済みのローカルファイル出力を利用してください。', 'このローカル Agent 専用の認証トークンを作成します。プレビューには表示せず、元に戻すと失効します。受信中はアプリを起動しておいてください。'],
  ko: ['이 Agent의 인증 설정은 검증되지 않았습니다. 수동으로 확인하세요. 기존 로컬 수집은 사용할 수 있습니다.', '시스템 자격 증명 저장소를 사용할 수 없습니다. HTTP 설정은 쓰지 않았습니다. 검증된 로컬 파일 출력을 사용하세요.', '이 로컬 Agent 전용 인증 토큰을 만듭니다. 미리 보기에는 표시하지 않으며 실행 취소 시 폐기합니다. 수신 중에는 앱을 실행해 두세요.'],
  es: ['La autenticación de este Agent no está verificada. Revísala manualmente; la recopilación local nativa sigue disponible.', 'El almacén de credenciales no está disponible. No se escribió la configuración HTTP; usa una salida local verificada.', 'Se creará un token propio para este Agent local. La vista previa lo oculta y deshacer lo revoca. Mantén esta aplicación abierta para recibir datos.'],
  fr: ['L’authentification de cet Agent n’est pas vérifiée. Vérifiez manuellement ; la collecte locale native reste disponible.', 'Le stockage système des identifiants est indisponible. La configuration HTTP n’a pas été écrite ; utilisez une sortie locale vérifiée.', 'Un jeton propre à cet Agent local sera créé. L’aperçu le masque et l’annulation le révoque. Gardez cette application ouverte pendant la réception.'],
  de: ['Die Authentifizierung dieses Agents ist nicht verifiziert. Manuell prüfen; die native lokale Erfassung bleibt verfügbar.', 'Der System-Anmeldeinformationsspeicher ist nicht verfügbar. HTTP wurde nicht eingerichtet; einen verifizierten lokalen Dateiexporter verwenden.', 'Für diesen lokalen Agent wird ein eigener Token erstellt. Die Vorschau verbirgt ihn; Rückgängigmachen widerruft ihn. Die App zum Empfang geöffnet lassen.'],
  'pt-BR': ['A autenticação deste Agent não foi verificada. Revise manualmente; a coleta local nativa continua disponível.', 'O armazenamento de credenciais está indisponível. A configuração HTTP não foi escrita; use uma saída local verificada.', 'Será criado um token exclusivo para este Agent local. A prévia o oculta; desfazer o revoga. Mantenha este aplicativo aberto para receber dados.'],
  ru: ['Аутентификация этого Agent не проверена. Проверьте вручную; нативный локальный сбор остаётся доступным.', 'Системное хранилище учётных данных недоступно. Настройки HTTP не записаны; используйте проверенный локальный файловый вывод.', 'Для этого локального Agent будет создан отдельный токен. Предпросмотр скрывает его, отмена отзывает. Оставьте приложение открытым для приёма данных.'],
};
for (const [locale, catalog] of Object.entries(telemetryCatalogs)) {
  const [unverified, unavailable, hint] = authenticationMessages[locale];
  catalog['telemetry.reason.authentication_unverified'] = unverified;
  catalog['telemetry.reason.credential_store_unavailable'] = unavailable;
  catalog['telemetry.receiverHint'] = hint;
}
