#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::cast_possible_truncation)]

use ink::storage::Mapping;

#[ink::contract]
mod carbon_credit_token {
    use super::*;

    // -------------------------------------------------
    // Tipos
    // -------------------------------------------------

    #[derive(scale::Encode, scale::Decode, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum CarbonError {
        Unauthorized,
        InsufficientBalance,
        InvalidAmount,
        Overflow,
    }

    // -------------------------------------------------
    // Eventos
    // -------------------------------------------------
    // Regras seguidas:
    // - Apenas AccountId como #[ink(topic)]
    // - Nenhum enum como topic
    // - Eventos pequenos e auditáveis

    #[ink(event)]
    pub struct CreditMinted {
        #[ink(topic)]
        to: AccountId,
        amount: u128,
        project_id: u64,
    }

    #[ink(event)]
    pub struct CreditTransferred {
        #[ink(topic)]
        from: AccountId,
        #[ink(topic)]
        to: AccountId,
        amount: u128,
    }

    #[ink(event)]
    pub struct CreditRetired {
        #[ink(topic)]
        from: AccountId,
        amount: u128,
        reason_hash: Hash,
    }

    // -------------------------------------------------
    // Storage
    // -------------------------------------------------

    #[ink(storage)]
    pub struct CarbonCreditToken {
        balances: Mapping<AccountId, u128>,
        total_supply: u128,
        retired_supply: u128,
        authority: AccountId,
    }

    // -------------------------------------------------
    // Implementação
    // -------------------------------------------------

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
            if self.env().caller() != self.authority {
                return Err(CarbonError::Unauthorized);
            }
            if amount == 0 {
                return Err(CarbonError::InvalidAmount);
            }

            let balance = self.balance_of(to);

            let new_balance = balance
                .checked_add(amount)
                .ok_or(CarbonError::Overflow)?;

            let new_supply = self
                .total_supply
                .checked_add(amount)
                .ok_or(CarbonError::Overflow)?;

            self.balances.insert(to, &new_balance);
            self.total_supply = new_supply;

            self.env().emit_event(CreditMinted {
                to,
                amount,
                project_id,
            });

            Ok(())
        }

        /// Transferência de créditos de carbono
        #[ink(message)]
        pub fn transfer_credit(
            &mut self,
            to: AccountId,
            amount: u128,
        ) -> Result<(), CarbonError> {
            let from = self.env().caller();

            if amount == 0 {
                return Err(CarbonError::InvalidAmount);
            }

            let from_balance = self.balance_of(from);
            if from_balance < amount {
                return Err(CarbonError::InsufficientBalance);
            }

            let to_balance = self.balance_of(to);

            let new_from = from_balance
                .checked_sub(amount)
                .ok_or(CarbonError::Overflow)?;

            let new_to = to_balance
                .checked_add(amount)
                .ok_or(CarbonError::Overflow)?;

            self.balances.insert(from, &new_from);
            self.balances.insert(to, &new_to);

            self.env().emit_event(CreditTransferred {
                from,
                to,
                amount,
            });

            Ok(())
        }

        /// Aposentadoria (retirement) de créditos
        #[ink(message)]
        pub fn retire_credit(
            &mut self,
            amount: u128,
            reason_hash: Hash,
        ) -> Result<(), CarbonError> {
            let caller = self.env().caller();

            if amount == 0 {
                return Err(CarbonError::InvalidAmount);
            }

            let balance = self.balance_of(caller);
            if balance < amount {
                return Err(CarbonError::InsufficientBalance);
            }

            let new_balance = balance
                .checked_sub(amount)
                .ok_or(CarbonError::Overflow)?;

            let new_retired = self
                .retired_supply
                .checked_add(amount)
                .ok_or(CarbonError::Overflow)?;

            self.balances.insert(caller, &new_balance);
            self.retired_supply = new_retired;

            self.env().emit_event(CreditRetired {
                from: caller,
                amount,
                reason_hash,
            });

            Ok(())
        }

        // -------------------------------------------------
        // Getters
        // -------------------------------------------------

        #[ink(message)]
        pub fn balance_of(&self, owner: AccountId) -> u128 {
            self.balances.get(owner).unwrap_or(0)
        }

        #[ink(message)]
        pub fn total_supply(&self) -> u128 {
            self.total_supply
        }

        #[ink(message)]
        pub fn retired_supply(&self) -> u128 {
            self.retired_supply
        }

        #[ink(message)]
        pub fn authority(&self) -> AccountId {
            self.authority
        }
    }
}
