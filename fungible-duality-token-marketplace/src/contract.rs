#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_json, to_json_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo,
    Order, Reply, Response, StdResult, SubMsg, Uint128, Uint256, WasmMsg,
};
use cw2::set_contract_version;
use cw20::{Cw20Contract, Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw721::{Cw721ExecuteMsg, Cw721ReceiveMsg};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, ReceiveMsg, ReceiveNftMsg};
use crate::state::{Config, Listing, CONFIG, LISTINGS, LISTING_COUNTER};

pub const CONTRACT_NAME: &str = "cw404-marketplace";
pub const CONTRACT_VERSION: &str = "0.1.0";

pub const LISTING_REPLY: u64 = 1;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        cw404_address: deps.api.addr_validate(&msg.cw404_address)?,
        cw20_address: deps.api.addr_validate(&msg.cw20_address)?,
    };

    CONFIG.save(deps.storage, &config)?;
    LISTING_COUNTER.save(deps.storage, &0u128)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("NFT", config.cw404_address)
        .add_attribute("Cw20 Token", config.cw20_address))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CancelListing { id } => execute_cancel_listing(deps, info, id),
        ExecuteMsg::Receive(msg) => execute_receive(deps, info, msg),
        ExecuteMsg::ReceiveNft(msg) => execute_receive_nft(deps, env, info, msg),
    }
}

pub fn execute_cancel_listing(
    deps: DepsMut,
    info: MessageInfo,
    id: String,
) -> Result<Response, ContractError> {
    let listing = LISTINGS.load(deps.storage, id.clone())?;

    if listing.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    let config = CONFIG.load(deps.storage)?;

    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.cw404_address.to_string(),
        msg: to_json_binary(&Cw721ExecuteMsg::TransferNft {
            recipient: listing.owner.to_string(),
            token_id: listing.nft_id.clone(),
        })?,
        funds: vec![],
    });

    LISTINGS.remove(deps.storage, id.clone());

    let _ = LISTING_COUNTER.update(deps.storage, |counter: u128| -> StdResult<u128> {
        Ok(counter.checked_sub(1u128).unwrap())
    });

    Ok(Response::new()
        .add_attribute("action", "cancel listing")
        .add_attribute("NFT", listing.nft_id)
        .add_message(msg))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(_deps: DepsMut, _env: Env, reply: Reply) -> Result<Response, ContractError> {
    match reply.id {
        LISTING_REPLY => Ok(Response::new().add_attribute("Operation", "NFT Listing")),
        _ => Err(ContractError::UnrecognizedReply {}),
    }
}

pub fn execute_receive(
    deps: DepsMut,
    info: MessageInfo,
    cw20_receive_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.cw20_address != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    let msg: ReceiveMsg = from_json(&cw20_receive_msg.msg)?;
    match msg {
        ReceiveMsg::Buy { id } => receive_buy(
            deps,
            id,
            cw20_receive_msg.sender,
            cw20_receive_msg.amount,
            info.sender,
        ),
    }
}

pub fn receive_buy(
    deps: DepsMut,
    id: String,
    sender: String,
    amount: Uint128,
    cw20_address: Addr,
) -> Result<Response, ContractError> {
    let listing = LISTINGS.load(deps.storage, id.clone())?;

    if Uint256::from_uint128(amount) != listing.price {
        return Err(ContractError::IncorrectPayment {
            price: listing.price,
        });
    }

    let config = CONFIG.load(deps.storage)?;

    let submsg = SubMsg::reply_on_success(
        WasmMsg::Execute {
            contract_addr: config.cw404_address.to_string(),
            msg: to_json_binary(&Cw721ExecuteMsg::TransferNft {
                recipient: sender.clone(),
                token_id: listing.nft_id.clone(),
            })?,
            funds: vec![],
        },
        LISTING_REPLY,
    );

    let mut res = Response::new()
        .add_attribute("action", "receive_buy")
        .add_attribute("NFT", listing.nft_id)
        .add_attribute("seller", listing.owner.clone().into_string())
        .add_attribute("buyer", sender)
        .add_submessage(submsg);

    let cw20 = Cw20Contract(cw20_address);

    let payment = cw20.call(Cw20ExecuteMsg::Transfer {
        recipient: listing.owner.into_string().clone(),
        amount,
    })?;

    LISTINGS.remove(deps.storage, id);
    let _ = LISTING_COUNTER.update(deps.storage, |counter: u128| -> StdResult<u128> {
        Ok(counter.checked_sub(1u128).unwrap())
    });
    res = res.add_message(payment);

    Ok(res)
}

pub fn execute_receive_nft(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    receive_msg: Cw721ReceiveMsg,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.cw404_address != info.sender {
        return Err(ContractError::Unauthorized {});
    };

    // info.sender is the NFT contract Address
    let sender = receive_msg.sender.clone();

    let msg: ReceiveNftMsg = from_json(&receive_msg.msg)?;
    match msg {
        ReceiveNftMsg::NewListing { price } => {
            receive_new_listing(deps, sender, receive_msg.token_id, price)
        }
    }
}

pub fn receive_new_listing(
    deps: DepsMut,
    sender: String,
    id: String,
    price: Uint256,
) -> Result<Response, ContractError> {
    let owner = deps.api.addr_validate(&sender)?;

    let new_listing = Listing {
        nft_id: id.clone(),
        price,
        owner,
    };

    LISTINGS.save(deps.storage, id.clone(), &new_listing)?;
    let _ = LISTING_COUNTER.update(deps.storage, |counter: u128| -> StdResult<u128> {
        Ok(counter.checked_add(1u128).unwrap())
    });

    let res = Response::new()
        .add_attribute("action", "new listing")
        .add_attribute("NFT", id)
        .add_attribute("owner", sender);

    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: Empty) -> Result<Response, ContractError> {
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetListing { id } => to_json_binary(&get_listing(deps, id)?),
        QueryMsg::GetListingsBySeller {
            seller,
            from_index,
            limit,
        } => to_json_binary(&get_listings_seller(deps, seller, from_index, limit)?),
        QueryMsg::GetAllListings { from_index, limit } => {
            to_json_binary(&get_all_listings(deps, from_index, limit)?)
        }
        QueryMsg::GetListingCount {} => to_json_binary(&get_listing_count(deps)?),
    }
}

pub fn get_listing_count(deps: Deps) -> StdResult<u128> {
    Ok(LISTING_COUNTER.load(deps.storage)?)
}

pub fn get_listing(deps: Deps, id: String) -> StdResult<Listing> {
    let listing = LISTINGS.load(deps.storage, id)?;
    Ok(listing)
}

pub fn get_listings_seller(
    deps: Deps,
    seller: String,
    from_index: Option<u64>,
    limit: Option<u64>,
) -> StdResult<Vec<Listing>> {
    let from_index = from_index.unwrap_or(0);
    let limit = limit.unwrap_or(10);

    let listings: StdResult<Vec<Listing>> = LISTINGS
        .range(deps.storage, None, None, Order::Ascending)
        .skip(from_index as usize)
        .take(limit as usize)
        .filter(|item| item.as_ref().unwrap().1.owner == seller)
        .map(|item| item.map(|(_, listing)| listing))
        .collect();
    listings
}

pub fn get_all_listings(
    deps: Deps,
    from_index: Option<u64>,
    limit: Option<u64>,
) -> StdResult<Vec<Listing>> {
    let from_index = from_index.unwrap_or(0);
    let limit = limit.unwrap_or(10);

    let listings: StdResult<Vec<Listing>> = LISTINGS
        .range(deps.storage, None, None, Order::Ascending)
        .skip(from_index as usize)
        .take(limit as usize)
        .map(|item| item.map(|(_, listing)| listing))
        .collect();
    listings
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{from_json, Addr};

    #[test]
    fn instantiate_contract() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            cw404_address: String::from(MOCK_CONTRACT_ADDR),
            cw20_address: String::from(MOCK_CONTRACT_ADDR),
        };
        let info = mock_info("creator", &[]);

        let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    }

    #[test]
    fn test_receive_list() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            cw404_address: String::from("cw404"),
            cw20_address: String::from("cw20"),
        };
        let mut info = mock_info("creator", &[]);

        let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
            sender: String::from("cw404"),
            token_id: "1".to_string(),
            msg: to_json_binary(&ReceiveNftMsg::NewListing {
                price: Uint256::from(5u128),
            })
            .unwrap(),
        });

        info.sender = Addr::unchecked("cw404");
        let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        // query listings
        let msg = QueryMsg::GetAllListings {
            from_index: Some(0),
            limit: Some(10),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();

        let listings: Vec<Listing> = from_json(&res).unwrap();
        assert_eq!(
            listings,
            vec![Listing {
                nft_id: "1".to_string(),
                price: Uint256::from(5u128),
                owner: Addr::unchecked("cw404"),
            }]
        );
    }

    #[test]
    fn test_receive_buy() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            cw404_address: String::from("cw404"),
            cw20_address: String::from("cw20"),
        };
        let mut info = mock_info("creator", &[]);

        let _res = instantiate(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        let msg = ExecuteMsg::ReceiveNft(Cw721ReceiveMsg {
            sender: String::from("cw404"),
            token_id: "1".to_string(),
            msg: to_json_binary(&ReceiveNftMsg::NewListing {
                price: Uint256::from(5u128),
            })
            .unwrap(),
        });

        info.sender = Addr::unchecked("cw404");
        let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

        // query listings
        let msg = QueryMsg::GetAllListings {
            from_index: Some(0),
            limit: Some(10),
        };
        let res = query(deps.as_ref(), mock_env(), msg).unwrap();

        let listings: Vec<Listing> = from_json(&res).unwrap();
        assert_eq!(
            listings,
            vec![Listing {
                nft_id: "1".to_string(),
                price: Uint256::from(5u128),
                owner: Addr::unchecked("cw404"),
            }]
        );

        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: String::from("cw20"),
            amount: Uint128::new(5),
            msg: to_json_binary(&ReceiveMsg::Buy {
                id: "1".to_string(),
            })
            .unwrap(),
        });

        info.sender = Addr::unchecked("cw20");
        let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();
    }
}
