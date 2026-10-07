// Copyright (c) LightPool Labs
// Author: xiaoyu1998

use async_trait::async_trait;
use lightpool_sdk::spot_events::{
    MarketOrderExecutedEvent, OrderCreatedEvent, OrderEventType, OrderFilledEvent,
};
use lightpool_sdk::types::SubmitTransactionResponse;
use lightpool_sdk::{
    extract_event_contract_created_from_events, extract_token_address_from_events, ActionBuilder,
    Address, AuthScheme, BridgeWithdrawParams, BurnEventContractParams, ContractAddress,
    CreateEventContractParams, CreateTokenParams, DepositVaultParams, EventData, EventType,
    CancelOrderParams, MintEventContractParams, OrderSide, PlaceOrderParams, SdkError, SdkResult,
    SetAgentParams, Signer, Signature, TimeInForce, TransactionBuilder, TransactionReceipt,
    VaultDepositedEvent, VaultWithdrawnEvent, WithdrawVaultParams, default_inbound_bridge_instance,
    LIGHTPOOL_EIP712_CHAIN_ID,
    LIGHTPOOL_EIP712_NAME, LIGHTPOOL_EIP712_VERIFYING_CONTRACT, LIGHTPOOL_EIP712_VERSION,
    TOKEN_SCALE,
};
use lightpool_sdk::lightpool_types::{SignedTransaction, Transaction};
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::clob::ClobIndexClient;
use crate::config::Config;
use crate::error::{AppError, AppResult};

pub struct CreateTokenResult {
    pub token_address: ContractAddress,
    pub tx_digest: String,
}

pub struct CreateEventContractResult {
    pub market_address: ContractAddress,
    pub collateral_token: ContractAddress,
    pub yes_token: ContractAddress,
    pub no_token: ContractAddress,
    pub yes_spot_market: ContractAddress,
    pub no_spot_market: ContractAddress,
    pub resolution_deadline: u64,
    pub state: String,
    pub tx_digest: String,
}

pub struct MintBurnResult {
    pub tx_digest: String,
    pub amount: u64,
}

pub struct VaultDepositWithdrawResult {
    pub tx_digest: String,
    pub amount: u64,
    pub shares: u64,
}

pub struct ChainClient {
    clob: Arc<ClobIndexClient>,
}

impl ChainClient {
    pub fn new(clob: Arc<ClobIndexClient>) -> Self {
        Self { clob }
    }

    pub async fn submit_transaction(
        &self,
        tx: lightpool_sdk::lightpool_types::SignedTransaction,
    ) -> AppResult<SubmitTransactionResponse> {
        self.clob.submit_transaction(tx).await
    }

    pub async fn create_token(
        &self,
        signer: &Signer,
        name: &str,
        symbol: &str,
        total_supply_whole: u64,
        mintable: bool,
    ) -> AppResult<CreateTokenResult> {
        let sender = signer.address();
        let params = CreateTokenParams {
            name: name.into(),
            symbol: symbol.into(),
            total_supply: total_supply_whole
                .checked_mul(TOKEN_SCALE)
                .ok_or_else(|| AppError::BadRequest("total_supply overflow".into()))?,
            mintable,
            to: sender,
        };

        let action = ActionBuilder::create_token(params)
            .map_err(|e| AppError::Internal(format!("build create_token action: {e}")))?;

        let tx = TransactionBuilder::new()
            .sender(sender)
            .expiration(u64::MAX)
            .add_action(action)
            .build_and_sign_only(signer)
            .map_err(|e| AppError::Internal(format!("sign create_token tx: {e}")))?;

        let response = self.submit_transaction(tx).await?;

        if !response.receipt.is_success() {
            return Err(AppError::Internal(format!(
                "create_token failed: {:?}",
                response.receipt.status
            )));
        }

        let token_address = extract_token_address_from_events(&response.receipt).ok_or_else(|| {
            AppError::Internal("token address missing from create_token receipt".into())
        })?;

        Ok(CreateTokenResult {
            token_address,
            tx_digest: response.digest,
        })
    }

    pub async fn create_event_contract(
        &self,
        signer: &Signer,
        question: &str,
        collateral_token: ContractAddress,
        oracle: Address,
        resolution_deadline: u64,
        tick_size: u64,
        min_order_size: u64,
        maker_fee_bps: u16,
        taker_fee_bps: u16,
        allow_market_orders: bool,
    ) -> AppResult<CreateEventContractResult> {
        let sender = signer.address();
        let params = CreateEventContractParams {
            question: question.to_string(),
            oracle,
            collateral_token,
            resolution_deadline,
            tick_size,
            min_order_size,
            maker_fee_bps,
            taker_fee_bps,
            allow_market_orders,
            neg_risk_group_id: None,
        };

        let action = ActionBuilder::create_event_contract(params)
            .map_err(|e| AppError::Internal(format!("build create_event_contract action: {e}")))?;

        let tx = TransactionBuilder::new()
            .sender(sender)
            .expiration(u64::MAX)
            .add_action(action)
            .build_and_sign_only(signer)
            .map_err(|e| AppError::Internal(format!("sign create_event_contract tx: {e}")))?;

        let response = self.submit_transaction(tx).await?;

        if !response.receipt.is_success() {
            return Err(AppError::Internal(format!(
                "create_event_contract failed: {:?}",
                response.receipt.status
            )));
        }

        let created = extract_event_contract_created_from_events(&response.receipt).ok_or_else(|| {
            AppError::Internal(
                "event contract created event missing from receipt".into(),
            )
        })?;

        Ok(CreateEventContractResult {
            market_address: created.market_address,
            collateral_token: created.collateral_token,
            yes_token: created.yes_token,
            no_token: created.no_token,
            yes_spot_market: created.yes_spot_market,
            no_spot_market: created.no_spot_market,
            resolution_deadline: created.resolution_deadline,
            state: created.state.to_string(),
            tx_digest: response.digest,
        })
    }

    pub async fn mint_event_contract(
        &self,
        signer: &Signer,
        account: Address,
        market_address: ContractAddress,
        collateral_token: ContractAddress,
        yes_token: ContractAddress,
        no_token: ContractAddress,
        amount: u64,
    ) -> AppResult<MintBurnResult> {
        let sender = signer.address();
        let params = MintEventContractParams {
            amount,
            collateral_token,
            yes_token,
            no_token,
            whitelist: None,
        };

        let action = ActionBuilder::mint_event_contract(market_address, params)
            .map_err(|e| AppError::Internal(format!("build mint_event_contract action: {e}")))?;

        let mut builder = TransactionBuilder::new()
            .sender(sender)
            .expiration(u64::MAX)
            .add_action(action);
        if account != sender {
            builder = builder.account(account);
        }
        let tx = builder
            .build_and_sign_only(signer)
            .map_err(|e| AppError::Internal(format!("sign mint_event_contract tx: {e}")))?;

        let response = self.submit_transaction(tx).await?;

        if !response.receipt.is_success() {
            return Err(AppError::Internal(format!(
                "mint_event_contract failed: {:?}",
                response.receipt.status
            )));
        }

        Ok(MintBurnResult {
            tx_digest: response.digest,
            amount,
        })
    }

    pub async fn burn_event_contract(
        &self,
        signer: &Signer,
        account: Address,
        market_address: ContractAddress,
        collateral_token: ContractAddress,
        yes_token: ContractAddress,
        no_token: ContractAddress,
        amount: u64,
    ) -> AppResult<MintBurnResult> {
        let sender = signer.address();
        let params = BurnEventContractParams {
            amount,
            collateral_token,
            yes_token,
            no_token,
            whitelist: None,
        };

        let action = ActionBuilder::burn_event_contract(market_address, params)
            .map_err(|e| AppError::Internal(format!("build burn_event_contract action: {e}")))?;

        let mut builder = TransactionBuilder::new()
            .sender(sender)
            .expiration(u64::MAX)
            .add_action(action);
        if account != sender {
            builder = builder.account(account);
        }
        let tx = builder
            .build_and_sign_only(signer)
            .map_err(|e| AppError::Internal(format!("sign burn_event_contract tx: {e}")))?;

        let response = self.submit_transaction(tx).await?;

        if !response.receipt.is_success() {
            return Err(AppError::Internal(format!(
                "burn_event_contract failed: {:?}",
                response.receipt.status
            )));
        }

        Ok(MintBurnResult {
            tx_digest: response.digest,
            amount,
        })
    }

    pub async fn deposit_vault(
        &self,
        signer: &Signer,
        vault_address: ContractAddress,
        quote_token: ContractAddress,
        share_token: ContractAddress,
        amount: u64,
    ) -> AppResult<VaultDepositWithdrawResult> {
        let sender = signer.address();
        let params = DepositVaultParams {
            amount,
            quote_token,
            share_token,
        };

        let action = ActionBuilder::deposit_vault(vault_address, params)
            .map_err(|e| AppError::Internal(format!("build deposit_vault action: {e}")))?;

        let tx = TransactionBuilder::new()
            .sender(sender)
            .expiration(u64::MAX)
            .add_action(action)
            .build_and_sign_only(signer)
            .map_err(|e| AppError::Internal(format!("sign deposit_vault tx: {e}")))?;

        let response = self.submit_transaction(tx).await?;

        if !response.receipt.is_success() {
            return Err(AppError::Internal(format!(
                "deposit_vault failed: {:?}",
                response.receipt.status
            )));
        }

        let (amount, shares) = extract_vault_deposit_amounts(&response.receipt)
            .unwrap_or((amount, 0));

        Ok(VaultDepositWithdrawResult {
            tx_digest: response.digest,
            amount,
            shares,
        })
    }

    pub async fn withdraw_vault(
        &self,
        signer: &Signer,
        vault_address: ContractAddress,
        quote_token: ContractAddress,
        share_token: ContractAddress,
        shares: u64,
    ) -> AppResult<VaultDepositWithdrawResult> {
        let sender = signer.address();
        let params = WithdrawVaultParams {
            shares,
            quote_token,
            share_token,
        };

        let action = ActionBuilder::withdraw_vault(vault_address, params)
            .map_err(|e| AppError::Internal(format!("build withdraw_vault action: {e}")))?;

        let tx = TransactionBuilder::new()
            .sender(sender)
            .expiration(u64::MAX)
            .add_action(action)
            .build_and_sign_only(signer)
            .map_err(|e| AppError::Internal(format!("sign withdraw_vault tx: {e}")))?;

        let response = self.submit_transaction(tx).await?;

        if !response.receipt.is_success() {
            return Err(AppError::Internal(format!(
                "withdraw_vault failed: {:?}",
                response.receipt.status
            )));
        }

        let (amount, shares) = extract_vault_withdraw_amounts(&response.receipt)
            .unwrap_or((0, shares));

        Ok(VaultDepositWithdrawResult {
            tx_digest: response.digest,
            amount,
            shares,
        })
    }

    pub async fn place_order(
        &self,
        signer: &Signer,
        account: Address,
        spot_market: ContractAddress,
        params: PlaceOrderParams,
    ) -> AppResult<SubmitTransactionResponse> {
        let sender = signer.address();

        let action = ActionBuilder::place_order(spot_market, params)
            .map_err(|e| AppError::Internal(format!("build place_order action: {e}")))?;

        let mut builder = TransactionBuilder::new()
            .sender(sender)
            .expiration(u64::MAX)
            .add_action(action);
        if account != sender {
            builder = builder.account(account);
        }
        let tx = builder
            .build_and_sign_only(signer)
            .map_err(|e| AppError::Internal(format!("sign place_order tx: {e}")))?;

        let response = self.submit_transaction(tx).await?;

        if !response.receipt.is_success() {
            return Err(AppError::Internal(format!(
                "place_order failed: {:?}",
                response.receipt.status
            )));
        }

        Ok(response)
    }

    pub async fn cancel_order(
        &self,
        signer: &Signer,
        account: Address,
        spot_market: ContractAddress,
        order_id: u64,
    ) -> AppResult<SubmitTransactionResponse> {
        let sender = signer.address();
        let params = CancelOrderParams { order_id };

        let action = ActionBuilder::cancel_order(spot_market, params)
            .map_err(|e| AppError::Internal(format!("build cancel_order action: {e}")))?;

        let mut builder = TransactionBuilder::new()
            .sender(sender)
            .expiration(u64::MAX)
            .add_action(action);
        if account != sender {
            builder = builder.account(account);
        }
        let tx = builder
            .build_and_sign_only(signer)
            .map_err(|e| AppError::Internal(format!("sign cancel_order tx: {e}")))?;

        let response = self.submit_transaction(tx).await?;

        if !response.receipt.is_success() {
            return Err(AppError::Internal(format!(
                "cancel_order failed: {:?}",
                response.receipt.status
            )));
        }

        Ok(response)
    }

    pub fn prepare_set_agent(
        &self,
        user: Address,
        agent: Address,
    ) -> AppResult<PreparedUserTx> {
        let action = ActionBuilder::set_agent(SetAgentParams { agent })
            .map_err(|e| AppError::Internal(format!("build set_agent action: {e}")))?;
        let tx = TransactionBuilder::new()
            .sender(user)
            .expiration(u64::MAX)
            .add_action(action)
            .build()
            .map_err(|e| AppError::Internal(format!("build set_agent tx: {e}")))?;
        PreparedUserTx::from_tx(tx)
    }

    pub fn prepare_bridge_withdraw(
        &self,
        user: Address,
        token: ContractAddress,
        amount: u64,
        evm_recipient: [u8; 20],
    ) -> AppResult<PreparedUserTx> {
        let action = ActionBuilder::bridge_withdraw(
            default_inbound_bridge_instance(),
            BridgeWithdrawParams {
                token,
                amount,
                foreign_recipient: evm_recipient,
            },
        )
        .map_err(|e| AppError::Internal(format!("build bridge_withdraw action: {e}")))?;
        let tx = TransactionBuilder::new()
            .sender(user)
            .expiration(u64::MAX)
            .add_action(action)
            .build()
            .map_err(|e| AppError::Internal(format!("build bridge_withdraw tx: {e}")))?;
        PreparedUserTx::from_tx(tx)
    }

    pub async fn submit_user_signed(
        &self,
        tx: Transaction,
        signature: Signature,
    ) -> AppResult<SubmitTransactionResponse> {
        let signed = SignedTransaction::new_with_scheme(tx, signature, AuthScheme::Eip712);
        let response = self.submit_transaction(signed).await?;
        if !response.receipt.is_success() {
            return Err(AppError::Internal(format!(
                "user-signed tx failed: {:?}",
                response.receipt.status
            )));
        }
        Ok(response)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Eip712DomainJson {
    pub name: String,
    pub version: String,
    #[serde(rename = "chainId")]
    pub chain_id: u64,
    #[serde(rename = "verifyingContract")]
    pub verifying_contract: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Eip712TypeField {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Eip712TypesJson {
    #[serde(rename = "LightPoolTx")]
    pub lightpool_tx: Vec<Eip712TypeField>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Eip712MessageJson {
    pub digest: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Eip712TypedDataJson {
    pub domain: Eip712DomainJson,
    pub types: Eip712TypesJson,
    #[serde(rename = "primaryType")]
    pub primary_type: String,
    pub message: Eip712MessageJson,
}

pub struct PreparedUserTx {
    pub digest_hex: String,
    pub unsigned_tx_hex: String,
    pub eip712: Eip712TypedDataJson,
}

impl PreparedUserTx {
    fn from_tx(tx: Transaction) -> AppResult<Self> {
        let digest = tx.digest();
        let digest_hex = format!("0x{}", hex::encode(digest.as_bytes()));
        let bytes = bincode::serialize(&tx)
            .map_err(|e| AppError::Internal(format!("serialize tx: {e}")))?;
        Ok(Self {
            digest_hex: digest_hex.clone(),
            unsigned_tx_hex: format!("0x{}", hex::encode(bytes)),
            eip712: eip712_typed_data(&digest_hex),
        })
    }
}

pub fn eip712_typed_data(digest_hex: &str) -> Eip712TypedDataJson {
    let verifying = format!("0x{}", hex::encode(LIGHTPOOL_EIP712_VERIFYING_CONTRACT));
    Eip712TypedDataJson {
        domain: Eip712DomainJson {
            name: LIGHTPOOL_EIP712_NAME.to_string(),
            version: LIGHTPOOL_EIP712_VERSION.to_string(),
            chain_id: LIGHTPOOL_EIP712_CHAIN_ID,
            verifying_contract: verifying,
        },
        types: Eip712TypesJson {
            lightpool_tx: vec![Eip712TypeField {
                name: "digest".into(),
                type_name: "bytes32".into(),
            }],
        },
        primary_type: "LightPoolTx".into(),
        message: Eip712MessageJson {
            digest: digest_hex.to_string(),
        },
    }
}

pub fn decode_unsigned_tx(hex_raw: &str) -> AppResult<Transaction> {
    let trimmed = hex_raw.trim();
    let body = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    let bytes =
        hex::decode(body).map_err(|e| AppError::BadRequest(format!("invalid tx hex: {e}")))?;
    bincode::deserialize(&bytes)
        .map_err(|e| AppError::BadRequest(format!("invalid unsigned tx: {e}")))
}

pub fn format_token_amount(raw: u64) -> String {
    let whole = raw / TOKEN_SCALE;
    let frac = raw % TOKEN_SCALE;
    if frac == 0 {
        return whole.to_string();
    }
    format!("{whole}.{frac:06}", frac = frac)
}

pub fn format_price_pieces(raw: u64) -> String {
    let numerator = raw.saturating_mul(100);
    let whole = numerator / TOKEN_SCALE;
    let frac = numerator % TOKEN_SCALE;
    if frac == 0 {
        return whole.to_string();
    }
    let frac_str = format!("{frac:06}");
    let trimmed = frac_str.trim_end_matches('0');
    format!("{whole}.{trimmed}")
}

pub fn parse_price_pieces(price: &str) -> AppResult<u64> {
    let trimmed = price.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("invalid price".into()));
    }

    let (whole_str, frac_str) = match trimmed.split_once('.') {
        Some((whole, frac)) => (whole, frac),
        None => (trimmed, ""),
    };

    if whole_str.is_empty() || !whole_str.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::BadRequest(format!("invalid price: {price}")));
    }
    if !frac_str.is_empty() && !frac_str.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::BadRequest(format!("invalid price: {price}")));
    }
    if frac_str.len() > 6 {
        return Err(AppError::BadRequest(format!("invalid price: {price}")));
    }

    let whole: u64 = whole_str
        .parse()
        .map_err(|e| AppError::BadRequest(format!("invalid price: {e}")))?;
    if whole > 100
        || (whole == 100 && !frac_str.is_empty() && frac_str.chars().any(|c| c != '0'))
    {
        return Err(AppError::BadRequest("price must be between 0 and 100".into()));
    }

    let frac: u64 = if frac_str.is_empty() {
        0
    } else {
        frac_str
            .parse()
            .map_err(|e| AppError::BadRequest(format!("invalid price: {e}")))?
    };

    let frac_digits = frac_str.len() as u32;
    let scale = 10_u64.pow(frac_digits);
    let cent_numerator = whole
        .checked_mul(scale)
        .and_then(|value| value.checked_add(frac))
        .ok_or_else(|| AppError::BadRequest(format!("invalid price: {price}")))?;

    cent_numerator
        .checked_mul(TOKEN_SCALE)
        .and_then(|value| value.checked_div(100 * scale))
        .ok_or_else(|| AppError::BadRequest(format!("invalid price: {price}")))
}

pub fn parse_order_size(size: &str) -> AppResult<u64> {
    let value: f64 = size
        .trim()
        .parse()
        .map_err(|e| AppError::BadRequest(format!("invalid size: {e}")))?;
    if value <= 0.0 {
        return Err(AppError::BadRequest("size must be greater than 0".into()));
    }
    let raw = (value * TOKEN_SCALE as f64).round() as u64;
    if raw == 0 {
        return Err(AppError::BadRequest("size must be greater than 0".into()));
    }
    Ok(raw)
}

fn extract_vault_deposit_amounts(receipt: &TransactionReceipt) -> Option<(u64, u64)> {
    for event in &receipt.events {
        let EventType::Call(action_name) = &event.event_type else {
            continue;
        };
        if action_name.as_str() != "vault_deposited" {
            continue;
        }
        let EventData::Bytes(data) = &event.data else {
            continue;
        };
        if let Ok(deposited) = bincode::deserialize::<VaultDepositedEvent>(data) {
            return Some((deposited.amount, deposited.shares));
        }
    }
    None
}

fn extract_vault_withdraw_amounts(receipt: &TransactionReceipt) -> Option<(u64, u64)> {
    for event in &receipt.events {
        let EventType::Call(action_name) = &event.event_type else {
            continue;
        };
        if action_name.as_str() != "vault_withdrawn" {
            continue;
        }
        let EventData::Bytes(data) = &event.data else {
            continue;
        };
        if let Ok(withdrawn) = bincode::deserialize::<VaultWithdrawnEvent>(data) {
            return Some((withdrawn.amount, withdrawn.shares));
        }
    }
    None
}

pub fn extract_order_created_from_receipt(
    receipt: &TransactionReceipt,
) -> Option<OrderCreatedEvent> {
    for event in &receipt.events {
        let EventType::Call(action_name) = &event.event_type else {
            continue;
        };
        if action_name.as_str() != "order_created" {
            continue;
        }
        let EventData::Bytes(data) = &event.data else {
            continue;
        };
        if let Ok(created) = bincode::deserialize::<OrderCreatedEvent>(data) {
            return Some(created);
        }
    }
    None
}

pub struct PlaceOrderPlacementContext {
    pub user: Address,
    pub spot_market: ContractAddress,
    pub side: OrderSide,
    pub amount: u64,
    pub limit_price: u64,
    pub is_market: bool,
    pub slippage: u64,
}

pub struct ResolvedPlaceOrder {
    pub created: OrderCreatedEvent,
    pub skip_book: bool,
    pub status: String,
    pub filled_raw: u64,
}

pub fn resolve_placed_order_from_receipt(
    receipt: &TransactionReceipt,
    ctx: &PlaceOrderPlacementContext,
) -> Option<ResolvedPlaceOrder> {
    if let Some(created) = extract_order_created_from_receipt(receipt) {
        return Some(ResolvedPlaceOrder {
            created,
            skip_book: false,
            status: "open".into(),
            filled_raw: 0,
        });
    }

    if ctx.is_market {
        if let Some(exec) = extract_market_order_executed_for_user(receipt, ctx.user) {
            let status = if exec.filled_amount >= exec.amount {
                "filled"
            } else {
                "partial_filled"
            };
            return Some(ResolvedPlaceOrder {
                created: OrderCreatedEvent {
                    cloid: None,
                    order_id: exec.order_id,
                    side: exec.side,
                    amount: exec.amount,
                    creator: exec.creator,
                    market: ctx.spot_market.to_address(),
                    order_type: OrderEventType::Market {
                        slippage: ctx.slippage,
                    },
                },
                skip_book: true,
                status: status.into(),
                filled_raw: exec.filled_amount,
            });
        }
    }

    if let Some((filled, filled_raw)) = extract_taker_order_filled_for_user(receipt, ctx.user) {
        let amount = filled.fill_amount.saturating_add(filled.remaining_amount);
        let status = if filled.is_fully_filled || filled.remaining_amount == 0 {
            "filled"
        } else {
            "partial_filled"
        };
        return Some(ResolvedPlaceOrder {
            created: OrderCreatedEvent {
                cloid: None,
                order_id: filled.order_id,
                side: filled.side,
                amount,
                creator: ctx.user,
                market: ctx.spot_market.to_address(),
                order_type: OrderEventType::Limit {
                    price: ctx.limit_price,
                    tif: TimeInForce::GTC,
                },
            },
            skip_book: true,
            status: status.into(),
            filled_raw,
        });
    }

    None
}

fn extract_market_order_executed_for_user(
    receipt: &TransactionReceipt,
    user: Address,
) -> Option<MarketOrderExecutedEvent> {
    for event in &receipt.events {
        let EventType::Call(action_name) = &event.event_type else {
            continue;
        };
        if action_name.as_str() != "market_order_executed" {
            continue;
        }
        let EventData::Bytes(data) = &event.data else {
            continue;
        };
        if let Ok(exec) = bincode::deserialize::<MarketOrderExecutedEvent>(data) {
            if exec.creator == user {
                return Some(exec);
            }
        }
    }
    None
}

fn extract_taker_order_filled_for_user(
    receipt: &TransactionReceipt,
    _user: Address,
) -> Option<(OrderFilledEvent, u64)> {
    let mut last: Option<OrderFilledEvent> = None;
    let mut filled_raw = 0_u64;

    for event in &receipt.events {
        let EventType::Call(action_name) = &event.event_type else {
            continue;
        };
        if action_name.as_str() != "order_filled" {
            continue;
        }
        let EventData::Bytes(data) = &event.data else {
            continue;
        };
        let Ok(filled) = bincode::deserialize::<OrderFilledEvent>(data) else {
            continue;
        };

        if last
            .as_ref()
            .is_some_and(|previous| previous.order_id != filled.order_id)
        {
            continue;
        }

        filled_raw = filled_raw.saturating_add(filled.fill_amount);
        last = Some(filled);
    }

    last.map(|filled| (filled, filled_raw))
}

pub fn market_uuid(market_address: &str) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, market_address.as_bytes())
}

/// Signing abstraction — swap implementation when wallet integration lands.
#[async_trait]
pub trait SignerService: Send + Sync {
    async fn dev_signer(&self) -> SdkResult<Signer>;
    async fn user_address(&self) -> Address;
}

pub struct LocalSignerService {
    secret_key_b64: String,
    address: Address,
}

fn load_signer_from_dev_secret_key(raw: &str) -> SdkResult<Signer> {
    let trimmed = raw.trim();
    let hex_body = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);

    let is_hex = !hex_body.is_empty()
        && hex_body.len() == 64
        && hex_body.chars().all(|c| c.is_ascii_hexdigit());

    if is_hex {
        let bytes = hex::decode(hex_body)?;
        let key_bytes: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
            SdkError::Crypto("DEV_SECRET_KEY hex must decode to 32 bytes".into())
        })?;
        return Signer::from_secret_key_bytes(&key_bytes);
    }

    Signer::from_secret_key_base64(trimmed)
}

impl LocalSignerService {
    pub fn from_config(config: &Config) -> SdkResult<Self> {
        let signer = if let Some(encoded) = &config.dev_secret_key {
            let signer = load_signer_from_dev_secret_key(encoded)?;
            tracing::info!(address = %signer.address(), "loaded dev signer from DEV_SECRET_KEY");
            signer
        } else {
            let signer = Signer::new();
            tracing::warn!(
                address = %signer.address(),
                secret_key = %signer.export_secret_key(),
                "DEV_SECRET_KEY not set; using ephemeral dev signer — add DEV_SECRET_KEY (hex or base64) to .env to keep this address across restarts"
            );
            signer
        };

        Ok(Self {
            secret_key_b64: signer.export_secret_key(),
            address: signer.address(),
        })
    }
}

#[async_trait]
impl SignerService for LocalSignerService {
    async fn dev_signer(&self) -> SdkResult<Signer> {
        Signer::from_secret_key_base64(&self.secret_key_b64)
    }

    async fn user_address(&self) -> Address {
        self.address
    }
}

pub type SharedChainClient = Arc<ChainClient>;
pub type SharedSignerService = Arc<dyn SignerService>;
