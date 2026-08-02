/** The daemon answered with a JSON-RPC error object. */
export class RpcFailure extends Error {
  readonly code: number;

  constructor(code: number, message: string) {
    super(message);
    this.name = "RpcFailure";
    this.code = code;
  }
}

let nextId = 1;

async function call<T>(method: string, params: unknown[]): Promise<T> {
  const response = await fetch("/rpc", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: nextId++, method, params }),
  });

  if (!response.ok) {
    throw new Error(`the daemon answered ${response.status}`);
  }

  const answer = await response.json();
  if (answer.error) {
    throw new RpcFailure(answer.error.code, answer.error.message);
  }
  return answer.result as T;
}

export function getProfile(): Promise<unknown> {
  return call("profile.get", []);
}

export function setProfile(profile: unknown): Promise<null> {
  return call<null>("profile.set", [profile]);
}
