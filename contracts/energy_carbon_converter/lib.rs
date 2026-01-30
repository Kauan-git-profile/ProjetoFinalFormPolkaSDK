#![cfg_attr(not(feature = "std"), no_std)]

use ink::env::call::{build_call, ExecutionInput, Selector};
use ink::storage::Mapping;

#[ink::contract]
mod energy_carbon_converter {

    /// Erros do conversor
    #[derive(scale::Encode, scale::Decode, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum ConvertError {
        Unauthorized,
        InvalidAmount,
        ConversionFailed,
    }

    /// Evento de conversão
    #[ink(event)]
    pub struct Converted {
        #[ink(topic)]
        user: AccountId,
        kwh_burned: u128,
        carbon_minted: u128,
    }

    #[ink(storage)]
    pub struct EnergyCarbonConverter {
        energy_token: AccountId,
        carbon_token: AccountId,
        /// Fator de emissão: tCO2e por kWh
        emission_factor: u128,
        authority: AccountId,
        used_nonces: Mapping<(AccountId, u64), bool>,
    }

    impl EnergyCarbonConverter {

        /// Construtor
        #[ink(constructor)]
        pub fn new(
            energy_token: AccountId,
            carbon_token: AccountId,
            emission_factor: u128,
        ) -> Self {
            Self {
                energy_token,
                carbon_token,
                emission_factor,
                authority: Self::env().caller(),
                used_nonces: Mapping::default(),
            }
        }

        /// Atualiza fator de emissão (governança)
        #[ink(message)]
        pub fn set_emission_factor(&mut self, new_factor: u128) -> Result<(), ConvertError> {
            if self.env().caller() != self.authority || new_factor == 0 {
                return Err(ConvertError::Unauthorized);
            }
            self.emission_factor = new_factor;
            Ok(())
        }

        /// Converte kWh em créditos de carbono
        #[ink(message)]
        pub fn convert(
            &mut self,
            kwh_amount: u128,
            nonce: u64,
        ) -> Result<(), ConvertError> {

            let caller = self.env().caller();

            if kwh_amount == 0 {
                return Err(ConvertError::InvalidAmount);
            }

            if self.used_nonces.get((caller, nonce)).unwrap_or(false) {
                return Err(ConvertError::ConversionFailed);
            }

            // 1️⃣ Burn no EnergyToken
            let burn_result: Result<(), ()> = build_call::<ink::env::DefaultEnvironment>()
                .call(self.energy_token)
                .exec_input(
                    ExecutionInput::new(Selector::new([0x00, 0x00, 0x00, 0x03])) // burn_kwh
                        .push_arg(kwh_amount),
                )
                .returns::<Result<(), ()>>()
                .invoke();

            if burn_result.is_err() {
                return Err(ConvertError::ConversionFailed);
            }

            // 2️⃣ Calcula créditos de carbono
            let carbon_amount = kwh_amount
                .checked_mul(self.emission_factor)
                .ok_or(ConvertError::ConversionFailed)?;

            // 3️⃣ Mint no CarbonCreditToken
            let mint_result: Result<(), ()> = build_call::<ink::env::DefaultEnvironment>()
                .call(self.carbon_token)
                .exec_input(
                    ExecutionInput::new(Selector::new([0x00, 0x00, 0x00, 0x01])) // mint_credit
                        .push_arg(caller)
                        .push_arg(carbon_amount)
                        .push_arg(1u64), // project_id
                )
                .returns::<Result<(), ()>>()
                .invoke();

            if mint_result.is_err() {
                return Err(ConvertError::ConversionFailed);
            }

            self.used_nonces.insert((caller, nonce), &true);

            self.env().emit_event(Converted {
                user: caller,
                kwh_burned: kwh_amount,
                carbon_minted: carbon_amount,
            });

            Ok(())
        }

        /// Consulta contratos integrados
        #[ink(message)]
        pub fn contracts(&self) -> (AccountId, AccountId) {
            (self.energy_token, self.carbon_token)
        }
    }
}
