import type { Hex } from "viem";

type EthereumProvider = {
  request: (args: { method: string; params?: unknown[] }) => Promise<unknown>;
};

function getProvider(): EthereumProvider {
  const ethereum = (window as Window & { ethereum?: EthereumProvider }).ethereum;
  if (!ethereum?.request) {
    throw new Error("MetaMask is not installed");
  }
  return ethereum;
}

export async function sendAndWait(tx: {
  from: string;
  to: string;
  data: string;
  gas: string;
}): Promise<Hex> {
  const provider = getProvider();
  const hash = String(
    await provider.request({
      method: "eth_sendTransaction",
      params: [tx],
    }),
  );
  if (!hash.startsWith("0x")) {
    throw new Error("MetaMask did not return a transaction hash");
  }
  for (let i = 0; i < 90; i += 1) {
    const receipt = (await provider.request({
      method: "eth_getTransactionReceipt",
      params: [hash],
    })) as { status?: string } | null;
    if (receipt) {
      if (receipt.status && receipt.status !== "0x1") {
        throw new Error("transaction reverted");
      }
      return hash as Hex;
    }
    await new Promise((resolve) => window.setTimeout(resolve, 500));
  }
  throw new Error("timed out waiting for the transaction receipt");
}

export async function readAllowance(input: {
  token: string;
  owner: string;
  spender: string;
}): Promise<bigint> {
  const provider = getProvider();
  const data =
    "0xdd62ed3e" +
    input.owner.toLowerCase().replace(/^0x/, "").padStart(64, "0") +
    input.spender.toLowerCase().replace(/^0x/, "").padStart(64, "0");
  const result = String(
    await provider.request({
      method: "eth_call",
      params: [{ to: input.token, data }, "latest"],
    }),
  );
  return BigInt(result);
}
