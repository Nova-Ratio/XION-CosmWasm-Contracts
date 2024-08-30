use cosmwasm_std::{
    attr, entry_point, from_slice, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, Uint128,
};
use cw2::set_contract_version;
use cw20::{Cw20ReceiveMsg, Denom};
use cw20_base::allowances::{
    deduct_allowance, execute_decrease_allowance, execute_increase_allowance, execute_send_from,
    execute_transfer_from, query_allowance,
};
use cw20_base::contract::{
    execute_burn, execute_mint, execute_send, execute_transfer, query_balance, query_token_info,
};
use cw20_base::state::{MinterData, TokenInfo, TOKEN_INFO};
use cw_asset::Asset;

use crate::error::ContractError;
use crate::msg::{CurveInfoResponse, ExecuteMsg, QueryMsg, ReceiveMsg};
use crate::state::{CW20Balance, CurveState, CURVE_STATE, CURVE_TYPE, MARKETING_INFO};
use bonding_types::curves::DecimalPlaces;
use bonding_types::msg::{
    CurveFactoryParamsResponse, CurveFactoryQueryMsg, CurveFn, CurveType, InstantiateMsg,
    MarketingInfoResponse,
};
use cw_utils::nonpayable;
// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw20-bonding";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    // This will fail if the response can not be parsed to our factory response type
    let factory = info.sender.clone();
    let _factory_response: CurveFactoryParamsResponse = deps
        .querier
        .query_wasm_smart(factory, &CurveFactoryQueryMsg::Params {})?; // store token info using cw20-base format

    let data = TokenInfo {
        name: msg.name,
        symbol: msg.symbol,
        decimals: msg.decimals,
        total_supply: Uint128::zero(),
        // set self as minter, so we can properly execute mint and burn
        mint: Some(MinterData {
            minter: env.contract.address,
            cap: None,
        }),
    };
    TOKEN_INFO.save(deps.storage, &data)?;
    let places = DecimalPlaces::new(msg.decimals, msg.reserve_decimals);
    let supply = CurveState::new(msg.reserve_denom, places);
    CURVE_STATE.save(deps.storage, &supply)?;

    CURVE_TYPE.save(deps.storage, &msg.curve_type)?;
    if msg.marketing_info.is_some() {
        MARKETING_INFO.save(deps.storage, &msg.marketing_info.unwrap())?;
    }
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    // default implementation stores curve info as enum, you can do something else in a derived
    // contract and just pass in your custom curve to do_execute
    let curve_type = CURVE_TYPE.load(deps.storage)?;
    let curve_fn = curve_type.to_curve_fn();
    do_execute(deps, env, info, msg, curve_fn)
}

/// We pull out logic here, so we can import this from another contract and set a different Curve.
/// This contacts sets a curve with an enum in InstantiateMsg and stored in state, but you may want
/// to use custom math not included - make this easily reusable
pub fn do_execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
    curve_fn: CurveFn,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => execute_receive(deps, env, info, msg, curve_fn),
        // we override these from cw20
        ExecuteMsg::Burn { amount } => Ok(execute_sell(deps, env, info, curve_fn, amount)?),
        ExecuteMsg::BurnFrom { owner, amount } => {
            Ok(execute_sell_from(deps, env, info, curve_fn, owner, amount)?)
        }
        ExecuteMsg::Buy {} => Ok(execute_buy(deps, env, info, None, curve_fn)?),
        // these all come from cw20-base to implement the cw20 standard
        ExecuteMsg::Transfer { recipient, amount } => {
            Ok(execute_transfer(deps, env, info, recipient, amount)?)
        }
        ExecuteMsg::Send {
            contract,
            amount,
            msg,
        } => Ok(execute_send(deps, env, info, contract, amount, msg)?),
        ExecuteMsg::IncreaseAllowance {
            spender,
            amount,
            expires,
        } => Ok(execute_increase_allowance(
            deps, env, info, spender, amount, expires,
        )?),
        ExecuteMsg::DecreaseAllowance {
            spender,
            amount,
            expires,
        } => Ok(execute_decrease_allowance(
            deps, env, info, spender, amount, expires,
        )?),
        ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => Ok(execute_transfer_from(
            deps, env, info, owner, recipient, amount,
        )?),
        ExecuteMsg::SendFrom {
            owner,
            contract,
            amount,
            msg,
        } => Ok(execute_send_from(
            deps, env, info, owner, contract, amount, msg,
        )?),
    }
}

pub fn execute_buy(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    balance: Option<CW20Balance>,
    curve_fn: CurveFn,
) -> Result<Response, ContractError> {
    let mut state = CURVE_STATE.load(deps.storage)?;
    // check if the denom matches
    let mut reciever: Addr;
    let payment = match (balance, info.funds) {
        (Some(balance), _) => {
            if balance.denom != state.reserve_denom {
                return Err(ContractError::WrongCw20Denom {});
            }
            reciever = balance.sender;
            balance.amount
        }
        (None, funds) => {
            if funds.len() != 1 {
                return Err(ContractError::AssetError {});
            }
            let coin = funds.get(0).unwrap();
            if Denom::Native(coin.denom.to_string()) != state.reserve_denom {
                return Err(ContractError::WrongNativeDenom {});
            }
            reciever = info.sender.clone();
            coin.amount
        }
        _ => return Err(ContractError::AssetError {}),
    };
    // calculate how many tokens can be purchased with this and mint them
    let curve = curve_fn(state.clone().decimals);
    state.reserve += payment;

    let new_supply = curve.supply(state.reserve);
    let minted = new_supply
        .checked_sub(state.supply)
        .map_err(StdError::overflow)?;
    state.supply = new_supply;

    CURVE_STATE.save(deps.storage, &state)?;

    // call into cw20-base to mint the token, call as self as no one else is allowed
    let sub_info = MessageInfo {
        sender: env.contract.address.clone(),
        funds: vec![],
    };
    execute_mint(deps, env, sub_info, reciever.to_string(), minted)?;

    let res = Response::new()
        .add_attribute("action", "buy")
        .add_attribute("from", info.sender)
        .add_attribute("reserve", payment)
        .add_attribute("supply", minted);
    Ok(res)
}

pub fn execute_receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    wrapper: Cw20ReceiveMsg,
    curve_fn: CurveFn,
) -> Result<Response, ContractError> {
    let msg = from_slice::<ReceiveMsg>(&wrapper.msg)?;
    let api = deps.api;
    let balance = CW20Balance {
        denom: Denom::Cw20(info.sender.clone()),
        amount: wrapper.amount,
        sender: api.addr_validate(&wrapper.sender)?,
    };
    match msg {
        ReceiveMsg::Buy {} => execute_buy(deps, env, info, Some(balance), curve_fn),
    }
}

pub fn execute_sell(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    curve_fn: CurveFn,
    amount: Uint128,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;
    let receiver = info.sender.clone();
    // do all the work
    let mut res = do_sell(deps, env, info, curve_fn, receiver, amount)?;

    // add our custom attributes
    res.attributes.push(attr("action", "burn"));
    Ok(res)
}

pub fn execute_sell_from(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    curve_fn: CurveFn,
    owner: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;
    let owner_addr = deps.api.addr_validate(&owner)?;
    let spender_addr = info.sender.clone();

    // deduct allowance before doing anything else have enough allowance
    deduct_allowance(deps.storage, &owner_addr, &spender_addr, &env.block, amount)?;

    // do all the work in do_sell
    let receiver_addr = info.sender;
    let owner_info = MessageInfo {
        sender: owner_addr,
        funds: info.funds,
    };
    let mut res = do_sell(
        deps,
        env,
        owner_info,
        curve_fn,
        receiver_addr.clone(),
        amount,
    )?;

    // add our custom attributes
    res.attributes.push(attr("action", "burn_from"));
    res.attributes.push(attr("by", receiver_addr));
    Ok(res)
}

fn do_sell(
    mut deps: DepsMut,
    env: Env,
    // info.sender is the one burning tokens
    info: MessageInfo,
    curve_fn: CurveFn,
    // receiver is the one who gains (same for execute_sell, diff for execute_sell_from)
    receiver: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    // burn from the caller, this ensures there are tokens to cover this
    execute_burn(deps.branch(), env, info.clone(), amount)?;

    // calculate how many tokens can be purchased with this and mint them
    let mut state = CURVE_STATE.load(deps.storage)?;
    let curve = curve_fn(state.clone().decimals);
    state.supply = state
        .supply
        .checked_sub(amount)
        .map_err(StdError::overflow)?;
    let new_reserve = curve.reserve(state.supply);
    let released = state
        .reserve
        .checked_sub(new_reserve)
        .map_err(StdError::overflow)?;
    state.reserve = new_reserve;
    CURVE_STATE.save(deps.storage, &state)?;

    // now send the tokens to the sender (TODO: for sell_from we do something else, right???)
    let asset = match state.reserve_denom {
        Denom::Native(denom) => Asset::native(denom, released),
        Denom::Cw20(denom) => Asset::cw20(denom, released),
    };
    let released_msg = asset.transfer_msg(&receiver)?;

    let res = Response::new()
        .add_message(released_msg)
        .add_attribute("from", info.sender)
        .add_attribute("supply", amount)
        .add_attribute("reserve", released);
    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    // default implementation stores curve info as enum, you can do something else in a derived
    // contract and just pass in your custom curve to do_execute
    let curve_type = CURVE_TYPE.load(deps.storage)?;
    let curve_fn = curve_type.to_curve_fn();
    do_query(deps, env, msg, curve_fn)
}

/// We pull out logic here, so we can import this from another contract and set a different Curve.
/// This contacts sets a curve with an enum in InstantitateMsg and stored in state, but you may want
/// to use custom math not included - make this easily reusable
pub fn do_query(deps: Deps, _env: Env, msg: QueryMsg, curve_fn: CurveFn) -> StdResult<Binary> {
    match msg {
        // custom queries
        QueryMsg::CurveInfo {} => to_binary(&query_curve_info(deps, curve_fn)?),
        // inherited from cw20-base
        QueryMsg::TokenInfo {} => to_binary(&query_token_info(deps)?),
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
        QueryMsg::Allowance { owner, spender } => {
            to_binary(&query_allowance(deps, owner, spender)?)
        }
        QueryMsg::MarketingInfo {} => to_binary(&query_marketing_info(deps)?),
    }
}

// write marketing info query
pub fn query_marketing_info(deps: Deps) -> StdResult<MarketingInfoResponse> {
    let marketing_info = MARKETING_INFO.load(deps.storage)?;
    let response = MarketingInfoResponse {
        description: marketing_info.description,
        project_url: marketing_info.project_url,
        logo: marketing_info.logo,
    };
    Ok(response)
}
pub fn query_curve_info(deps: Deps, curve_fn: CurveFn) -> StdResult<CurveInfoResponse> {
    let CurveState {
        reserve,
        supply,
        reserve_denom,
        decimals,
    } = CURVE_STATE.load(deps.storage)?;

    // This we can get from the local digits stored in instantiate
    let curve = curve_fn(decimals);
    let spot_price = curve.spot_price(supply);
    let curve_type = CURVE_TYPE.load(deps.storage)?;

    Ok(CurveInfoResponse {
        reserve,
        supply,
        spot_price,
        reserve_denom: reserve_denom,
        curve_type,
    })
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::str::FromStr;

    use super::*;
    use bonding_types::msg::CurveType;
    use cosmwasm_std::testing::{
        mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage,
    };
    use cosmwasm_std::{
        BankMsg, Coin, CosmosMsg, Decimal, MemoryStorage, OverflowError, OverflowOperation,
        OwnedDeps, StdError, SubMsg, WasmMsg,
    };
    use cw20::Cw20ExecuteMsg;
    use cw20_base::allowances::execute_burn_from;

    const CREATOR: &str = "creator";
    const INVESTOR: &str = "investor";
    const CW20_DENOM: &str = "cw20_denom";

    fn default_instantiate(
        decimals: u8,
        reserve_decimals: u8,
        curve_type: CurveType,
    ) -> InstantiateMsg {
        InstantiateMsg {
            name: "Bonded".to_string(),
            symbol: "EPOXY".to_string(),
            decimals,
            reserve_denom: Denom::Cw20(Addr::unchecked(CW20_DENOM)),
            reserve_decimals,
            curve_type,
            marketing_info: Some(MarketingInfoResponse {
                description: Some("Epoxy is a bonding curve token".to_string()),
                project_url: Some("https://epoxy.finance".to_string()),
                logo: Some("https://epoxy.finance/logo.png".to_string()),
            }),
        }
    }

    fn get_balance<U: Into<String>>(deps: Deps, addr: U) -> Uint128 {
        query_balance(deps, addr.into()).unwrap().balance
    }

    fn setup_test(
        env: Env,
        curve_type: CurveType,
        decimals: u8,
        reserve_decimals: u8,
        reserve_denom: Denom,
    ) -> OwnedDeps<MockStorage, MockApi, MockQuerier> {
        let mut deps = mock_dependencies();
        let data = TokenInfo {
            name: "Test".to_string(),
            symbol: "TEST".to_string(),
            decimals: decimals,
            total_supply: Uint128::zero(),
            // set self as minter, so we can properly execute mint and burn
            mint: Some(MinterData {
                minter: env.clone().contract.address,
                cap: None,
            }),
        };
        TOKEN_INFO.save(deps.as_mut().storage, &data);

        let curve_fn = curve_type.clone().to_curve_fn();
        let decimals = DecimalPlaces::new(decimals, reserve_decimals);
        let curve_state = CurveState::new(reserve_denom, decimals);
        CURVE_STATE.save(deps.as_mut().storage, &curve_state);
        CURVE_TYPE.save(deps.as_mut().storage, &curve_type);

        deps
    }
    // This needs multitest to work so it will be commented out for now
    // #[test]
    // fn proper_instantiation() {
    //     let mut deps = mock_dependencies();

    //     // this matches `linear_curve` test case from curves.rs
    //     let creator = String::from("creator");
    //     let curve_type = CurveType::SquareRoot {
    //         slope: Uint128::new(1),
    //         scale: 1,
    //     };
    //     let msg = default_instantiate(2, 8, curve_type.clone());
    //     let info = mock_info(&creator, &[]);

    //     // make sure we can instantiate with this
    //     let res = instantiate(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();
    //     assert_eq!(0, res.messages.len());

    //     // token info is proper
    //     let token = query_token_info(deps.as_ref()).unwrap();
    //     assert_eq!(&token.name, &msg.name);
    //     assert_eq!(&token.symbol, &msg.symbol);
    //     assert_eq!(token.decimals, 2);
    //     assert_eq!(token.total_supply, Uint128::zero());

    //     // curve state is sensible
    //     let state = query_curve_info(deps.as_ref(), curve_type.to_curve_fn()).unwrap();
    //     assert_eq!(state.reserve, Uint128::zero());
    //     assert_eq!(state.supply, Uint128::zero());
    //     assert_eq!(
    //         state.reserve_denom,
    //         Denom::Cw20(Addr::unchecked(CW20_DENOM))
    //     );
    //     // spot price 0 as supply is 0
    //     assert_eq!(state.spot_price, Decimal::zero());

    //     // curve type is stored properly
    //     let curve = CURVE_TYPE.load(&deps.storage).unwrap();
    //     assert_eq!(curve_type, curve);

    //     // no balance
    //     assert_eq!(get_balance(deps.as_ref(), &creator), Uint128::zero());
    // }

    #[test]
    fn buy_issues_tokens() {
        let curve_type = CurveType::SquareRoot {
            slope: Uint128::new(1),
            scale: 0,
        };
        let env = mock_env();
        let mut deps = setup_test(
            env.clone(),
            curve_type.clone(),
            6,
            6,
            Denom::Native("uusd".to_string()),
        );

        let info = mock_info(
            "buyer1",
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(10000000),
            }],
        );
        let _res = execute_buy(
            deps.as_mut(),
            env.clone(),
            info,
            None,
            curve_type.clone().to_curve_fn(),
        );

        // query state
        let _state = query_curve_info(deps.as_ref(), curve_type.clone().to_curve_fn()).unwrap();

        // sell tokens
        let info = mock_info("buyer1", &[]);
        let _res = execute_sell(
            deps.as_mut(),
            env,
            info,
            curve_type.to_curve_fn(),
            Uint128::new(6082000u128),
        );

        // query state
        let _state = query_curve_info(deps.as_ref(), curve_type.to_curve_fn()).unwrap();
    }

    #[test]
    fn burning_sends_reserve() {
        let env = mock_env();
        let curve_type = CurveType::Linear {
            slope: Uint128::new(1),
            scale: 1,
            starting_price: Uint128::new(1),
        };
        let mut deps = setup_test(
            env,
            curve_type.clone(),
            2,
            8,
            Denom::Native("uusd".to_string()),
        );

        // succeeds with proper token (20 BTC = 20*10^8 satoshi)
        let info = mock_info(
            INVESTOR.into(),
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(2000000000),
            }],
        );
        execute_buy(
            deps.as_mut(),
            mock_env(),
            info,
            None,
            curve_type.clone().to_curve_fn(),
        )
        .unwrap();

        assert_eq!(get_balance(deps.as_ref(), INVESTOR), Uint128::new(1999));
        // cannot burn too much
        let info = mock_info(INVESTOR, &[]);
        let burn = ExecuteMsg::Burn {
            amount: Uint128::new(3000),
        };

        let err =
            execute_burn(deps.as_mut(), mock_env(), info, Uint128::new(3000u128)).unwrap_err();
        assert_eq!(
            err,
            cw20_base::ContractError::Std(StdError::Overflow {
                source: OverflowError::new(OverflowOperation::Sub, "1999", "3000")
            })
        );
        let info = mock_info(INVESTOR, &[]);

        let res = execute_sell(
            deps.as_mut(),
            mock_env(),
            info,
            curve_type.clone().to_curve_fn(),
            Uint128::new(1000u128),
        )
        .unwrap();
        // balance is lower
        assert_eq!(get_balance(deps.as_ref(), INVESTOR), Uint128::new(999));
        // ensure we got our money back
        assert_eq!(1, res.messages.len());
        assert_eq!(
            &res.messages[0],
            &SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: INVESTOR.into(),
                amount: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::new(1500999491),
                }],
            }))
        );

        // check curve info updated
        let curve = query_curve_info(deps.as_ref(), curve_type.to_curve_fn()).unwrap();
        assert_eq!(curve.reserve, Uint128::new(499000509));
        assert_eq!(curve.supply, Uint128::new(999));
        assert_eq!(curve.spot_price, Decimal::from_str("0.99900001").unwrap());
        // check token info updated
        let token = query_token_info(deps.as_ref()).unwrap();
        assert_eq!(token.decimals, 2);
        assert_eq!(token.total_supply, Uint128::new(999));
    }
    #[test]
    fn cw20_imports_work() {
        let env = mock_env();
        let curve_type = CurveType::Constant {
            value: Uint128::new(15),
            scale: 1,
        };
        let curve_fn = curve_type.to_curve_fn();
        let mut deps = setup_test(
            env.clone(),
            curve_type,
            2,
            8,
            Denom::Native("uusd".to_string()),
        );

        let alice: &str = "alice";
        let bob: &str = "bobby";
        let carl: &str = "carl";

        let info = mock_info(
            bob,
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(30_000_000),
            }],
        );
        let res = execute_buy(deps.as_mut(), env.clone(), info, None, curve_fn).unwrap();

        // check balances
        assert_eq!(get_balance(deps.as_ref(), bob), Uint128::new(20));
        assert_eq!(get_balance(deps.as_ref(), carl), Uint128::zero());

        // send coins to carl
        let bob_info = mock_info(bob, &[]);
        execute_transfer(
            deps.as_mut(),
            env.clone(),
            bob_info.clone(),
            carl.into(),
            Uint128::new(10),
        )
        .unwrap();
        assert_eq!(get_balance(deps.as_ref(), bob), Uint128::new(10));
        assert_eq!(get_balance(deps.as_ref(), carl), Uint128::new(10));

        // allow alice
        execute_increase_allowance(
            deps.as_mut(),
            env,
            bob_info,
            alice.into(),
            Uint128::new(10u128),
            None,
        )
        .unwrap();
        assert_eq!(get_balance(deps.as_ref(), bob), Uint128::new(10));
        assert_eq!(get_balance(deps.as_ref(), alice), Uint128::zero());
        assert_eq!(
            query_allowance(deps.as_ref(), bob.into(), alice.into())
                .unwrap()
                .allowance,
            Uint128::new(10)
        );

        // alice takes some for herself
        let self_pay = ExecuteMsg::TransferFrom {
            owner: bob.into(),
            recipient: alice.into(),
            amount: Uint128::new(25_000_000),
        };
        let alice_info = mock_info(alice, &[]);
        execute_transfer_from(
            deps.as_mut(),
            mock_env(),
            alice_info,
            bob.into(),
            alice.into(),
            Uint128::new(10),
        );
        assert_eq!(get_balance(deps.as_ref(), bob), Uint128::new(0));
        assert_eq!(get_balance(deps.as_ref(), alice), Uint128::new(10));
        assert_eq!(get_balance(deps.as_ref(), carl), Uint128::new(10));
        assert_eq!(
            query_allowance(deps.as_ref(), bob.into(), alice.into())
                .unwrap()
                .allowance,
            Uint128::new(0)
        );
    }
    #[test]
    pub fn test_native() {
        // init
        let mut deps = mock_dependencies();
        let env = mock_env();
        let curve_type = CurveType::Linear {
            slope: Uint128::new(1),
            scale: 1,
            starting_price: Uint128::new(1),
        };
        let data = TokenInfo {
            name: "Test".to_string(),
            symbol: "TEST".to_string(),
            decimals: 6,
            total_supply: Uint128::zero(),
            // set self as minter, so we can properly execute mint and burn
            mint: Some(MinterData {
                minter: env.clone().contract.address,
                cap: None,
            }),
        };
        TOKEN_INFO.save(&mut deps.storage, &data);

        let curve_fn = curve_type.clone().to_curve_fn();
        let decimals = DecimalPlaces::new(2, 8);
        let curve_state = CurveState::new(Denom::Native("uusd".to_string()), decimals);
        CURVE_STATE.save(&mut deps.storage, &curve_state);

        let info = mock_info("bob", &[Coin::new(1000000, "uusd")]);
        let res = execute_buy(deps.as_mut(), env.clone(), info, None, curve_fn).unwrap();
        assert_eq!(get_balance(deps.as_ref(), "bob"), Uint128::new(44));

        let curve_fn = curve_type.clone().to_curve_fn();
        // sell
        let info = mock_info("bob", &[]);
        let res = execute_sell(deps.as_mut(), env, info, curve_fn, Uint128::new(44u128)).unwrap();
        assert_eq!(
            res.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "bob".to_string(),
                amount: vec![Coin::new(1000000, "uusd")],
            })
        );
        assert_eq!(get_balance(deps.as_ref(), "bob"), Uint128::new(0));
    }
}
