export interface ScreenshotConfig {
  savePath?: string;
  format?: 'png' | 'jpg' | 'webp';
  quality?: number;
  mosaicBlockSize?: number;
  defaultColor?: string;
  defaultSize?: number;
  locale?: 'zh-CN' | 'en';
  showDebugHud?: boolean;
  metricsIntervalMs?: number;
}

export interface ScreenshotSession extends NodeJS.EventEmitter {
  cancel(): void;
}

export function start(config?: ScreenshotConfig): ScreenshotSession;
