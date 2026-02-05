#![cfg_attr(not(feature = "std"), no_std, no_main)]
#![allow(clippy::cast_possible_truncation)]

use ink::storage::Mapping;

#[ink::contract]
mod energy_token {

    use super::*;

    /// Fonte de energia
    #[derive(scale::Encode, scale::Decode, Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum EnergySource {
        Solar,
        Wind,
        Biomass,
        Hydro,
    }

    /// Erros do contrato
    #[derive(scale::Encode, scale::Decode, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum EnergyError {
        Unauthorized,
        InsufficientBalance,
        InvalidAmount,
        Overflow,
    }

    /// Evento de emissão
    #[ink(event)]
    pub struct Minted {
        #[ink(topic)]
        to: AccountId,
        amount: u128,
        source: EnergySource,
    }

    /// Evento de transferência
    #[ink(event)]
    pub struct Transferred {
        #[ink(topic)]
        from: AccountId,
        to: AccountId,
        amount: u128,
    }

    /// Evento de queima
    #[ink(event)]
    pub struct Burned {
        #[ink(topic)]
        from: AccountId,
        amount: u128,
    }

    #[ink(storage)]
    pub struct EnergyToken {
        balances: Mapping<AccountId, u128>,
        total_supply: u128,
        oracle: AccountId,
    }

    impl EnergyToken {

        /// Construtor
        #[ink(constructor)]
        pub fn new(oracle: AccountId) -> Self {
            Self {
                balances: Mapping::default(),
                total_supply: 0,
                oracle,
            }
        }

        /// Emissão de kWh (somente oráculo autorizado)
        #[ink(message)]
        pub fn mint_kwh(
            &mut self,
            to: AccountId,
            amount: u128,
            source: EnergySource,
        ) -> Result<(), EnergyError> {

            let caller = self.env().caller();
            if caller != self.oracle {
                return Err(EnergyError::Unauthorized);
            }

            if amount == 0 {
                return Err(EnergyError::InvalidAmount);
            }

            let balance = self.balance_of(to);
            let new_balance = balance
                .checked_add(amount)
                .ok_or(EnergyError::InvalidAmount)?;

            self.balances.insert(to, &new_balance);

            self.total_supply = self.total_supply
                .checked_add(amount)
                .ok_or(EnergyError::InvalidAmount)?;

            self.env().emit_event(Minted {
                to,
                amount,
                source,
            });

            Ok(())
        }

        /// Transferência de kWh
        #[ink(message)]
        pub fn transfer_kwh(
            &mut self,
            to: AccountId,
            amount: u128,
        ) -> Result<(), EnergyError> {
            let from = self.env().caller();
            if amount == 0 {
                return Err(EnergyError::InvalidAmount);
            }

            let from_balance = self.balance_of(from);
            if from_balance < amount {
                return Err(EnergyError::InsufficientBalance);
            }

            let to_balance = self.balance_of(to);

            let new_from = from_balance
                .checked_sub(amount)
                .ok_or(EnergyError::Overflow)?;

            let new_to = to_balance
                .checked_add(amount)
                .ok_or(EnergyError::Overflow)?;

            self.balances.insert(from, &new_from);
            self.balances.insert(to, &new_to);

            self.env().emit_event(Transferred { from, to, amount });

            Ok(())
        }

        /// Queima de kWh
        #[ink(message)]
        pub fn burn_kwh(&mut self, amount: u128) -> Result<(), EnergyError> {
            let caller = self.env().caller();
            if amount == 0 {
                return Err(EnergyError::InvalidAmount);
            }

            let balance = self.balance_of(caller);
            if balance < amount {
                return Err(EnergyError::InsufficientBalance);
            }

            let new_balance = balance
                .checked_sub(amount)
                .ok_or(EnergyError::Overflow)?;

            let new_supply = self
                .total_supply
                .checked_sub(amount)
                .ok_or(EnergyError::Overflow)?;

            self.balances.insert(caller, &new_balance);
            self.total_supply = new_supply;

            self.env().emit_event(Burned {
                from: caller,
                amount,
            });

            Ok(())
        }

        /// Consulta saldo
        #[ink(message)]
        pub fn balance_of(&self, owner: AccountId) -> u128 {
            self.balances.get(owner).unwrap_or(0)
        }

        /// Consulta supply total
        #[ink(message)]
        pub fn total_supply(&self) -> u128 {
            self.total_supply
        }

        /// Consulta oráculo
        #[ink(message)]
        pub fn oracle(&self) -> AccountId {
            self.oracle
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
            let mut token = EnergyToken::new(accounts.alice);

            assert_eq!(
                token.mint_kwh(accounts.bob, 100, EnergySource::Solar),
                Ok(())
            );

            assert_eq!(token.balance_of(accounts.bob), 100);
        }

        #[ink::test]
        fn transfer_works() {
            let accounts = test::default_accounts::<ink::env::DefaultEnvironment>();
            let mut token = EnergyToken::new(accounts.alice);

            token.mint_kwh(accounts.alice, 50, EnergySource::Wind).unwrap();
            assert_eq!(token.transfer_kwh(accounts.bob, 20), Ok(()));
            assert_eq!(token.balance_of(accounts.bob), 20);
        }

        #[ink::test]
        fn burn_works() {
            let accounts = test::default_accounts::<ink::env::DefaultEnvironment>();
            let mut token = EnergyToken::new(accounts.alice);

            token.mint_kwh(accounts.alice, 30, EnergySource::Hydro).unwrap();
            assert_eq!(token.burn_kwh(10), Ok(()));
            assert_eq!(token.balance_of(accounts.alice), 20);
        }
    }

}
