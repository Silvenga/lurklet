export class RequestFailure extends Error {
  readonly status: number | undefined;
  readonly retryable: boolean;
  readonly retryAt: number;

  constructor(response?: Response, problem?: unknown) {
    const status = response?.status;
    super(
      !response
        ? "The browser could not reach Lurklet."
        : response.ok
          ? "Lurklet returned an unreadable response."
          : problem &&
              typeof problem === "object" &&
              "title" in problem &&
              typeof problem.title === "string"
            ? problem.title
            : "",
    );
    this.status = status;
    this.retryable =
      !response ||
      response.ok ||
      status === 429 ||
      status === 408 ||
      (status !== undefined && status >= 500 && status !== 501 && status !== 505);

    const headers = response?.headers;
    const age = Number(headers?.get("Age") ?? 0) || 0;
    const maxAge = Number(
      /(?:^|,)\s*max-age=(\d+)/i.exec(headers?.get("Cache-Control") ?? "")?.[1] ?? 0,
    );
    const retry = headers?.get("Retry-After") ?? "";
    const now = Date.now();
    const retryTime = /^\d+$/.test(retry) ? now + (Number(retry) - age) * 1000 : Date.parse(retry);
    this.retryAt = Math.max(
      now + (maxAge - age) * 1000,
      Number.isFinite(retryTime) ? retryTime : 0,
    );
  }
}
