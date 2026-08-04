// Minimal declaration for the KernelSU WebUI module (injected at runtime by
// the KSU manager / WebUI X host). Never bundled — kept external in vite.
declare module 'kernelsu' {
  interface ExecResult {
    errno?: number;
    stdout?: string;
    stderr?: string;
  }
  type ExecFn = (cmd: string) => ExecResult | Promise<ExecResult>;
  const kernelsu: { exec?: ExecFn };
  export default kernelsu;
  export const exec: ExecFn;
}
