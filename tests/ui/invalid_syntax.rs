use praborrow_defense::Constitution;
use praborrow_core::CheckProtocol; // Trait required for derive

#[derive(Constitution)]
struct BadInvariant {
    #[invariant(this is not a valid expression >>>)]
    value: i32,
}

fn main() {}
