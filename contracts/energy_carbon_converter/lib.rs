#![cfg_attr(not(feature = "std"), no_std, no_main)]
#![allow(clippy::cast_possible_truncation)]

use ink::env::{
    call::{
        build_call,
        ExecutionInput,
        Selector,
    },
    DefaultEnvironment,
};
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
        CarbonTokenNotSet,
        CarbonTokenAlreadySet,
        Unauthorized,
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
        /// Endereço do contrato EnergyToken
        energy_token: AccountId,
        /// Endereço do contrato CarbonCreditToken (configurado depois)
        carbon_token: Option<AccountId>,
        /// Dono do conversor (governança mínima)
        owner: AccountId,
        /// Proteção contra replay (caller, nonce)
        used_nonces: Mapping<(AccountId, u64), bool>,
        /// Fatores de emissão por fonte (ponto fixo)
        factors: Mapping<EnergySource, u128>,
    }
    // -------------------------------------------------
    // Implementação
    // -------------------------------------------------

    impl EnergyCarbonConverter {
        /// Construtor: cria o conversor apenas com o EnergyToken
        #[ink(constructor)]
        pub fn new(energy_token: AccountId) -> Self {
            let caller = Self::env().caller();
            let mut factors = Mapping::default();

            // Fatores padrão (tCO2/kWh * SCALE)
            factors.insert(EnergySource::Solar, &600);    // 0,0006
            factors.insert(EnergySource::Wind, &500);     // 0,0005
            factors.insert(EnergySource::Hydro, &400);    // 0,0004
            factors.insert(EnergySource::Biomass, &800);  // 0,0008

            Self {
                energy_token,
                carbon_token: None,
                owner: caller,
                used_nonces: Mapping::default(),
                factors,
            }
        }

        /// Setter protegido para registrar o CarbonCreditToken
        #[ink(message)]
        pub fn set_carbon_token(&mut self, carbon_token: AccountId) -> Result<(), ConvertError> {
            if self.env().caller() != self.owner {
                return Err(ConvertError::Unauthorized);
            }

            if self.carbon_token.is_some() {
                return Err(ConvertError::CarbonTokenAlreadySet);
            }

            self.carbon_token = Some(carbon_token);
            Ok(())
        }

        /// Conversão de kWh para créditos de carbono
        ///
        /// A fonte de energia é consultada diretamente
        /// no contrato EnergyToken.
        /// Converte energia (kWh) em créditos de carbono
        #[ink(message)]
        pub fn convert(&mut self, kwh_amount: u128, nonce: u64) -> Result<(), ConvertError> {
            if kwh_amount == 0 {
                return Err(ConvertError::InvalidAmount);
            }

            let caller = self.env().caller();

            // Proteção contra replay
            if self.used_nonces.get((caller, nonce)).unwrap_or(false) {
                return Err(ConvertError::Replay);
            }

            let carbon_token = match self.carbon_token {
                Some(addr) => addr,
                None => return Err(ConvertError::CarbonTokenNotSet),
            };

            // 1 Consultar fonte da energia
            let source_call = build_call::<DefaultEnvironment>()
                .call(self.energy_token)
                .exec_input(
                    ExecutionInput::new(
                        Selector::new(ink::selector_bytes!("source")),
                    ),
                )
                .returns::<EnergySource>()
                .try_invoke();

            let source = match source_call {
                Ok(Ok(value)) => value,
                _ => return Err(ConvertError::SourceQueryFailed),
            };

            // 2 Obter fator de emissão
            let factor = self
                .factors
                .get(source)
                .ok_or(ConvertError::MissingFactor)?;

            // 3 Queimar energia
            let burn_call = build_call::<DefaultEnvironment>()
                .call(self.energy_token)
                .exec_input(
                    ExecutionInput::new(
                        Selector::new(ink::selector_bytes!("burn_from")),
                    )
                    .push_arg(caller)
                    .push_arg(kwh_amount),
                )
                .returns::<Result<(), ()>>()
                .try_invoke();

            match burn_call {
                Ok(Ok(Ok(()))) => {}
                _ => return Err(ConvertError::BurnFailed),
            }

            // 4 Calcular créditos de carbono (ponto fixo)
            let carbon_amount = kwh_amount
                .checked_mul(factor)
                .ok_or(ConvertError::MissingFactor)?
                / SCALE;

            if carbon_amount == 0 {
                return Err(ConvertError::MissingFactor);
            }

            // 5 Emitir créditos de carbono
            let mint_call = build_call::<DefaultEnvironment>()
                .call(carbon_token)
                .exec_input(
                    ExecutionInput::new(
                        Selector::new(ink::selector_bytes!("mint_credit")),
                    )
                    .push_arg(caller)
                    .push_arg(carbon_amount)
                    .push_arg(1u64),
                )
                .returns::<Result<(), ()>>()
                .try_invoke();

            match mint_call {
                Ok(Ok(Ok(()))) => {}
                _ => return Err(ConvertError::MintFailed),
            }

            // 6️⃣ Consumir nonce
            self.used_nonces.insert((caller, nonce), &true);

            Ok(())
        }

        // -------------------------------------------------
        // Getters
        // -------------------------------------------------

        // Retorna o fator padrão (escalado) para uma fonte
        #[ink(message)]
        pub fn default_factor(&self, source: EnergySource) -> u128 {
            self.factors.get(source).unwrap_or(0)
        }

        // Retorna os contratos integrados
        #[ink(message)]
        pub fn contracts(&self) -> (AccountId, Option<AccountId>) {
            (self.energy_token, self.carbon_token)
        }

        // Retorna a escala usada no ponto fixo
        #[ink(message)]
        pub fn scale(&self) -> u128 {
            SCALE
        }

        // Retorna o dono do contrato
        #[ink(message)]
        pub fn owner(&self) -> AccountId {
            self.owner
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
        //!   `try_invoke`, `burn_from` ou `mint_credit`
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
