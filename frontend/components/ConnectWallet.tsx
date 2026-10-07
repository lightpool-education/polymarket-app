"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { usePathname, useRouter } from "next/navigation";
import {
  useAccount,
  useConnect,
  useDisconnect,
  useSignMessage,
  useSignTypedData,
} from "wagmi";
import { api } from "@/lib/api";
import { formatAddress } from "@/lib/address";
import {
  clearSession,
  getSessionAddress,
  getSessionToken,
  notifySessionChanged,
  SESSION_CHANGED_EVENT,
  setSession,
} from "@/lib/session";
import {
  compactRsFromEthereumSignature,
  lightPoolTypedDataFromPrepared,
} from "@/lib/lpSign";
import { requestPortfolioRefresh } from "@/lib/portfolio";

export const OPEN_CASH_DEPOSIT_EVENT = "open-cash-deposit";

export function ConnectWallet() {
  const { address, isConnected } = useAccount();
  const { connectAsync, connectors, isPending } = useConnect();
  const { disconnectAsync } = useDisconnect();
  const { signMessageAsync } = useSignMessage();
  const { signTypedDataAsync } = useSignTypedData();
  const router = useRouter();
  const pathname = usePathname();
  const [sessionAddress, setSessionAddress] = useState<string | null>(null);
  const [agentAuthorized, setAgentAuthorized] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const loginAttemptedFor = useRef<string | null>(null);
  const skipAutoLogin = useRef(false);
  const [copied, setCopied] = useState(false);

  const refreshSession = useCallback(() => {
    setSessionAddress(getSessionAddress());
  }, []);

  useEffect(() => {
    refreshSession();
    window.addEventListener(SESSION_CHANGED_EVENT, refreshSession);
    return () => window.removeEventListener(SESSION_CHANGED_EVENT, refreshSession);
  }, [refreshSession]);

  const refreshAgent = useCallback(async () => {
    if (!getSessionToken()) {
      setAgentAuthorized(null);
      return null;
    }
    try {
      const agent = await api.getAgent();
      setAgentAuthorized(agent.authorized);
      return agent.authorized;
    } catch {
      setAgentAuthorized(null);
      return null;
    }
  }, []);

  useEffect(() => {
    void refreshAgent();
  }, [sessionAddress, refreshAgent]);

  const authorizeAgent = useCallback(async () => {
    setError(null);
    const prepared = await api.prepareSetAgent();
    const typed = lightPoolTypedDataFromPrepared(prepared);
    const ethSig = await signTypedDataAsync(typed);
    const signature = compactRsFromEthereumSignature(ethSig);
    await api.submitSetAgent(signature, prepared.unsigned_tx_hex);
    setAgentAuthorized(true);
    return true;
  }, [signTypedDataAsync]);

  const login = useCallback(async (explicitAddress?: string) => {
    const account = (explicitAddress || address || "").trim();
    if (!account) {
      return false;
    }
    setBusy(true);
    setError(null);
    try {
      const { message } = await api.authNonce(account);
      const signature = await signMessageAsync({ message });
      const verified = await api.authVerify(account, signature);
      setSession(verified.token, verified.address);
      setSessionAddress(verified.address);
      notifySessionChanged();
      setAgentAuthorized(verified.agent_authorized);
      requestPortfolioRefresh();

      if (!verified.agent_authorized) {
        await authorizeAgent();
      }
      return true;
    } catch (e) {
      // Keep loginAttemptedFor set so rejecting MetaMask does not retry in a loop.
      setError(e instanceof Error ? e.message : "Login failed");
      return false;
    } finally {
      setBusy(false);
    }
  }, [address, authorizeAgent, signMessageAsync]);

  const loggedIn =
    !!address &&
    !!sessionAddress &&
    sessionAddress.toLowerCase() === address.toLowerCase() &&
    !!getSessionToken();

  useEffect(() => {
    if (!isConnected || !address) {
      skipAutoLogin.current = false;
      return;
    }
    if (busy || loggedIn || skipAutoLogin.current) {
      return;
    }
    const key = address.toLowerCase();
    if (loginAttemptedFor.current === key) {
      return;
    }
    loginAttemptedFor.current = key;
    void login(address);
  }, [isConnected, address, busy, loggedIn, login]);

  async function onConnectClick() {
    setError(null);
    skipAutoLogin.current = false;
    try {
      const connector = connectors[0];
      if (!connector) {
        setError("No wallet connector found");
        return;
      }
      let account = address;
      if (!isConnected) {
        const connected = await connectAsync({ connector });
        account = connected.accounts[0];
      }
      if (!account) {
        setError("No account from MetaMask");
        return;
      }
      loginAttemptedFor.current = null;
      await login(account);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Connect failed");
    }
  }

  async function onAuthorizeAgentClick() {
    setBusy(true);
    setError(null);
    try {
      await authorizeAgent();
    } catch (e) {
      setError(e instanceof Error ? e.message : "set_agent failed");
      setAgentAuthorized(false);
    } finally {
      setBusy(false);
    }
  }

  function onDeposit() {
    if (pathname === "/cash") {
      window.dispatchEvent(new Event(OPEN_CASH_DEPOSIT_EVENT));
      return;
    }
    router.push("/cash?action=deposit");
  }

  async function onCopyAddress() {
    if (!address) {
      return;
    }
    try {
      await navigator.clipboard.writeText(address);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // ignore
    }
  }

  async function onDisconnect() {
    setError(null);
    setBusy(true);
    skipAutoLogin.current = true;
    loginAttemptedFor.current = address?.toLowerCase() ?? "disconnected";
    try {
      await disconnectAsync();
      clearSession();
      setSessionAddress(null);
      setAgentAuthorized(null);
      notifySessionChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Disconnect failed");
    } finally {
      setBusy(false);
    }
  }

  if (!isConnected || !address) {
    return (
      <div className="flex flex-col items-end gap-1">
        <button
          type="button"
          className="rounded-full border border-brand-gold-dim px-4 py-2 text-xs font-semibold text-brand-gold transition hover:border-brand-gold hover:bg-[#22c55e14]"
          disabled={isPending || busy}
          onClick={() => void onConnectClick()}
        >
          {isPending || busy ? "Connecting…" : "Connect wallet"}
        </button>
        {error && (
          <p className="max-w-xs text-right text-[11px] text-red-600">{error}</p>
        )}
      </div>
    );
  }

  const needsAgent = loggedIn && agentAuthorized === false;
  const statusLabel = (() => {
    if (busy && !loggedIn) {
      return "Signing in…";
    }
    if (busy && needsAgent) {
      return "Authorizing agent…";
    }
    if (loggedIn && agentAuthorized === true) {
      return null;
    }
    if (!loggedIn) {
      return "Sign in…";
    }
    return null;
  })();

  return (
    <div className="flex flex-col items-end gap-1">
      <div className="flex items-center gap-2">
        {statusLabel ? (
          <span className="text-xs text-slate-500">{statusLabel}</span>
        ) : needsAgent ? (
          <button
            type="button"
            className="rounded-full border border-brand-gold-dim px-3 py-1.5 text-xs font-medium text-brand-gold hover:bg-[#22c55e14]"
            disabled={busy}
            onClick={() => void onAuthorizeAgentClick()}
          >
            Authorize agent
          </button>
        ) : loggedIn && agentAuthorized === true ? (
          <button
            type="button"
            className="rounded-full border border-brand-gold-dim px-3 py-2 text-center transition hover:bg-[#22c55e14]"
            onClick={onDeposit}
          >
            <span className="block text-xs font-medium text-brand-gold">
              Deposit
            </span>
          </button>
        ) : (
          <button
            type="button"
            className="rounded-full border border-brand-gold-dim px-3 py-1.5 text-xs font-semibold text-brand-gold hover:bg-[#22c55e14]"
            disabled={busy}
            onClick={() => void login()}
          >
            Sign in
          </button>
        )}
        <button
          type="button"
          className="rounded-full border border-brand-line px-3 py-1.5 font-mono text-xs font-medium text-brand-muted hover:border-brand-gold-dim hover:text-brand-gold"
          title={copied ? "Copied" : `Click to copy ${address}`}
          onClick={() => void onCopyAddress()}
        >
          {copied ? "Copied" : formatAddress(address)}
        </button>
        <button
          type="button"
          className="rounded-full border border-brand-line px-3 py-1.5 text-xs font-medium text-brand-muted hover:border-red-300 hover:bg-red-50 hover:text-red-600"
          disabled={busy}
          onClick={() => void onDisconnect()}
        >
          Disconnect
        </button>
      </div>
      {error && (
        <div className="flex max-w-xs flex-col items-end gap-1">
          <p className="text-right text-[11px] text-red-600">{error}</p>
          <button
            type="button"
            className="text-[11px] text-brand-gold underline"
            disabled={busy}
            onClick={() => {
              loginAttemptedFor.current = null;
              if (needsAgent) {
                void onAuthorizeAgentClick();
              } else {
                void login();
              }
            }}
          >
            Retry
          </button>
        </div>
      )}
    </div>
  );
}
