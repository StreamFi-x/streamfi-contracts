use starknet::ContractAddress;

#[starknet::interface]
pub trait IStreamFi<TContractState> {
    fn transfer_tokens(ref self: TContractState, recipient: ContractAddress, amount: u256);
}