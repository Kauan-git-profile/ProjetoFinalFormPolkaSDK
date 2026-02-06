#![cfg_attr(not(feature = "std"), no_std, no_main)]
#![allow(clippy::cast_possible_truncation)]

use ink::storage::Mapping;
use energy_types::EnergySource;

#[ink::contract]
mod energy_token {
    use super::*;

    // -------------------------------------------------
    // Tipos
    // -------------------------------------------------

    #[derive(scale::Encode, scale::Decode, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum EnergyError {
        Unauthorized,
        InvalidAmount,
        InsufficientBalance,
        Overflow,
    }

    // -------------------------------------------------
    // Eventos
    // -------------------------------------------------

    #[ink(event)]
    pub struct Minted {
        #[ink(topic)]
        to: AccountId,
        amount: u128,
        source: EnergySource,
    }

    #[ink(event)]
    pub struct Transferred {
        #[ink(topic)]
        from: AccountId,
        #[ink(topic)]
        to: AccountId,
        amount: u128,
    }

    #[ink(event)]
    pub struct Burned {
        #[ink(topic)]
        from: AccountId,
        amount: u128,
    }

    // -------------------------------------------------
    // Storage
    // -------------------------------------------------

    #[ink(storage)]
    pub struct EnergyToken {
        /// Conta autorizada a emitir energia (oracle)
        oracle: AccountId,
        /// Contrato autorizado a queimar energia por delegação
        converter: AccountId,
        /// Fonte da energia (Solar, Wind, etc.)
        source: EnergySource,
        /// Saldo de energia por conta (kWh)
        balances: Mapping<AccountId, u128>,
        /// Total de energia emitida (kWh)
        total_supply: u128,
    }

    // -------------------------------------------------
    // Implementação
    // -------------------------------------------------

    impl EnergyToken {
        /// Construtor
        ///
        /// Cada instância do contrato representa
        /// UMA fonte específica de energia.
        #[ink(constructor)]
        pub fn new(
            oracle: AccountId,
            converter: AccountId,
            source: EnergySource,
        ) -> Self {
            Self {
                oracle,
                converter,
                source,
                balances: Mapping::default(),
                total_supply: 0,
            }
        }

        /// Emissão de energia (kWh) — somente oracle
        #[ink(message)]
        pub fn mint_kwh(
            &mut self,
            to: AccountId,
            amount: u128,
        ) -> Result<(), EnergyError> {
            if self.env().caller() != self.oracle {
                return Err(EnergyError::Unauthorized);
            }
            if amount == 0 {
                return Err(EnergyError::InvalidAmount);
            }

            let balance = self.balance_of(to);

            let new_balance = balance
                .checked_add(amount)
                .ok_or(EnergyError::Overflow)?;

            let new_supply = self
                .total_supply
                .checked_add(amount)
                .ok_or(EnergyError::Overflow)?;

            self.balances.insert(to, &new_balance);
            self.total_supply = new_supply;

            self.env().emit_event(Minted {
                to,
                amount,
                source: self.source,
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

            self.env().emit_event(Transferred {
                from,
                to,
                amount,
            });

            Ok(())
        }

        /// Queima de energia (kWh)
        ///
        /// Usada pelo conversor para conversão em carbono
        #[ink(message)]
        pub fn burn_kwh(
            &mut self,
            amount: u128,
        ) -> Result<(), EnergyError> {
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

        // burn por delegação, para permitir ao EnergyCarbonConverter::convert executar a queima para outro usuario
        #[ink(message)]
        pub fn burn_from(
            &mut self,
            owner: AccountId,
            amount: u128,
        ) -> Result<(), EnergyError> {
            let caller = self.env().caller();

            // Apenas o conversor pode queimar por delegação
            if caller != self.converter {
                return Err(EnergyError::Unauthorized);
            }

            let balance = self.balances.get(owner).unwrap_or(0);
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

            self.balances.insert(owner, &new_balance);
            self.total_supply = new_supply;

            self.env().emit_event(Burned {
                from: caller,
                amount,
            });

            Ok(())
        }

        #[ink(message)]
        pub fn set_converter(&mut self, converter: AccountId) -> Result<(), EnergyError> {
            if self.env().caller() != self.oracle {
                return Err(EnergyError::Unauthorized);
            }
            self.converter = converter;
            Ok(())
        }


        // -------------------------------------------------
        // Getters
        // -------------------------------------------------

        // Retorna o saldo de kWh de uma conta
        #[ink(message)]
        pub fn balance_of(&self, owner: AccountId) -> u128 {
            self.balances.get(owner).unwrap_or(0)
        }

        // Retorna o total de kWh emitidos
        #[ink(message)]
        pub fn total_supply(&self) -> u128 {
            self.total_supply
        }

        /// Retorna o oracle autorizado a emitir energia
        #[ink(message)]
        pub fn oracle(&self) -> AccountId {
            self.oracle
        }

        // conversor autorizad: EnergyCarbonConverter
        #[ink(message)]
        pub fn converter(&self) -> AccountId {
            self.converter
        }

        // Retorna a fonte de energia deste token
        #[ink(message)]
        pub fn source(&self) -> EnergySource {
            self.source
        }
    }

    #[cfg(test)]
    mod tests {
        //! -------------------------------------------------
        //! Testes Unitários — EnergyToken
        //! -------------------------------------------------
        //!
        //! Estes testes validam a lógica interna do contrato
        //! de tokenização de energia elétrica (kWh).
        //!
        //! Escopo dos testes unitários:
        //! - Regras de autorização (oracle)
        //! - Validação de parâmetros de entrada
        //! - Atualização correta de saldos
        //! - Atualização do total de energia emitida
        //! - Persistência e exposição da fonte de energia
        //!
        //! Fora do escopo dos testes unitários:
        //! - Conversão de energia em créditos de carbono
        //! - Chamadas cross-contract
        //! - Avaliação de custos ou desempenho
        //!
        //! Esses aspectos são tratados em testes de integração
        //! (end-to-end) executados no `substrate-contracts-node`.

        use super::*;
        use ink::env::test;

        /// Retorna um conjunto padrão de contas para os testes
        fn accounts() -> test::DefaultAccounts<ink::env::DefaultEnvironment> {
            test::default_accounts::<ink::env::DefaultEnvironment>()
        }

        /// Testa se o construtor inicializa corretamente
        /// o oracle e a fonte de energia.
        ///
        /// Objetivo:
        /// - Garantir que cada contrato representa
        ///   uma única fonte de energia
        /// - Validar a configuração inicial do estado
        #[ink::test]
        fn constructor_sets_oracle_and_source() {
            let acc = accounts();

            let token = EnergyToken::new(
                acc.alice,
                EnergySource::Solar,
            );

            assert_eq!(token.oracle(), acc.alice);
            assert_eq!(token.source(), EnergySource::Solar);
            assert_eq!(token.total_supply(), 0);
        }

        /// Testa se a emissão de energia (mint) funciona
        /// quando chamada pelo oracle autorizado.
        ///
        /// Objetivo:
        /// - Validar controle de acesso
        /// - Garantir atualização correta de saldo e supply
        #[ink::test]
        fn mint_by_oracle_works() {
            let acc = accounts();
            let mut token = EnergyToken::new(
                acc.alice,
                EnergySource::Wind,
            );

            assert_eq!(
                token.mint_kwh(acc.bob, 100),
                Ok(())
            );

            assert_eq!(token.balance_of(acc.bob), 100);
            assert_eq!(token.total_supply(), 100);
        }

        /// Testa se a emissão de energia falha
        /// quando chamada por uma conta não autorizada.
        ///
        /// Objetivo:
        /// - Garantir que apenas o oracle pode emitir energia
        #[ink::test]
        fn mint_by_non_oracle_fails() {
            let acc = accounts();
            let mut token = EnergyToken::new(
                acc.alice,
                EnergySource::Hydro,
            );

            // Define o chamador como alguém diferente do oracle
            test::set_caller::<ink::env::DefaultEnvironment>(acc.bob);

            assert_eq!(
                token.mint_kwh(acc.bob, 50),
                Err(EnergyError::Unauthorized)
            );
        }

        /// Testa se a transferência de energia entre contas
        /// atualiza corretamente os saldos.
        ///
        /// Objetivo:
        /// - Validar movimentação de kWh
        /// - Garantir preservação do total_supply
        #[ink::test]
        fn transfer_kwh_works() {
            let acc = accounts();
            let mut token = EnergyToken::new(
                acc.alice,
                EnergySource::Solar,
            );

            token.mint_kwh(acc.alice, 80).unwrap();

            assert_eq!(
                token.transfer_kwh(acc.bob, 30),
                Ok(())
            );

            assert_eq!(token.balance_of(acc.alice), 50);
            assert_eq!(token.balance_of(acc.bob), 30);
            assert_eq!(token.total_supply(), 80);
        }

        /// Testa se a transferência falha quando
        /// o saldo é insuficiente.
        ///
        /// Objetivo:
        /// - Evitar saldos negativos
        /// - Garantir consistência do estado
        #[ink::test]
        fn transfer_fails_with_insufficient_balance() {
            let acc = accounts();
            let mut token = EnergyToken::new(
                acc.alice,
                EnergySource::Biomass,
            );

            assert_eq!(
                token.transfer_kwh(acc.bob, 10),
                Err(EnergyError::InsufficientBalance)
            );
        }

        /// Testa se a queima de energia (burn)
        /// reduz corretamente o saldo e o total emitido.
        ///
        /// Objetivo:
        /// - Garantir que kWh convertidos sejam removidos
        /// - Preparar corretamente o fluxo de conversão
        #[ink::test]
        fn burn_kwh_works() {
            let acc = accounts();
            let mut token = EnergyToken::new(
                acc.alice,
                EnergySource::Solar,
            );

            token.mint_kwh(acc.alice, 60).unwrap();

            assert_eq!(
                token.burn_kwh(25),
                Ok(())
            );

            assert_eq!(token.balance_of(acc.alice), 35);
            assert_eq!(token.total_supply(), 35);
        }

        /// Testa se a queima falha quando
        /// o valor solicitado é inválido.
        ///
        /// Objetivo:
        /// - Validar parâmetros de entrada
        #[ink::test]
        fn burn_fails_with_zero_amount() {
            let acc = accounts();
            let mut token = EnergyToken::new(
                acc.alice,
                EnergySource::Solar,
            );

            assert_eq!(
                token.burn_kwh(0),
                Err(EnergyError::InvalidAmount)
            );
        }

        /// Testa os getters do contrato.
        ///
        /// Objetivo:
        /// - Garantir que funções de leitura
        ///   retornem valores consistentes
        /// - Facilitar auditoria e integração off-chain
        #[ink::test]
        fn getters_work_correctly() {
            let acc = accounts();
            let token = EnergyToken::new(
                acc.alice,
                EnergySource::Hydro,
            );

            assert_eq!(token.oracle(), acc.alice);
            assert_eq!(token.source(), EnergySource::Hydro);
            assert_eq!(token.balance_of(acc.bob), 0);
            assert_eq!(token.total_supply(), 0);
        }
    }

}
