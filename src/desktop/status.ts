export type DesktopStatus = { version: string };

export async function readDesktopStatus(
  invoke: (command: string) => Promise<unknown>,
): Promise<DesktopStatus> {
  let response: unknown;
  try {
    response = await invoke('desktop_status');
  } catch (cause) {
    throw new Error('DESKTOP_CALL_FAILED', { cause });
  }

  if (
    typeof response !== 'object' ||
    response === null ||
    !('version' in response) ||
    typeof response.version !== 'string' ||
    response.version.trim() === ''
  ) {
    throw new Error('INVALID_DESKTOP_STATUS');
  }

  return { version: response.version };
}
