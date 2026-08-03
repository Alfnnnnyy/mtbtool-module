export class ApiError extends Error {
  constructor(message: string, public cause?: unknown) {
    super(message);
    this.name = 'ApiError';
  }
}

export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export function shellEscapeArg(arg: string): string {
  return `'${arg.replace(/'/g, "'\"'\"'")}'`;
}

export function buildCliCommand(cmd: string, args?: Record<string, string>): string {
  let command = cmd;
  if (args) {
    for (const [k, v] of Object.entries(args)) {
      if (v === '' || v === undefined || v === null) {
        command += ` --${k}`;
      } else {
        command += ` --${k} ${shellEscapeArg(v)}`;
      }
    }
  }
  return command;
}

export async function execRaw(cmd: string, args?: Record<string, string>): Promise<unknown> {
  const modDir = typeof localStorage !== 'undefined' && localStorage.getItem('mtbtool_moddir')
    ? localStorage.getItem('mtbtool_moddir')
    : '/data/adb/modules/mtbtool';

  const fullCmd = buildCliCommand(cmd, args);
  const binaryPath = `${modDir}/bin/mtbctl`;
  const ksuCmd = `${binaryPath} ${fullCmd}`;

  // 1. Try KernelSU dynamic import (optional platform module for WebUI under KernelSU environment)
  try {
    const ksu = await import(/* @vite-ignore */ 'kernelsu') as { exec?: (cmd: string) => Promise<{ stdout: string }> };
    if (ksu && typeof ksu.exec === 'function') {
      const res = await ksu.exec(ksuCmd);
      if (res && typeof res.stdout === 'string') {
        const parsed = JSON.parse(res.stdout) as { ok?: boolean; error?: string; message?: string };
        if (parsed && parsed.ok === false) {
          throw new ApiError(parsed.error || parsed.message || 'CLI command returned ok: false');
        }
        return parsed;
      }
    }
  } catch (err: unknown) {
    if (err instanceof ApiError) {
      throw err;
    }
    // Dynamic import or ksu.exec failed, proceed to fallback HTTP API
  }

  // 2. HTTP Fallback: fetch http://127.0.0.1:28082/api or localhost (NEVER Promise.withResolvers per contract line 106)
  const postData = { cmd, args: args || {} };

  let lastError: unknown = null;
  const attempts = [
    'http://127.0.0.1:28082/api',
    'http://localhost:28082/api',
    'http://127.0.0.1:28082/api'
  ];

  for (let i = 0; i < attempts.length; i++) {
    if (i > 0) {
      await sleep(500);
    }
    try {
      const resp = await fetch(attempts[i], {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(postData)
      });

      if (!resp.ok) {
        throw new Error(`HTTP ${resp.status} ${resp.statusText}`);
      }

      const parsed = await resp.json() as { ok?: boolean; error?: string; message?: string };
      if (parsed && parsed.ok === false) {
        throw new ApiError(parsed.error || parsed.message || 'API returned ok: false');
      }
      return parsed;
    } catch (err: unknown) {
      if (err instanceof ApiError) {
        throw err;
      }
      lastError = err;
    }
  }

  throw new ApiError(
    `Failed to execute '${cmd}' via KSU and HTTP fallback: ${lastError instanceof Error ? lastError.message : String(lastError)}`,
    lastError
  );
}

export async function api(cmd: string, args?: Record<string, string>): Promise<unknown> {
  return execRaw(cmd, args);
}
