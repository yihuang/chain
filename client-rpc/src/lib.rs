pub mod program;
pub mod rpc;
pub mod server;

pub fn run() {
    crate::program::run_electron();
}
