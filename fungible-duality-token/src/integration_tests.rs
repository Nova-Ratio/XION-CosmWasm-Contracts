#[cfg(test)]
pub mod tests {
    use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
    use cosmwasm_std::{Addr, Empty, Uint128};
    use cw20::{BalanceResponse, TokenInfoResponse};
    use cw721::{Cw721ExecuteMsg, Cw721QueryMsg, NumTokensResponse, OwnerOfResponse};
    use cw_multi_test::{App, Contract, ContractWrapper, Executor};

    pub fn challenge_contract() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            crate::contract::execute,
            crate::contract::instantiate,
            crate::contract::query,
        );

        Box::new(contract)
    }

    pub const ADMIN: &str = "admin";
    pub const USER1: &str = "user1";

    pub fn proper_instantiate() -> (App, Addr) {
        let mut app = App::default();
        let challenge_id = app.store_code(challenge_contract());

        // Init challenge
        let challenge_inst = InstantiateMsg {
            name: "Name".to_string(),
            symbol: "Symbol".to_string(),
            decimals: 6u8,
            total_native_supply: 1,
            token_uri: "token_uri".to_string(),
        };

        let contract_addr = app
            .instantiate_contract(
                challenge_id,
                Addr::unchecked(ADMIN),
                &challenge_inst,
                &[],
                "test",
                None,
            )
            .unwrap();

        // Minting
        app.execute_contract(
            Addr::unchecked(ADMIN),
            contract_addr.clone(),
            &ExecuteMsg::Mint {},
            &[],
        )
        .unwrap();

        let n_tokens: NumTokensResponse = app
            .wrap()
            .query_wasm_smart(contract_addr.clone(), &Cw721QueryMsg::NumTokens {})
            .unwrap();
        assert_eq!(n_tokens.count, 1);

        (app, contract_addr)
    }

    #[test]
    fn instantiate_check() {
        let (mut app, contract_addr) = proper_instantiate();

        let balance_of: BalanceResponse = app
            .wrap()
            .query_wasm_smart(
                contract_addr.clone(),
                &QueryMsg::Balance {
                    address: ADMIN.to_string(),
                },
            )
            .unwrap();
        assert_eq!(balance_of.balance, Uint128::from(1000000u128));

        let token_info: TokenInfoResponse = app
            .wrap()
            .query_wasm_smart(contract_addr.clone(), &QueryMsg::TokenInfo {})
            .unwrap();

        assert_eq!(token_info.total_supply, Uint128::from(1000000u128));
    }

    #[test]
    fn send_token() {
        let (mut app, contract_addr) = proper_instantiate();

        app.execute_contract(
            Addr::unchecked(ADMIN),
            contract_addr.clone(),
            &Cw721ExecuteMsg::TransferNft {
                recipient: USER1.to_string(),
                token_id: "0".to_string(),
            },
            &[],
        )
        .unwrap();
        let owner_of: OwnerOfResponse = app
            .wrap()
            .query_wasm_smart(
                contract_addr.clone(),
                &QueryMsg::OwnerOf {
                    token_id: "0".to_string(),
                },
            )
            .unwrap();
        assert_eq!(owner_of.owner, Addr::unchecked(USER1));
    }
}
