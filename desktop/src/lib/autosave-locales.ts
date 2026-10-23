const keys = ['autoSaveHint', 'pending', 'saving', 'retrySave', 'invalidProvider'];
const phrases: Record<string, string[]> = {
  'zh-CN': ['设置会自动保存；输入停止片刻或离开输入框后生效。', '等待保存…', '正在保存…', '重试保存', '请完整填写供应商、地区和渠道。'],
  'zh-TW': ['設定會自動儲存；停止輸入片刻或離開輸入框後生效。', '等待儲存…', '正在儲存…', '重試儲存', '請完整填寫供應商、地區及通道。'],
  en: ['Settings save automatically after a short pause in typing or when you leave a field.', 'Waiting to save…', 'Saving…', 'Retry saving', 'Complete the provider, region and channel fields.'],
  ja: ['設定は自動保存されます。入力を少し止めるか、入力欄を離れると反映されます。', '保存待ち…', '保存中…', '保存を再試行', 'プロバイダー、地域、チャネルをすべて入力してください。'],
  ko: ['설정은 자동 저장됩니다. 입력을 잠시 멈추거나 입력란을 벗어나면 적용됩니다.', '저장 대기 중…', '저장 중…', '저장 재시도', '공급자, 지역 및 채널을 모두 입력하세요.'],
  es: ['Los ajustes se guardan automáticamente tras una breve pausa al escribir o al salir del campo.', 'Esperando para guardar…', 'Guardando…', 'Reintentar guardado', 'Completa el proveedor, la región y el canal.'],
  fr: ['Les paramètres sont enregistrés automatiquement après une courte pause de saisie ou à la sortie du champ.', 'En attente d’enregistrement…', 'Enregistrement…', 'Réessayer l’enregistrement', 'Renseignez le fournisseur, la région et le canal.'],
  de: ['Einstellungen werden nach einer kurzen Eingabepause oder beim Verlassen des Felds automatisch gespeichert.', 'Warten auf Speichern…', 'Speichern…', 'Speichern wiederholen', 'Anbieter, Region und Kanal vollständig ausfüllen.'],
  'pt-BR': ['As configurações são salvas automaticamente após uma breve pausa na digitação ou ao sair do campo.', 'Aguardando para salvar…', 'Salvando…', 'Tentar salvar novamente', 'Preencha o provedor, a região e o canal.'],
  ru: ['Настройки сохраняются автоматически после короткой паузы ввода или при выходе из поля.', 'Ожидание сохранения…', 'Сохранение…', 'Повторить сохранение', 'Заполните поля поставщика, региона и канала.'],
};

export const autosaveCatalogs = Object.fromEntries(Object.entries(phrases).map(([locale, values]) =>
  [locale, Object.fromEntries(keys.map((key, index) => [`settings.${key}`, values[index]]))]
)) as Record<string, Record<string, string>>;
