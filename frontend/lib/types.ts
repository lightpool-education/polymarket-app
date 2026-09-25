export type BookLevel = {
  price: string;
  size: string;
};

export type BookResponse = {
  sequence: number;
  bids: BookLevel[];
  asks: BookLevel[];
  last_trade_price?: string | null;
};

export type MarketInfo = {
  last_price: string | null;
  state: string;
  min_order_size: string;
  tick_size: string;
  maker_fee_bps: number;
  taker_fee_bps: number;
  allow_market_orders: boolean;
};

export type Market = {
  id: string;
  slug: string;
  question: string;
  icon_url?: string | null;
  event_slug?: string | null;
  group_item_title?: string | null;
  /** Yes mark price in cents (0-100): last trade or (best bid + best ask) / 2. */
  yes_rate?: string | null;
  market_address: string;
  collateral_token: string;
  yes_token: string;
  no_token: string;
  yes_spot_market: string;
  no_spot_market: string;
  state: string;
  resolution_deadline: number;
};

export type MarketsPage = {
  markets: Market[];
  total: number;
  limit: number;
  offset: number;
};

export type VaultAsset = {
  market: string;
  amount: string;
  last_price?: string | null;
  quote_value?: string | null;
};

export type Vault = {
  id: string;
  name: string;
  vault_address: string;
  vault_account: string;
  manager: string;
  quote_token: string;
  share_token: string;
  equity: string;
  user_deposit?: string;
  portfolio?: VaultAsset[];
  allow_deposit: boolean;
  is_closed: boolean;
};

export type VaultsPage = {
  vaults: Vault[];
  total: number;
  limit: number;
  offset: number;
};

export type Order = {
  id: string;
  market_id: string;
  market_slug: string;
  question: string;
  outcome: string;
  side: string;
  price: string;
  size: string;
  status: string;
  chain_order_id?: string;
  spot_market?: string;
  user_address?: string;
  size_raw?: number;
  filled_raw?: number;
  /** Archive time from indexer (unix ms). */
  status_ts_ms?: number;
};

export type BalanceEntry = {
  token: string;
  symbol: string;
  total: string;
  locked: string;
  available: string;
};

export type PlaceOrderRequest = {
  market_slug: string;
  outcome: "yes" | "no";
  side: "buy" | "sell";
  price: string;
  size: string;
  order_type?: "limit" | "market";
};

export type CreateTokenRequest = {
  name: string;
  symbol: string;
  total_supply: number;
  mintable?: boolean;
};

export type CreateTokenResponse = {
  token_address: string;
  tx_digest: string;
  name: string;
  symbol: string;
  creator: string;
};

export type CashToken = {
  symbol: string;
  address: string;
};

export type CreateEventContractRequest = {
  question: string;
  icon_url?: string;
  event_slug?: string;
  group_item_title?: string;
  collateral_token?: string;
  oracle?: string;
  resolution_deadline: number;
  tick_size?: number;
  min_order_size?: number;
  maker_fee_bps?: number;
  taker_fee_bps?: number;
  allow_market_orders?: boolean;
};

export type UpdateMarketMetadataRequest = {
  event_slug?: string;
  group_item_title?: string;
  icon_url?: string;
};

export type MintBurnRequest = {
  amount: string;
};

export type MintBurnResponse = {
  slug: string;
  amount: string;
  tx_digest: string;
};

export type VaultDepositRequest = {
  amount: string;
};

export type VaultWithdrawRequest = {
  shares: string;
};

export type VaultDepositWithdrawResponse = {
  vault_address: string;
  amount: string;
  shares: string;
  tx_digest: string;
};

export type CreateEventContractResponse = {
  market_id: string;
  slug: string;
  question: string;
  icon_url?: string | null;
  event_slug?: string | null;
  group_item_title?: string | null;
  market_address: string;
  collateral_token: string;
  yes_token: string;
  no_token: string;
  yes_spot_market: string;
  no_spot_market: string;
  state: string;
  resolution_deadline: number;
  tx_digest: string;
  creator: string;
};
