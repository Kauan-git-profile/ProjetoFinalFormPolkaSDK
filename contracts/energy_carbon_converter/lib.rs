#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::cast_possible_truncation)]

use ink::env::call::{build_call, ExecutionInput};
use ink::storage::Mapping;

#[ink::contract]
mod energy_carbon_converter {
    use super::*;

    // -------------------------------------------------
    // Tipos
    // -------------------------------------------------

    #[derive(scale::Encode, scale::Decode, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum ConvertError {
        InvalidAmount,
        Replay,
        Overflow,
        BurnFailed,
        MintFailed,
    }

    // -------------------------------------------------
    // Eventos
    // -------------------------------------------------
    // Regras:
    // - Apenas AccountId como topic
    // - Evento pequeno e auditável

    #[ink(event)]
    pub struct Converted {
        #[ink(topic)]
        user: AccountId,
        kwh_burned: u128,
        carbon_minted: u128,
    }

    // -------------------------------------------------
    // Storage
    // -------------------------------------------------

    #[ink(storage)]
    pub struct EnergyCarbonConverter {
        energy_token: AccountId,
        carbon_token: AccountId,
        emission_factor: u128,
        used_nonces: Mapping<(AccountId, u64), bool>,
    }

    // -------------------------------------------------
    // Implementação
    // -------------------------------------------------

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
                used_nonces: Mapping::default(),
            }
        }

        /// Conversão de kWh → créditos de carbono
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
            // Burn de energia
            // -----------------------------

            let burn_result = build_call::<ink::env::DefaultEnvironment>()
                .call(self.energy_token)
                .exec_input(
                    ExecutionInput::new(ink::selector_bytes!("burn_kwh").into())
                        .push_arg(kwh_amount),
                )
                .returns::<Result<(), ()>>()
                .invoke();

            if burn_result.is_err() {
                return Err(ConvertError::BurnFailed);
            }

            // -----------------------------
            // Cálculo de carbono (seguro)
            // -----------------------------

            let carbon_amount = kwh_amount
                .checked_mul(self.emission_factor)
                .ok_or(ConvertError::Overflow)?;

            // -----------------------------
            // Mint de crédito de carbono
            // -----------------------------

            let mint_result = build_call::<ink::env::DefaultEnvironment>()
                .call(self.carbon_token)
                .exec_input(
                    ExecutionInput::new(ink::selector_bytes!("mint_credit").into())
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
                kwh_burned: kwh_amount,
                carbon_minted: carbon_amount,
            });

            Ok(())
        }

        // -------------------------------------------------
        // Getters
        // -------------------------------------------------

        #[ink(message)]
        pub fn emission_factor(&self) -> u128 {
            self.emission_factor
        }

        #[ink(message)]
        pub fn contracts(&self) -> (AccountId, AccountId) {
            (self.energy_token, self.carbon_token)
        }
    }
}
