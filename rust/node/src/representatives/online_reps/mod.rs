mod builder;
mod online_reps_container;
mod representative;
mod representative_register;

pub use builder::{RepresentativeRegisterBuilder, DEFAULT_ONLINE_WEIGHT_MINIMUM};
pub use representative::Representative;
pub use representative_register::{
    ConfirmationQuorum, RegisterRepresentativeResult, RepresentativeRegister,
};
