/** The state of one round trip: not yet started is `null`, held by whoever owns this value. */
export type Async<T> =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ok"; data: T };
