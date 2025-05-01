use starknet::{ContractAddress, contract_address_const};

use snforge_std::{
    declare, ContractClassTrait, DeclareResultTrait, DeclareResult, start_cheat_caller_address,
    stop_cheat_caller_address, spy_events, EventSpyAssertionsTrait,
};
use streamfi_contracts::interfaces::ierc20::{IERC20Dispatcher, IERC20DispatcherTrait};
use streamfi_contracts::interfaces::istreamfi::{IStreamFiDispatcher, IStreamFiDispatcherTrait};

fn deploy_streamfi() -> (ContractAddress, IStreamFiDispatcher) {
    let contract = declare("StreamFi").unwrap().contract_class();
    let constructor_calldata = ArrayTrait::new();

    let (contract_address, _) = contract.deploy(@constructor_calldata).unwrap();

    let dispatcher = IStreamFiDispatcher { contract_address };

    (contract_address, dispatcher)
}

fn deploy_mock_token() -> ContractAddress {
    let token_owner = contract_address_const::<'token_owner'>();
    let owner = token_owner.into();
    let token_contract = declare("MockToken").unwrap().contract_class();
    let initial_supply = 1000000;
    let mut constructor_calldata = array![initial_supply, owner];

    let (token_address, _) = token_contract.deploy(@constructor_calldata).unwrap();

    token_address
}

fn setup() -> (ContractAddress, IStreamFiDispatcher, ContractAddress) {
    let (streamfi_address, streamfi_dispatcher) = deploy_streamfi();
    let mock_token_address = deploy_mock_token();

    (streamfi_address, streamfi_dispatcher, mock_token_address)
}

fn zero_address() -> ContractAddress {
    contract_address_const::<0>()
}

fn sender() -> ContractAddress {
    contract_address_const::<'sender'>()
}

#[test]
#[should_panic(expected: 'Zero Address Recipient')]
fn test_transfer_tokens_to_zero_address_should_fail() {
    let (streamfi_address, streamfi_dispatcher, mock_token_address) = setup();

    let zero_address = zero_address();
    let sender = sender();
    let token = IERC20Dispatcher { contract_address: mock_token_address };

    let contract = IStreamFiDispatcher { contract_address: streamfi_address };

    start_cheat_caller_address(streamfi_address, zero_address);

    contract.transfer_tokens(sender, 200);

    assert(token.balance_of(zero_address) == 800, 'Transfer failed');

    stop_cheat_caller_address(streamfi_address)
}
