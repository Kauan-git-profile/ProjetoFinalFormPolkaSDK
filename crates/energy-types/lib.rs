#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[derive(
    scale::Encode,
    scale::Decode,
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq
)]
#[cfg_attr(
    feature = "std",
    derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
)]
pub enum EnergySource {
    Solar,
    Wind,
    Hydro,
    Biomass,
}
