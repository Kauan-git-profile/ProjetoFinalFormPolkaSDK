#![cfg_attr(not(feature = "std"), no_std)]

use ink::storage::Mapping;

#[ink::contract]
mod carbon_credit_token {

    use super::*;

    /// Erros do contrato
    #[derive(scale::Encode, scale::Decode, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum CarbonError {
        Unauthorized,
        InsufficientBalance,
        InvalidAmount,
        AlreadyRetired,
    }

    /// Evento de emissão
    #[ink(event)]
    pub struct CreditMinted {
        #[ink(topic)]
        to: AccountId,
        amount: u128,
        project_id: u64,
    }

    /// Evento de transferência
    #[ink(event)]
    pub struct CreditTransferred {
        #[ink(topic)]
        from: AccountId,
        #[ink(topic)]
        to: AccountId,
        amount: u128,
    }

    /// Evento de aposentadoria (retirement)
    #[ink(event)]
    pub struct CreditRetired {
        #[ink(topic)]
        from: AccountId,
        amount: u128,
        reason_hash: Hash,
    }

    #[ink(storage)]
    pub struct CarbonCreditToken {
        balances: Mapping<AccountId, u128>,
        total_supply: u128,
        retired_supply: u128,
        authority: AccountId,
    }

    impl CarbonCreditToken {

        /// Construtor
        #[ink(constructor)]
        pub fn new(authority: AccountId) -> Self {
            Self {
                balances: Mapping::default(),
                total_supply: 0,
                retired_supply: 0,
                authority,
            }
        }

        /// Emissão de créditos de carbono (somente autoridade)
        #[ink(message)]
        pub fn mint_credit(
            &mut self,
            to: AccountId,
            amount: u128,
            project_id: u64,
        ) -> Result<(), CarbonError> {

            let caller = self.env().caller();
            if caller != self.authority {
                return Err(CarbonError::Unauthorized);
            }

            if amount == 0 {
                return Err(CarbonError::InvalidAmount);
            }

            let balance = self.balance_of(to);
            self.balances.insert(to, &(balance + amount));
            self.total_supply += amount;

            self.env().emit_event(CreditMinted {
                to,
                amount,
                project_id,
            });

            Ok(())
        }

        /// Transferência de créditos
        #[ink(message)]
        pub fn transfer_credit(
            &mut self,
            to: AccountId,
            amount: u128,
        ) -> Result<(), CarbonError> {

            let from = self.env().caller();
            let from_balance = self.balance_of(from);

            if amount == 0 || from_balance < amount {
                return Err(CarbonError::InsufficientBalance);
            }

            self.balances.insert(from, &(from_balance - amount));
            let to_balance = self.balance_of(to);
            self.balances.insert(to, &(to_balance + amount));

            self.env().emit_event(CreditTransferred {
                from,
                to,
                amount,
            });

            Ok(())
        }

        /// Aposentadoria de créditos (retirement)
        #[ink(message)]
        pub fn retire_credit(
            &mut self,
            amount: u128,
            reason_hash: Hash,
        ) -> Result<(), CarbonError> {

            let caller = self.env().caller();
            let balance = self.balance_of(caller);

            if amount == 0 || balance < amount {
                return Err(CarbonError::InsufficientBalance);
            }

            self.balances.insert(caller, &(balance - amount));
            self.retired_supply += amount;

            self.env().emit_event(CreditRetired {
                from: caller,
                amount,
                reason_hash,
            });

            Ok(())
        }

        /// Consulta saldo
        #[ink(message)]
        pub fn balance_of(&self, owner: AccountId) -> u128 {
            self.balances.get(owner).unwrap_or(0)
        }

        /// Supply total emitido
        #[ink(message)]
        pub fn total_supply(&self) -> u128 {
            self.total_supply
        }

        /// Supply aposentado
        #[ink(message)]
        pub fn retired_supply(&self) -> u128 {
            self.retired_supply
        }

        /// Autoridade emissora
        #[ink(message)]
        pub fn authority(&self) -> AccountId {
            self.authority
        }
    }

    // -------------------------
    // Testes unitários
    // -------------------------
    #[cfg(test)]
    mod tests {
        use super::*;
        use ink::env::test;

        #[ink::test]
        fn mint_works() {
            let accounts = test::default_accounts::<ink::env::DefaultEnvironment>();
            let mut token = CarbonCreditToken::new(accounts.alice);

            assert_eq!(
                token.mint_credit(accounts.bob, 100, 1),
                Ok(())
            );
            assert_eq!(token.balance_of(accounts.bob), 100);
        }

        #[ink::test]
        fn transfer_works() {
            let accounts = test::default_accounts::<ink::env::DefaultEnvironment>();
            let mut token = CarbonCreditToken::new(accounts.alice);

            token.mint_credit(accounts.alice, 50, 1).unwrap();
            assert_eq!(token.transfer_credit(accounts.bob, 20), Ok(()));
            assert_eq!(token.balance_of(accounts.bob), 20);
        }

        #[ink::test]
        fn retire_works() {
            let accounts = test::default_accounts::<ink::env::DefaultEnvironment>();
            let mut token = CarbonCreditToken::new(accounts.alice);

            token.mint_credit(accounts.alice, 30, 1).unwrap();
            let reason = Hash::from([0x01; 32]);
            assert_eq!(token.retire_credit(10, reason), Ok(()));
            assert_eq!(token.balance_of(accounts.alice), 20);
            assert_eq!(token.retired_supply(), 10);
        }
    }
}
