# Praborrow Defense

Procedural macros for the "Garuda" invariant verification system. This crate generates runtime checks to enforce constitutional integrity on data structures.

## Usage

Derive `Constitution` on your structs and use the `#[invariant]` attribute to define validation logic.

```rust
use praborrow_defense::Constitution;
use praborrow_core::CheckProtocol; // Trait definition

#[derive(Constitution)]
struct StateBudget {
    #[invariant(self.amount > 0)]
    amount: i32,
    
    #[invariant(self.year >= 2024)]
    year: i32,
}

fn main() {
    let budget = StateBudget { amount: 1000, year: 2025 };
    budget.enforce_law(); // Passes

    let corrupt = StateBudget { amount: -50, year: 2025 };
    // corrupt.enforce_law(); // Panics: "CONSTITUTIONAL CRISIS"
}
```

## Protocol

The macro generates an implementation of the `CheckProtocol` trait. Note that `CheckProtocol` must be in scope.

---

# Praborrow Defense (Bahasa Indonesia)

Macro prosedural untuk sistem verifikasi invarian "Garuda". Crate ini menghasilkan pemeriksaan runtime (runtime checks) untuk menegakkan integritas konstitusional pada struktur data.

## Penggunaan (Usage)

Gunakan derive `Constitution` pada struct Anda dan gunakan atribut `#[invariant]` untuk mendefinisikan logika validasi.

```rust
use praborrow_defense::Constitution;
use praborrow_core::CheckProtocol; // Definisi trait

#[derive(Constitution)]
struct StateBudget {
    #[invariant(self.amount > 0)]
    amount: i32,
    
    #[invariant(self.year >= 2024)]
    year: i32,
}

fn main() {
    let budget = StateBudget { amount: 1000, year: 2025 };
    budget.enforce_law(); // Berhasil

    let corrupt = StateBudget { amount: -50, year: 2025 };
    // corrupt.enforce_law(); // Panic: "CONSTITUTIONAL CRISIS" (Krisis Konstitusi)
}
```

## Protokol (Protocol)

Macro ini menghasilkan implementasi dari trait `CheckProtocol`. Perhatikan bahwa `CheckProtocol` harus berada dalam lingkup (in scope).

