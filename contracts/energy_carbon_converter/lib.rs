#![cfg_attr(not(feature = "std"), no_std, no_main)]
#![allow(clippy::cast_possible_truncation)]

use ink::env::call::{build_call, ExecutionInput};
use ink::storage::Mapping;
use energy_types::EnergySource;

#[ink::contract]
mod energy_carbon_converter {
    use super::*;

    // -------------------------------------------------
    // Constantes
    // -------------------------------------------------

    /// Escala para aritmética de ponto fixo
    /// fator_real = fator_onchain / SCALE
    const SCALE: u128 = 1_000_000;

    // -------------------------------------------------
    // Tipos (DEVE bater com o EnergyToken)
    // -------------------------------------------------



    #[derive(scale::Encode, scale::Decode, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum ConvertError {
        InvalidAmount,
        Replay,
        Overflow,
        MissingFactor,
        SourceQueryFailed,
        BurnFailed,
        MintFailed,
    }

    // -------------------------------------------------
    // Eventos
    // -------------------------------------------------

    #[ink(event)]
    pub struct Converted {
        #[ink(topic)]
        user: AccountId,
        source: EnergySource,
        kwh_burned: u128,
        carbon_minted: u128,
        factor_used: u128,
    }

    // -------------------------------------------------
    // Storage
    // -------------------------------------------------

    #[ink(storage)]
    pub struct EnergyCarbonConverter {
        energy_token: AccountId,
        carbon_token: AccountId,
        /// fatores padrão por fonte (EnergySource as u8)
        default_factors: Mapping<u8, u128>,
        used_nonces: Mapping<(AccountId, u64), bool>,
    }

    // -------------------------------------------------
    // Implementação
    // -------------------------------------------------

    impl EnergyCarbonConverter {
        /// Construtor
        /// Inicializa fatores padrão por fonte (escala 10^6)
        #[ink(constructor)]
        pub fn new(
            energy_token: AccountId,
            carbon_token: AccountId,
        ) -> Self {
            let mut default_factors = Mapping::default();

            // Fatores exemplificativos (kgCO₂/kWh × 10^6)
            default_factors.insert(EnergySource::Solar as u8, &45_000);
            default_factors.insert(EnergySource::Wind as u8, &12_000);
            default_factors.insert(EnergySource::Hydro as u8, &84_000);
            default_factors.insert(EnergySource::Biomass as u8, &230_000);

            Self {
                energy_token,
                carbon_token,
                default_factors,
                used_nonces: Mapping::default(),
            }
        }

        /// Conversão de kWh → créditos de carbono
        ///
        /// A fonte de energia é consultada diretamente
        /// no contrato EnergyToken.
        #[ink(message)]
        pub fn convert(
            &mut self,
            kwh_amount: u128,
            nonce: u64,
        ) -> Result<(), ConvertError> {
            let caller = self.env().caller();

            // -----------------------------
            // Checks
            // -----------------------------

            if kwh_amount == 0 {
                return Err(ConvertError::InvalidAmount);
            }

            if self.used_nonces.get((caller, nonce)).unwrap_or(false) {
                return Err(ConvertError::Replay);
            }

            // -----------------------------
            // Consulta da fonte no EnergyToken
            // -----------------------------

            let source: EnergySource = build_call::<ink::env::DefaultEnvironment>()
                .call(self.energy_token)
                .exec_input(
                    ExecutionInput::new(
                        ink::selector_bytes!("source").into()
                    )
                )
                .returns::<EnergySource>()
                .try_invoke()
                .map_err(|_| ConvertError::SourceQueryFailed)?
                .unwrap();


            let factor = self
                .default_factors
                .get(source as u8)
                .ok_or(ConvertError::MissingFactor)?;

            // -----------------------------
            // Burn no EnergyToken
            // -----------------------------

            let burn_result = build_call::<ink::env::DefaultEnvironment>()
                .call(self.energy_token)
                .exec_input(
                    ExecutionInput::new(
                        ink::selector_bytes!("burn_kwh").into()
                    )
                    .push_arg(kwh_amount),
                )
                .returns::<Result<(), ()>>()
                .invoke();

            if burn_result.is_err() {
                return Err(ConvertError::BurnFailed);
            }

            // -----------------------------
            // Cálculo fracionário seguro
            // carbon = (kwh * factor) / SCALE
            // -----------------------------

            let carbon_amount = kwh_amount
                .checked_mul(factor)
                .ok_or(ConvertError::Overflow)?
                .checked_div(SCALE)
                .ok_or(ConvertError::Overflow)?;

            // -----------------------------
            // Mint no CarbonCreditToken
            // -----------------------------

            let mint_result = build_call::<ink::env::DefaultEnvironment>()
                .call(self.carbon_token)
                .exec_input(
                    ExecutionInput::new(
                        ink::selector_bytes!("mint_credit").into()
                    )
                    .push_arg(caller)
                    .push_arg(carbon_amount)
                    .push_arg(1u64), // project_id (exemplo)
                )
                .returns::<Result<(), ()>>()
                .invoke();

            if mint_result.is_err() {
                return Err(ConvertError::MintFailed);
            }

            // -----------------------------
            // Effects
            // -----------------------------

            self.used_nonces.insert((caller, nonce), &true);

            self.env().emit_event(Converted {
                user: caller,
                source,
                kwh_burned: kwh_amount,
                carbon_minted: carbon_amount,
                factor_used: factor,
            });

            Ok(())
        }

        // -------------------------------------------------
        // Getters
        // -------------------------------------------------

        /// Retorna o fator padrão (escalado) para uma fonte
        #[ink(message)]
        pub fn default_factor(&self, source: EnergySource) -> u128 {
            self.default_factors
                .get(source as u8)
                .unwrap_or(0)
        }

        /// Retorna os contratos integrados
        #[ink(message)]
        pub fn contracts(&self) -> (AccountId, AccountId) {
            (self.energy_token, self.carbon_token)
        }

        /// Retorna a escala usada no ponto fixo
        #[ink(message)]
        pub fn scale(&self) -> u128 {
            SCALE
        }
    }

    #[cfg(test)]
    mod tests {
        //! -------------------------------------------------
        //! Testes Unitários — EnergyCarbonConverter
        //! -------------------------------------------------
        //!
        //! Estes testes validam exclusivamente a lógica interna
        //! do contrato conversor, conforme as limitações do
        //! ambiente de testes unitários do ink!.
        //!
        //! Importante:
        //! - Chamadas cross-contract (EnergyToken, CarbonCreditToken)
        //!   NÃO são executáveis em testes unitários.
        //! - Portanto, testes que envolvem `build_call`,
        //!   `try_invoke`, `burn_kwh` ou `mint_credit`
        //!   são avaliados apenas em testes de integração (e2e).
        //!
        //! Os testes abaixo cobrem:
        //! - Validação de parâmetros de entrada
        //! - Proteção contra replay (uso de nonce)
        //! - Consistência da configuração inicial
        //! - Correção dos getters
        //!
        //! Isso garante que a lógica local do contrato esteja correta
        //! antes da validação em ambiente de execução real.

        use super::*;
        use ink::env::test;

        /// Retorna um conjunto padrão de contas para testes
        fn accounts() -> test::DefaultAccounts<ink::env::DefaultEnvironment> {
            test::default_accounts::<ink::env::DefaultEnvironment>()
        }

        /// Testa se a conversão falha quando a quantidade de energia é zero.
        ///
        /// Objetivo:
        /// - Garantir validação de parâmetros de entrada
        /// - Evitar conversões inválidas ou sem efeito
        #[ink::test]
        fn zero_amount_fails() {
            let acc = accounts();
            let mut converter = EnergyCarbonConverter::new(
                acc.alice,
                acc.bob,
            );

            assert_eq!(
                converter.convert(0, 0),
                Err(ConvertError::InvalidAmount)
            );
        }

        /// Testa o mecanismo de proteção contra replay.
        ///
        /// Objetivo:
        /// - Garantir que um mesmo `nonce` não possa ser reutilizado
        /// - Evitar múltiplas conversões com os mesmos parâmetros
        /// - Proteger o contrato contra reexecuções indevidas
        #[ink::test]
        fn replay_is_blocked() {
            let acc = accounts();
            let mut converter = EnergyCarbonConverter::new(
                acc.alice,
                acc.bob,
            );

            // Define o chamador do teste
            test::set_caller::<ink::env::DefaultEnvironment>(acc.charlie);

            // Marca o nonce como já utilizado
            converter.used_nonces.insert((acc.charlie, 1), &true);

            assert_eq!(
                converter.convert(10, 1),
                Err(ConvertError::Replay)
            );
        }

        /// Testa se todos os fatores padrão por fonte de energia
        /// estão corretamente inicializados.
        ///
        /// Objetivo:
        /// - Garantir que nenhuma fonte esteja sem fator associado
        /// - Evitar falhas de conversão por configuração incompleta
        #[ink::test]
        fn default_factor_exists_for_all_sources() {
            let acc = accounts();
            let converter = EnergyCarbonConverter::new(
                acc.alice,
                acc.bob,
            );

            assert!(converter.default_factor(EnergySource::Solar) > 0);
            assert!(converter.default_factor(EnergySource::Wind) > 0);
            assert!(converter.default_factor(EnergySource::Hydro) > 0);
            assert!(converter.default_factor(EnergySource::Biomass) > 0);
        }

        /// Testa se a constante de escala utilizada na aritmética
        /// de ponto fixo está correta.
        ///
        /// Objetivo:
        /// - Garantir precisão determinística na conversão
        /// - Evitar divergência entre contratos e aplicações off-chain
        #[ink::test]
        fn scale_is_correct() {
            let acc = accounts();
            let converter = EnergyCarbonConverter::new(
                acc.alice,
                acc.bob,
            );

            assert_eq!(converter.scale(), 1_000_000);
        }

        /// Testa se os endereços dos contratos integrados
        /// são corretamente armazenados e retornados.
        ///
        /// Objetivo:
        /// - Garantir que o conversor está ligado aos contratos corretos
        /// - Facilitar auditoria e validação off-chain
        #[ink::test]
        fn contracts_are_returned_correctly() {
            let acc = accounts();
            let converter = EnergyCarbonConverter::new(
                acc.alice,
                acc.bob,
            );

            let (energy, carbon) = converter.contracts();
            assert_eq!(energy, acc.alice);
            assert_eq!(carbon, acc.bob);
        }
    }


}
