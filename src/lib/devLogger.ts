const IS_DEV = import.meta.env.DEV;

function format(scope: string, message: string): string {
  return `[${scope}] ${message}`;
}

export function devLog(scope: string, message: string, ...args: unknown[]): void {
  if (!IS_DEV) return;
  console.log(format(scope, message), ...args);
}

export function devWarn(scope: string, message: string, ...args: unknown[]): void {
  if (!IS_DEV) return;
  console.warn(format(scope, message), ...args);
}
