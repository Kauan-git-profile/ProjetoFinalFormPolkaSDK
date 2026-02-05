#![cfg_attr(not(feature = "std"), no_std,  no_main)]
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

        // Quantidade de créditos de carbono disponíveis (não aposentados) pertencentes a uma conta
        #[ink(message)]
        pub fn balance_of(&self, owner: AccountId) -> u128 {
            self.balances.get(owner).unwrap_or(0)
        }

        // Total de créditos de carbono emitidos e ainda ativos no sistema
        #[ink(message)]
        pub fn total_supply(&self) -> u128 {
            self.total_supply
        }

        // Quantidade total de créditos aposentados (retired)
        #[ink(message)]
        pub fn retired_supply(&self) -> u128 {
            self.retired_supply
        }

        // Endereço da autoridade emissora de créditos de carbono.
        #[ink(message)]
        pub fn authority(&self) -> AccountId {
            self.authority
        }
    }


    #[cfg(test)]
    mod tests {
        //! -------------------------------------------------
        //! Testes Unitários — CarbonCreditToken
        //! -------------------------------------------------
        //!
        //! Estes testes validam a lógica interna do contrato
        //! de créditos de carbono.
        //!
        //! Escopo dos testes unitários:
        //! - Controle de acesso para emissão (authority)
        //! - Emissão correta de créditos de carbono
        //! - Transferência entre contas
        //! - Aposentadoria (retirement) de créditos
        //! - Atualização correta dos contadores globais
        //! - Funcionamento dos getters
        //!
        //! Fora do escopo dos testes unitários:
        //! - Conversão de energia em créditos (EnergyCarbonConverter)
        //! - Integração com oráculos externos
        //! - Avaliação econômica ou regulatória
        //!
        //! Esses aspectos são tratados em testes de integração
        //! (end-to-end), executados em ambiente de execução real.

        use super::*;
        use ink::env::test;

        /// Retorna um conjunto padrão de contas para os testes
        fn accounts() -> test::DefaultAccounts<ink::env::DefaultEnvironment> {
            test::default_accounts::<ink::env::DefaultEnvironment>()
        }

        /// Testa se o construtor inicializa corretamente
        /// a autoridade emissora e os contadores globais.
        ///
        /// Objetivo:
        /// - Garantir configuração inicial correta
        /// - Validar governança básica do contrato
        #[ink::test]
        fn constructor_sets_authority() {
            let acc = accounts();
            let token = CarbonCreditToken::new(acc.alice);

            assert_eq!(token.authority(), acc.alice);
            assert_eq!(token.total_supply(), 0);
            assert_eq!(token.retired_supply(), 0);
        }

        /// Testa se a emissão de créditos funciona
        /// quando realizada pela autoridade autorizada.
        ///
        /// Objetivo:
        /// - Validar controle de acesso
        /// - Garantir atualização correta de saldo e supply
        #[ink::test]
        fn mint_credit_by_authority_works() {
            let acc = accounts();
            let mut token = CarbonCreditToken::new(acc.alice);

            assert_eq!(
                token.mint_credit(acc.bob, 100, 1),
                Ok(())
            );

            assert_eq!(token.balance_of(acc.bob), 100);
            assert_eq!(token.total_supply(), 100);
            assert_eq!(token.retired_supply(), 0);
        }

        /// Testa se a emissão de créditos falha
        /// quando chamada por uma conta não autorizada.
        ///
        /// Objetivo:
        /// - Impedir emissão arbitrária de créditos
        #[ink::test]
        fn mint_credit_by_non_authority_fails() {
            let acc = accounts();
            let mut token = CarbonCreditToken::new(acc.alice);

            test::set_caller::<ink::env::DefaultEnvironment>(acc.bob);

            assert_eq!(
                token.mint_credit(acc.charlie, 50, 1),
                Err(CarbonError::Unauthorized)
            );
        }

        /// Testa se a transferência de créditos entre contas
        /// atualiza corretamente os saldos.
        ///
        /// Objetivo:
        /// - Permitir circulação de créditos
        /// - Garantir preservação do total_supply
        #[ink::test]
        fn transfer_credit_works() {
            let acc = accounts();
            let mut token = CarbonCreditToken::new(acc.alice);

            token.mint_credit(acc.alice, 80, 1).unwrap();

            assert_eq!(
                token.transfer_credit(acc.bob, 30),
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
            let mut token = CarbonCreditToken::new(acc.alice);

            assert_eq!(
                token.transfer_credit(acc.bob, 10),
                Err(CarbonError::InsufficientBalance)
            );
        }

        /// Testa a aposentadoria (retirement) de créditos.
        ///
        /// Objetivo:
        /// - Garantir que créditos aposentados
        ///   não possam mais circular
        /// - Atualizar corretamente o contador de aposentados
        #[ink::test]
        fn retire_credit_works() {
            let acc = accounts();
            let mut token = CarbonCreditToken::new(acc.alice);

            token.mint_credit(acc.alice, 60, 1).unwrap();
            let reason = Hash::from([0x01; 32]);

            assert_eq!(
                token.retire_credit(25, reason),
                Ok(())
            );

            assert_eq!(token.balance_of(acc.alice), 35);
            assert_eq!(token.total_supply(), 60);
            assert_eq!(token.retired_supply(), 25);
        }

        /// Testa se a aposentadoria falha quando
        /// o valor solicitado é inválido.
        ///
        /// Objetivo:
        /// - Validar parâmetros de entrada
        #[ink::test]
        fn retire_fails_with_zero_amount() {
            let acc = accounts();
            let mut token = CarbonCreditToken::new(acc.alice);

            let reason = Hash::from([0x00; 32]);

            assert_eq!(
                token.retire_credit(0, reason),
                Err(CarbonError::InvalidAmount)
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
            let token = CarbonCreditToken::new(acc.alice);

            assert_eq!(token.authority(), acc.alice);
            assert_eq!(token.balance_of(acc.bob), 0);
            assert_eq!(token.total_supply(), 0);
            assert_eq!(token.retired_supply(), 0);
        }
    }


}
