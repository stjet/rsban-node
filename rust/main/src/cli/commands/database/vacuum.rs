use clap::Parser;

#[derive(Parser)]
pub(crate) struct VacuumArgs;

impl VacuumArgs {
    pub(crate) fn vacuum(&self) {}
}
