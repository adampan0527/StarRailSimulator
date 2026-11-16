import { invoke } from '@tauri-apps/api/core';
import { readDesktopStatus } from './status';

export const checkDesktop = () => readDesktopStatus(invoke);
