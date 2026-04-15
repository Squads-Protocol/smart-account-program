import { PublicKey } from "@solana/web3.js";
import { Payload } from "@sqds/smart-account/lib/generated";
import { TransactionPayloadDetails } from "@sqds/smart-account/src/generated/types";

/** Returns true if the given unix epoch is within a couple of seconds of now. */
export function isCloseToNow(
  unixEpoch: number | bigint,
  timeWindow: number = 2000
) {
  const timestamp = Number(unixEpoch) * 1000;
  return Math.abs(timestamp - Date.now()) < timeWindow;
}

/** Returns an array of numbers from min to max (inclusive) with the given step. */
export function range(min: number, max: number, step: number = 1) {
  const result = [];
  for (let i = min; i <= max; i += step) {
    result.push(i);
  }
  return result;
}

export function comparePubkeys(a: PublicKey, b: PublicKey) {
  return a.toBuffer().compare(b.toBuffer());
}

/**
 * Extracts the TransactionPayloadDetails from a Payload.
 * @param transactionPayload - The Payload to extract the TransactionPayloadDetails from.
 * @returns The TransactionPayloadDetails.
 */
export function extractTransactionPayloadDetails(
  transactionPayload: Payload
): TransactionPayloadDetails {
  if (transactionPayload.__kind === "TransactionPayload") {
    return transactionPayload.fields[0] as TransactionPayloadDetails;
  } else {
    throw new Error("Invalid transaction payload");
  }
}
