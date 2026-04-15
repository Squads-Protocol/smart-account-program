import { ErrorWithLogs, initCusper } from "@metaplex-foundation/cusper";
import { errorFromCode } from "./generated";

const cusper = initCusper(errorFromCode);

export function translateAndThrowAnchorError(err: unknown): never {
  if (!isErrorWithLogs(err)) {
    throw err;
  }

  const translatedError = cusper.errorFromProgramLogs(err.logs) ?? err;

  if (typeof Error.captureStackTrace === "function") {
    Error.captureStackTrace(translatedError, translateAndThrowAnchorError);
  }

  try {
    (translatedError as unknown as ErrorWithLogs).logs = err.logs;
  } catch {
    Object.defineProperty(translatedError, 'logs', {
      value: err.logs,
      writable: true,
      enumerable: true,
      configurable: true,
    });
  }

  throw translatedError;
}

export const isErrorWithLogs = (err: unknown): err is ErrorWithLogs => {
  return Boolean(
    err && typeof err === "object" && "logs" in err && Array.isArray(err.logs)
  );
};
