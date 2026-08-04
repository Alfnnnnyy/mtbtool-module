// Bridge layer between the Svelte WebUI and the mtbctl backend.
//
// The WebUI executes EXACTLY ONE fixed command shape:
//   /data/adb/modules/mtbtool/bin/mtbctl rpc --b64 <base64url(json)>
// No user-controlled string is ever interpolated into a shell command; the
// payload is validated by the backend allowlist. Module path is hardcoded.
//
// Host bridge resolution order:
//   1. the official `kernelsu` npm package (static, bundled import — the
//      KernelSU/ReSukiSU manager WebView provides its native implementation)
//   2. a `window.kernelsu.exec` shim exposed by alternate hosts (e.g. WebUI X)
// Neither adapter is guessed: each is detected by documented surface.
import { exec as ksuExec } from 'kernelsu';

import { writable } from 'svelte/store';

export class ApiError extends Error {
  constructor(message: string, public cause?: unknown, public stderr?: string) {
    super(message);
    this.name = 'ApiError';
  }
}

export function sleep(ms: number): Promise<void> {
  // No Promise.withResolvers: old WebViews (Chrome < 119) break on it.
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export function encodeBase64Url(input: string | Uint8Array): string {
  const bytes = typeof input === 'string' ? new TextEncoder().encode(input) : input;
  let binary = '';
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  const base64 = typeof btoa === 'function' ? btoa(binary) : Buffer.from(bytes).toString('base64');
  return base64.replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
}

const MODDIR = '/data/adb/modules/mtbtool';
export const MTBCTL_PATH = `${MODDIR}/bin/mtbctl`;

type ExecResult = { errno: number; stdout: string; stderr: string };
type AsyncExecFn = (cmd: string) => Promise<ExecResult> | ExecResult;

export type BridgeKind = 'kernelsu' | 'window-kernelsu' | 'none';

interface WindowWithKsu {
  kernelsu?: { exec?: AsyncExecFn };
}
/**
 * Resolve the exec bridge once. Detection is by documented surface only:
 *   1. `window.kernelsu.exec` — the shim exposed by alternate hosts
 *      (WebUI X documents a kernelsu-compatible API) and older KSU managers
 *   2. the official bundled `kernelsu` package (its `exec` calls the `ksu`
 *      global that the KernelSU/ReSukiSU manager WebView injects)
 */
function detectBridge(): { kind: BridgeKind; exec: AsyncExecFn | null } {
  const win = typeof window !== 'undefined' ? (window as unknown as WindowWithKsu) : undefined;
  if (win?.kernelsu && typeof win.kernelsu.exec === 'function') {
    return { kind: 'window-kernelsu', exec: win.kernelsu.exec };
  }
  if (typeof ksuExec === 'function') {
    return { kind: 'kernelsu', exec: ksuExec as AsyncExecFn };
  }
  return { kind: 'none', exec: null };
}

const bridge = detectBridge();

/**
 * Global bridge status consumed by every screen for fail-safe gating.
 * `ready` is true only after the self-test runs `mtbctl probe` through the
 * bridge and gets valid JSON + a version back.
 */
export interface BridgeStatus {
  detected: BridgeKind;
  /** Bridge present but self-test not (yet) confirmed. */
  ready: boolean;
  selfTest: {
    ran: boolean;
    ok: boolean;
    errno: number | null;
    stderr: string;
    /** mtbctl_version reported by the self-test probe. */
    version: string;
  };
  probe: { ok: boolean; mtb_exists?: boolean; mtb_executable?: boolean; model?: string } | null;
  error: string | null;
}

function initialStatus(): BridgeStatus {
  return {
    detected: bridge.kind,
    ready: false,
    selfTest: { ran: false, ok: false, errno: null, stderr: '', version: '' },
    probe: null,
    error: bridge.kind === 'none' ? 'No exec bridge available — run under KernelSU/ReSukiSU or a WebUI X host' : null,
  };
}

export const bridgeStatus = writable<BridgeStatus>(initialStatus());

/** Run one fixed command through the bridge; sync or async ExecResult. */
async function runExec(cmd: string): Promise<ExecResult> {
  if (!bridge.exec) {
    throw new ApiError(initialStatus().error || 'No exec bridge available');
  }
  try {
    const res = await bridge.exec(cmd);
    if (res === undefined || res === null) {
      return { errno: -1, stdout: '', stderr: 'exec returned no result' };
    }
    if (typeof res === 'string') {
      return { errno: 0, stdout: res, stderr: '' };
    }
    return {
      errno: typeof res.errno === 'number' ? res.errno : -1,
      stdout: typeof res.stdout === 'string' ? res.stdout : '',
      stderr: typeof res.stderr === 'string' ? res.stderr : '',
    };
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    return { errno: -1, stdout: '', stderr: msg };
  }
}

/**
 * Self-test: runs ONLY `mtbctl probe` through the bridge and requires valid
 * JSON with a mtbctl_version. Updates the global bridgeStatus store.
 */
export async function runSelfTest(): Promise<BridgeStatus> {
  const res = await runExec(`${MTBCTL_PATH} probe`);
  let version = '';
  let ok = res.errno === 0;
  let stderr = res.stderr || '';
  try {
    const parsed = JSON.parse(res.stdout) as { ok?: boolean; mtbctl_version?: string };
    if (!parsed || parsed.ok !== true || !parsed.mtbctl_version) {
      ok = false;
      stderr = stderr || `probe returned unexpected JSON: ${res.stdout.slice(0, 200)}`;
    } else {
      version = parsed.mtbctl_version;
    }
  } catch (e) {
    ok = false;
    stderr = stderr || `probe did not return JSON: ${res.stdout.slice(0, 200)}`;
  }
  updateStatus({
    ready: ok,
    selfTest: { ran: true, ok, errno: res.errno, stderr: stderr.slice(0, 500), version },
    error: ok ? null : 'Bridge self-test failed — mtbctl probe did not succeed',
  });
  return currentStatus();
}

function currentStatus(): BridgeStatus {
  let s: BridgeStatus;
  bridgeStatus.subscribe((v) => (s = v))();
  return s!;
}

function updateStatus(patch: Partial<BridgeStatus>) {
  bridgeStatus.update((s) => ({ ...s, ...patch }));
}

/** Refresh the full probe result and keep gating state in sync. */
export async function refreshProbe(): Promise<BridgeStatus> {
  if (!bridge.exec) {
    return currentStatus();
  }
  const res = await runExec(`${MTBCTL_PATH} probe`);
  let probe: BridgeStatus['probe'] = null;
  try {
    probe = JSON.parse(res.stdout) as BridgeStatus['probe'];
  } catch {
    probe = null;
  }
  const ready = res.errno === 0 && !!probe && probe.ok === true;
  updateStatus({
    probe,
    ready,
    error: ready ? null : 'Probe failed — modem tool is not responding',
  });
  return currentStatus();
}

/** Fixed RPC call: `mtbctl rpc --b64 <payload>`, JSON parsed from stdout. */
export async function rpc(method: string, params?: Record<string, unknown>): Promise<unknown> {
  const payloadObj = { method, params: params || {} };
  const b64Payload = encodeBase64Url(JSON.stringify(payloadObj));
  const cmd = `${MTBCTL_PATH} rpc --b64 ${b64Payload}`;

  const res = await runExec(cmd);
  if (res.errno !== 0) {
    throw new ApiError(`Exec failed (errno ${res.errno})`, undefined, res.stderr);
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(res.stdout);
  } catch (err: unknown) {
    throw new ApiError(
      `Failed to parse RPC JSON response: ${err instanceof Error ? err.message : String(err)}`,
      err,
      res.stderr,
    );
  }

  if (parsed && typeof parsed === 'object') {
    const r = parsed as { ok?: boolean; error?: string };
    if (r.ok === false) {
      throw new ApiError(r.error || 'RPC command returned ok: false', undefined, res.stderr);
    }
  }
  return parsed;
}