import { invoke } from '@tauri-apps/api/core';
import { errorCode } from '../training/client';

export type AccountSelection = {
  active_uid: number | null;
  accounts: { uid: number; remark: string }[];
};

let scope: string | undefined;

export function setAccountScope(uid: number | null) {
  scope = uid === null ? 'local' : String(uid);
}

export function accountScope() {
  return scope;
}

export function invokeForAccount<T>(
  command: string,
  args?: Record<string, unknown>,
  expectedScope = accountScope(),
) {
  return invoke<T>(command, { ...args, scope: expectedScope });
}

export const gameAccounts = {
  list: () => invoke<AccountSelection>('account_list'),
  select: (uid: number) => invoke<AccountSelection>('account_select', { uid }),
  rename: (uid: number, remark: string) =>
    invoke<AccountSelection>('account_rename', { uid, remark }),
};

export function accountError(error: unknown) {
  switch (errorCode(error)) {
    case 'invalid_account_selection':
      return '当前账号已变更，请确认账号后重试。';
    case 'invalid_account_remark':
      return '账号备注最多 48 个字符，请使用可显示的文字。';
    default:
      return '无法读取或保存账号信息，请重试。';
  }
}
