use clap::Parser;

#[derive(Parser)]
pub(crate) struct VacuumOptions;

impl VacuumOptions {
    pub(crate) fn run(&self) {}
}
