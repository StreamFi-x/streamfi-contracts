use starknet::{ContractAddress, contract_address_const};

use snforge_std::{
    declare, ContractClassTrait, DeclareResultTrait, DeclareResult, start_cheat_caller_address,
    stop_cheat_caller_address, spy_events, EventSpyAssertionsTrait,
};
use openzeppelin::token::erc20::interface::{IERC20Dispatcher, IERC20DispatcherTrait};
use streamfi_contracts::interfaces::istreamfi::{IStreamFiDispatcher, IStreamFiDispatcherTrait};

fn deploy_streamfi(token_address: ContractAddress) -> (ContractAddress, IStreamFiDispatcher) {
    let contract = declare("StreamFi").unwrap().contract_class();
    let constructor_calldata = array![token_address.into()];

    let (contract_address, _) = contract.deploy(@constructor_calldata).unwrap();

    let dispatcher = IStreamFiDispatcher { contract_address };

    (contract_address, dispatcher)
}

fn deploy_mock_token() -> (ContractAddress, ContractAddress) { //Mock token address, owner address
    let token_owner = contract_address_const::<'token_owner'>();
    let owner = token_owner.into();
    let token_contract = declare("MockToken").unwrap().contract_class();
    let initial_supply = 1000000;
    let mut constructor_calldata = array![initial_supply, owner];

    let (token_address, _) = token_contract.deploy(@constructor_calldata).unwrap();

    (token_address, token_owner)
}

fn setup() -> (ContractAddress, IStreamFiDispatcher, ContractAddress, ContractAddress) {
    let (mock_token_address, token_owner) = deploy_mock_token();
    let (streamfi_address, streamfi_dispatcher) = deploy_streamfi(mock_token_address);

    (streamfi_address, streamfi_dispatcher, mock_token_address, token_owner)
}

fn zero_address() -> ContractAddress {
    contract_address_const::<0>()
}

fn sender() -> ContractAddress {
    contract_address_const::<'sender'>()
}

#[test]
fn test_transfer_tokens_successful() {
    let (streamfi_address, _, mock_token_address, token_owner) = setup();

    let another_recipient = contract_address_const::<'another_recipient'>();
    
    let contract = IStreamFiDispatcher { contract_address: streamfi_address };
    let token = IERC20Dispatcher { contract_address: mock_token_address };
    
    // Approve the amount (or greater) to the contract as the token owner
    start_cheat_caller_address(mock_token_address, token_owner);
    assert(token.balance_of(token_owner) == 1000000, 'Wrong owner setup'); // confirm the mint worked
    let current_token_owner_balance = token.balance_of(token_owner); // get the current token balance of the owner
    let approval = token.approve(streamfi_address, 1000); // approve the funds
    assert(approval, 'Approval failed');
    stop_cheat_caller_address(mock_token_address);
    
    // Now the contract can spend the funds on behalf of the owner
    start_cheat_caller_address(streamfi_address, token_owner); 

    let current_recipient_balance = token.balance_of(another_recipient);

    contract.transfer_tokens(another_recipient, 500);

    let new_token_owner_balance = token.balance_of(token_owner);

    let new_recipient_balance = token.balance_of(another_recipient);

    stop_cheat_caller_address(streamfi_address);

    // Get the changes in their balances
    let token_owner_balance_change = current_token_owner_balance - new_token_owner_balance;
    let recipient_balance_change = new_recipient_balance - current_recipient_balance;

    assert(token_owner_balance_change == 500, 'Transfer failed');
    assert(recipient_balance_change == 500, 'Transfer failed')
}

#[test]
#[should_panic(expected: 'Zero Address Sender')]
fn test_transfer_tokens_from_zero_address_should_fail() {
    let (streamfi_address, _, mock_token_address, _) = setup();

    let zero_address = zero_address();
    let sender = sender();
    let token = IERC20Dispatcher { contract_address: mock_token_address };

    let contract = IStreamFiDispatcher { contract_address: streamfi_address };

    start_cheat_caller_address(streamfi_address, zero_address);

    // Zero address has no tokens, but the fact that he is calling the transfer function should cause the panic
    contract.transfer_tokens(sender, 200);

    assert(token.balance_of(zero_address) == 800, 'Transfer failed');

    stop_cheat_caller_address(streamfi_address)
}

#[test]
#[should_panic(expected: 'Zero Address Recipient')]
fn test_transfer_tokens_to_zero_address_should_fail() {
    let (streamfi_address, _, _, _) = setup();

    let zero_address = zero_address();
    let sender = sender();

    let contract = IStreamFiDispatcher { contract_address: streamfi_address };

    start_cheat_caller_address(streamfi_address, sender);

    // The sender trying to transfer to a zero recipient should fail
    contract.transfer_tokens(zero_address, 200);

    stop_cheat_caller_address(streamfi_address)
}

#[test]
#[should_panic(expected: 'Amount Overflows Balance')]
fn test_transfer_more_than_balance_should_fail() {
    let (streamfi_address, _, _, token_owner) = setup();

    let another_recipient = contract_address_const::<'another_recipient'>();

    let contract = IStreamFiDispatcher { contract_address: streamfi_address };

    start_cheat_caller_address(streamfi_address, token_owner);

    // The sender trying to transfer more than his balance should fail
    contract.transfer_tokens(another_recipient, 1000000000);

    stop_cheat_caller_address(streamfi_address)
}
