use praborrow_defense::Constitution;
use praborrow_core::CheckProtocol;

#[derive(Constitution)]
struct TestStruct {
    #[invariant(self.val > 10)]
    val: i32,
}

#[test]
fn test_no_panic() {
    let t = TestStruct { val: 5 };
    let result = t.enforce_law();
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(err.to_string().contains("Invariant 'self.val > 10' breached"));
}

#[test]
fn test_ok() {
    let t = TestStruct { val: 15 };
    assert!(t.enforce_law().is_ok());
}
