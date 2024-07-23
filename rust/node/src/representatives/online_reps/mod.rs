mod online_reps_container;
mod representative;
mod representative_register;

pub use representative::Representative;
pub use representative_register::{
    ConfirmationQuorum, RegisterRepresentativeResult, RepresentativeRegister,
    RepresentativeRegisterBuilder, DEFAULT_ONLINE_WEIGHT_MINIMUM,
};
