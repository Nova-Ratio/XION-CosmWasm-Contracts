#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo, Response,
    StdError, StdResult, Storage, Uint128,
};
use cw2::set_contract_version;
use cw20::{BalanceResponse, Cw20ReceiveMsg, MinterResponse, TokenInfoResponse};
use cw404_utils::{check_funds, process_fee, CREATOR, INSTANTIATE_FEE, MINT_FEE};
use cw721::{Cw721ReceiveMsg, NftInfoResponse, NumTokensResponse, OwnerOfResponse, TokensResponse};

use crate::error::ContractError;
use crate::msg::{ConfigResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{
    Config, Metadata, ALLOWANCE, BALANCE_OF, CONFIG, GET_APPROVED, IS_APPROVED_FOR_ALL,
    LAST_MINT_SECONDS, METADATA, MINTED, OWNED, OWNED_INDEX, OWNER_OF, WHITELIST,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw404";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let mut msgs: Vec<CosmosMsg> = vec![];

    // if info.sender.to_string() != CREATOR {
    //     check_funds(&info, INSTANTIATE_FEE)?;
    //     process_fee(INSTANTIATE_FEE, &mut msgs)?;
    // }

    let config = Config {
        admin: info.sender.clone(),
    };
    CONFIG.save(deps.storage, &config)?;

    let metadata = Metadata {
        name: msg.name,
        symbol: msg.symbol,
        decimals: msg.decimals,
        total_supply: msg.total_native_supply * ((10u32).pow(msg.decimals as u32) as u128),
        token_uri: msg.token_uri,
    };
    METADATA.save(deps.storage, &metadata)?;

    MINTED.save(deps.storage, &0)?;

    BALANCE_OF.save(
        deps.storage,
        env.contract.address.clone(),
        &Uint128::new(metadata.total_supply),
    )?;

    WHITELIST.save(deps.storage, env.contract.address.clone(), &true)?;

    Ok(Response::new().add_messages(msgs))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::SetWhitelist { target, state } => {
            execute_set_whitelist(deps, env, info, target, state)
        }
        ExecuteMsg::Approve {
            spender,
            amount_or_id,
        } => execute_approve(deps, env, info, spender, amount_or_id),
        ExecuteMsg::SetApprovalForAll { operator, approved } => {
            execute_set_approval_for_all(deps, env, info, operator, approved)
        }
        ExecuteMsg::Transfer { recipient, amount } => {
            execute_transfer(deps, env, info, None, recipient, amount, vec![])
        }
        ExecuteMsg::TransferNft {
            recipient,
            token_id,
        } => execute_transfer_nft(deps, env, info, recipient, token_id),
        ExecuteMsg::Send {
            contract,
            amount,
            msg,
        } => execute_send(deps, env, info, contract, amount, msg),
        ExecuteMsg::SendNft {
            contract,
            token_id,
            msg,
        } => execute_send_nft(deps, env, info, contract, token_id, msg),
        ExecuteMsg::IncreaseAllowance { spender, amount } => {
            execute_increase_allowance(deps, env, info, spender, amount)
        }
        ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => execute_transfer_from(deps, env, info, owner, recipient, amount),
        // wrapper code
        ExecuteMsg::Mint {} => {
            let mut msgs: Vec<CosmosMsg> = vec![];

            // check_funds(&info, MINT_FEE)?;
            // process_fee(MINT_FEE, &mut msgs)?;

            if BALANCE_OF.load(deps.storage, env.contract.address.clone())? == Uint128::zero() {
                return Err(ContractError::OutOfSupply {});
            }

            let current_seconds = env.block.time.nanos() / 1_000_000_000;
            let last_mint_seconds = LAST_MINT_SECONDS
                .load(deps.storage, info.sender.clone())
                .unwrap_or(0);

            if current_seconds < last_mint_seconds + 86_400 {
                return Err(ContractError::AlreadyMintedToday {});
            }

            LAST_MINT_SECONDS.save(deps.storage, info.sender.clone(), &current_seconds)?;

            let unit = get_unit(deps.storage)?.u128();
            execute_transfer(
                deps,
                env.clone(),
                info.clone(),
                Some(env.contract.address.to_string()),
                info.sender.to_string(),
                unit,
                msgs,
            )
        }
        ExecuteMsg::AirdropNft {
            recipient,
            nft_amount,
        } => {
            let config = CONFIG.load(deps.storage)?;
            if info.sender != config.admin {
                return Err(ContractError::Unauthorized {});
            }

            let unit = get_unit(deps.storage)?.u128();
            execute_transfer(
                deps,
                env.clone(),
                info,
                Some(env.contract.address.to_string()),
                recipient,
                nft_amount * unit,
                vec![],
            )
        }
    }
}

fn execute_set_whitelist(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    target: String,
    state: bool,
) -> Result<Response, ContractError> {
    let addr = deps.api.addr_validate(&target)?;

    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    WHITELIST.save(deps.storage, addr, &state)?;

    Ok(Response::new()
        .add_attribute("action", "set_whitelist")
        .add_attribute("address", target)
        .add_attribute("state", state.to_string()))
}

fn execute_transfer(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    from: Option<String>,
    to: String,
    amount: u128,
    msgs: Vec<CosmosMsg>,
) -> Result<Response, ContractError> {
    let sender = match from {
        Some(from) => deps.api.addr_validate(&from)?,
        None => info.sender.clone(),
    };
    let receiver = deps.api.addr_validate(&to)?;

    _transfer_token(deps.storage, sender.clone(), receiver.clone(), amount)?;

    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("action", "transfer")
        .add_attribute("sender", sender)
        .add_attribute("receiver", to))
}

fn execute_transfer_nft(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    recipient: String,
    token_id: String,
) -> Result<Response, ContractError> {
    _transfer_nft(
        deps.storage,
        info.sender.clone(),
        deps.api.addr_validate(&recipient)?,
        token_id,
    )?;
    Ok(Response::new()
        .add_attribute("action", "transfer")
        .add_attribute("sender", info.sender.to_string())
        .add_attribute("receiver", recipient))
}
fn execute_send(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    contract: String,
    amount: u128,
    msg: Binary,
) -> Result<Response, ContractError> {
    let sender = info.sender.clone();
    let receiver = deps.api.addr_validate(&contract)?;

    _transfer_token(deps.storage, sender.clone(), receiver.clone(), amount)?;

    Ok(Response::new()
        .add_message(
            Cw20ReceiveMsg {
                sender: info.sender.to_string(),
                amount: Uint128::new(amount),
                msg,
            }
            .into_cosmos_msg(contract)?,
        )
        .add_attribute("action", "send")
        .add_attribute("sender", sender)
        .add_attribute("receiver", receiver))
}

fn execute_send_nft(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    contract: String,
    token_id: String,
    msg: Binary,
) -> Result<Response, ContractError> {
    let sender = info.sender.clone();
    let receiver = deps.api.addr_validate(&contract)?;

    _transfer_nft(
        deps.storage,
        sender.clone(),
        receiver.clone(),
        token_id.clone(),
    )?;

    Ok(Response::new()
        .add_message(
            Cw721ReceiveMsg {
                sender: info.sender.to_string(),
                token_id,
                msg,
            }
            .into_cosmos_msg(contract)?,
        )
        .add_attribute("action", "send_nft")
        .add_attribute("sender", sender)
        .add_attribute("receiver", receiver))
}

fn execute_increase_allowance(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    spender: String,
    amount: u128,
) -> Result<Response, ContractError> {
    let spender_addr = deps.api.addr_validate(&spender)?;
    let owner = info.sender.clone();

    let allowance = ALLOWANCE
        .load(deps.storage, (owner.clone(), spender_addr.clone()))
        .unwrap_or(Uint128::zero());
    ALLOWANCE.save(
        deps.storage,
        (owner, spender_addr),
        &allowance.checked_add(Uint128::new(amount))?,
    )?;

    Ok(Response::new()
        .add_attribute("action", "increase_allowance")
        .add_attribute("spender", spender)
        .add_attribute("amount", amount.to_string()))
}

fn execute_transfer_from(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    from: String,
    to: String,
    amount: u128,
) -> Result<Response, ContractError> {
    let from_addr = deps.api.addr_validate(&from)?;
    let to_addr = deps.api.addr_validate(&to)?;

    if amount == 0 {
        return Err(ContractError::Unauthorized {});
    }

    let allowance = ALLOWANCE.load(deps.storage, (from_addr.clone(), info.sender.clone()))?;
    if allowance < Uint128::new(amount) {
        return Err(ContractError::Unauthorized {});
    }

    ALLOWANCE.save(
        deps.storage,
        (from_addr.clone(), info.sender.clone()),
        &allowance.checked_sub(Uint128::new(amount))?,
    )?;

    _transfer_token(deps.storage, from_addr.clone(), to_addr.clone(), amount)?;

    Ok(Response::new()
        .add_attribute("action", "transfer_from")
        .add_attribute("from", from)
        .add_attribute("to", to)
        .add_attribute("amount", amount.to_string()))
}

fn execute_approve(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    spender: String,
    amount_or_id: u128,
) -> Result<Response, ContractError> {
    let spender_addr = deps.api.addr_validate(&spender)?;
    let minted = MINTED.load(deps.storage)?;

    if amount_or_id <= minted && amount_or_id > 0 {
        let owner = OWNER_OF.load(deps.storage, amount_or_id)?;

        if info.sender != owner && !IS_APPROVED_FOR_ALL.load(deps.storage, (owner, info.sender))? {
            return Err(ContractError::Unauthorized {});
        }

        GET_APPROVED.save(deps.storage, amount_or_id, &spender_addr)?;
    } else {
        ALLOWANCE.save(
            deps.storage,
            (info.sender, spender_addr),
            &Uint128::new(amount_or_id),
        )?;
    }

    Ok(Response::new()
        .add_attribute("action", "approve")
        .add_attribute("spender", spender)
        .add_attribute("amount_or_id", amount_or_id.to_string()))
}

fn execute_set_approval_for_all(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    operator: String,
    approved: bool,
) -> Result<Response, ContractError> {
    let operator_addr = deps.api.addr_validate(&operator)?;
    IS_APPROVED_FOR_ALL.save(deps.storage, (info.sender, operator_addr), &approved)?;

    Ok(Response::new()
        .add_attribute("action", "set_approval_for_all")
        .add_attribute("operator", operator)
        .add_attribute("approved", approved.to_string()))
}

fn _mint(storage: &mut dyn Storage, to: Addr) -> Result<(), ContractError> {
    let minted = MINTED.load(storage)?;
    MINTED.save(storage, &(minted + 1))?;

    let id = minted;

    if OWNER_OF.has(storage, id) {
        return Err(ContractError::AlreadyExists {});
    };

    OWNER_OF.save(storage, id, &to)?;

    let mut owned_ids = OWNED.load(storage, to.clone()).unwrap_or(vec![]);
    owned_ids.push(id);
    OWNED.save(storage, to.clone(), &owned_ids)?;

    OWNED_INDEX.save(storage, id, &(owned_ids.len() as u128 - 1))?;

    Ok(())
}

fn _burn(storage: &mut dyn Storage, from: Addr) -> Result<(), ContractError> {
    let mut owned = OWNED.load(storage, from.clone())?;

    let id = owned[owned.len() - 1];

    owned.pop();
    OWNED.save(storage, from, &owned)?;

    OWNED_INDEX.remove(storage, id);
    OWNER_OF.remove(storage, id);
    GET_APPROVED.remove(storage, id);

    Ok(())
}

fn _transfer_token(
    storage: &mut dyn Storage,
    sender: Addr,
    receiver: Addr,
    amount: u128,
) -> Result<(), ContractError> {
    let unit = get_unit(storage)?;

    let balance_before_sender = BALANCE_OF
        .load(storage, sender.clone())
        .unwrap_or(Uint128::zero());
    let balance_before_receiver = BALANCE_OF
        .load(storage, receiver.clone())
        .unwrap_or(Uint128::zero());

    BALANCE_OF.save(
        storage,
        sender.clone(),
        &(balance_before_sender - Uint128::new(amount)),
    )?;
    BALANCE_OF.save(
        storage,
        receiver.clone(),
        &(balance_before_receiver + Uint128::new(amount)),
    )?;

    if !WHITELIST.load(storage, sender.clone()).unwrap_or(false) {
        let tokens_to_burn = balance_before_sender.checked_div(unit)?
            - BALANCE_OF
                .load(storage, sender.clone())?
                .checked_div(unit)?;
        for _ in 0..tokens_to_burn.u128() {
            _burn(storage, sender.clone())?;
        }
    }

    if !WHITELIST.load(storage, receiver.clone()).unwrap_or(false) {
        let tokens_to_mint = BALANCE_OF
            .load(storage, receiver.clone())?
            .checked_div(unit)?
            - balance_before_receiver.checked_div(unit)?;
        for _ in 0..tokens_to_mint.u128() {
            _mint(storage, receiver.clone())?;
        }
    }

    Ok(())
}

fn _transfer_nft(
    storage: &mut dyn Storage,
    sender: Addr,
    receiver: Addr,
    token_id: String,
) -> Result<(), ContractError> {
    let id: u128 = token_id.parse()?;

    let unit = get_unit(storage)?;

    if sender != OWNER_OF.load(storage, id)? {
        return Err(ContractError::InvalidSender {});
    }

    // if !IS_APPROVED_FOR_ALL
    //         .load(deps.storage, (sender.clone(), Addr::unchecked(from)))
    //         .unwrap_or(false)
    //     && Addr::unchecked(from) != GET_APPROVED.load(deps.storage, id)?
    // {
    //     return Err(ContractError::Unauthorized {});
    // }

    let sender_balance_of = BALANCE_OF
        .load(storage, sender.clone())
        .unwrap_or(Uint128::zero());
    let receiver_balance_of = BALANCE_OF
        .load(storage, receiver.clone())
        .unwrap_or(Uint128::zero());

    BALANCE_OF.save(
        storage,
        sender.clone(),
        &(sender_balance_of.checked_sub(unit)?),
    )?;
    BALANCE_OF.save(
        storage,
        receiver.clone(),
        &(receiver_balance_of.checked_add(unit)?),
    )?;

    OWNER_OF.save(storage, id, &receiver)?;

    GET_APPROVED.remove(storage, id);

    let mut sender_owned = OWNED.load(storage, sender.clone()).unwrap_or(vec![]);
    let mut receiver_owned = OWNED.load(storage, receiver.clone()).unwrap_or(vec![]);

    let update_id = sender_owned[sender_owned.len() - 1];

    let owned_index_amount_or_id = OWNED_INDEX.load(storage, id)?;

    sender_owned[owned_index_amount_or_id as usize] = update_id;
    sender_owned.pop();
    OWNED.save(storage, sender.clone(), &sender_owned)?;

    OWNED_INDEX.save(storage, update_id, &owned_index_amount_or_id)?;

    receiver_owned.push(id);
    OWNED.save(storage, receiver.clone(), &receiver_owned)?;

    OWNED_INDEX.save(storage, id, &(receiver_owned.len() as u128 - 1))?;

    Ok(())
}

fn get_unit(storage: &dyn Storage) -> Result<Uint128, ContractError> {
    let metadata = METADATA.load(storage)?;
    Ok(Uint128::new(
        (10u32).pow(metadata.decimals as u32 + 1) as u128
    ))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let new_total_supply = 100000000000000u128;
    let mut metadata: Metadata = METADATA.load(deps.storage)?;

    // orig total supply - balance of
    let balance = BALANCE_OF.load(deps.storage, env.contract.address.clone())?;
    let used = Uint128::new(metadata.total_supply) - balance;
    BALANCE_OF.save(
        deps.storage,
        env.contract.address.clone(),
        &(Uint128::new(new_total_supply) - used),
    )?;

    metadata.total_supply = new_total_supply;
    METADATA.save(deps.storage, &metadata)?;

    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => query_config(deps, env),
        QueryMsg::Balance { address } => query_balance(deps, env, address),
        QueryMsg::TokenInfo {} => query_token_info(deps, env),
        QueryMsg::Minter {} => query_minter(deps, env),
        QueryMsg::OwnerOf { token_id } => query_owner_of(deps, env, token_id),
        QueryMsg::Tokens { owner } => query_owned(deps, env, owner),
        QueryMsg::NumTokens {} => query_num_tokens(deps, env),
        QueryMsg::NftInfo { token_id } => query_nft_info(deps, env, token_id),
    }
}

fn query_config(deps: Deps, _env: Env) -> StdResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    to_json_binary(&ConfigResponse {
        admin: config.admin.to_string(),
    })
}

fn query_balance(deps: Deps, _env: Env, address: String) -> StdResult<Binary> {
    let addr = deps.api.addr_validate(&address)?;
    let balance = BALANCE_OF.load(deps.storage, addr)?;
    Ok(to_json_binary(&BalanceResponse { balance })?)
}

fn query_token_info(deps: Deps, _env: Env) -> StdResult<Binary> {
    let metadata = METADATA.load(deps.storage)?;
    Ok(to_json_binary(&TokenInfoResponse {
        name: metadata.name,
        symbol: metadata.symbol,
        decimals: metadata.decimals,
        total_supply: Uint128::new(metadata.total_supply),
    })?)
}

fn query_minter(deps: Deps, _env: Env) -> StdResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    let metadata = METADATA.load(deps.storage)?;
    Ok(to_json_binary(&MinterResponse {
        minter: config.admin.to_string(),
        cap: Some(Uint128::new(metadata.total_supply)),
    })?)
}

fn query_owner_of(deps: Deps, _env: Env, token_id: String) -> StdResult<Binary> {
    let res = token_id.parse();
    if res.is_err() {
        return Err(StdError::generic_err("Invalid token ID"));
    }
    let id = res.unwrap();
    let owner = OWNER_OF.load(deps.storage, id)?;
    Ok(to_json_binary(&OwnerOfResponse {
        owner: owner.to_string(),
        approvals: vec![],
    })?)
}

fn query_owned(deps: Deps, _env: Env, address: String) -> StdResult<Binary> {
    let addr = deps.api.addr_validate(&address)?;
    let owned = OWNED.load(deps.storage, addr)?;
    Ok(to_json_binary(&TokensResponse {
        tokens: owned.iter().map(|o| o.to_string()).collect(),
    })?)
}

fn query_num_tokens(deps: Deps, _env: Env) -> StdResult<Binary> {
    let minted = MINTED.load(deps.storage)?;
    Ok(to_json_binary(&NumTokensResponse {
        count: minted as u64,
    })?)
}

fn query_nft_info(deps: Deps, _env: Env, token_id: String) -> StdResult<Binary> {
    let res = token_id.parse();
    if res.is_err() {
        return Err(StdError::generic_err("Invalid token ID"));
    }
    let id: u128 = res.unwrap();
    let token_uri_id = (id % 10) + 1;
    let metadata = METADATA.load(deps.storage)?;
    let token_uri_string = format!("{}/{}.png", metadata.token_uri, token_uri_id);
    Ok(to_json_binary(&NftInfoResponse {
        token_uri: Some(token_uri_string),
        extension: Empty {},
    })?)
}
