export {};
declare global {
  interface Window {
    LLMUsageDocsTheme: {
      apply: (theme: 'auto' | 'light' | 'dark', persist?: boolean) => void;
      updatePickers: () => void;
    };
  }
}
