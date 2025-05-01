
#[starknet::contract]
pub mod StreamFi {
    use starknet::event::EventEmitter;
    use streamfi_contracts::interfaces::istreamfi::IStreamFi;
    // use streamfi_contracts::interfaces::ierc20::{IERC20Dispatcher, IERC20DispatcherTrait};
    use openzeppelin::token::erc20::interface::{IERC20Dispatcher, IERC20DispatcherTrait};
    use starknet::{contract_address_const, ContractAddress, get_caller_address, get_contract_address};
    use core::num::traits::zero::Zero;
    use starknet::storage::{StoragePathEntry, StoragePointerWriteAccess, StoragePointerReadAccess, Map};

    // fn strk_token() -> ContractAddress{
    //     contract_address_const::<0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d>()
    // }

    #[storage]
    pub struct Storage {
        token: ContractAddress,
    }

    #[event]
    #[derive(Copy, Drop, starknet::Event)]
    pub enum Event {
        TransferSuccessful: TransferSuccessful
    }

    #[derive(Copy, Drop, starknet::Event)]
    pub struct TransferSuccessful {
        sender: ContractAddress,
        recipient: ContractAddress,
        amount: u256
    }

    #[constructor]
    fn constructor(ref self: ContractState, token_address: ContractAddress){
        self.token.write(token_address);
    }

    #[abi(embed_v0)]
    pub impl StreamFiImpl of IStreamFi<ContractState> {
        fn transfer_tokens(ref self: ContractState, recipient: ContractAddress, amount: u256) {
            let sender = get_caller_address();
            assert(!sender.is_zero(), 'Zero Address Sender');
            assert(!recipient.is_zero(), 'Zero Address Recipient');
            
            let token_address = self.token.read();
            let this_contract = get_contract_address();
            let token = IERC20Dispatcher { contract_address: token_address };

            let user_token_balance = token.balance_of(sender);

            assert(user_token_balance >= amount, 'Amount Overflows Balance');

            let transfer = token.transfer_from(sender, recipient, amount);

            assert(transfer, 'Transfer Failed');

            self
                .emit(
                    TransferSuccessful {
                        sender, recipient, amount
                    }
                )
        }
    }
}