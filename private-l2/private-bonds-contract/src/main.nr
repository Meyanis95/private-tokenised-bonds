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
        // Issuer's admin address - can distribute tokens and manage whitelist
        owner: PublicMutable<AztecAddress, Context>,
        // Public balances - visible for transparency (used for public operations)
        public_balances: Map<AztecAddress, PublicMutable<u64, Context>, Context>,
        // Private balances - encrypted notes, only visible to note owner
        private_balances: Owned<BalanceSet<Context>, Context>,
        // List of authorized addresses (KYC'd investors)
        whitelist: Map<AztecAddress, PublicMutable<bool, Context>, Context>,
        // Total bond supply - fixed at initialization, never changes
        total_supply: PublicMutable<u64, Context>,
        // Maturity date of the bond
        maturity_date: PublicMutable<u64, Context>,
    }

    #[initializer]
    #[external("private")]
    fn initialize(total_supply: u64, maturity: u64) {
        let issuer = self.msg_sender().unwrap();

        // Set public state via enqueued calls
        self.enqueue_self._set_owner(issuer);
        self.enqueue_self._set_maturity(maturity);
        self.enqueue_self._set_total_supply(total_supply);

        // Mint entire supply to issuer's private balance
        // This is the ONLY minting that ever happens
        self.storage.private_balances.at(issuer).add(total_supply as u128).deliver(
            MessageDelivery.CONSTRAINED_ONCHAIN,
        );
    }

    #[external("public")]
    #[only_self]
    fn _set_owner(owner: AztecAddress) {
        self.storage.owner.write(owner);
    }

    #[external("public")]
    #[only_self]
    fn _set_maturity(maturity: u64) {
        self.storage.maturity_date.write(maturity);
    }

    #[external("public")]
    #[only_self]
    fn _set_total_supply(total_supply: u64) {
        self.storage.total_supply.write(total_supply);
    }

    #[external("public")]
    fn add_to_whitelist(investor: AztecAddress) {
        assert_eq(
            self.msg_sender().unwrap(),
            self.storage.owner.read(),
            "Only owner can manage whitelist",
        );
        assert(!self.storage.whitelist.at(investor).read(), "Investor is already listed");

        self.storage.whitelist.at(investor).write(true);
    }

    #[external("public")]
    fn ban_from_whitelist(investor: AztecAddress) {
        assert_eq(
            self.msg_sender().unwrap(),
            self.storage.owner.read(),
            "Only owner can manage whitelist",
        );
        assert(self.storage.whitelist.at(investor).read(), "Investor is already unlisted");

        self.storage.whitelist.at(investor).write(false);
    }

    #[external("private")]
    fn distribute_private(investor: AztecAddress, amount: u64) {
        // Only issuer can distribute
        self.enqueue_self._assert_is_owner(self.msg_sender().unwrap());

        // Check investor is whitelisted
        self.enqueue_self._assert_is_whitelisted(investor);

        // Transfer from issuer's private balance to investor
        let issuer = self.msg_sender().unwrap();
        self.storage.private_balances.at(issuer).sub(amount as u128).deliver(
            MessageDelivery.CONSTRAINED_ONCHAIN,
        );
        self.storage.private_balances.at(investor).add(amount as u128).deliver(
            MessageDelivery.CONSTRAINED_ONCHAIN,
        );
    }

    #[external("private")]
    fn transfer_private(to: AztecAddress, amount: u64) {
        let sender = self.msg_sender().unwrap();

        // Spend sender's notes
        self.storage.private_balances.at(sender).sub(amount as u128).deliver(
            MessageDelivery.CONSTRAINED_ONCHAIN,
        );

        // Check recipient is whitelisted (reveals recipient address)
        self.enqueue_self._assert_is_whitelisted(to);

        // Create new notes for recipient
        self.storage.private_balances.at(to).add(amount as u128).deliver(
            MessageDelivery.CONSTRAINED_ONCHAIN,
        );
    }

    #[external("private")]
    fn redeem_private(amount: u64) {
        let sender = self.msg_sender().unwrap();

        // Check maturity reached
        self.enqueue_self._assert_maturity_reached();

        // Transfer from investor back to issuer
        self.storage.private_balances.at(sender).sub(amount as u128).deliver(
            MessageDelivery.CONSTRAINED_ONCHAIN,
        );

        // Credit issuer's balance (need to read owner from public state)
        self.enqueue_self._credit_issuer_private(amount);
    }

    #[external("public")]
    #[only_self]
    fn _credit_issuer_private(amount: u64) {
        // This is a workaround - we need to credit issuer but can't read owner in private
        // In practice, this would need a different pattern (e.g., pass owner as param and verify)
        // For PoC, we'll handle this via public credit to issuer
        let issuer = self.storage.owner.read();
        let balance = self.storage.public_balances.at(issuer).read();
        self.storage.public_balances.at(issuer).write(balance + amount);
    }

    #[external("public")]
    fn transfer_public(to: AztecAddress, amount: u64) {
        let sender = self.msg_sender().unwrap();
        let sender_balance = self.storage.public_balances.at(sender).read();
        assert(sender_balance >= amount, "Insufficient Bond tokens");

        // Deduct from sender
        self.storage.public_balances.at(sender).write(sender_balance - amount);

        // Check recipient is whitelisted
        assert(self.storage.whitelist.at(to).read(), "Recipient is not whitelisted");

        // Add to recipient
        let recipient_balance = self.storage.public_balances.at(to).read();
        self.storage.public_balances.at(to).write(recipient_balance + amount);
    }

    #[external("private")]
    fn public_to_private(amount: u64) {
        let sender = self.msg_sender().unwrap();

        // Deduct from public balance
        self.enqueue_self._deduct_public_balance(sender, amount);

        // Add to private balance
        self.storage.private_balances.at(sender).add(amount as u128).deliver(
            MessageDelivery.CONSTRAINED_ONCHAIN,
        );
    }

    #[external("private")]
    fn private_to_public(amount: u64) {
        let sender = self.msg_sender().unwrap();

        // Remove from private balance
        self.storage.private_balances.at(sender).sub(amount as u128).deliver(
            MessageDelivery.CONSTRAINED_ONCHAIN,
        );

        // Credit public balance
        self.enqueue_self._credit_public_balance(sender, amount);
    }

    #[external("public")]
    fn transfer_ownership(new_owner: AztecAddress) {
        assert_eq(
            self.msg_sender().unwrap(),
            self.storage.owner.read(),
            "Only current owner can transfer ownership",
        );
        self.storage.owner.write(new_owner);
    }

    #[external("utility")]
    unconstrained fn private_balance_of(owner: AztecAddress) -> pub u128 {
        self.storage.private_balances.at(owner).balance_of()
    }

    #[external("utility")]
    unconstrained fn public_balance_of(owner: AztecAddress) -> pub u64 {
        self.storage.public_balances.at(owner).read()
    }

    #[external("utility")]
    unconstrained fn get_total_supply() -> pub u64 {
        self.storage.total_supply.read()
    }

    #[external("utility")]
    unconstrained fn get_maturity_date() -> pub u64 {
        self.storage.maturity_date.read()
    }

    #[external("utility")]
    unconstrained fn is_whitelisted(address: AztecAddress) -> pub bool {
        self.storage.whitelist.at(address).read()
    }

    #[external("public")]
    #[only_self]
    fn _assert_is_owner(address: AztecAddress) {
        assert_eq(address, self.storage.owner.read(), "Only the Issuer can perform this action");
    }

    #[external("public")]
    #[only_self]
    fn _assert_is_whitelisted(address: AztecAddress) {
        assert(self.storage.whitelist.at(address).read(), "Address is not whitelisted");
    }

    #[external("public")]
    #[only_self]
    fn _assert_maturity_reached() {
        let block_timestamp = self.context.timestamp();
        let maturity_date = self.storage.maturity_date.read();
        assert(block_timestamp >= maturity_date, "Bond has not reached maturity yet");
    }

    #[external("public")]
    #[only_self]
    fn _deduct_public_balance(owner: AztecAddress, amount: u64) {
        let balance = self.storage.public_balances.at(owner).read();
        assert(balance >= amount, "Insufficient public bond tokens");
        self.storage.public_balances.at(owner).write(balance - amount);
    }

    #[external("public")]
    #[only_self]
    fn _credit_public_balance(owner: AztecAddress, amount: u64) {
        let balance = self.storage.public_balances.at(owner).read();
        self.storage.public_balances.at(owner).write(balance + amount);
    }
}
