use aztec::macros::aztec;

#[aztec]
pub contract PrivateBonds {
    use aztec::{
        macros::{functions::{external, initializer, only_self}, storage::storage},
        protocol_types::address::AztecAddress,
        state_vars::{Map, Owned, public_mutable::PublicMutable},
    };
    use aztec::messages::message_delivery::MessageDelivery;
    use balance_set::BalanceSet;

    #[storage]
    struct Storage<Context> {
        // Issuer's admin address - can mint token
        owner: PublicMutable<AztecAddress, Context>,
        // Public balances - visible for transparency
        public_balances: Map<AztecAddress, PublicMutable<u64, Context>, Context>,
        // Private balances - only the issuer can see these
        private_balances: Owned<BalanceSet<Context>, Context>,
    }

    #[initializer]
    #[external("public")]
    fn initialize() {
        // Issuer becomes the owner who can mint bond tokens
        self.storage.owner.write(self.msg_sender().unwrap());
    }

    #[external("public")]
    fn mint_public(investor: AztecAddress, amount: u64) {
        // Only Issuer can mint tokens
        assert_eq(
            self.msg_sender().unwrap(),
            self.storage.owner.read(),
            "Only Issuer can mint bond tokens",
        );

        // Add tokens to investor's public balance
        let current_balance = self.storage.public_balances.at(investor).read();
        self.storage.public_balances.at(investor).write(current_balance + amount);
    }

    #[external("public")]
    fn transfer_public(to: AztecAddress, amount: u64) {
        let sender = self.msg_sender().unwrap();
        let sender_balance = self.storage.public_balances.at(sender).read();
        assert(sender_balance >= amount, "Insufficient BOB tokens");

        // Deduct from sender
        self.storage.public_balances.at(sender).write(sender_balance - amount);

        // Add to recipient
        let recipient_balance = self.storage.public_balances.at(to).read();
        self.storage.public_balances.at(to).write(recipient_balance + amount);
    }

    #[external("public")]
    fn transfer_ownership(new_owner: AztecAddress) {
        assert_eq(
            self.msg_sender().unwrap(),
            self.storage.owner.read(),
            "Only current admin can transfer ownership",
        );
        self.storage.owner.write(new_owner);
    }

    #[external("private")]
    fn public_to_private(amount: u64) {
        let sender = self.msg_sender().unwrap();
        // This will enqueue a public function to deduct from public balance
        self.enqueue_self._deduct_public_balance(sender, amount);
        // Add to private balance
        self.storage.private_balances.at(sender).add(amount as u128).deliver(
            MessageDelivery.CONSTRAINED_ONCHAIN,
        );
    }

    #[external("public")]
    #[only_self]
    fn _deduct_public_balance(owner: AztecAddress, amount: u64) {
        let balance = self.storage.public_balances.at(owner).read();
        assert(balance >= amount, "Insufficient public bond tokens");
        self.storage.public_balances.at(owner).write(balance - amount);
    }

    #[external("private")]
    fn transfer_private(to: AztecAddress, amount: u64) {
        let sender = self.msg_sender().unwrap();
        // Spend sender's notes (consumes existing notes)
        self.storage.private_balances.at(sender).sub(amount as u128).deliver(
            MessageDelivery.CONSTRAINED_ONCHAIN,
        );
        // Create new notes for recipient
        self.storage.private_balances.at(to).add(amount as u128).deliver(
            MessageDelivery.CONSTRAINED_ONCHAIN,
        );
    }

    #[external("utility")]
    unconstrained fn private_balance_of(owner: AztecAddress) -> pub u128 {
        self.storage.private_balances.at(owner).balance_of()
    }

    #[external("utility")]
    unconstrained fn public_balance_of(owner: AztecAddress) -> pub u64 {
        self.storage.public_balances.at(owner).read()
    }
}
