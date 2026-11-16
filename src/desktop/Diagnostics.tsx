import { useState } from 'react';
import { checkDesktop } from './client';

type CheckState =
  | { kind: 'idle' }
  | { kind: 'pending' }
  | { kind: 'ready'; version: string }
  | { kind: 'failed'; message: string };

function describeCheck(state: CheckState) {
  switch (state.kind) {
    case 'idle':
      return '等待检测。';
    case 'pending':
      return '正在检测桌面连接…';
    case 'ready':
      return `连接成功 · 应用版本 ${state.version}`;
    case 'failed':
      return state.message;
  }
}

export function Diagnostics() {
  const [state, setState] = useState<CheckState>({ kind: 'idle' });

  async function runCheck() {
    setState({ kind: 'pending' });
    try {
      const { version } = await checkDesktop();
      setState({ kind: 'ready', version });
    } catch (error) {
      const badResponse =
        error instanceof Error && error.message === 'INVALID_DESKTOP_STATUS';
      setState({
        kind: 'failed',
        message: badResponse
          ? '未能读取应用版本。请重试。'
          : '无法连接。请在桌面应用中重试。',
      });
    }
  }

  return (
    <details className="diagnostics">
      <summary>桌面连接诊断</summary>
      <div className="form-actions">
        <button
          type="button"
          className="secondary"
          onClick={runCheck}
          disabled={state.kind === 'pending'}
        >
          检测连接
        </button>
        <span role="status">{describeCheck(state)}</span>
      </div>
    </details>
  );
}
