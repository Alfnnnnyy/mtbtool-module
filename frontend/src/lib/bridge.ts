export class ApiError extends Error {
  constructor(message: string, public cause?: unknown) {
    super(message);
    this.name = 'ApiError';
  }
}

export function sleep(ms: number): Promise<void> {
  const { promise, resolve } = Promise.withResolvers<void>();
  setTimeout(resolve, ms);
  return promise;
}

export function encodeBase64Url(input: string | Uint8Array): string {
  let bytes: Uint8Array;
  if (typeof input === 'string') {
    bytes = new TextEncoder().encode(input);
  } else {
    bytes = input;
  }
  let binary = '';
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  const base64 = typeof btoa === 'function' ? btoa(binary) : Buffer.from(bytes).toString('base64');
  return base64
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=/g, '');
}

const MODDIR = '/data/adb/modules/mtbtool';

type ExecResult = { errno?: number; stdout?: string; stderr?: string } | string;
type ExecFn = (cmd: string) => Promise<ExecResult> | ExecResult;

interface KsuModule {
  exec?: ExecFn;
  default?: { exec?: ExecFn };
}

interface WindowWithKsu {
  kernelsu?: { exec?: ExecFn };
}

interface RpcResponse {
  ok?: boolean;
  error?: string;
}

export async function rpc(method: string, params?: Record<string, unknown>): Promise<unknown> {
  const payloadObj = { method, params: params || {} };
  const jsonStr = JSON.stringify(payloadObj);
  const b64Payload = encodeBase64Url(jsonStr);
  const cmd = `${MODDIR}/bin/mtbctl rpc --b64 ${b64Payload}`;

  let execFn: ExecFn | null = null;

  try {
    // Platform-specific module that does not exist everywhere in standard browser
    const ksu = (await import(/* @vite-ignore */ 'kernelsu')) as KsuModule;
    if (ksu && typeof ksu.exec === 'function') {
      execFn = ksu.exec;
    } else if (ksu && ksu.default && typeof ksu.default.exec === 'function') {
      execFn = ksu.default.exec;
    }
  } catch {
    // Dynamic import failed, check window fallback next
  }

  if (!execFn) {
    const win = typeof window !== 'undefined' ? (window as unknown as WindowWithKsu) : undefined;
    if (win?.kernelsu && typeof win.kernelsu.exec === 'function') {
      execFn = win.kernelsu.exec;
    }
  }

  if (!execFn) {
    throw new ApiError('No exec bridge available — run under KernelSU/ReSukiSU or a WebUI X host');
  }

  let execRes: ExecResult;
  try {
    execRes = await execFn(cmd);
  } catch (err: unknown) {
    throw new ApiError(`Exec bridge call failed: ${err instanceof Error ? err.message : String(err)}`, err);
  }

  let stdoutStr = '';
  if (typeof execRes === 'string') {
    stdoutStr = execRes;
  } else if (execRes && typeof execRes.stdout === 'string') {
    stdoutStr = execRes.stdout;
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(stdoutStr);
  } catch (err: unknown) {
    throw new ApiError(`Failed to parse RPC JSON response: ${err instanceof Error ? err.message : String(err)}`, err);
  }

  if (parsed && typeof parsed === 'object') {
    const res = parsed as RpcResponse;
    if (res.ok === false) {
      throw new ApiError(res.error || 'RPC command returned ok: false');
    }
  }

  return parsed;
}
